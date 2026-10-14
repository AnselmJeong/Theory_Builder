use super::*;

/// The inspector never renders unbounded text: long sources and request records are windowed.
const SOURCE_WINDOW: usize = 60;
const JSON_LIMIT: usize = 40_000;

impl Workspace {
    pub(super) fn content(&self, cx: &mut Context<Self>) -> AnyElement {
        let body = match self.page {
            Page::Inquiry => match self.current_run() {
                Some(run) => self.run_view(run, cx),
                None => self.landing(cx),
            },
            Page::Notes => self.notes(cx),
            Page::Sources => self.sources(cx),
            Page::History => self.history(cx),
            Page::Settings => self.settings_view(cx),
            Page::Direction => self.direction_view(cx),
        };
        div()
            .id("main-content")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scrollbar()
            .child(
                div()
                    .px_8()
                    .py_6()
                    .w_full()
                    .max_w(rems(52.))
                    .mx_auto()
                    .child(body),
            )
            .into_any_element()
    }
    fn landing(&self, cx: &mut Context<Self>) -> AnyElement {
        let author = self.settings.author();
        let critic = self.settings.critic();
        let writer = self.settings.writer;
        let step = |n: &str, title: String, text: &str, color: Hsla, cx: &App| {
            v_flex()
                .flex_1()
                .min_w(rems(10.))
                .gap_1()
                .p_3()
                .rounded(cx.theme().radius_lg)
                .bg(color.opacity(0.06))
                .border_1()
                .border_color(color.opacity(0.2))
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(color)
                                .child(n.to_string()),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(text.to_string()),
                )
        };
        let a = provider_color(author.label(), cx);
        let k = provider_color(critic.label(), cx);
        let w = provider_color(writer.label(), cx);
        let mut body = v_flex()
            .gap_6()
            .pt_8()
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("어떤 이론을 세워 볼까요?"),
                    )
                    .child(muted(
                        "질문을 입력하면 두 모델이 서로 비판하고 수정하며 가설을 다듬습니다. 모든 주장은 원문 근거·가정·추측으로 구분되고, 해소되지 않은 이견은 그대로 남습니다.",
                        cx,
                    )),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .items_stretch()
                    .gap_3()
                    .child(step(
                        "1",
                        "계획과 문헌".into(),
                        "질문을 작업으로 나누고 PubMed·로컬 자료에서 근거를 모읍니다.",
                        cx.theme().muted_foreground,
                        cx,
                    ))
                    .child(step(
                        "2",
                        format!("{} 초안", author.display()),
                        "기제 후보를 세우고 인용은 원문과 글자 단위로 대조합니다.",
                        a,
                        cx,
                    ))
                    .child(step(
                        "3",
                        format!("{} 비판 ↔ 수정", critic.display()),
                        "비판마다 응답·수정하고 비판자가 해소 여부를 판정합니다. 반복합니다.",
                        k,
                        cx,
                    ))
                    .child(step(
                        "4",
                        format!("{} 정리", writer.display()),
                        "결론·근거·남은 이견을 종합하고, 그림과 용어 풀이가 있는 읽기 쉬운 보고서를 씁니다.",
                        w,
                        cx,
                    )),
            );
        let mut examples = h_flex().flex_wrap().items_stretch().gap_3();
        for (id, title, question) in [
            (
                "example-mechanism",
                "기제 후보 세우기",
                "활동 의존적 가소성과 느린 조절 과정이 함께 장기적인 행동 변화를 만들 수 있는 서로 다른 기제 후보를 제안해 줘. 새 가정과 문헌 근거를 구분해 줘.",
            ),
            (
                "example-evidence",
                "주장 검토",
                "‘세포 수준의 변화가 행동 상태 전환을 일으킨다’고 써도 될까? 실제 구절과 인과 추론의 범위를 대조해 줘.",
            ),
            (
                "example-concept",
                "개념 구분",
                "Metastability와 multistability를 회로 설명에서 구분해 줘. 새로운 이론은 만들지 말고 정의와 적용 조건을 분리해 줘.",
            ),
            (
                "example-multilevel",
                "수준 연결",
                "회로 수준의 상태 공간과 더 긴 시간 척도의 기분 경과를 연결할 수 있을까? 각 수준의 변수와 시간 척도를 명시해 줘.",
            ),
        ] {
            examples = examples.child(observed(
                v_flex()
                    .id(id)
                    .flex_1()
                    .min_w(rems(18.))
                    .gap_1()
                    .p_3()
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(cx.theme().border)
                    .cursor_pointer()
                    .hover(|s| s.bg(cx.theme().muted))
                    .on_click(cx.listener(move |this, _, w, cx| {
                        if this.busy() {
                            return;
                        }
                        this.prompt.update(cx, |p, cx| {
                            p.set_value(question, w, cx);
                            p.focus(w, cx);
                        });
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(question),
                    ),
            ));
        }
        body = body.child(
            v_flex()
                .gap_2()
                .child(section_title("예시 질문", cx))
                .child(examples),
        );
        let disconnected: Vec<&str> = self
            .settings
            .roles()
            .iter()
            .filter(|p| !self.connection(**p).is_some_and(|c| c.authenticated))
            .map(|p| p.display())
            .collect();
        let sources = self
            .snapshot
            .as_ref()
            .map(|s| s.project.sources.iter().filter(|s| s.active).count())
            .unwrap_or(0);
        body = body.child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .when(!disconnected.is_empty(), |v| {
                    v.child(
                        Button::new("empty-connect")
                            .small()
                            .icon(Icon::new(Lucide::Plug))
                            .label(format!("{} 연결 확인", disconnected.join(" · ")))
                            .on_click(cx.listener(|this, _, _, cx| this.open_settings(cx))),
                    )
                })
                .child(
                    Button::new("empty-add-source")
                        .small()
                        .ghost()
                        .icon(Icon::new(Lucide::Upload))
                        .label(if sources == 0 {
                            "자료 추가… (없어도 시작 가능)".to_string()
                        } else {
                            format!("자료 {sources}개 · 더 추가…")
                        })
                        .disabled(self.busy())
                        .on_click(cx.listener(|this, _, _, cx| this.choose_path("source", cx))),
                ),
        );
        body.into_any_element()
    }
    fn notes(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut filter = h_flex()
            .gap_0p5()
            .p_0p5()
            .rounded(cx.theme().radius)
            .bg(cx.theme().muted);
        for (f, label) in [
            (NotesFilter::All, "전체"),
            (NotesFilter::Claims, "주장·후보"),
            (NotesFilter::Assumptions, "가정·용어"),
        ] {
            filter = filter.child(
                Button::new(SharedString::from(format!("notes-filter-{label}")))
                    .ghost()
                    .xsmall()
                    .selected(self.notes_filter == f)
                    .label(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.notes_filter = f;
                        cx.notify();
                    })),
            );
        }
        let mut body = v_flex()
            .gap_4()
            .child(page_header(
                "주장·가정",
                "모델이 만든 항목은 모두 ‘제안’으로 남습니다. 채택은 연구자의 판단이며 과학적 확정을 뜻하지 않습니다. 항목을 누르면 근거·비판·의존 관계를 봅니다.",
                cx,
            ))
            .child(h_flex().child(filter));
        let mut count = 0;
        let mut list = v_flex().gap_0p5();
        if let Some(s) = &self.snapshot {
            for e in s.project.entries.iter().rev() {
                let assumption = matches!(e.proposal.kind, EntryKind::Assumption | EntryKind::Term);
                if (self.notes_filter == NotesFilter::Claims && assumption)
                    || (self.notes_filter == NotesFilter::Assumptions && !assumption)
                {
                    continue;
                }
                count += 1;
                let id = e.proposal.id.clone();
                let withdrawn = e.adoption == Adoption::Withdrawn;
                list = list.child(observed(
                    v_flex()
                        .id(SharedString::from(format!("entry-{}", e.proposal.id)))
                        .gap_1p5()
                        .px_3()
                        .py_2p5()
                        .rounded(cx.theme().radius)
                        .cursor_pointer()
                        .hover(|s| s.bg(cx.theme().muted))
                        .when(withdrawn, |v| v.opacity(0.55))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_inspector(Inspector::Entry(id.clone()), cx)
                        }))
                        .child(div().text_sm().child(e.proposal.statement.clone()))
                        .child(
                            h_flex()
                                .gap_1()
                                .flex_wrap()
                                .child(neutral_chip(entry_kind_label(e.proposal.kind), cx))
                                .child(chip(
                                    claim_kind_label(e.proposal.claim_kind),
                                    claim_kind_color(e.proposal.claim_kind, cx),
                                ))
                                .child(chip(review_label(e.review), review_color(e.review, cx)))
                                .child(match e.adoption {
                                    Adoption::Adopted => chip("채택", cx.theme().success),
                                    Adoption::Withdrawn => chip("철회", cx.theme().danger),
                                    a => neutral_chip(adoption_label(a), cx),
                                }),
                        ),
                ));
            }
        }
        body = body.child(list);
        if count == 0 {
            body = body.child(muted(
                "아직 기록된 항목이 없습니다. 질문을 실행하면 토론을 거쳐 검증을 통과한 최종 항목이 여기에 쌓입니다.",
                cx,
            ));
        }
        body.into_any_element()
    }
    fn sources(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.search.read(cx).value().to_lowercase();
        let mut body = v_flex()
            .gap_4()
            .child(page_header(
                "자료",
                "인용은 여기 저장된 원문과 글자 단위로 대조됩니다. PDF·EPUB·DOCX·Markdown·TXT를 읽고, 스캔 PDF는 OCR이 필요합니다.",
                cx,
            ))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.search)
                                .id("source-search")
                                .prefix(Icon::new(Lucide::Search)),
                        ),
                    )
                    .child(
                        Button::new("import-source")
                            .small()
                            .icon(Icon::new(Lucide::Upload))
                            .label("파일 추가…")
                            .tooltip("⌘O")
                            .disabled(self.busy())
                            .on_click(cx.listener(|this, _, _, cx| this.choose_path("source", cx))),
                    )
                    .child(
                        Button::new("toggle-excerpt")
                            .small()
                            .ghost()
                            .icon(Icon::new(Lucide::Quote))
                            .label("구절 붙여 넣기")
                            .selected(self.show_excerpt_form)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_excerpt_form = !this.show_excerpt_form;
                                cx.notify();
                            })),
                    ),
            );
        if self.show_excerpt_form {
            body = body.child(
                card(cx)
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("원문 구절 추가"),
                    )
                    .child(Input::new(&self.source_title).id("source-title"))
                    .child(Input::new(&self.source_locator).id("source-location"))
                    .child(
                        Textarea::new(&self.source_text)
                            .aria_label("원문 구절")
                            .accessibility_id("source-excerpt"),
                    )
                    .child(
                        h_flex().child(
                            Button::new("save-excerpt")
                                .primary()
                                .small()
                                .label("구절 추가")
                                .disabled(self.busy())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.submit(
                                        Command::Excerpt {
                                            title: this.source_title.read(cx).value().to_string(),
                                            text: this.source_text.read(cx).value().to_string(),
                                            locator: this
                                                .source_locator
                                                .read(cx)
                                                .value()
                                                .to_string(),
                                            access: Access::ProvidedExcerpt,
                                        },
                                        cx,
                                    );
                                })),
                        ),
                    ),
            );
        }
        if let Some(s) = &self.snapshot {
            let active: Vec<_> = s.project.sources.iter().filter(|s| s.active).collect();
            let ready = active.iter().filter(|s| !s.text.trim().is_empty()).count();
            body = body.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(muted(
                        format!(
                            "{}개 · 본문 확보 {ready} · 본문 없음 {}",
                            active.len(),
                            active.len() - ready
                        ),
                        cx,
                    ))
                    .child(div().flex_1())
                    .when_some(s.project.workspace_folder.clone(), |v, folder| {
                        v.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .max_w(rems(20.))
                                .child(folder),
                        )
                        .child(
                            Button::new("refresh-resources")
                                .ghost()
                                .xsmall()
                                .icon(Icon::new(Lucide::RefreshCw))
                                .tooltip("폴더 자료 다시 확인")
                                .disabled(self.busy())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.service.cancel.store(false, Ordering::SeqCst);
                                    this.submit(Command::SyncResources, cx);
                                })),
                        )
                    }),
            );
            for source in active {
                let title_hit = source.title.to_lowercase().contains(&query);
                let hits: Vec<String> = if query.is_empty() || title_hit {
                    vec![]
                } else {
                    source
                        .text
                        .lines()
                        .enumerate()
                        .filter(|(_, l)| l.to_lowercase().contains(&query))
                        .take(3)
                        .map(|(i, l)| format!("{}: {}", i + 1, short(l, 110)))
                        .collect()
                };
                if !query.is_empty() && !title_hit && hits.is_empty() {
                    continue;
                }
                let id = source.id.clone();
                body = body.child(observed(
                    v_flex()
                        .id(SharedString::from(format!("source-{}", source.id)))
                        .gap_1()
                        .px_3()
                        .py_2p5()
                        .rounded(cx.theme().radius)
                        .cursor_pointer()
                        .hover(|s| s.bg(cx.theme().muted))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_inspector(Inspector::Source(id.clone(), None), cx)
                        }))
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    Icon::new(if source.access == Access::MetadataOnly {
                                        Lucide::FileX
                                    } else {
                                        Lucide::FileText
                                    })
                                    .small()
                                    .text_color(cx.theme().muted_foreground),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_sm()
                                        .child(source.title.clone()),
                                )
                                .child(chip(
                                    access_label(source.access),
                                    access_color(source.access, cx),
                                )),
                        )
                        .child(
                            div()
                                .pl(rems(1.5))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "{}행 · {}",
                                    source.text.lines().count(),
                                    if source.import_note.is_empty() {
                                        source.locator.clone()
                                    } else {
                                        source.import_note.clone()
                                    }
                                )),
                        )
                        .children(hits.into_iter().map(|h| {
                            div()
                                .pl(rems(1.5))
                                .text_xs()
                                .text_color(cx.theme().foreground.opacity(0.8))
                                .child(h)
                        })),
                ));
            }
        }
        body.into_any_element()
    }
    fn history(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = v_flex().gap_4().child(page_header(
            "판본",
            "과거 판본을 복원해도 기록은 지워지지 않고 새 작업안이 생깁니다. 자료와 실행 이력은 유지됩니다.",
            cx,
        ));
        if let Some(s) = &self.snapshot {
            let mut list = v_flex();
            for r in s.project.history.iter().rev() {
                let number = r.number;
                list = list.child(
                    h_flex()
                        .gap_3()
                        .py_2()
                        .items_center()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .w(rems(3.))
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!("r{}", r.number)),
                        )
                        .child(div().flex_1().min_w_0().text_sm().child(r.reason.clone()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("항목 {}", r.entries.len())),
                        )
                        .child(
                            Button::new(SharedString::from(format!("revision-{number}")))
                                .ghost()
                                .xsmall()
                                .label("차이")
                                .selected(self.selected_revision == Some(number))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.selected_revision = Some(number);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new(SharedString::from(format!("restore-{number}")))
                                .ghost()
                                .xsmall()
                                .icon(Icon::new(Lucide::Undo2))
                                .label("복원")
                                .disabled(self.busy())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.submit(Command::Restore(number), cx)
                                })),
                        ),
                );
            }
            if let Some(r) = self
                .selected_revision
                .and_then(|n| s.project.history.iter().find(|r| r.number == n))
            {
                let mut diff = card(cx).child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("r{} → 현재 r{}", r.number, s.project.revision)),
                );
                let mut changes = 0;
                for e in &s.project.entries {
                    let old = r
                        .entries
                        .iter()
                        .find(|old| old.proposal.id == e.proposal.id);
                    let (label, color) = match old {
                        None => ("추가", cx.theme().success),
                        Some(old)
                            if old.adoption != e.adoption
                                || old.review != e.review
                                || old.proposal.statement != e.proposal.statement =>
                        {
                            ("변경", amber_color())
                        }
                        _ => continue,
                    };
                    changes += 1;
                    diff = diff.child(
                        h_flex()
                            .gap_2()
                            .items_start()
                            .child(chip(label, color))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .gap_0p5()
                                    .child(div().text_sm().child(e.proposal.statement.clone()))
                                    .when_some(old, |v, old| {
                                        v.child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!(
                                                    "{} · {} → {} · {}",
                                                    adoption_label(old.adoption),
                                                    review_label(old.review),
                                                    adoption_label(e.adoption),
                                                    review_label(e.review)
                                                )),
                                        )
                                    }),
                            ),
                    );
                }
                for e in &r.entries {
                    if !s
                        .project
                        .entries
                        .iter()
                        .any(|c| c.proposal.id == e.proposal.id)
                    {
                        changes += 1;
                        diff = diff.child(
                            h_flex()
                                .gap_2()
                                .child(chip("제외", cx.theme().danger))
                                .child(
                                    div().flex_1().text_sm().child(e.proposal.statement.clone()),
                                ),
                        );
                    }
                }
                if changes == 0 {
                    diff = diff.child(muted("원장 항목에는 차이가 없습니다.", cx));
                }
                body = body.child(diff);
            }
            body = body.child(list);
        }
        body.into_any_element()
    }
    pub(super) fn inspector(&self, cx: &mut Context<Self>) -> AnyElement {
        let title = match &self.inspector {
            Inspector::Entry(_) => "주장·가정",
            Inspector::Source(..) => "원문",
            Inspector::Task(_) => "요청·응답 기록",
            Inspector::Critique(..) => "비판의 경과",
            Inspector::None => "검토",
        };
        let mut body = v_flex().gap_4();
        if let Some(s) = &self.snapshot {
            match &self.inspector {
                Inspector::Entry(id) => body = self.inspect_entry(s, id, body, cx),
                Inspector::Source(id, evidence) => {
                    body = self.inspect_source(s, id, evidence.as_ref(), body, cx)
                }
                Inspector::Task(id) => body = self.inspect_task(s, id, body, cx),
                Inspector::Critique(run_id, id) => {
                    body = self.inspect_critique(s, run_id, id, body, cx)
                }
                Inspector::None => {
                    body = body.child(muted(
                        "답변의 이론 항목, 비판, 근거를 선택하면 원문 구절·접근 범위·의존 가정을 여기서 확인합니다.",
                        cx,
                    ))
                }
            }
        }
        v_flex()
            .size_full()
            .min_w_0()
            .bg(cx.theme().sidebar)
            .border_l_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .px_4()
                    .h(rems(2.75))
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        Button::new("close-inspector")
                            .ghost()
                            .xsmall()
                            .icon(Icon::new(Lucide::X))
                            .tooltip("닫기")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_inspector = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("inspector-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(div().p_4().child(body)),
            )
            .into_any_element()
    }
    fn inspect_entry(&self, s: &Snapshot, id: &str, mut body: Div, cx: &mut Context<Self>) -> Div {
        // Ledger entries, or a draft proposal that only exists inside a run.
        let entry = s.project.entries.iter().find(|e| e.proposal.id == id);
        let proposal = entry.map(|e| e.proposal.clone()).or_else(|| {
            s.project
                .runs
                .iter()
                .flat_map(|r| r.attempts.iter())
                .filter_map(|a| a.output.as_ref())
                .flat_map(|o| o.proposed_changes.iter())
                .find(|p| p.id == id)
                .cloned()
        });
        let Some(p) = proposal else {
            return body.child(muted("항목을 찾지 못했습니다.", cx));
        };
        body = body
            .child(
                h_flex()
                    .gap_1()
                    .flex_wrap()
                    .child(neutral_chip(entry_kind_label(p.kind), cx))
                    .child(chip(
                        claim_kind_label(p.claim_kind),
                        claim_kind_color(p.claim_kind, cx),
                    ))
                    .when_some(entry, |v, e| {
                        v.child(chip(review_label(e.review), review_color(e.review, cx)))
                            .child(neutral_chip(adoption_label(e.adoption), cx))
                    })
                    .when(entry.is_none(), |v| {
                        v.child(neutral_chip("토론 중 초안 (원장 미등록)", cx))
                    }),
            )
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::MEDIUM)
                    .child(p.statement.clone()),
            )
            .when(!p.scope.is_empty(), |v| {
                v.child(labeled("적용 범위", &p.scope, cx))
            })
            .when(!p.rationale.is_empty(), |v| {
                v.child(labeled("도입 이유", &p.rationale, cx))
            });
        if let Some(e) = entry {
            let adopt = id.to_string();
            let withdraw = id.to_string();
            let target = id.to_string();
            body = body.child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("adopt-entry")
                            .small()
                            .primary()
                            .icon(Icon::new(Lucide::Check))
                            .label("채택")
                            .disabled(
                                self.busy()
                                    || e.adoption == Adoption::Withdrawn
                                    || e.adoption == Adoption::Adopted,
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.submit(Command::Adopt(adopt.clone()), cx)
                            })),
                    )
                    .child(
                        Button::new("withdraw-entry")
                            .small()
                            .label("철회")
                            .disabled(self.busy() || e.adoption == Adoption::Withdrawn)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.submit(Command::Withdraw(withdraw.clone()), cx)
                            })),
                    )
                    .child(
                        Button::new("target-entry")
                            .small()
                            .ghost()
                            .icon(Icon::new(Lucide::CornerDownRight))
                            .label("후속 질문")
                            .on_click(cx.listener(move |this, _, w, cx| {
                                this.targets = vec![target.clone()];
                                this.page = Page::Inquiry;
                                this.selected_run = None;
                                this.prompt.update(cx, |p, cx| {
                                    p.set_value("이 항목의 가정과 근거를 다시 검토해 줘.", w, cx);
                                    p.focus(w, cx);
                                });
                                cx.notify();
                            })),
                    ),
            );
        }
        if !p.depends_on.is_empty() {
            let mut deps = v_flex().gap_1();
            for d in &p.depends_on {
                let dep = d.clone();
                let label = s
                    .project
                    .entries
                    .iter()
                    .find(|e| e.proposal.id == *d)
                    .map(|e| e.proposal.statement.clone())
                    .unwrap_or_else(|| d.clone());
                deps = deps.child(observed(
                    h_flex()
                        .id(SharedString::from(format!("dep-{d}")))
                        .gap_2()
                        .items_start()
                        .px_2()
                        .py_1()
                        .rounded(cx.theme().radius)
                        .cursor_pointer()
                        .hover(|s| s.bg(cx.theme().muted))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_inspector(Inspector::Entry(dep.clone()), cx)
                        }))
                        .child(Icon::new(Lucide::CornerDownRight).xsmall())
                        .child(div().flex_1().text_xs().child(short(&label, 140))),
                ));
            }
            body = body.child(section_title("의존하는 항목", cx)).child(deps);
        }
        let mut evidence = v_flex().gap_2();
        let mut n = 0;
        for run in &s.project.runs {
            for a in run
                .attempts
                .iter()
                .filter(|a| a.state == TaskState::Succeeded)
            {
                let Some(o) = &a.output else { continue };
                for e in o.evidence_refs.iter().filter(|ev| ev.claim_id == id) {
                    let ev = e.clone();
                    evidence = evidence.child(observed(
                        v_flex()
                            .id(SharedString::from(format!("entry-ev-{}-{n}", a.id)))
                            .gap_1()
                            .pl_3()
                            .border_l_2()
                            .border_color(relation_color(e.relation, cx).opacity(0.6))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_inspector(
                                    Inspector::Source(ev.source_id.clone(), Some(ev.clone())),
                                    cx,
                                )
                            }))
                            .child(div().text_xs().italic().child(format!("“{}”", e.quote)))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(chip(
                                        relation_label(e.relation),
                                        relation_color(e.relation, cx),
                                    ))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!(
                                                "{} · 원문 보기",
                                                lines_label(e.start_line, e.end_line)
                                            )),
                                    ),
                            ),
                    ));
                    n += 1;
                }
            }
        }
        body = body.child(section_title(format!("원문 근거 · {n}"), cx));
        body = if n == 0 {
            body.child(muted(
                "검증된 원문 인용이 없습니다. 가정 또는 추측으로 다루세요.",
                cx,
            ))
        } else {
            body.child(evidence)
        };
        let mut critiques = v_flex().gap_2();
        let mut k = 0;
        for run in &s.project.runs {
            for c in run.critiques.iter().filter(|c| c.critique.target_id == id) {
                k += 1;
                critiques = critiques.child(self.critique_card(run, c, cx));
            }
        }
        if k > 0 {
            body = body
                .child(section_title(format!("이 항목에 대한 비판 · {k}"), cx))
                .child(critiques);
        }
        body
    }
    fn inspect_critique(
        &self,
        s: &Snapshot,
        run_id: &str,
        id: &str,
        mut body: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        let Some(run) = s.project.runs.iter().find(|r| r.id == run_id) else {
            return body;
        };
        let Some(c) = run.critiques.iter().find(|c| c.id == id) else {
            return body;
        };
        let target_id = c.critique.target_id.clone();
        body = body.child(self.critique_card(run, c, cx));
        if !target_id.is_empty() {
            body = body.child(
                h_flex().child(
                    Button::new("critique-target")
                        .ghost()
                        .small()
                        .icon(Icon::new(Lucide::ArrowRight))
                        .label("비판 대상 항목 보기")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_inspector(Inspector::Entry(target_id.clone()), cx)
                        })),
                ),
            );
        }
        if !c.critique.suggested_action.is_empty() {
            body = body.child(labeled("제안된 수정", &c.critique.suggested_action, cx));
        }
        let mut timeline =
            v_flex().gap_3().child(
                h_flex()
                    .gap_2()
                    .items_start()
                    .child(avatar(&c.provider, cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).child(
                                format!(
                                    "{} · {} 제기",
                                    who(&c.provider),
                                    if c.round == 0 {
                                        "첫 비판에서".to_string()
                                    } else {
                                        format!("{}차 재검토에서", c.round)
                                    }
                                ),
                            ))
                            .child(div().text_xs().child(c.critique.reasoning.clone())),
                    ),
            );
        for e in &c.events {
            timeline = timeline.child(
                h_flex()
                    .gap_2()
                    .items_start()
                    .child(avatar(&e.provider, cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(format!("{} · {}차", who(&e.provider), e.round)),
                                    )
                                    .when_some(e.stance, |v, st| {
                                        v.child(chip(st.label(), stance_color(st, cx)))
                                    })
                                    .when_some(e.status, |v, st| {
                                        v.child(chip(st.label(), status_color(st, cx)))
                                    }),
                            )
                            .child(div().text_xs().child(e.reasoning.clone())),
                    ),
            );
        }
        body.child(section_title("주고받은 내용", cx))
            .child(timeline)
    }
    fn inspect_source(
        &self,
        s: &Snapshot,
        id: &str,
        evidence: Option<&Evidence>,
        mut body: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        let Some(source) = s.project.sources.iter().find(|s| s.id == id) else {
            return body.child(muted("출처를 찾지 못했습니다.", cx));
        };
        body = body
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::MEDIUM)
                    .child(source.title.clone()),
            )
            .child(
                h_flex()
                    .gap_1()
                    .flex_wrap()
                    .child(chip(
                        access_label(source.access),
                        access_color(source.access, cx),
                    ))
                    .child(neutral_chip(format!("판본 {}", source.revision), cx))
                    .child(neutral_chip(
                        format!("{}행", source.text.lines().count()),
                        cx,
                    )),
            )
            .child(labeled("위치", &source.locator, cx))
            .when(!source.import_note.is_empty(), |v| {
                v.child(labeled("가져오기", &source.import_note, cx))
            });
        if source.asset_hash.is_some() || theory_engine::search::safe_public_url(&source.locator) {
            let id = id.to_string();
            body = body.child(
                h_flex().child(
                    Button::new("open-original")
                        .small()
                        .icon(Icon::new(Lucide::ExternalLink))
                        .label("원문 파일·페이지 열기")
                        .disabled(self.busy())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.submit(Command::OpenSource(id.clone()), cx)
                        })),
                ),
            );
        }
        if let Some(e) = evidence {
            body = body.child(
                v_flex()
                    .gap_1()
                    .p_3()
                    .rounded(cx.theme().radius)
                    .bg(relation_color(e.relation, cx).opacity(0.08))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(chip(
                                relation_label(e.relation),
                                relation_color(e.relation, cx),
                            ))
                            .child(div().text_xs().child(format!(
                                "{} · 문자열 일치 확인",
                                lines_label(e.start_line, e.end_line)
                            ))),
                    )
                    .child(div().text_sm().italic().child(format!("“{}”", e.quote)))
                    .when(!e.reasoning.is_empty(), |v| {
                        v.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(e.reasoning.clone()),
                        )
                    }),
            );
        }
        if source.text.is_empty() {
            return body.child(muted(
                "본문을 확보하지 못했습니다. 이 자료로는 원문 대조를 하지 않습니다. 관련 구절을 자료 화면에서 직접 추가하세요.",
                cx,
            ));
        }
        // Show a window around the cited lines instead of the whole book.
        let lines: Vec<&str> = source.text.lines().collect();
        let (from, to) = match evidence {
            Some(e) => (
                (e.start_line as usize).saturating_sub(12).max(1),
                ((e.end_line as usize) + 12).min(lines.len()),
            ),
            None => (1, SOURCE_WINDOW.min(lines.len())),
        };
        let mut text = v_flex().gap_0p5();
        for (i, line) in lines.iter().enumerate().take(to).skip(from - 1) {
            let n = i + 1;
            let hit = evidence.is_some_and(|e| n as u32 >= e.start_line && n as u32 <= e.end_line);
            text = text.child(
                h_flex()
                    .gap_2()
                    .items_start()
                    .when(hit, |v| v.bg(cx.theme().warning.opacity(0.18)))
                    .child(
                        div()
                            .flex_none()
                            .w(rems(2.5))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(n.to_string()),
                    )
                    .child(div().flex_1().min_w_0().text_xs().child(line.to_string())),
            );
        }
        body = body
            .child(section_title(
                format!("본문 · 행 {from}–{to} / {}", lines.len()),
                cx,
            ))
            .child(text);
        body.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!("출처 ID {}\nSHA-256 {}", source.id, source.hash)),
        )
    }
    fn inspect_task(&self, s: &Snapshot, id: &str, body: Div, cx: &mut Context<Self>) -> Div {
        let Some(a) = s
            .project
            .runs
            .iter()
            .flat_map(|r| r.attempts.iter())
            .find(|a| a.id == *id)
        else {
            return body;
        };
        let json = |v: serde_json::Value| {
            let text = serde_json::to_string_pretty(&v).unwrap_or_default();
            if text.len() > JSON_LIMIT {
                let cut = text
                    .char_indices()
                    .map(|(i, _)| i)
                    .take_while(|i| *i <= JSON_LIMIT)
                    .last()
                    .unwrap_or(0);
                format!(
                    "{}\n… {} bytes 생략 (내보내기 bundle에 전체 기록)",
                    &text[..cut],
                    text.len() - cut
                )
            } else {
                text
            }
        };
        body.child(
            h_flex()
                .gap_2()
                .items_center()
                .child(avatar(&a.provider, cx))
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(step_title(a))),
        )
        .child(
            h_flex()
                .gap_1()
                .flex_wrap()
                .child(neutral_chip(progress::task_state(a.state), cx))
                .child(neutral_chip(
                    if a.model.is_empty() { "계정 기본 모델".to_string() } else { a.model.clone() },
                    cx,
                )),
        )
        .when_some(a.error.clone(), |v, e| {
            v.child(div().text_sm().text_color(cx.theme().danger).child(friendly_error(&e)))
        })
        .when(!a.validation_notes.is_empty(), |v| {
            v.child(section_title("검증 기록", cx)).child(
                v_flex().gap_1().children(
                    a.validation_notes
                        .iter()
                        .map(|n| div().text_xs().child(format!("· {n}"))),
                ),
            )
        })
        .child(document(
            "task-details",
            format!(
                "#### 실제 입력\n```json\n{}\n```\n\n#### 모델 원본 응답\n```json\n{}\n```\n\n#### 검증을 통과한 출력\n```json\n{}\n```\n\n#### 사용량\n```json\n{}\n```",
                json(a.context.clone()),
                json(a.response.clone().unwrap_or_default()),
                json(serde_json::to_value(&a.output).unwrap_or_default()),
                json(a.usage.clone().unwrap_or_default())
            ),
        ))
    }
}
fn labeled(label: &str, value: &str, cx: &App) -> Div {
    v_flex()
        .gap_0p5()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .child(div().text_sm().child(value.to_string()))
}
fn amber_color() -> Hsla {
    gpui_kit::component::amber_500()
}
