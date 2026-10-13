use crate::{execution, providers::*, storage::*};
use anyhow::{Context, Result, ensure};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
};
use theory_core::*;

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Snapshot {
    pub project: Project,
    pub projects: Vec<(String, String)>,
    pub settings: Settings,
}
#[derive(Clone, Debug)]
pub enum Event {
    Snapshot(Box<Snapshot>),
    Progress(Box<Project>, String),
    Status(String),
    Connection(Connection),
    Error(String),
    Finished,
    Background(bool),
}
#[derive(Clone, Debug)]
pub enum Command {
    NewProject(String),
    OpenProject(String),
    Rename(String),
    Draft(String),
    ResearchDirection {
        goal: String,
        guidance: String,
    },
    OpenFolder(PathBuf),
    ShareProject,
    SyncResources,
    Probe(ProviderKind),
    Import(PathBuf),
    Excerpt {
        title: String,
        text: String,
        locator: String,
        access: Access,
    },
    Run {
        question: String,
        targets: Vec<String>,
        budget: Budget,
    },
    Adopt(String),
    Withdraw(String),
    Restore(u64),
    Export(PathBuf),
    ImportBundle(PathBuf),
    OpenSource(String),
    Settings(Settings),
    Inspect(ProviderKind),
    Login(ProviderKind),
    LogoutCodex,
    Shutdown,
}
pub struct Service {
    pub sender: Sender<Command>,
    pub events: Receiver<Event>,
    pub cancel: Cancel,
}
impl Service {
    pub fn start(root: PathBuf) -> Self {
        let (sender, commands) = mpsc::channel();
        let (updates, events) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let token = cancel.clone();
        std::thread::spawn(move || {
            if let Err(e) = serve(root, commands, &updates, &token) {
                let _ = updates.send(Event::Error(e.to_string()));
                let _ = updates.send(Event::Finished);
            }
        });
        Self {
            sender,
            events,
            cancel,
        }
    }
    pub fn send(&self, command: Command) -> Result<()> {
        self.sender
            .send(command)
            .context("저장 작업자 연결이 종료되었습니다")
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.cancel();
        let _ = self.sender.send(Command::Shutdown);
    }
}
fn serve(
    root: PathBuf,
    commands: Receiver<Command>,
    updates: &Sender<Event>,
    cancel: &Cancel,
) -> Result<()> {
    let _lock = lock_root(&root)?;
    let settings_path = root.join("settings.json");
    let mut settings: Settings = if settings_path.exists() {
        serde_json::from_slice(&read_bounded(&settings_path, 65536)?)
            .context("설정 파일을 읽지 못했습니다. 원본을 보존했습니다.")?
    } else {
        Settings::default()
    };
    let list = projects(&root)?;
    let mut store = if list.is_empty() {
        Store::create(&root, "신경생물학 연구")?
    } else {
        let selected = list
            .iter()
            .find(|(id, _)| id == &settings.selected_project)
            .unwrap_or(&list[0]);
        Store::open(&root, &selected.0)?
    };
    settings.selected_project = store.project.selection_key();
    let snapshot = |store: &Store, settings: &Settings| -> Result<()> {
        updates.send(Event::Snapshot(Box::new(Snapshot {
            project: store.project.clone(),
            projects: projects(&root)?,
            settings: settings.clone(),
        })))?;
        Ok(())
    };
    snapshot(&store, &settings)?;
    // Connection state is shown from the start; checking it must not hold the UI.
    {
        let (root, settings, updates) = (root.clone(), settings.clone(), updates.clone());
        std::thread::spawn(move || {
            let quiet: Cancel = Arc::new(AtomicBool::new(false));
            for p in [ProviderKind::Claude, ProviderKind::Codex] {
                let c = Subscriptions::new(&root, settings.clone())
                    .inspect(p, &quiet)
                    .unwrap_or_else(|e| Connection {
                        provider: p.label().into(),
                        installed: !e.to_string().contains("찾지 못했습니다"),
                        message: e.to_string(),
                        ..Default::default()
                    });
                if updates.send(Event::Connection(c)).is_err() {
                    break;
                }
            }
        });
    }
    if store.project.workspace_folder.is_some() {
        updates.send(Event::Background(true))?;
        if let Err(e) = store.sync_resources(cancel, &mut |s| {
            let _ = updates.send(Event::Status(s));
        }) {
            updates.send(Event::Error(e.to_string()))?;
        }
        snapshot(&store, &settings)?;
        updates.send(Event::Background(false))?;
    }
    loop {
        let command = match commands.recv_timeout(std::time::Duration::from_secs(15)) {
            Ok(command) => command,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if store.project.workspace_folder.is_some() && !cancel.load(Ordering::SeqCst) {
                    updates.send(Event::Background(true))?;
                    match store.sync_resources(cancel, &mut |s| {
                        let _ = updates.send(Event::Status(s));
                    }) {
                        Ok(true) => {
                            snapshot(&store, &settings)?;
                        }
                        Ok(false) => {}
                        Err(e) => {
                            updates.send(Event::Status(format!("자료 확인 필요: {e}")))?;
                        }
                    }
                    updates.send(Event::Background(false))?;
                }
                continue;
            }
        };
        if matches!(command, Command::Shutdown) {
            break;
        }
        if let Command::Draft(text) = &command {
            store.project.draft = text.clone();
            if let Err(e) = store.save("입력 초안 저장") {
                let _ = updates.send(Event::Error(e.to_string()));
            }
            continue;
        }
        // Cancellation belongs to an operation. The UI clears it before sending a new long command.
        let before = store.project.clone();
        let result = (|| -> Result<()> {
            match command {
                Command::NewProject(title) => {
                    ensure!(!title.trim().is_empty(), "프로젝트 제목을 입력하세요");
                    store = if root == data_directory() && scholarpen_root().is_dir() {
                        let name = title.replace(['/', ':', '\\'], "-");
                        ensure!(
                            !name.trim().is_empty() && name != "." && name != "..",
                            "다른 프로젝트 제목을 입력하세요"
                        );
                        let folder = scholarpen_root().join(name);
                        ensure!(
                            !folder.exists(),
                            "동일한 폴더가 있습니다. 프로젝트 목록에서 여세요"
                        );
                        for path in [
                            "documents",
                            "drafts",
                            "resources/articles",
                            "resources/books",
                            "figures",
                            "exports",
                        ] {
                            std::fs::create_dir_all(folder.join(path))?;
                        }
                        Store::open_folder(&root, &folder)?
                    } else {
                        Store::create(&root, &title)?
                    };
                    settings.selected_project = store.project.selection_key();
                    atomic_write(&settings_path, &serde_json::to_vec_pretty(&settings)?)?;
                }
                Command::OpenProject(id) => {
                    if store.project.selection_key() != id {
                        store = Store::open(&root, &id)?;
                    }
                    store.sync_resources(cancel, &mut |s| {
                        let _ = updates.send(Event::Status(s));
                    })?;
                    settings.selected_project = store.project.selection_key();
                    atomic_write(&settings_path, &serde_json::to_vec_pretty(&settings)?)?;
                }
                Command::Rename(title) => {
                    ensure!(!title.trim().is_empty(), "제목을 입력하세요");
                    store.project.title = title;
                    store.save("프로젝트 이름 변경")?;
                }
                Command::Draft(text) => {
                    store.project.draft = text;
                    store.save("입력 초안 저장")?;
                }
                Command::ResearchDirection { goal, guidance } => {
                    ensure!(
                        goal.len() + guidance.len() <= 30000,
                        "연구 방향이 너무 깁니다"
                    );
                    store.project.research_goal = goal;
                    store.project.research_guidance = Some(guidance);
                    store.project.profiles = vec!["general-research".into()];
                    store.project.checkpoint("사용자가 연구 방향 수정");
                    store.save("연구 방향 저장")?;
                }
                Command::OpenFolder(path) => {
                    let key = format!("folder:{}", path.canonicalize()?.to_string_lossy());
                    if store.project.selection_key() != key {
                        store = Store::open_folder(&root, &path)?;
                    }
                    settings.selected_project = store.project.selection_key();
                    atomic_write(&settings_path, &serde_json::to_vec_pretty(&settings)?)?;
                    store.sync_resources(cancel, &mut |s| {
                        let _ = updates.send(Event::Status(s));
                    })?;
                }
                Command::ShareProject => {
                    ensure!(
                        store.project.workspace_folder.is_none(),
                        "이미 공유 프로젝트입니다"
                    );
                    let name = store.project.title.replace(['/', ':', '\\'], "-");
                    store = store.share(&root, &scholarpen_root().join(name))?;
                    settings.selected_project = store.project.selection_key();
                    atomic_write(&settings_path, &serde_json::to_vec_pretty(&settings)?)?;
                    store.sync_resources(cancel, &mut |s| {
                        let _ = updates.send(Event::Status(s));
                    })?;
                }
                Command::SyncResources => {
                    store.sync_resources(cancel, &mut |s| {
                        let _ = updates.send(Event::Status(s));
                    })?;
                }
                Command::Import(path) => store.import_file_cancellable(&path, cancel)?,
                Command::Excerpt {
                    title,
                    text,
                    locator,
                    access,
                } => {
                    ensure!(
                        !title.trim().is_empty() && !text.trim().is_empty(),
                        "출처 제목과 구절을 입력하세요"
                    );
                    ensure!(
                        text.len() <= 100_000,
                        "구절이 너무 깁니다. 파일로 가져오세요."
                    );
                    store
                        .project
                        .sources
                        .push(Source::new(title, text, locator, access));
                    store.project.checkpoint("원문 구절 추가");
                    store.save("구절 저장")?;
                }
                Command::Run {
                    question,
                    targets,
                    budget,
                } => {
                    store.sync_resources(cancel, &mut |s| {
                        let _ = updates.send(Event::Status(s));
                    })?;
                    let provider = Subscriptions::new(&root, settings.clone());
                    execution::run(
                        &mut store,
                        execution::RunInput::new(question, budget).with_targets(targets),
                        &settings,
                        &provider,
                        cancel,
                        &mut |p, status| {
                            let _ = updates.send(Event::Progress(Box::new(p.clone()), status));
                        },
                    )?;
                }
                Command::Adopt(id) => {
                    let proposal = store
                        .project
                        .entries
                        .iter()
                        .find(|e| e.proposal.id == id)
                        .context("제안 없음")?
                        .clone();
                    ensure!(
                        proposal.adoption != Adoption::Withdrawn,
                        "철회 항목은 과거 판본에서 복원하세요"
                    );
                    if let Some(old) = &proposal.proposal.replaces_id {
                        store.project.withdraw(old)?;
                    }
                    store
                        .project
                        .entries
                        .iter_mut()
                        .find(|e| e.proposal.id == id)
                        .unwrap()
                        .adoption = Adoption::Adopted;
                    store.project.checkpoint(&format!("{id} 사용자 채택"));
                    store.save("사용자 채택; 과학적 확정과 구별")?;
                }
                Command::Withdraw(id) => {
                    store.project.withdraw(&id)?;
                    store.save("의존 항목 재검토")?;
                }
                Command::Restore(n) => {
                    let r = store
                        .project
                        .history
                        .iter()
                        .find(|r| r.number == n)
                        .context("판본 없음")?
                        .clone();
                    store.project.entries = r.entries;
                    store
                        .project
                        .checkpoint(&format!("판본 {n}의 원장을 새 작업안으로 복원"));
                    store.save("새 판본으로 복원")?;
                }
                Command::Export(path) => {
                    let bundle = store.export(&path)?;
                    updates.send(Event::Status(format!("내보냄: {}", bundle.display())))?;
                }
                Command::ImportBundle(path) => {
                    store = Store::import_bundle(&root, &path)?;
                    settings.selected_project = store.project.selection_key();
                    atomic_write(&settings_path, &serde_json::to_vec_pretty(&settings)?)?;
                }
                Command::OpenSource(id) => {
                    let source = store
                        .project
                        .sources
                        .iter()
                        .find(|s| s.id == id)
                        .context("출처 없음")?;
                    if source.asset_hash.is_none()
                        && crate::search::safe_public_url(&source.locator)
                    {
                        crate::platform::open_external(&source.locator)?;
                    } else {
                        crate::platform::open_external(store.source_path(&id)?)?;
                    }
                }
                Command::Settings(mut s) => {
                    s.selected_project = store.project.selection_key();
                    atomic_write(&settings_path, &serde_json::to_vec_pretty(&s)?)?;
                    settings = s;
                }
                Command::Probe(p) => {
                    let adapter = Subscriptions::new(&root, settings.clone());
                    let response = adapter.generate(p, &Request::new("Connection test: return an object with status = ok.".into(), serde_json::json!({"type":"object","properties":{"status":{"type":"string"}},"required":["status"],"additionalProperties":false}), settings.model(p).into(), std::time::Instant::now()+std::time::Duration::from_secs(90)),cancel,&mut |s| { let _=updates.send(Event::Status(s)); })?;
                    let mut c = adapter.inspect(p, cancel)?;
                    c.message = format!("실제 응답 확인 완료 · {}", response.model);
                    updates.send(Event::Connection(c))?;
                }
                Command::Inspect(p) => {
                    let connection =
                        Subscriptions::new(&root, settings.clone()).inspect(p, cancel)?;
                    updates.send(Event::Connection(connection))?;
                }
                Command::Login(p) => {
                    let connection =
                        Subscriptions::new(&root, settings.clone()).login(p, cancel, &mut |s| {
                            let _ = updates.send(Event::Status(s));
                        })?;
                    updates.send(Event::Connection(connection))?;
                }
                Command::LogoutCodex => {
                    Subscriptions::new(&root, settings.clone()).logout_codex(cancel)?;
                    updates.send(Event::Connection(Connection {
                        provider: "Codex".into(),
                        message: "이 앱의 ChatGPT 로그아웃 완료".into(),
                        installed: true,
                        ..Default::default()
                    }))?;
                }
                Command::Shutdown => {}
            }
            Ok(())
        })();
        if let Err(e) = result {
            // Reload persisted state after a failed mutation; never advertise unsaved data.
            if store.project.id == before.id {
                let _ = store.reload();
            }
            updates.send(Event::Error(e.to_string()))?;
        }
        snapshot(&store, &settings)?;
        updates.send(Event::Finished)?;
    }
    Ok(())
}
