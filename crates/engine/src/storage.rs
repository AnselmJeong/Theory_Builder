mod book;
mod shared;
pub use shared::scholarpen_root;
const MAX_ASSET: u64 = 256 * 1024 * 1024;
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use rusqlite::{Connection, params};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use theory_core::*;

pub fn data_directory() -> PathBuf {
    std::env::var_os("THEORY_STUDIO_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join("Library/Application Support/Theory Studio")
        })
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("상위 경로 없음")?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".{}.tmp", id("write")));
    let result = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&temp, path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
pub fn read_bounded(path: &Path, max: u64) -> Result<Vec<u8>> {
    let mut bytes = vec![];
    File::open(path)?.take(max + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= max,
        "파일이 허용 크기({max} bytes)를 초과합니다"
    );
    Ok(bytes)
}
fn asset_name(digest: &str, extension: &str) -> String {
    let ext = if ["pdf", "epub", "docx", "md", "qmd", "txt"].contains(&extension) {
        extension
    } else {
        "bin"
    };
    format!("{digest}.{ext}")
}
fn stored_asset(directory: &Path, source: &Source) -> Result<PathBuf> {
    let digest = source.asset_hash.as_ref().context("원본 파일 기록 없음")?;
    ensure!(safe_hash(digest), "자산 hash 오류");
    let typed = directory
        .join("assets")
        .join(asset_name(digest, &source.extension));
    if typed.is_file() {
        return Ok(typed);
    }
    // Read legacy extensionless assets without renaming or modifying imported bundles.
    let legacy = directory.join("assets").join(digest);
    ensure!(
        legacy.is_file(),
        "저장된 원본 파일을 찾지 못했습니다: {}. 자료를 다시 추가하세요",
        source.title
    );
    Ok(legacy)
}
pub fn lock_root(root: &Path) -> Result<File> {
    fs::create_dir_all(root)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join("app.lock"))?;
    file.try_lock_exclusive()
        .context("Theory Studio가 이미 이 데이터 폴더를 사용하고 있습니다")?;
    Ok(file)
}
pub fn projects(root: &Path) -> Result<Vec<(String, String)>> {
    let dir = root.join("projects");
    fs::create_dir_all(&dir)?;
    let mut list = vec![];
    for entry in fs::read_dir(dir)? {
        let path = entry?.path().join("project.sqlite");
        if !path.is_file() {
            continue;
        }
        let db = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let json: String =
            db.query_row("SELECT json FROM snapshot WHERE id=1", [], |r| r.get(0))?;
        let p: Project = serde_json::from_str(&json)?;
        list.push((p.id, format!("[앱 내부] {}", p.title)));
    }
    for folder in shared::shared_folders(root) {
        {
            let key = format!("folder:{}", folder.to_string_lossy());
            let title = folder
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            list.push((key, title));
        }
    }
    list.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(list)
}
pub struct Store {
    db: Connection,
    directory: PathBuf,
    pub project: Project,
    resource_stamps: std::collections::HashMap<String, (u64, std::time::SystemTime)>,
    _project_lock: File,
}
impl Store {
    pub fn create(root: &Path, title: &str) -> Result<Self> {
        let p = Project::new(title);
        Self::open_project(root, p)
    }
    pub fn open(root: &Path, id: &str) -> Result<Self> {
        if let Some(folder) = id.strip_prefix("folder:") {
            return Self::open_folder(root, Path::new(folder));
        }
        ensure!(safe_id(id), "잘못된 프로젝트 ID");
        let dir = root.join("projects").join(id);
        let db = Connection::open(dir.join("project.sqlite"))?;
        let p: Project = serde_json::from_str(&db.query_row::<String, _, _>(
            "SELECT json FROM snapshot WHERE id=1",
            [],
            |r| r.get(0),
        )?)?;
        ensure!(p.id == id, "프로젝트 ID 불일치");
        drop(db);
        Self::open_project(root, p)
    }
    fn open_project(root: &Path, project: Project) -> Result<Self> {
        ensure!(safe_id(&project.id), "잘못된 프로젝트 ID");
        let directory = root.join("projects").join(&project.id);
        Self::open_directory(directory, project)
    }
    fn open_directory(directory: PathBuf, project: Project) -> Result<Self> {
        fs::create_dir_all(directory.join("assets"))?;
        let project_lock = lock_root(&directory)?;
        let mut db = Connection::open(directory.join("project.sqlite"))?;
        db.busy_timeout(std::time::Duration::from_secs(3))?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;",
        )?;
        let version: i32 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        ensure!(
            version <= 1,
            "더 새로운 앱에서 만든 프로젝트입니다. 원본을 보존했습니다."
        );
        if version == 0 {
            let tx = db.transaction()?;
            tx.execute_batch("CREATE TABLE IF NOT EXISTS snapshot(id INTEGER PRIMARY KEY CHECK(id=1),json TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS revisions(number INTEGER PRIMARY KEY,json TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runs(id TEXT PRIMARY KEY,json TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS attempts(id TEXT PRIMARY KEY,run_id TEXT NOT NULL,json TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS sources(id TEXT PRIMARY KEY,hash TEXT NOT NULL,json TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS ledger(id TEXT PRIMARY KEY,json TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS events(id INTEGER PRIMARY KEY,created INTEGER NOT NULL,revision INTEGER NOT NULL,message TEXT NOT NULL);
            PRAGMA user_version=1;")?;
            tx.commit()?;
        }
        let mut store = Self {
            db,
            directory,
            project,
            resource_stamps: Default::default(),
            _project_lock: project_lock,
        };
        let recovered = store.project.recover();
        store.save(if recovered {
            "재시작 복구: 미완료 호출을 자동 재전송하지 않음"
        } else {
            "프로젝트 열기"
        })?;
        Ok(store)
    }
    pub fn reload(&mut self) -> Result<()> {
        self.project = serde_json::from_str(&self.db.query_row::<String, _, _>(
            "SELECT json FROM snapshot WHERE id=1",
            [],
            |r| r.get(0),
        )?)?;
        Ok(())
    }
    pub fn save(&mut self, event: &str) -> Result<()> {
        let tx = self.db.transaction()?;
        tx.execute(
            "INSERT INTO snapshot VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET json=excluded.json",
            [serde_json::to_string(&self.project)?],
        )?;
        for r in &self.project.history {
            tx.execute(
                "INSERT OR IGNORE INTO revisions VALUES(?1,?2)",
                params![r.number as i64, serde_json::to_string(r)?],
            )?;
        }
        for r in &self.project.runs {
            tx.execute(
                "INSERT INTO runs VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET json=excluded.json",
                params![r.id, serde_json::to_string(r)?],
            )?;
            for a in &r.attempts {
                tx.execute("INSERT INTO attempts VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET json=excluded.json",params![a.id,r.id,serde_json::to_string(a)?])?;
            }
        }
        for s in &self.project.sources {
            tx.execute("INSERT INTO sources VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET hash=excluded.hash,json=excluded.json",params![s.id,s.hash,serde_json::to_string(s)?])?;
        }
        tx.execute("DELETE FROM ledger", [])?;
        for e in &self.project.entries {
            tx.execute(
                "INSERT INTO ledger VALUES(?1,?2)",
                params![e.proposal.id, serde_json::to_string(e)?],
            )?;
        }
        tx.execute(
            "INSERT INTO events(created,revision,message) VALUES(?1,?2,?3)",
            params![now() as i64, self.project.revision as i64, event],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn import_file(&mut self, path: &Path) -> Result<()> {
        self.import_file_cancellable(
            path,
            &std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
    }
    pub fn import_file_cancellable(
        &mut self,
        path: &Path,
        cancel: &crate::providers::Cancel,
    ) -> Result<()> {
        let extension = path
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_lowercase();
        ensure!(
            ["md", "txt", "qmd", "pdf", "epub", "docx"].contains(&extension.as_str()),
            "Markdown, TXT, PDF, EPUB, DOCX를 추가할 수 있습니다"
        );
        let bytes = read_bounded(path, MAX_ASSET)?;
        let asset_hash = hash(&bytes);
        if self.project.workspace_folder.is_none()
            && self
                .project
                .sources
                .iter()
                .any(|s| s.active && s.asset_hash.as_ref() == Some(&asset_hash))
        {
            return Ok(());
        }
        let title = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if let Some(folder) = &self.project.workspace_folder {
            let dir = PathBuf::from(folder).join(if extension == "epub" {
                "resources/books"
            } else {
                "resources/articles"
            });
            fs::create_dir_all(&dir)?;
            let mut dest = dir.join(&title);
            if dest.exists() && hash(&read_bounded(&dest, MAX_ASSET)?) != asset_hash {
                dest = dir.join(format!(
                    "{}-{}.{}",
                    path.file_stem().unwrap_or_default().to_string_lossy(),
                    &asset_hash[..8],
                    extension
                ));
                ensure!(
                    !dest.exists(),
                    "같은 이름의 자료가 있습니다. 원본을 보존했습니다"
                );
            }
            if !dest.exists() {
                atomic_write(&dest, &bytes)?;
            }
            self.sync_resources(cancel, &mut |_| {})?;
            return Ok(());
        }
        let (text, note) = shared::extract(path, &extension, &bytes, cancel);
        let access = if text.trim().is_empty() {
            Access::MetadataOnly
        } else {
            Access::ProvidedText
        };
        let mut source = Source::new(title, text, "가져온 본문의 행 번호".into(), access);
        source.import_note = note;
        source.asset_hash = Some(asset_hash.clone());
        source.extension = extension;
        atomic_write(
            &self
                .directory
                .join("assets")
                .join(asset_name(&asset_hash, &source.extension)),
            &bytes,
        )?;
        self.project.sources.push(source);
        self.project.checkpoint("자료 추가");
        self.save("자료와 hash 저장")
    }
    pub fn source_path(&self, id: &str) -> Result<PathBuf> {
        let s = self
            .project
            .sources
            .iter()
            .find(|s| s.id == id)
            .context("출처 없음")?;
        let h = s
            .asset_hash
            .as_ref()
            .context("붙여 넣은 구절에는 외부 파일이 없습니다")?;
        ensure!(safe_hash(h), "자산 hash 오류");
        let asset = stored_asset(&self.directory, s)?;
        ensure!(
            hash(&read_bounded(&asset, MAX_ASSET)?) == *h,
            "자산이 변경되었습니다"
        );
        // Give LaunchServices the original file type without exposing arbitrary paths.
        let preview = self
            .directory
            .join("preview")
            .join(asset_name(h, &s.extension));
        atomic_write(&preview, &read_bounded(&asset, MAX_ASSET)?)?;
        Ok(preview)
    }
    pub fn export(&self, destination: &Path) -> Result<PathBuf> {
        fs::create_dir_all(destination)?;
        let bundle = destination.join(format!(
            "theory-{}-{}.theorybundle",
            now(),
            &self.project.id[8..16]
        ));
        ensure!(!bundle.exists(), "내보내기 경로가 이미 있습니다");
        let staging = destination.join(id(".export"));
        fs::create_dir_all(staging.join("assets"))?;
        let result = (|| {
            atomic_write(
                &staging.join("project.json"),
                &serde_json::to_vec_pretty(&self.project)?,
            )?;
            atomic_write(
                &staging.join("report.md"),
                markdown(&self.project).as_bytes(),
            )?;
            self.db
                .backup(rusqlite::MAIN_DB, staging.join("project.sqlite"), None)?;
            for s in &self.project.sources {
                if let Some(h) = &s.asset_hash {
                    ensure!(safe_hash(h), "자산 hash 오류");
                    let b = read_bounded(&stored_asset(&self.directory, s)?, MAX_ASSET)?;
                    ensure!(hash(&b) == *h, "자산 hash 불일치");
                    atomic_write(
                        &staging.join("assets").join(asset_name(h, &s.extension)),
                        &b,
                    )?;
                }
            }
            atomic_write(&staging.join("manifest.json"),serde_json::to_string_pretty(&serde_json::json!({"format":"theory-studio-bundle","version":2,"revision":self.project.revision,"project_hash":hash(&serde_json::to_vec_pretty(&self.project)?)}))?.as_bytes())?;
            fs::rename(&staging, &bundle)?;
            Ok(bundle)
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(staging);
        }
        result
    }
    pub fn import_bundle(root: &Path, bundle: &Path) -> Result<Self> {
        let manifest: serde_json::Value =
            serde_json::from_slice(&read_bounded(&bundle.join("manifest.json"), 65536)?)?;
        ensure!(
            manifest["format"] == "theory-studio-bundle"
                && matches!(manifest["version"].as_u64(), Some(1 | 2)),
            "지원하지 않는 bundle"
        );
        let bytes = read_bounded(&bundle.join("project.json"), 64 * 1024 * 1024)?;
        ensure!(
            manifest["project_hash"] == hash(&bytes),
            "프로젝트 hash 불일치"
        );
        let mut p: Project = serde_json::from_slice(&bytes)?;
        for s in &p.sources {
            ensure!(hash(s.text.as_bytes()) == s.hash, "출처 텍스트 hash 불일치");
            if let Some(h) = &s.asset_hash {
                ensure!(safe_hash(h), "잘못된 자산 경로");
                let bytes = read_bounded(&stored_asset(bundle, s)?, MAX_ASSET)?;
                ensure!(hash(&bytes) == *h, "첨부 자산 hash 불일치");
            }
        }
        p.workspace_folder = None;
        p.id = id("project");
        p.title = format!("{} · 가져온 사본", p.title);
        let dir = root.join("projects").join(&p.id);
        fs::create_dir_all(dir.join("assets"))?;
        for s in &p.sources {
            if let Some(h) = &s.asset_hash {
                atomic_write(
                    &dir.join("assets").join(asset_name(h, &s.extension)),
                    &read_bounded(&stored_asset(bundle, s)?, MAX_ASSET)?,
                )?;
            }
        }
        Self::open_project(root, p)
    }
}
fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
fn safe_hash(h: &str) -> bool {
    h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit())
}
pub fn markdown(p: &Project) -> String {
    let mut out = format!(
        "# {}\n\n작업 판본: {} · 프로젝트: {}\n\n",
        p.title, p.revision, p.id
    );
    for r in &p.runs {
        out.push_str(&format!(
            "## {}\n\n상태: {} · 기준 판본 {} · 호출 {}/{}\n\n{}\n\n",
            r.question,
            r.state.label(),
            r.base_revision,
            r.calls,
            r.budget.calls(),
            r.report
        ));
        for l in &r.limitations {
            out.push_str(&format!("- 한계: {l}\n"));
        }
        for a in &r.attempts {
            if let Some(o) = &a.output {
                for e in &o.evidence_refs {
                    out.push_str(&format!(
                        "\n> {}\n\n출처 [{}](#{}), 행 {}–{}, hash `{}`, 판정 {:?}: {}\n",
                        e.quote,
                        e.source_id,
                        e.source_id,
                        e.start_line,
                        e.end_line,
                        e.source_hash,
                        e.relation,
                        e.reasoning
                    ));
                }
                for c in &o.critiques {
                    out.push_str(&format!(
                        "\n- 미해결 비판 / {} / {}: {}\n",
                        c.target_id, c.severity, c.reasoning
                    ));
                }
            }
        }
    }
    out.push_str("\n## 주장·가정 원장\n\n");
    for e in &p.entries {
        out.push_str(&format!(
            "- **{}** ({:?}, {:?}, {:?}): {}\n  - 범위: {}\n  - 근거/도입 이유: {}\n  - 의존: {}\n",
            e.proposal.id,
            e.proposal.kind,
            e.adoption,
            e.review,
            e.proposal.statement,
            e.proposal.scope,
            e.proposal.rationale,
            e.proposal.depends_on.join(", ")
        ));
    }
    out.push_str("\n## 출처\n\n");
    for s in &p.sources {
        out.push_str(&format!("<a id=\"{}\"></a>\n### {}\n\nID: {} · {:?} · 판본 {} · 위치: {}\n\n텍스트 hash: `{}`\n\n```text\n{}\n```\n\n",s.id,s.title,s.id,s.access,s.revision,s.locator,s.hash,s.numbered()));
    }
    out
}
