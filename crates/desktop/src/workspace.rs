mod content;
mod progress;
mod run_view;
mod settings;
mod style;
use gpui_kit::{
    assets::IconName as Lucide,
    component::{
        ActiveTheme, Disableable, Icon, Selectable, Sizable, Theme, ThemeMode, TitleBar, WindowExt,
        button::{Button, ButtonVariants},
        h_flex,
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
        scroll::ScrollableElement,
        sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem},
        spinner::Spinner,
        switch::Switch,
        text::TextView,
        v_flex,
    },
    prelude::*,
    *,
};
use progress::friendly_error;
use std::{
    path::PathBuf,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use style::*;
use theory_core::Role;
use theory_core::*;
use theory_engine::{
    providers::{Connection, ProviderKind, Settings},
    service::{Command, Event, Service, Snapshot},
    storage::data_directory,
};

actions!(
    theory,
    [
        RunQuestion,
        NewQuestion,
        NewProject,
        ImportSource,
        ExportProject,
        OpenSettings,
        FindSource,
        Quit
    ]
);
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Inquiry,
    Notes,
    Sources,
    History,
    Settings,
    Direction,
}
/// Views of one question: the answer, the model exchange, the cited text, the raw record.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RunTab {
    Answer,
    Debate,
    Evidence,
    Log,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum NotesFilter {
    All,
    Claims,
    Assumptions,
}
#[derive(Clone)]
enum Inspector {
    Entry(String),
    Source(String, Option<Evidence>),
    Task(String),
    Critique(String, String),
    None,
}
pub struct Workspace {
    service: Service,
    snapshot: Option<Snapshot>,
    page: Page,
    run_tab: RunTab,
    notes_filter: NotesFilter,
    inspector: Inspector,
    selected_run: Option<String>,
    selected_revision: Option<u64>,
    prompt: Entity<TextareaState>,
    search: Entity<InputState>,
    source_title: Entity<InputState>,
    source_locator: Entity<InputState>,
    source_text: Entity<TextareaState>,
    new_title: Entity<InputState>,
    research_goal: Entity<TextareaState>,
    research_guidance: Entity<TextareaState>,
    show_advanced: bool,
    show_excerpt_form: bool,
    last_clock: Instant,
    directory: PathBuf,
    codex_model: Entity<InputState>,
    claude_model: Entity<InputState>,
    codex_path: Entity<InputState>,
    claude_path: Entity<InputState>,
    settings: Settings,
    connections: Vec<Connection>,
    budget: Budget,
    targets: Vec<String>,
    pending: usize,
    background: bool,
    status: String,
    error: Option<String>,
    focus: FocusHandle,
    show_inspector: bool,
    draft_changed: Option<Instant>,
    layout_changed: Option<Instant>,
    _subscriptions: Vec<Subscription>,
    _ticker: Task<()>,
}
impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_directory(data_directory(), window, cx)
    }
    pub fn with_directory(directory: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let prompt = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(3)
                .placeholder("설명하고 싶은 현상, 세우고 싶은 가설, 검토할 주장을 적으세요…")
        });
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder("제목이나 본문에서 찾기…"));
        let source_title =
            cx.new(|cx| InputState::new(window, cx).placeholder("논문 제목 또는 출처"));
        let source_locator =
            cx.new(|cx| InputState::new(window, cx).placeholder("예: Results, p. 4, paragraph 2"));
        let source_text = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(7)
                .placeholder("실제 원문 구절을 그대로 붙여 넣으세요")
        });
        let new_title = cx.new(|cx| InputState::new(window, cx).placeholder("새 연구 프로젝트"));
        let research_goal = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(3)
                .placeholder("이 프로젝트에서 설명하거나 알아내려는 현상")
        });
        let research_guidance = cx.new(|cx| TextareaState::new(window,cx).rows(5).placeholder("중시할 관점, 비교 기준, 범위와 제약을 자유롭게 적으세요. 비워 두면 질문에 맞춰 선택합니다."));
        let codex_model = cx.new(|cx| InputState::new(window, cx).placeholder("계정 기본 모델"));
        let claude_model =
            cx.new(|cx| InputState::new(window, cx).placeholder("Claude Code 계정 기본 모델"));
        let codex_path = cx.new(|cx| InputState::new(window, cx).placeholder("자동 탐지"));
        let claude_path = cx.new(|cx| InputState::new(window, cx).placeholder("자동 탐지"));
        let subscriptions = vec![
            cx.subscribe_in(&prompt, window, |this, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.draft_changed = Some(Instant::now());
                    cx.notify();
                }
                if matches!(
                    event,
                    InputEvent::PressEnter {
                        secondary: true,
                        shift: false
                    }
                ) {
                    this.start_run(cx);
                }
            }),
            cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify()),
        ];
        let ticker = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if this
                    .update_in(cx, |this, window, cx| this.poll(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            service: Service::start(directory.clone()),
            directory,
            research_goal,
            research_guidance,
            show_advanced: false,
            show_excerpt_form: false,
            last_clock: Instant::now(),
            snapshot: None,
            page: Page::Inquiry,
            run_tab: RunTab::Answer,
            notes_filter: NotesFilter::All,
            inspector: Inspector::None,
            selected_run: None,
            selected_revision: None,
            prompt,
            search,
            source_title,
            source_locator,
            source_text,
            new_title,
            codex_model,
            claude_model,
            codex_path,
            claude_path,
            settings: Settings::default(),
            connections: vec![],
            budget: Budget::Standard,
            targets: vec![],
            pending: 0,
            background: false,
            status: "연구 기록을 여는 중".into(),
            error: None,
            focus: cx.focus_handle(),
            show_inspector: false,
            draft_changed: None,
            layout_changed: None,
            _subscriptions: subscriptions,
            _ticker: ticker,
        }
    }
    pub fn shutdown(&mut self, cx: &App) {
        self.service.cancel();
        if self.snapshot.is_some() {
            let _ = self
                .service
                .send(Command::Draft(self.prompt.read(cx).value().to_string()));
        }
        let _ = self.service.send(Command::Shutdown);
    }
    fn busy(&self) -> bool {
        self.pending > 0 || self.background || self.snapshot.is_none()
    }
    fn running(&self) -> Option<&Run> {
        self.snapshot
            .as_ref()?
            .project
            .runs
            .iter()
            .rev()
            .find(|r| r.state == RunState::Running)
    }
    fn submit(&mut self, command: Command, cx: &mut Context<Self>) {
        if !self.busy() {
            self.service.cancel.store(false, Ordering::SeqCst);
        }
        if matches!(
            &command,
            Command::NewProject(_)
                | Command::OpenProject(_)
                | Command::OpenFolder(_)
                | Command::ShareProject
                | Command::ImportBundle(_)
        ) && self.snapshot.is_some()
        {
            let _ = self
                .service
                .send(Command::Draft(self.prompt.read(cx).value().to_string()));
            self.draft_changed = None;
        }
        if let Err(e) = self.service.send(command) {
            self.error = Some(e.to_string());
        } else {
            self.pending += 1;
            self.error = None;
            self.status = "처리 중".into();
        }
        cx.notify();
    }
    fn poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let events: Vec<_> = self.service.events.try_iter().collect();
        let changed = !events.is_empty();
        for event in events {
            match event {
                Event::Snapshot(s) => {
                    let new_project = self
                        .snapshot
                        .as_ref()
                        .is_none_or(|old| old.project.id != s.project.id);
                    let first = self.snapshot.is_none();
                    if new_project {
                        self.research_goal.update(cx, |f, cx| {
                            f.set_value(s.project.research_goal.clone(), window, cx)
                        });
                        self.research_guidance
                            .update(cx, |f, cx| f.set_value(s.project.guidance(), window, cx));
                        self.prompt
                            .update(cx, |p, cx| p.set_value(s.project.draft.clone(), window, cx));
                        self.new_title.update(cx, |field, cx| {
                            field.set_value(s.project.title.clone(), window, cx)
                        });
                        self.selected_run = s.project.runs.last().map(|r| r.id.clone());
                        self.inspector = Inspector::None;
                        self.show_inspector = false;
                        self.targets.clear();
                    }
                    if first {
                        self.settings = s.settings.clone();
                        self.sync_settings(window, cx);
                        Theme::change(
                            if self.settings.dark {
                                ThemeMode::Dark
                            } else {
                                ThemeMode::Light
                            },
                            Some(window),
                            cx,
                        );
                        if self.settings.zoom >= 12 {
                            Theme::update(cx, |t| t.font_size = px(self.settings.zoom as f32));
                        }
                        self.status = "준비됨".into();
                        self.prompt.update(cx, |p, cx| p.focus(window, cx));
                    }
                    self.snapshot = Some(*s);
                }
                Event::Progress(p, status) => {
                    if p.runs.last().map(|r| r.state) == Some(RunState::Running) {
                        self.selected_run = p.runs.last().map(|r| r.id.clone());
                    }
                    if let Some(s) = &mut self.snapshot {
                        s.project = *p;
                    }
                    self.status = status;
                }
                Event::Status(s) => self.status = s,
                Event::Connection(c) => {
                    self.connections.retain(|old| old.provider != c.provider);
                    self.connections.push(c);
                }
                Event::Error(e) => {
                    self.error = Some(e);
                    self.status = "작업을 완료하지 못했습니다".into();
                }
                Event::Finished => {
                    self.pending = self.pending.saturating_sub(1);
                    if self.pending == 0 && !self.background {
                        self.status = self
                            .current_run()
                            .filter(|r| r.state != RunState::Running)
                            .map(|r| r.state.label().to_string())
                            .unwrap_or_else(|| "준비됨".into());
                    }
                }
                Event::Background(active) => self.background = active,
            }
        }
        if !self.busy()
            && self
                .draft_changed
                .is_some_and(|t| t.elapsed() > Duration::from_millis(650))
        {
            self.draft_changed = None;
            let _ = self
                .service
                .send(Command::Draft(self.prompt.read(cx).value().to_string()));
        }
        if !self.busy()
            && self
                .layout_changed
                .is_some_and(|t| t.elapsed() > Duration::from_millis(800))
        {
            self.layout_changed = None;
            self.submit(Command::Settings(self.settings.clone()), cx);
        }
        let clock_tick = self.busy() && self.last_clock.elapsed() >= Duration::from_secs(1);
        if clock_tick {
            self.last_clock = Instant::now();
        }
        if changed || clock_tick {
            cx.notify();
        }
    }
    fn start_run(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let question = self.prompt.read(cx).value().to_string();
        if question.trim().is_empty() {
            self.error = Some("질문을 입력하세요".into());
            cx.notify();
            return;
        }
        self.page = Page::Inquiry;
        self.run_tab = RunTab::Debate;
        self.service.cancel.store(false, Ordering::SeqCst);
        self.save_settings(cx);
        let _ = self.service.send(Command::Draft(question.clone()));
        self.submit(
            Command::Run {
                question,
                targets: self.targets.clone(),
                budget: self.budget,
            },
            cx,
        );
    }
    fn clear_question(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.page = Page::Inquiry;
        self.selected_run = None;
        self.targets.clear();
        self.prompt.update(cx, |p, cx| {
            p.set_value("", window, cx);
            p.focus(window, cx);
        });
        cx.notify();
    }
    fn open_inspector(&mut self, inspector: Inspector, cx: &mut Context<Self>) {
        self.inspector = inspector;
        self.show_inspector = true;
        cx.notify();
    }
    fn project_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let field = self.new_title.clone();
        let view = cx.entity().downgrade();
        let projects = self
            .snapshot
            .as_ref()
            .map(|s| (s.projects.clone(), s.project.selection_key()))
            .unwrap_or_default();
        self.new_title
            .update(cx, |s, cx| s.set_value("", window, cx));
        window.open_dialog(cx, move |dialog, _, cx| {
            let mut list = v_flex().gap_1();
            for (id, title) in &projects.0 {
                let view = view.clone();
                let id = id.clone();
                let current = id == projects.1;
                list = list.child(observed(
                    row_button(
                        SharedString::from(format!("switch-{id}")),
                        if current {
                            Lucide::FolderOpen
                        } else {
                            Lucide::Folder
                        },
                        title.clone(),
                        current,
                        cx,
                    )
                    .on_click(move |_, window, cx| {
                        let _ = view.update(cx, |this, cx| {
                            if !current {
                                this.submit(Command::OpenProject(id.clone()), cx);
                            }
                        });
                        window.close_dialog(cx);
                    }),
                ));
            }
            let open_view = view.clone();
            let bundle_view = view.clone();
            let create_view = view.clone();
            dialog
                .title("프로젝트")
                .w(px(560.))
                .child(
                    v_flex()
                        .gap_4()
                        .child(v_flex().gap_1().child(muted("열 프로젝트", cx)).child(list))
                        .child(
                            v_flex().gap_2().child(muted("새 프로젝트", cx)).child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(Input::new(&field).id("project-title")),
                                    )
                                    .child(
                                        Button::new("create-project")
                                            .primary()
                                            .label("만들기")
                                            .on_click(move |_, window, cx| {
                                                let _ = create_view.update(cx, |this, cx| {
                                                    let title =
                                                        this.new_title.read(cx).value().to_string();
                                                    if title.trim().is_empty() {
                                                        return;
                                                    }
                                                    this.submit(Command::NewProject(title), cx);
                                                    window.close_dialog(cx);
                                                });
                                            }),
                                    ),
                            ),
                        ),
                )
                .footer(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("open-project-folder")
                                .icon(Icon::new(Lucide::FolderInput))
                                .label("연구 폴더 열기…")
                                .on_click(move |_, window, cx| {
                                    let _ = open_view
                                        .update(cx, |this, cx| this.choose_path("project", cx));
                                    window.close_dialog(cx);
                                }),
                        )
                        .child(
                            Button::new("import-bundle")
                                .ghost()
                                .label("bundle 가져오기…")
                                .on_click(move |_, window, cx| {
                                    let _ = bundle_view
                                        .update(cx, |this, cx| this.choose_path("bundle", cx));
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }
    fn choose_path(&mut self, kind: &str, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let kind = kind.to_string();
        let files = kind == "source";
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files,
            directories: !files,
            multiple: files,
            prompt: Some(
                match kind.as_str() {
                    "source" => "자료 추가 (PDF · EPUB · DOCX · 텍스트)",
                    "project" => "연구 프로젝트 폴더 선택",
                    "bundle" => "Theory bundle 폴더 열기",
                    _ => "내보낼 폴더 선택",
                }
                .into(),
            ),
        });
        cx.spawn(async move |this, cx| match receiver.await {
            Ok(Ok(Some(paths))) => {
                let _ = this.update(cx, |this, cx| {
                    if this.busy() {
                        return;
                    }
                    for path in paths {
                        this.submit(
                            match kind.as_str() {
                                "source" => Command::Import(path),
                                "project" => Command::OpenFolder(path),
                                "bundle" => Command::ImportBundle(path),
                                _ => Command::Export(path),
                            },
                            cx,
                        );
                    }
                });
            }
            Ok(Err(e)) => {
                let _ = this.update(cx, |this, cx| {
                    this.error = Some(e.to_string());
                    cx.notify();
                });
            }
            _ => {}
        })
        .detach();
    }
    fn current_run(&self) -> Option<&Run> {
        let p = &self.snapshot.as_ref()?.project;
        self.selected_run
            .as_ref()
            .and_then(|id| p.runs.iter().find(|r| r.id == *id))
    }
    fn connection(&self, provider: ProviderKind) -> Option<&Connection> {
        self.connections
            .iter()
            .find(|c| c.provider == provider.label())
    }
    fn nav_item(
        &self,
        page: Page,
        label: &str,
        icon: Lucide,
        count: Option<usize>,
        cx: &mut Context<Self>,
    ) -> SidebarMenuItem {
        let mut item = SidebarMenuItem::new(label.to_string())
            .icon(Icon::new(icon))
            .active(self.page == page)
            .on_click(cx.listener(move |this, _, _, cx| {
                if page == Page::Settings {
                    this.open_settings(cx);
                } else {
                    this.page = page;
                }
                cx.notify();
            }));
        if let Some(n) = count.filter(|n| *n > 0) {
            item = item.suffix(move |_, cx| {
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(n.to_string())
            });
        }
        item
    }
    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = self.snapshot.as_ref();
        let project_title = snapshot
            .map(|s| s.project.title.clone())
            .unwrap_or_else(|| "여는 중…".into());
        let mut questions = SidebarMenu::new();
        if let Some(s) = snapshot {
            for r in s.project.runs.iter().rev().take(40) {
                let id = r.id.clone();
                let icon = match r.state {
                    RunState::Running => Lucide::LoaderCircle,
                    RunState::Answered => Lucide::CircleCheck,
                    RunState::EvidenceInsufficient | RunState::Partial => Lucide::CircleAlert,
                    RunState::Failed => Lucide::CircleX,
                    _ => Lucide::CircleMinus,
                };
                questions = questions.child(
                    SidebarMenuItem::new(short(&r.question, 26))
                        .icon(Icon::new(icon))
                        .active(
                            self.page == Page::Inquiry && self.selected_run.as_ref() == Some(&r.id),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selected_run = Some(id.clone());
                            this.page = Page::Inquiry;
                            cx.notify();
                        })),
                );
            }
        }
        let (notes, sources) = snapshot
            .map(|s| {
                (
                    s.project
                        .entries
                        .iter()
                        .filter(|e| e.adoption != Adoption::Withdrawn)
                        .count(),
                    s.project.sources.iter().filter(|s| s.active).count(),
                )
            })
            .unwrap_or_default();
        let library = SidebarMenu::new()
            .child(self.nav_item(Page::Notes, "주장·가정", Lucide::Lightbulb, Some(notes), cx))
            .child(self.nav_item(Page::Sources, "자료", Lucide::Library, Some(sources), cx))
            .child(self.nav_item(Page::Direction, "연구 방향", Lucide::Compass, None, cx))
            .child(self.nav_item(Page::History, "판본", Lucide::GitBranch, None, cx));
        let has_runs = snapshot.is_some_and(|s| !s.project.runs.is_empty());
        Sidebar::new("navigation")
            .w_full()
            .h_full()
            .header(
                v_flex()
                    .w_full()
                    .gap_3()
                    .pt_1()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(Lucide::BrainCircuit)
                                    .text_color(provider_color("Claude", cx)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Theory Studio"),
                            ),
                    )
                    .child(observed(
                        row_button(
                            "project-switcher",
                            Lucide::ChevronsUpDown,
                            project_title,
                            false,
                            cx,
                        )
                        .border_1()
                        .border_color(cx.theme().sidebar_border)
                        .on_click(cx.listener(|this, _, w, cx| this.project_dialog(w, cx))),
                    ))
                    .child(
                        Button::new("new-question")
                            .outline()
                            .w_full()
                            .icon(Icon::new(Lucide::SquarePen))
                            .label("새 질문")
                            .tooltip("⌘N")
                            .disabled(self.busy())
                            .on_click(cx.listener(|this, _, w, cx| this.clear_question(w, cx))),
                    ),
            )
            .child(SidebarGroup::new("질문").child(if has_runs {
                questions
            } else {
                SidebarMenu::new().child(SidebarMenuItem::new("아직 질문이 없습니다").disable(true))
            }))
            .child(SidebarGroup::new("연구 노트").child(library))
            .footer(
                v_flex()
                    .w_full()
                    .gap_0p5()
                    .child(observed(
                        row_button(
                            "nav-settings",
                            Lucide::Settings,
                            "설정",
                            self.page == Page::Settings,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.open_settings(cx))),
                    ))
                    .child(observed(
                        row_button("nav-export", Lucide::Share, "내보내기…", false, cx)
                            .on_click(cx.listener(|this, _, _, cx| this.choose_path("export", cx))),
                    )),
            )
    }
    fn composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.busy();
        let mut scopes = h_flex()
            .gap_0p5()
            .p_0p5()
            .rounded(cx.theme().radius)
            .bg(cx.theme().muted);
        for b in [Budget::Quick, Budget::Standard, Budget::Deep] {
            scopes = scopes.child(
                Button::new(match b {
                    Budget::Quick => "quick",
                    Budget::Standard => "standard",
                    Budget::Deep => "deep",
                })
                .ghost()
                .xsmall()
                .selected(self.budget == b)
                .label(b.name())
                .tooltip(b.detail())
                .disabled(busy)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.budget = b;
                    cx.notify();
                })),
            );
        }
        let author = self.settings.primary;
        let pairing = if self.settings.cross_review {
            format!(
                "{} 구성 · {} 비판",
                author.display(),
                author.other().display()
            )
        } else {
            format!("{} 단독", author.display())
        };
        let targets = self.snapshot.as_ref().map(|s| {
            self.targets
                .iter()
                .filter_map(|t| s.project.entries.iter().find(|e| e.proposal.id == *t))
                .map(|e| short(&e.proposal.statement, 60))
                .collect::<Vec<_>>()
        });
        v_flex().px_6().pb_4().pt_2().child(
            v_flex()
                .w_full()
                .max_w(rems(52.))
                .mx_auto()
                .gap_2()
                .p_2()
                .rounded(cx.theme().radius_lg)
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .shadow_sm()
                .when_some(targets.filter(|t| !t.is_empty()), |v, targets| {
                    v.child(
                        h_flex()
                            .px_1()
                            .gap_2()
                            .child(Icon::new(Lucide::Crosshair).xsmall())
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("후속 질문 대상 · {}", targets.join(" / "))),
                            )
                            .child(
                                Button::new("clear-target")
                                    .ghost()
                                    .xsmall()
                                    .icon(Icon::new(Lucide::X))
                                    .tooltip("대상 해제")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.targets.clear();
                                        cx.notify();
                                    })),
                            ),
                    )
                })
                .child(
                    Textarea::new(&self.prompt)
                        .appearance(false)
                        .aria_label("연구 질문")
                        .accessibility_id("question-input")
                        .readonly(busy),
                )
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_2()
                        .items_center()
                        .child(scopes)
                        .child(
                            Button::new("pairing")
                                .ghost()
                                .xsmall()
                                .icon(Icon::new(Lucide::ArrowLeftRight))
                                .label(pairing)
                                .tooltip(if self.settings.cross_review {
                                    "클릭하면 구성과 비판 모델을 서로 바꿉니다"
                                } else {
                                    "클릭하면 다른 모델로 바꿉니다 · 교차 비판은 설정에서"
                                })
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.settings.primary = this.settings.primary.other();
                                    this.save_settings(cx);
                                })),
                        )
                        .child(
                            Switch::new("search-enabled")
                                .xsmall()
                                .label("문헌 검색")
                                .checked(self.settings.search_enabled)
                                .disabled(busy)
                                .on_click(cx.listener(|this, v: &bool, _, cx| {
                                    this.settings.search_enabled = *v;
                                    this.save_settings(cx);
                                })),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("⌘↵"),
                        )
                        .child(
                            Button::new("run-question")
                                .primary()
                                .small()
                                .icon(Icon::new(Lucide::Send))
                                .label(if self.running().is_some() {
                                    "토론 중"
                                } else {
                                    "실행"
                                })
                                .disabled(busy || self.prompt.read(cx).value().trim().is_empty())
                                .on_click(cx.listener(|this, _, _, cx| this.start_run(cx))),
                        ),
                ),
        )
    }
    fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut models = h_flex().gap_3();
        for provider in [ProviderKind::Claude, ProviderKind::Codex] {
            let c = self.connection(provider);
            let (color, tip) = match c {
                Some(c) if c.authenticated => (cx.theme().success, c.message.clone()),
                Some(c) => (cx.theme().danger, c.message.clone()),
                None => (cx.theme().muted_foreground, "연결 상태 확인 전".into()),
            };
            models = models.child(
                Button::new(SharedString::from(format!("status-{}", provider.label())))
                    .ghost()
                    .xsmall()
                    .child(
                        h_flex()
                            .gap_1p5()
                            .items_center()
                            .child(dot(color))
                            .child(provider.display()),
                    )
                    .tooltip(tip)
                    .on_click(cx.listener(|this, _, _, cx| this.open_settings(cx))),
            );
        }
        h_flex()
            .px_3()
            .h(rems(2.))
            .gap_3()
            .items_center()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .text_xs()
            .when(self.busy(), |v| v.child(Spinner::new().xsmall()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.status.clone()),
            )
            .when(self.busy(), |v| {
                v.child(
                    Button::new("cancel-operation")
                        .xsmall()
                        .danger()
                        .label("중단")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.service.cancel();
                            this.status = "중단 요청됨 · 완료한 결과 보존 중".into();
                            cx.notify();
                        })),
                )
            })
            .child(models)
    }
}
impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self
            .snapshot
            .as_ref()
            .map(|s| format!("{} — Theory Studio", s.project.title))
            .unwrap_or_else(|| "Theory Studio".into());
        let content = self.content(cx);
        let inspector = self.inspector(cx);
        // Fixed columns: a resizable group rescales every panel whenever the window or a
        // neighbouring panel changes, which made the sidebar jump.
        let panes = h_flex()
            .size_full()
            .child(
                div()
                    .flex_none()
                    .w(rems(15.))
                    .h_full()
                    .child(self.sidebar(cx)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .bg(cx.theme().background)
                    .child(content)
                    .when(self.page == Page::Inquiry, |v| v.child(self.composer(cx))),
            )
            .when(self.show_inspector, |v| {
                v.child(div().flex_none().w(rems(26.)).h_full().child(inspector))
            });
        v_flex()
            .size_full()
            .key_context("TheoryStudio")
            .track_focus(&self.focus)
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|this, _: &RunQuestion, _, cx| this.start_run(cx)))
            .on_action(cx.listener(|this, _: &NewQuestion, w, cx| this.clear_question(w, cx)))
            .on_action(cx.listener(|this, _: &NewProject, w, cx| this.project_dialog(w, cx)))
            .on_action(cx.listener(|this, _: &ImportSource, _, cx| this.choose_path("source", cx)))
            .on_action(cx.listener(|this, _: &ExportProject, _, cx| this.choose_path("export", cx)))
            .on_action(cx.listener(|this, _: &OpenSettings, _, cx| {
                this.open_settings(cx);
            }))
            .on_action(cx.listener(|this, _: &FindSource, w, cx| {
                this.page = Page::Sources;
                this.search.update(cx, |s, cx| s.focus(w, cx));
                cx.notify();
            }))
            .child(
                TitleBar::new().child(
                    h_flex()
                        .flex_1()
                        .justify_center()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(title),
                ),
            )
            .when_some(self.error.clone(), |v, error| {
                v.child(
                    h_flex()
                        .items_start()
                        .gap_3()
                        .px_4()
                        .py_2()
                        .bg(cx.theme().danger.opacity(0.08))
                        .border_b_1()
                        .border_color(cx.theme().danger.opacity(0.3))
                        .child(Icon::new(Lucide::TriangleAlert).text_color(cx.theme().danger))
                        .child(div().flex_1().text_sm().child(friendly_error(&error)))
                        .when(self.snapshot.is_none(), |v| {
                            v.child(
                                Button::new("retry-startup")
                                    .small()
                                    .label("다시 열기")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.service = Service::start(this.directory.clone());
                                        this.pending = 0;
                                        this.error = None;
                                        cx.notify();
                                    })),
                            )
                        })
                        .child(
                            Button::new("dismiss-error")
                                .ghost()
                                .xsmall()
                                .icon(Icon::new(Lucide::X))
                                .tooltip("닫기")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.error = None;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(div().flex_1().min_h_0().min_w_0().child(panes))
            .child(self.status_bar(cx))
    }
}
fn muted(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}
fn short(s: &str, n: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s
    }
}
fn document(id: impl Into<ElementId>, text: String) -> TextView {
    TextView::markdown(id, text)
        .selectable(true)
        .scrollable(false)
        .on_link_click(|url, _, _, cx| {
            if url.starts_with("https://") || url.starts_with("http://") {
                cx.open_url(url);
            }
        })
}

#[cfg(test)]
mod tests;
