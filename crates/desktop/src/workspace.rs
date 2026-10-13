mod content;
mod progress;
mod settings;
use gpui_kit::{
    component::{
        ActiveTheme, Disableable, Selectable, Sizable, Theme, ThemeMode, TitleBar, WindowExt,
        button::{Button, ButtonVariants},
        h_flex,
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
        resizable::{h_resizable, resizable_panel},
        scroll::ScrollableElement,
        sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem},
        text::TextView,
        v_flex,
    },
    prelude::*,
    *,
};
use progress::{friendly_error, task_state};
use std::{
    path::PathBuf,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
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
    Claims,
    Sources,
    Assumptions,
    History,
    Settings,
    Direction,
}
#[derive(Clone)]
enum Inspector {
    Entry(String),
    Source(String, Option<Evidence>),
    Task(String),
    None,
}
pub struct Workspace {
    service: Service,
    snapshot: Option<Snapshot>,
    page: Page,
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
    show_tasks: bool,
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
                .placeholder("설명하고 싶은 현상이나 검토할 주장을 적으세요…")
        });
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("현재 자료에서 찾기…"));
        let source_title =
            cx.new(|cx| InputState::new(window, cx).placeholder("논문 제목 또는 출처"));
        let source_locator =
            cx.new(|cx| InputState::new(window, cx).placeholder("예: Results, p. 4, paragraph 2"));
        let source_text = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(7)
                .placeholder("실제 원문 구절을 붙여 넣으세요")
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
            last_clock: Instant::now(),
            snapshot: None,
            page: Page::Inquiry,
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
            show_tasks: false,
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
                        self.status = "로컬 연구 기록 준비됨".into();
                        self.prompt.update(cx, |p, cx| p.focus(window, cx));
                    }
                    self.snapshot = Some(*s);
                }
                Event::Progress(p, status) => {
                    self.selected_run = p.runs.last().map(|r| r.id.clone());
                    if let Some(s) = &mut self.snapshot {
                        s.project = *p;
                    }
                    self.status = status;
                }
                Event::Status(s) => self.status = s,
                Event::Connection(c) => {
                    self.connections.retain(|old| old.provider != c.provider);
                    self.status = c.message.clone();
                    self.connections.push(c);
                }
                Event::Error(e) => {
                    self.error = Some(e);
                    self.status = "작업을 완료하지 못했습니다".into();
                }
                Event::Finished => self.pending = self.pending.saturating_sub(1),
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
        self.show_tasks = false;
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
    fn project_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let field = self.new_title.clone();
        let view = cx.entity().downgrade();
        self.new_title
            .update(cx, |s, cx| s.set_value("", window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            let view = view.clone();
            dialog
                .title("새 프로젝트")
                .child(
                    v_flex()
                        .gap_2()
                        .child("연구 제목")
                        .child(Input::new(&field).id("project-title")),
                )
                .footer(
                    Button::new("create-project")
                        .primary()
                        .label("프로젝트 만들기")
                        .on_click(move |_, window, cx| {
                            let _ = view.update(cx, |this, cx| {
                                let title = this.new_title.read(cx).value().to_string();
                                if title.trim().is_empty() {
                                    return;
                                }
                                this.submit(Command::NewProject(title), cx);
                                window.close_dialog(cx);
                            });
                        }),
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
                    "project" => "ScholarPen 연구 프로젝트 폴더 선택",
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
    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut menu = SidebarMenu::new();
        for (page, label) in [
            (Page::Inquiry, "연구"),
            (Page::Direction, "연구 방향"),
            (Page::Claims, "연구 노트"),
            (Page::Sources, "자료"),
            (Page::History, "판본"),
            (Page::Settings, "설정"),
        ] {
            menu = menu.child(
                SidebarMenuItem::new(label)
                    .active(self.page == page)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if page == Page::Settings {
                            this.open_settings(cx);
                        } else {
                            this.page = page;
                        }
                        cx.notify();
                    })),
            );
        }
        let mut recent = SidebarMenu::new();
        if let Some(s) = &self.snapshot {
            for (index, r) in s.project.runs.iter().enumerate().rev().take(30) {
                let id = r.id.clone();
                recent = recent.child(
                    SidebarMenuItem::new(format!(
                        "{} · {} · {}",
                        index + 1,
                        r.state.label(),
                        short(&r.question, 20)
                    ))
                    .active(self.selected_run.as_ref() == Some(&r.id))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_run = Some(id.clone());
                        this.page = Page::Inquiry;
                        cx.notify();
                    })),
                );
            }
        }
        let mut projects = SidebarMenu::new();
        if let Some(s) = &self.snapshot {
            for (id, title) in &s.projects {
                let id = id.clone();
                projects = projects.child(
                    SidebarMenuItem::new(short(title, 23))
                        .active(s.project.selection_key() == id)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.busy() {
                                this.submit(Command::OpenProject(id.clone()), cx);
                            }
                        })),
                );
            }
        }
        Sidebar::new("navigation")
            .w_full()
            .h_full()
            .header(
                v_flex()
                    .w_full()
                    .gap_3()
                    .py_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Theory Studio"),
                    )
                    .child(
                        Button::new("open-project-folder")
                            .label("프로젝트 폴더 열기…")
                            .disabled(self.busy())
                            .on_click(
                                cx.listener(|this, _, _, cx| this.choose_path("project", cx)),
                            ),
                    )
                    .child(
                        Button::new("new-project")
                            .label("새 프로젝트…")
                            .disabled(self.busy())
                            .on_click(cx.listener(|this, _, w, cx| this.project_dialog(w, cx))),
                    ),
            )
            .child(SidebarGroup::new("현재 프로젝트").child(menu))
            .child(SidebarGroup::new("프로젝트 폴더").child(projects))
            .child(SidebarGroup::new("최근 질문").child(recent))
            .footer(
                div()
                    .py_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("로컬 저장 · 구독 모델 연결"),
            )
    }
    fn composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut scopes = h_flex().gap_1();
        for b in [Budget::Quick, Budget::Standard, Budget::Deep] {
            scopes = scopes.child(
                Button::new(match b {
                    Budget::Quick => "quick",
                    Budget::Standard => "standard",
                    Budget::Deep => "deep",
                })
                .ghost()
                .small()
                .selected(self.budget == b)
                .label(b.label())
                .disabled(self.busy())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.budget = b;
                    cx.notify();
                })),
            );
        }
        v_flex()
            .gap_3()
            .p_4()
            .border_t_1()
            .border_color(cx.theme().border)
            .when(!self.targets.is_empty(), |v| {
                v.child(
                    h_flex()
                        .gap_2()
                        .child(muted(
                            format!("선택한 항목 {}개를 대상으로 후속 질문", self.targets.len()),
                            cx,
                        ))
                        .child(
                            Button::new("clear-target")
                                .ghost()
                                .small()
                                .label("대상 해제")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.targets.clear();
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(
                Textarea::new(&self.prompt)
                    .aria_label("연구 질문")
                    .accessibility_id("question-input")
                    .readonly(self.busy()),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(scopes)
                    .child(
                        gpui_kit::component::checkbox::Checkbox::new("search-enabled")
                            .label("문헌 검색")
                            .checked(self.settings.search_enabled)
                            .disabled(self.busy())
                            .on_change(cx.listener(|this, v, _, cx| {
                                this.settings.search_enabled = *v;
                                this.save_settings(cx);
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("run-question")
                            .primary()
                            .label(if self.busy() {
                                "진행 중"
                            } else {
                                "질문 실행"
                            })
                            .tooltip("⌘Enter")
                            .disabled(self.busy() || self.prompt.read(cx).value().trim().is_empty())
                            .on_click(cx.listener(|this, _, _, cx| this.start_run(cx))),
                    ),
            )
    }
}
impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self
            .snapshot
            .as_ref()
            .map(|s| s.project.title.clone())
            .unwrap_or_else(|| "연구 기록 열기".into());
        let content = self.content(cx);
        let inspector = self.inspector(cx);
        let rem = window.rem_size();
        let mut panes = h_resizable("research-panes")
            .on_resize(cx.listener(
                |this,
                 state: &Entity<gpui_kit::component::resizable::ResizableState>,
                 window,
                 cx| {
                    this.settings.pane_widths = state
                        .read(cx)
                        .sizes()
                        .iter()
                        .map(|s| *s / window.rem_size())
                        .collect();
                    this.layout_changed = Some(Instant::now());
                },
            ))
            .child(
                resizable_panel()
                    .size(
                        rem * self
                            .settings
                            .pane_widths
                            .first()
                            .copied()
                            .unwrap_or(14.0)
                            .clamp(11.0, 20.0),
                    )
                    .size_range(rem * 11.0..rem * 20.0)
                    .child(self.sidebar(cx)),
            )
            .child(
                resizable_panel().size_range(rem * 28.0..rem * 200.0).child(
                    v_flex()
                        .size_full()
                        .min_w_0()
                        .child(content)
                        .child(self.activity_panel(cx))
                        .when(self.page == Page::Inquiry, |v| v.child(self.composer(cx))),
                ),
            );
        if self.show_inspector {
            panes = panes.child(
                resizable_panel()
                    .size(
                        rem * self
                            .settings
                            .pane_widths
                            .get(2)
                            .copied()
                            .unwrap_or(23.0)
                            .clamp(18.0, 40.0),
                    )
                    .size_range(rem * 18.0..rem * 50.0)
                    .child(inspector),
            );
        }
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
            .child(TitleBar::new().child(div().flex_1().text_center().text_sm().child(title)))
            .child(
                h_flex()
                    .px_4()
                    .py_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                self.snapshot
                                    .as_ref()
                                    .map(|s| {
                                        let ready = s
                                            .project
                                            .sources
                                            .iter()
                                            .filter(|s| s.active && !s.text.is_empty())
                                            .count();
                                        format!(
                                            "{} · 자료 {}개 · 본문 준비 {}개 · r{}",
                                            if s.project.workspace_folder.is_some() {
                                                "ScholarPen 공유 프로젝트"
                                            } else {
                                                "기존 로컬 프로젝트"
                                            },
                                            s.project.sources.iter().filter(|s| s.active).count(),
                                            ready,
                                            s.project.revision
                                        )
                                    })
                                    .unwrap_or_default(),
                            ),
                    )
                    .child(
                        Button::new("new-question")
                            .ghost()
                            .small()
                            .label("새 질문")
                            .disabled(self.busy())
                            .on_click(cx.listener(|this, _, w, cx| this.clear_question(w, cx))),
                    )
                    .child(
                        Button::new("export")
                            .ghost()
                            .small()
                            .label("내보내기…")
                            .disabled(self.busy())
                            .on_click(cx.listener(|this, _, _, cx| this.choose_path("export", cx))),
                    )
                    .child(
                        Button::new("inspector-toggle")
                            .ghost()
                            .small()
                            .label(if self.show_inspector {
                                "검토 패널 닫기"
                            } else {
                                "검토 패널"
                            })
                            .selected(self.show_inspector)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_inspector = !this.show_inspector;
                                cx.notify();
                            })),
                    ),
            )
            .when_some(self.error.clone(), |v, error| {
                v.child(
                    h_flex()
                        .items_start()
                        .gap_3()
                        .px_4()
                        .py_2()
                        .bg(cx.theme().muted)
                        .child(
                            div()
                                .flex_1()
                                .text_sm()
                                .text_color(cx.theme().danger)
                                .child(friendly_error(&error)),
                        )
                        .when(self.snapshot.is_none(), |v| {
                            v.child(Button::new("retry-startup").label("다시 열기").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.service = Service::start(this.directory.clone());
                                    this.pending = 0;
                                    this.error = None;
                                    cx.notify();
                                }),
                            ))
                        })
                        .child(
                            Button::new("dismiss-error")
                                .ghost()
                                .small()
                                .label("닫기")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.error = None;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(div().flex_1().min_h_0().min_w_0().child(panes))
            .child(
                h_flex()
                    .px_4()
                    .h_9()
                    .gap_3()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .child(self.status.clone()),
                    )
                    .when(self.busy(), |v| {
                        v.child(
                            Button::new("cancel-operation")
                                .small()
                                .label("중단")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.service.cancel();
                                    this.status = "중단 요청됨 · 완료한 결과 보존 중".into();
                                    cx.notify();
                                })),
                        )
                    })
                    .when(self.current_run().is_some(), |v| {
                        v.child(
                            Button::new("show-tasks")
                                .ghost()
                                .small()
                                .label("진행 상황 보기")
                                .selected(self.show_tasks)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show_tasks = !this.show_tasks;
                                    this.page = Page::Inquiry;
                                    cx.notify();
                                })),
                        )
                    }),
            )
    }
}
fn muted(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}
fn short(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s.into()
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
