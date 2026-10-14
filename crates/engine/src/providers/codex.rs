use super::{Connection, Request, Response, process::*};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

pub struct CodexClient {
    process: Process,
    next_id: u64,
    pending: VecDeque<Value>,
    scratch: PathBuf,
    executable: PathBuf,
}
impl CodexClient {
    pub fn new(root: &Path, configured: &str, cancel: &Cancel) -> Result<Self> {
        let home = root.join("providers/codex");
        let scratch = root.join("providers/codex-scratch");
        private_dir(&home)?;
        private_dir(&scratch)?;
        let executable = executable("codex", configured)?;
        let mut command = isolated_command(&executable, &scratch);
        command.env("CODEX_HOME", &home);
        command.arg("app-server");
        for option in [
            "forced_login_method=\"chatgpt\"",
            "model_provider=\"openai\"",
            "cli_auth_credentials_store=\"file\"",
            "approval_policy=\"never\"",
            "sandbox_mode=\"read-only\"",
            "web_search=\"disabled\"",
            "features.shell_tool=false",
            "features.unified_exec=false",
            "features.multi_agent=false",
            "features.apps=false",
            "features.plugins=false",
            "features.hooks=false",
            "features.code_mode=false",
            "features.code_mode_host=false",
            "features.apply_patch_freeform=false",
            "project_doc_max_bytes=0",
            "mcp_servers={}",
            "analytics.enabled=false",
        ] {
            command.args(["-c", option]);
        }
        let mut client = Self {
            process: Process::spawn(command)?,
            next_id: 0,
            pending: VecDeque::new(),
            scratch,
            executable,
        };
        client.rpc("initialize",json!({"clientInfo":{"name":"theory_studio","title":"Theory Studio","version":"0.4.0"},"capabilities":{"experimentalApi":false}}),cancel,Instant::now()+Duration::from_secs(20))?;
        client.process.send(&json!({"method":"initialized"}))?;
        Ok(client)
    }
    fn rpc(
        &mut self,
        method: &str,
        params: Value,
        cancel: &Cancel,
        deadline: Instant,
    ) -> Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        self.process
            .send(&json!({"id":id,"method":method,"params":params}))?;
        loop {
            let v = self.read(cancel, deadline)?;
            if v.get("id") == Some(&json!(id)) && v.get("method").is_none() {
                if let Some(e) = v.get("error") {
                    bail!(
                        "Codex {method}: {}",
                        e["message"].as_str().unwrap_or("요청 실패")
                    );
                }
                return v.get("result").cloned().context("Codex 결과 누락");
            }
            ensure!(self.pending.len() < 512, "Codex 알림 버퍼 한도 초과");
            self.pending.push_back(v);
        }
    }
    fn read(&mut self, cancel: &Cancel, deadline: Instant) -> Result<Value> {
        loop {
            let line = self.process.line(cancel, deadline)?;
            if line.trim().is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(&line).context("Codex JSON 프레임 오류")?;
            if v.get("id").is_some() && v.get("method").is_some() {
                self.process.send(&json!({"id":v["id"],"error":{"code":-32601,"message":"Theory Studio does not allow tool, approval, or token requests"}}))?;
                continue;
            }
            return Ok(v);
        }
    }
    fn event(&mut self, cancel: &Cancel, deadline: Instant) -> Result<Value> {
        match self.pending.pop_front() {
            Some(v) => Ok(v),
            None => self.read(cancel, deadline),
        }
    }
    pub fn inspect(&mut self, cancel: &Cancel) -> Result<Connection> {
        let deadline = Instant::now() + Duration::from_secs(30);
        let account = self.rpc(
            "account/read",
            json!({"refreshToken":true}),
            cancel,
            deadline,
        )?;
        let authenticated = account["account"]["type"] == "chatgpt";
        let mut connection = Connection {
            provider: "Codex".into(),
            installed: true,
            authenticated,
            executable: self.executable.display().to_string(),
            ..Default::default()
        };
        if authenticated {
            connection.models = self
                .models(cancel, deadline)?
                .iter()
                .filter_map(|m| m["model"].as_str().map(String::from))
                .collect();
            match self.rpc("account/rateLimits/read", json!({}), cancel, deadline) {
                Ok(v) => {
                    connection.message = if v["ordinaryUsageAllowed"] == true {
                        "ChatGPT 구독 연결 · 계정 사용량 확인됨"
                    } else {
                        "ChatGPT 로그인됨 · 현재 구독 사용 가능 여부 확인 필요"
                    }
                    .into();
                    connection.allowance = Some(v);
                }
                Err(e) => connection.message = format!("로그인됨 · 사용량 조회 실패: {e}"),
            }
        } else {
            connection.message = "이 앱에서 ChatGPT 로그인이 필요합니다".into();
        }
        Ok(connection)
    }
    fn models(&mut self, cancel: &Cancel, deadline: Instant) -> Result<Vec<Value>> {
        let mut models = vec![];
        let mut cursor = Value::Null;
        let mut seen = HashSet::new();
        loop {
            let page = self.rpc(
                "model/list",
                json!({"limit":100,"cursor":cursor,"includeHidden":false}),
                cancel,
                deadline,
            )?;
            let data = page["data"]
                .as_array()
                .context("Codex 모델 목록 형식 오류")?;
            models.extend(data.iter().filter(|m| m["hidden"] != true).cloned());
            cursor = page["nextCursor"].clone();
            if cursor.is_null() {
                break;
            }
            ensure!(
                seen.insert(cursor.to_string()) && seen.len() < 20,
                "모델 목록 cursor 오류"
            );
        }
        Ok(models)
    }
    pub fn login(&mut self, cancel: &Cancel, progress: &mut dyn FnMut(String)) -> Result<()> {
        let response = self.rpc(
            "account/login/start",
            json!({"type":"chatgpt"}),
            cancel,
            Instant::now() + Duration::from_secs(30),
        )?;
        let url = response["authUrl"].as_str().context("로그인 URL 누락")?;
        ensure!(
            url.starts_with("https://auth.openai.com/")
                || url.starts_with("https://auth0.openai.com/"),
            "예상하지 못한 로그인 호스트"
        );
        crate::platform::open_external(url)?;
        progress("브라우저에서 ChatGPT 로그인을 완료하세요. 이 앱 전용 로그인입니다.".into());
        let deadline = Instant::now() + Duration::from_secs(300);
        loop {
            match self.event(cancel, deadline) {
                Ok(v) if v["method"] == "account/login/completed" => {
                    ensure!(v["params"]["success"] == true, "ChatGPT 로그인 실패");
                    return Ok(());
                }
                Ok(_) => {}
                Err(e) => {
                    let _=self.process.send(&json!({"id":999999,"method":"account/login/cancel","params":{"loginId":response["loginId"]}}));
                    return Err(e);
                }
            }
        }
    }
    pub fn logout(&mut self, cancel: &Cancel) -> Result<()> {
        self.rpc(
            "account/logout",
            json!({}),
            cancel,
            Instant::now() + Duration::from_secs(15),
        )?;
        Ok(())
    }
    pub fn generate(
        &mut self,
        r: &Request,
        cancel: &Cancel,
        progress: &mut dyn FnMut(String),
    ) -> Result<Response> {
        let deadline = r.deadline;
        let account = self.rpc(
            "account/read",
            json!({"refreshToken":true}),
            cancel,
            deadline,
        )?;
        ensure!(
            account["account"]["type"] == "chatgpt",
            "이 앱에서 ChatGPT 구독 로그인이 필요합니다"
        );
        let models = self.models(cancel, deadline)?;
        let model = if r.model.is_empty() {
            models.iter().find(|m| m["isDefault"] == true)
        } else {
            models.iter().find(|m| m["model"] == r.model)
        }
        .context("선택한 모델을 계정에서 사용할 수 없습니다. 설정에서 다시 선택하세요.")?;
        let slug = model["model"]
            .as_str()
            .context("모델 식별자 누락")?
            .to_string();
        let allowance = self.rpc("account/rateLimits/read", json!({}), cancel, deadline)?;
        check_allowance(&allowance, &slug)?;
        let thread=self.rpc("thread/start",json!({"model":slug,"cwd":self.scratch,"approvalPolicy":"never","sandbox":"read-only","ephemeral":true,"baseInstructions":include_str!("../../../../resources/roles/research-v2.txt"),"serviceTier":"default"}),cancel,deadline)?;
        let thread_id = thread["thread"]["id"]
            .as_str()
            .context("thread ID 누락")?
            .to_string();
        let turn=self.rpc("turn/start",json!({"threadId":thread_id,"input":[{"type":"text","text":r.prompt,"text_elements":[]}],"outputSchema":r.schema,"effort":model["defaultReasoningEffort"],"serviceTierForTurn":"default"}),cancel,deadline)?;
        let turn_id = turn["turn"]["id"]
            .as_str()
            .context("turn ID 누락")?
            .to_string();
        let mut texts: HashMap<String, String> = HashMap::new();
        let mut order = vec![];
        let mut usage = None;
        progress(format!("Codex · {slug} 응답 작성 중"));
        loop {
            let v = match self.event(cancel, deadline) {
                Ok(v) => v,
                Err(e) => {
                    let _=self.process.send(&json!({"id":999999,"method":"turn/interrupt","params":{"threadId":thread_id,"turnId":turn_id}}));
                    return Err(e);
                }
            };
            let p = &v["params"];
            if p.get("threadId").is_some_and(|t| t != &json!(thread_id))
                || p.get("turnId").is_some_and(|t| t != &json!(turn_id))
            {
                continue;
            }
            match v["method"].as_str().unwrap_or("") {
                "item/agentMessage/delta" => {
                    let id = p["itemId"].as_str().unwrap_or("answer").to_string();
                    if !texts.contains_key(&id) {
                        order.push(id.clone());
                    }
                    texts
                        .entry(id)
                        .or_default()
                        .push_str(p["delta"].as_str().unwrap_or(""));
                    ensure!(
                        texts.values().map(String::len).sum::<usize>() < MAX_FRAME as usize,
                        "응답 크기 한도 초과"
                    );
                }
                "item/completed" if p["item"]["type"] == "agentMessage" => {
                    let id = p["item"]["id"].as_str().unwrap_or("answer").to_string();
                    if let std::collections::hash_map::Entry::Vacant(entry) =
                        texts.entry(id.clone())
                    {
                        order.push(id);
                        entry.insert(p["item"]["text"].as_str().unwrap_or("").into());
                    }
                }
                "thread/tokenUsage/updated" => usage = Some(p["tokenUsage"].clone()),
                "turn/completed" => {
                    ensure!(!cancel.load(Ordering::SeqCst), "cancelled");
                    ensure!(
                        p["turn"]["status"] == "completed",
                        "Codex 작업 미완료: {}",
                        p["turn"]["error"]
                    );
                    let text = order
                        .iter()
                        .map(|id| texts[id].as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    return Ok(Response {
                        value: serde_json::from_str(&text).context("Codex 구조화 출력 오류")?,
                        model: slug,
                        usage,
                    });
                }
                _ => {}
            }
        }
    }
}
/// A positive authoritative ordinary-use signal is required; credits never grant permission.
pub fn check_allowance(value: &Value, model: &str) -> Result<()> {
    ensure!(
        value["ordinaryUsageAllowed"] == true,
        "구독 포함 사용량을 확인할 수 없거나 소진했습니다. 사용량을 새로 확인하세요."
    );
    let buckets: Vec<&Value> = match value["rateLimitsByLimitId"]
        .as_object()
        .filter(|m| !m.is_empty())
    {
        Some(m) => m.values().collect(),
        None => vec![&value["rateLimits"]],
    };
    for b in buckets {
        let normal = b["normalModelSlug"].as_str();
        let id = b["limitId"].as_str().unwrap_or("");
        if normal.is_some_and(|s| s != model) || id.contains("review") {
            continue;
        }
        ensure!(
            b["spendControlReached"] != true,
            "계정 지출 제한에 도달했습니다"
        );
        for key in ["primary", "secondary"] {
            if !b[key].is_null() {
                let used = b[key]["usedPercent"]
                    .as_f64()
                    .context("사용량 형식이 불명확합니다")?;
                ensure!(
                    used.is_finite() && (0.0..100.0).contains(&used),
                    "구독 사용량이 소진되었거나 확인할 수 없습니다"
                );
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allowance_blocks_denial_unknown_exhaustion() {
        for v in [
            json!({}),
            json!({"ordinaryUsageAllowed":false,"credits":{"balance":"100"}}),
            json!({"ordinaryUsageAllowed":true,"rateLimits":{"primary":{"usedPercent":100}}}),
            json!({"ordinaryUsageAllowed":true,"rateLimits":{"secondary":{"usedPercent":-1}}}),
        ] {
            assert!(check_allowance(&v, "m").is_err());
        }
        assert!(
            check_allowance(
                &json!({"ordinaryUsageAllowed":true,"rateLimits":{"primary":{"usedPercent":50}}}),
                "m"
            )
            .is_ok()
        );
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    use std::{
        os::unix::fs::PermissionsExt,
        sync::{Arc, atomic::AtomicBool},
    };
    fn fixture(scenario: &str) -> (tempfile::TempDir, String) {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("fake-codex");
        std::fs::write(
            &path,
            include_str!("../../../../tests/fixtures/fake_codex.py"),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(d.path().join("scenario"), scenario).unwrap();
        (d, path.to_string_lossy().into_owned())
    }
    fn request() -> Request {
        Request::new(
            "Synthetic".into(),
            json!({"type":"object"}),
            String::new(),
            Instant::now() + Duration::from_secs(5),
        )
    }
    #[test]
    fn early_unicode_events_deduplicate_and_reject_tools() {
        let (d, path) = fixture("normal");
        let cancel = Arc::new(AtomicBool::new(false));
        let mut client = CodexClient::new(d.path(), &path, &cancel).unwrap();
        let r = client.generate(&request(), &cancel, &mut |_| {}).unwrap();
        assert_eq!(r.value, json!({"status":"한글 ok"}));
        assert_eq!(r.model, "fixture-model");
        drop(client);
        let log = std::fs::read_to_string(d.path().join("requests.jsonl")).unwrap();
        assert!(log.contains("-32601"));
        assert!(log.contains("\"model\": \"fixture-model\""));
    }
    #[test]
    fn auth_and_quota_denials_never_start_thread() {
        for scenario in ["api", "denied", "exhausted"] {
            let (d, path) = fixture(scenario);
            let cancel = Arc::new(AtomicBool::new(false));
            let mut client = CodexClient::new(d.path(), &path, &cancel).unwrap();
            assert!(client.generate(&request(), &cancel, &mut |_| {}).is_err());
            drop(client);
            let log = std::fs::read_to_string(d.path().join("requests.jsonl")).unwrap();
            assert!(!log.contains("thread/start"));
        }
    }
    #[test]
    fn malformed_oversized_and_failed_turns_reject() {
        for scenario in ["malformed", "oversized", "failed"] {
            let (d, path) = fixture(scenario);
            let cancel = Arc::new(AtomicBool::new(false));
            let mut client = CodexClient::new(d.path(), &path, &cancel).unwrap();
            assert!(client.generate(&request(), &cancel, &mut |_| {}).is_err());
        }
    }
    #[test]
    fn quiet_process_cancel_is_bounded() {
        let (d, path) = fixture("silent");
        let cancel = Arc::new(AtomicBool::new(false));
        let token = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            token.store(true, Ordering::SeqCst);
        });
        let now = Instant::now();
        assert!(CodexClient::new(d.path(), &path, &cancel).is_err());
        assert!(now.elapsed() < Duration::from_secs(2));
    }
}
