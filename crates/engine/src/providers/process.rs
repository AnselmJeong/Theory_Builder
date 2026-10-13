use anyhow::{Context, Result, bail, ensure};
use std::os::unix::process::CommandExt;
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};
pub type Cancel = Arc<AtomicBool>;
pub const MAX_FRAME: u64 = 4 * 1024 * 1024;

pub fn executable(name: &str, configured: &str) -> Result<PathBuf> {
    let mut candidates = vec![];
    if !configured.is_empty() {
        candidates.push(PathBuf::from(configured));
    } else {
        if let Some(home) = std::env::var_os("HOME") {
            candidates.push(PathBuf::from(home).join(".local/bin").join(name));
        }
        candidates.extend([
            PathBuf::from("/opt/homebrew/bin").join(name),
            PathBuf::from("/usr/local/bin").join(name),
        ]);
        if let Some(path) = std::env::var_os("PATH") {
            candidates.extend(std::env::split_paths(&path).map(|p| p.join(name)));
        }
    }
    candidates.into_iter().find(|p| p.is_file()).ok_or_else(|| {
        anyhow::anyhow!("{name} CLI를 찾지 못했습니다. 설정에서 실행 파일 경로를 지정하세요.")
    })
}
pub fn private_dir(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}
pub fn isolated_command(exe: &Path, cwd: &Path) -> Command {
    let mut c = Command::new(exe);
    c.env_clear();
    for key in [
        "HOME",
        "USER",
        "LOGNAME",
        "TMPDIR",
        "LANG",
        "LC_ALL",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "NO_PROXY",
    ] {
        if let Some(v) = std::env::var_os(key) {
            c.env(key, v);
        }
    }
    // Finder launch does not inherit a terminal's PATH. No shell is involved.
    let local = std::env::var("HOME")
        .map(|h| format!("{h}/.local/bin:"))
        .unwrap_or_default();
    c.env(
        "PATH",
        format!("{local}/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"),
    );
    c.current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0);
    c
}
pub struct Process {
    child: Child,
    pub input: Option<ChildStdin>,
    lines: Receiver<Result<String, String>>,
    diagnostics: Arc<Mutex<String>>,
}
impl Process {
    pub fn spawn(mut command: Command) -> Result<Self> {
        command.stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .context("CLI 프로세스를 시작할 수 없습니다")?;
        let diagnostics = Arc::new(Mutex::new(String::new()));
        if let Some(mut stderr) = child.stderr.take() {
            let target = diagnostics.clone();
            std::thread::spawn(move || {
                let mut bytes = vec![];
                let _ = stderr.by_ref().take(16384).read_to_end(&mut bytes);
                let text = String::from_utf8_lossy(&bytes);
                let safe = text
                    .lines()
                    .filter(|l| {
                        let l = l.to_lowercase();
                        ![
                            "token",
                            "api_key",
                            "apikey",
                            "authorization",
                            "https://auth",
                        ]
                        .iter()
                        .any(|s| l.contains(s))
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if let Ok(mut target) = target.lock() {
                    *target = safe;
                }
                // Keep draining without retaining unbounded diagnostics or closing the pipe.
                let _ = std::io::copy(&mut stderr, &mut std::io::sink());
            });
        }
        let input = child.stdin.take().context("stdin missing")?;
        let output = child.stdout.take().context("stdout missing")?;
        let (tx, lines) = mpsc::sync_channel(256);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut bytes = Vec::new();
                match reader
                    .by_ref()
                    .take(MAX_FRAME + 1)
                    .read_until(b'\n', &mut bytes)
                {
                    Ok(0) => break,
                    Ok(_) if bytes.len() as u64 > MAX_FRAME => {
                        let _ = tx.send(Err("CLI 응답 프레임이 너무 큽니다".into()));
                        break;
                    }
                    Ok(_) => match String::from_utf8(bytes) {
                        Ok(s) => {
                            if tx.send(Ok(s)).is_err() {
                                break;
                            }
                        }
                        Err(_) => {
                            let _ = tx.send(Err("CLI 응답이 UTF-8이 아닙니다".into()));
                            break;
                        }
                    },
                    Err(e) => {
                        let _ = tx.send(Err(e.to_string()));
                        break;
                    }
                }
            }
        });
        Ok(Self {
            child,
            input: Some(input),
            lines,
            diagnostics,
        })
    }
    pub fn send(&mut self, value: &serde_json::Value) -> Result<()> {
        let input = self.input.as_mut().context("stdin closed")?;
        serde_json::to_writer(&mut *input, value)?;
        input.write_all(b"\n")?;
        input.flush()?;
        Ok(())
    }
    pub fn line(&mut self, cancel: &Cancel, deadline: Instant) -> Result<String> {
        loop {
            ensure!(!cancel.load(Ordering::SeqCst), "cancelled");
            ensure!(
                Instant::now() < deadline,
                "CLI 응답 시간 한도를 초과했습니다"
            );
            match self.lines.recv_timeout(Duration::from_millis(75)) {
                Ok(Ok(s)) => return Ok(s),
                Ok(Err(e)) => bail!("{e}"),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    bail!("CLI 연결이 종료되었습니다. 로그인 상태와 실행 파일을 확인하세요.")
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }
    pub fn text(
        mut command: Command,
        cancel: &Cancel,
        deadline: Instant,
        input: Option<&str>,
    ) -> Result<String> {
        command.stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        // spawn expects piped stdin; closing input communicates EOF to print-mode clients.
        command.stdin(Stdio::piped());
        let mut p = Self::spawn(command)?;
        if let Some(s) = input {
            p.input.as_mut().unwrap().write_all(s.as_bytes())?;
            p.input.as_mut().unwrap().flush()?;
        }
        drop(p.input.take());
        let mut out = String::new();
        loop {
            ensure!(!cancel.load(Ordering::SeqCst), "cancelled");
            ensure!(
                Instant::now() < deadline,
                "CLI 응답 시간 한도를 초과했습니다"
            );
            match p.lines.recv_timeout(Duration::from_millis(75)) {
                Ok(Ok(s)) => {
                    out.push_str(&s);
                    ensure!(out.len() as u64 <= MAX_FRAME, "CLI 출력 한도 초과");
                }
                Ok(Err(e)) => bail!("{e}"),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let status = loop {
                        ensure!(!cancel.load(Ordering::SeqCst), "cancelled");
                        ensure!(
                            Instant::now() < deadline,
                            "CLI 종료 시간 한도를 초과했습니다"
                        );
                        if let Some(status) = p.child.try_wait()? {
                            break status;
                        }
                        std::thread::sleep(Duration::from_millis(20));
                    };
                    ensure!(
                        status.success(),
                        "CLI 실행 실패 ({}): {} {}",
                        status,
                        out.chars().take(600).collect::<String>(),
                        p.diagnostics
                            .lock()
                            .map(|s| s.chars().take(1200).collect::<String>())
                            .unwrap_or_default()
                    );
                    return Ok(out);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGTERM);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_stdout_does_not_bypass_deadline() {
        let mut command = isolated_command(Path::new("/bin/sh"), Path::new("/tmp"));
        command.args(["-c", "exec 1>&-; sleep 10"]);
        let start = Instant::now();
        let result = Process::text(
            command,
            &Arc::new(AtomicBool::new(false)),
            start + Duration::from_millis(150),
            None,
        );
        assert!(result.is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn allowlist_drops_provider_and_runtime_injection() {
        let c = isolated_command(Path::new("/bin/echo"), Path::new("/tmp"));
        let keys: Vec<_> = c
            .get_envs()
            .map(|(k, _)| k.to_string_lossy().to_string())
            .collect();
        for key in [
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "NODE_OPTIONS",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "OPENAI_BASE_URL",
        ] {
            assert!(!keys.contains(&key.to_string()));
        }
        assert!(keys.contains(&"HOME".to_string()));
    }
}
