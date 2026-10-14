//! The reader report: what the person who asked actually reads. A direct answer, key points
//! with how firm each one is, figures, plain definitions, what is still uncertain, concrete
//! next steps, the references — and two ways to keep going: ask a follow-up question, or
//! give the debate more rounds with a direction.
use super::*;
use std::sync::Arc;
use theory_engine::report::{CiteStyle, Palette, ReaderView, figure_svg, reader_view};

/// Widest a figure is drawn in the content column.
const FIGURE_MAX: f32 = 720.;

pub(super) fn standing_color(s: Standing, cx: &App) -> Hsla {
    match s {
        Standing::Supported => cx.theme().success,
        Standing::Reasoned => gpui_kit::component::sky_500(),
        Standing::Hypothesis => gpui_kit::component::amber_500(),
        Standing::Speculation => cx.theme().muted_foreground,
    }
}
/// The report's accent: the theme's primary is near-black, so use a calm blue.
pub(super) fn accent(cx: &App) -> Hsla {
    if cx.theme().is_dark() {
        gpui_kit::component::blue_400()
    } else {
        gpui_kit::component::blue_600()
    }
}
fn next_icon(kind: NextKind) -> Lucide {
    match kind {
        NextKind::AskHere => Lucide::MessageCircleQuestionMark,
        NextKind::Theory => Lucide::SquareFunction,
        NextKind::DataAnalysis => Lucide::ChartSpline,
        NextKind::Experiment => Lucide::FlaskConical,
        NextKind::Reading => Lucide::BookOpen,
    }
}
/// A report section heading: larger than the app's small section labels.
fn heading(text: &str, cx: &App) -> Div {
    div()
        .pt_3()
        .text_lg()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().foreground)
        .child(text.to_string())
}
fn numbered(n: usize, color: Hsla) -> Div {
    div()
        .flex_none()
        .size(rems(1.5))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(color.opacity(0.14))
        .text_color(color)
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .child(n.to_string())
}
impl Workspace {
    /// Markdown with citations shown as author–year links.
    fn prose(&self, id: String, text: &str, view: &ReaderView) -> TextView {
        document(
            SharedString::from(id),
            view.references.display(text, CiteStyle::Markdown),
        )
    }
    fn svg_image(&self, key: String, svg: &str) -> Arc<Image> {
        let mut cache = self.figure_cache.borrow_mut();
        cache
            .entry(key)
            .or_insert_with(|| {
                Arc::new(Image::from_bytes(ImageFormat::Svg, svg.as_bytes().to_vec()))
            })
            .clone()
    }
    fn figure_view(
        &self,
        run: &Run,
        view: &ReaderView,
        id: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let r = &view.report;
        let Some(f) = r.figures.iter().find(|f| f.id == id) else {
            return div().into_any_element();
        };
        let n = r.figure_number(id).unwrap_or(0);
        let label = if f.kind == FigureKind::Table {
            "표"
        } else {
            "그림"
        };
        let mut body = card(cx).gap_3().child(
            div().text_sm().child(
                h_flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{label} {n}. {}", f.title)),
                    )
                    .when(!f.caption.is_empty(), |v| {
                        v.child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child(f.caption.clone()),
                        )
                    }),
            ),
        );
        match f.kind {
            FigureKind::Diagram => {
                let dark = cx.theme().is_dark();
                let palette = if dark { Palette::DARK } else { Palette::LIGHT }.embedded();
                if let Some((svg, w, h)) = figure_svg(f, &palette) {
                    let written = run.reading.as_ref().map(|r| r.written).unwrap_or(0);
                    let image =
                        self.svg_image(format!("{}-{written}-{}-{dark}", run.id, f.id), &svg);
                    let scale = (FIGURE_MAX / w).min(1.25);
                    body = body.child(
                        h_flex()
                            .w_full()
                            .justify_center()
                            .child(img(image).w(px(w * scale)).h(px(h * scale))),
                    );
                }
                let notes: Vec<&DiagramNode> =
                    f.nodes.iter().filter(|n| !n.note.is_empty()).collect();
                if !notes.is_empty() {
                    let mut list = v_flex().gap_1();
                    for node in notes {
                        list = list.child(
                            h_flex()
                                .gap_2()
                                .items_start()
                                .text_xs()
                                .child(
                                    div()
                                        .flex_none()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(node.label.clone()),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(node.note.clone()),
                                ),
                        );
                    }
                    body = body.child(list);
                }
            }
            FigureKind::Table => {
                let cell = |text: &str, head: bool, cx: &App| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .px_2()
                        .py_1p5()
                        .text_sm()
                        .when(head, |v| v.font_weight(FontWeight::SEMIBOLD))
                        .text_color(if head {
                            cx.theme().muted_foreground
                        } else {
                            cx.theme().foreground
                        })
                        .child(view.references.display(text, CiteStyle::Text))
                };
                let mut table = v_flex().w_full();
                let mut head = h_flex()
                    .w_full()
                    .bg(cx.theme().muted)
                    .rounded(cx.theme().radius);
                for c in &f.columns {
                    head = head.child(cell(c, true, cx));
                }
                table = table.child(head);
                for row in &f.rows {
                    let mut line = h_flex()
                        .w_full()
                        .items_start()
                        .border_b_1()
                        .border_color(cx.theme().border);
                    for c in row {
                        line = line.child(cell(c, false, cx));
                    }
                    table = table.child(line);
                }
                body = body.child(table);
            }
        }
        body.into_any_element()
    }
    pub(super) fn report_tab(&self, run: &Run, cx: &mut Context<Self>) -> AnyElement {
        let Some(snapshot) = &self.snapshot else {
            return div().into_any_element();
        };
        if run.state == RunState::Running && run.reading.is_none() {
            return card(cx)
                .child(div().font_weight(FontWeight::MEDIUM).child("두 모델이 이론을 다듬고 있습니다"))
                .child(muted(
                    "초안 → 비판 → 응답·수정 → 재검토가 반복된 뒤, 서기가 토론 결과를 읽기 쉬운 보고서로 정리합니다. 진행 중인 의견 교환은 토론 탭에서 볼 수 있습니다.",
                    cx,
                ))
                .child(
                    h_flex().child(
                        Button::new("goto-debate")
                            .small()
                            .icon(Icon::new(Lucide::MessagesSquare))
                            .label("토론 보기")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.run_tab = RunTab::Debate;
                                cx.notify();
                            })),
                    ),
                )
                .into_any_element();
        }
        let view = reader_view(&snapshot.project, run);
        let r = &view.report;
        let rid = run.id.clone();
        let mut body = v_flex().gap_5();
        if view.fallback && run.state != RunState::Running {
            let failure = run
                .limitations
                .iter()
                .rev()
                .find(|l| l.starts_with("읽기 쉬운 보고서"))
                .cloned();
            let id = run.id.clone();
            body = body.child(
                card(cx)
                    .border_color(gpui_kit::component::amber_500().opacity(0.5))
                    .bg(gpui_kit::component::amber_500().opacity(0.06))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(Icon::new(Lucide::PenLine).small())
                            .child(div().font_weight(FontWeight::SEMIBOLD).child("아직 읽기 쉬운 보고서가 없습니다")),
                    )
                    .child(muted(
                        "아래는 토론 결과를 그대로 정리한 것이라 용어가 어렵고 번호가 많습니다. 서기가 토론을 읽고 그림·용어 풀이·다음 단계까지 담아 다시 써 드립니다.",
                        cx,
                    ))
                    .when_some(failure, |v, f| v.child(muted(friendly_error(&f), cx)))
                    .child(
                        h_flex().child(
                            Button::new("write-report")
                                .primary()
                                .small()
                                .icon(Icon::new(Lucide::Sparkles))
                                .label("읽기 쉬운 보고서 만들기 · 호출 1회")
                                .disabled(self.busy())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.submit(Command::WriteReport(id.clone()), cx)
                                })),
                        ),
                    ),
            );
        }
        // Who wrote this report, and a way to have the chosen writer write it again.
        if let Some(reading) = run
            .reading
            .as_ref()
            .filter(|_| run.state != RunState::Running)
        {
            let writer = self.settings.writer;
            let id = run.id.clone();
            body = body.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{}가 정리한 보고서", who(&reading.provider))),
                    )
                    .child(
                        Button::new("rewrite-report")
                            .ghost()
                            .xsmall()
                            .icon(Icon::new(Lucide::RefreshCw))
                            .label(format!("{}로 다시 쓰기 · 호출 1회", writer.display()))
                            .tooltip("설정의 정리 모델과 지금의 작성 규칙으로 보고서를 다시 씁니다. 토론과 이어서 묻기 기록은 그대로 둡니다.")
                            .disabled(self.busy())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.submit(Command::WriteReport(id.clone()), cx)
                            })),
                    ),
            );
        }
        // The direct answer.
        let accent = accent(cx);
        body = body.child(
            v_flex()
                .gap_1()
                .p_4()
                .rounded(cx.theme().radius_lg)
                .border_1()
                .border_l_4()
                .border_color(accent.opacity(0.45))
                .bg(accent.opacity(0.06))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(accent)
                        .child("답"),
                )
                .child(div().text_base().child(self.prose(
                    format!("answer-{rid}"),
                    &r.answer,
                    &view,
                ))),
        );
        if !r.key_points.is_empty() {
            body = body.child(heading("핵심 내용", cx));
            let mut points = v_flex().gap_3();
            for (i, k) in r.key_points.iter().enumerate() {
                let item = card(cx).child(
                    h_flex()
                        .gap_3()
                        .items_start()
                        .child(numbered(i + 1, accent))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_1()
                                .child(
                                    h_flex()
                                        .id(SharedString::from(format!("standing-{rid}-{i}")))
                                        .child(chip(
                                            k.standing.label(),
                                            standing_color(k.standing, cx),
                                        ))
                                        .tooltip({
                                            let text = k.standing.explain();
                                            move |window, cx| {
                                                gpui_kit::component::tooltip::Tooltip::new(text)
                                                    .build(window, cx)
                                            }
                                        }),
                                )
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(k.heading.clone()),
                                )
                                .child(div().text_sm().child(self.prose(
                                    format!("point-{rid}-{i}"),
                                    &k.body,
                                    &view,
                                ))),
                        ),
                );
                points = points.child(item);
            }
            body = body.child(points);
            for f in r.key_point_figures() {
                body = body.child(self.figure_view(run, &view, &f.id, cx));
            }
        }
        if !r.sections.is_empty() {
            body = body.child(heading("자세한 설명", cx));
            for (i, s) in r.sections.iter().enumerate() {
                let mut sec = v_flex()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(s.heading.clone()),
                    )
                    .child(div().text_sm().child(self.prose(
                        format!("section-{rid}-{i}"),
                        &s.body,
                        &view,
                    )));
                for id in &s.figure_ids {
                    sec = sec.child(self.figure_view(run, &view, id, cx));
                }
                body = body.child(sec);
            }
        }
        for f in r.unplaced_figures() {
            body = body.child(self.figure_view(run, &view, &f.id, cx));
        }
        if !r.glossary.is_empty() {
            body = body.child(heading("용어 풀이", cx));
            let mut grid = h_flex().flex_wrap().gap_2();
            for g in &r.glossary {
                grid = grid.child(
                    v_flex()
                        .w(rems(16.))
                        .flex_grow(1.)
                        .gap_0p5()
                        .p_3()
                        .rounded(cx.theme().radius_lg)
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(g.term.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(g.plain.clone()),
                        ),
                );
            }
            body = body.child(grid);
        }
        if !r.open_questions.is_empty() {
            body = body.child(heading("아직 확실하지 않은 것", cx));
            for (i, q) in r.open_questions.iter().enumerate() {
                let line = |label: &str, text: &str, cx: &App| {
                    h_flex()
                        .gap_2()
                        .items_start()
                        .text_sm()
                        .child(
                            div()
                                .flex_none()
                                .w(rems(5.5))
                                .text_xs()
                                .pt_0p5()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(cx.theme().muted_foreground)
                                .child(label.to_string()),
                        )
                        .child(div().flex_1().min_w_0().child(text.to_string()))
                };
                body = body.child(
                    card(cx)
                        .child(
                            h_flex()
                                .gap_2()
                                .items_start()
                                .child(
                                    Icon::new(Lucide::CircleQuestionMark)
                                        .small()
                                        .text_color(gpui_kit::component::amber_500()),
                                )
                                .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child(
                                    self.prose(format!("open-{rid}-{i}"), &q.question, &view),
                                )),
                        )
                        .when(!q.why_it_matters.is_empty(), |v| {
                            v.child(line("왜 중요한가", &q.why_it_matters, cx))
                        })
                        .when(!q.how_to_settle.is_empty(), |v| {
                            v.child(line("판가름하려면", &q.how_to_settle, cx))
                        }),
                );
            }
        }
        if !r.next_steps.is_empty() {
            body = body.child(heading("다음에 해 볼 수 있는 일", cx));
            for kind in NextKind::ALL {
                let steps: Vec<(usize, &NextStep)> = r
                    .next_steps
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| n.kind == kind)
                    .collect();
                if steps.is_empty() {
                    continue;
                }
                let mut group = v_flex().gap_1p5().child(
                    h_flex()
                        .gap_1p5()
                        .items_center()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().muted_foreground)
                        .child(Icon::new(next_icon(kind)).xsmall())
                        .child(kind.label()),
                );
                for (i, n) in steps {
                    let ask = (kind == NextKind::AskHere).then(|| n.action.clone());
                    group = group.child(
                        h_flex()
                            .gap_3()
                            .items_start()
                            .pl_3()
                            .py_1()
                            .border_l_2()
                            .border_color(accent.opacity(0.5))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_0p5()
                                    .child(div().text_sm().child(n.action.clone()))
                                    .when(!n.purpose.is_empty(), |v| {
                                        v.child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(n.purpose.clone()),
                                        )
                                    }),
                            )
                            .when_some(ask, |v, question| {
                                v.child(
                                    Button::new(SharedString::from(format!("ask-suggested-{i}")))
                                        .ghost()
                                        .xsmall()
                                        .icon(Icon::new(Lucide::MessageCircleReply))
                                        .label("이 질문 하기")
                                        .disabled(self.busy())
                                        .on_click(cx.listener(move |this, _, w, cx| {
                                            this.followup.update(cx, |f, cx| {
                                                f.set_value(question.clone(), w, cx);
                                                f.focus(w, cx);
                                            });
                                            cx.notify();
                                        })),
                                )
                            }),
                    );
                }
                body = body.child(group);
            }
        }
        // How the answer was made, and the way into the process views.
        let settled = run
            .critiques
            .iter()
            .filter(|c| c.status.is_settled())
            .count();
        body = body
            .child(heading("이 답변은 어떻게 만들어졌나", cx))
            .child(
                v_flex()
                    .gap_2()
                    .child(div().text_sm().child(if r.how_made.trim().is_empty() {
                        theory_engine::report::how_made(run)
                    } else {
                        r.how_made.clone()
                    }))
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_1()
                            .child(neutral_chip(
                                format!("수정 {}회", run.deliberation.rounds),
                                cx,
                            ))
                            .child(neutral_chip(format!("비판 {}개", run.critiques.len()), cx))
                            .child(chip(format!("정리 {settled}"), cx.theme().success))
                            .when(run.critiques.len() > settled, |v| {
                                v.child(chip(
                                    format!("남음 {}", run.critiques.len() - settled),
                                    gpui_kit::component::amber_500(),
                                ))
                            })
                            .child(neutral_chip(
                                format!("문헌 인용 {}", view.references.entries.len()),
                                cx,
                            )),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("report-goto-debate")
                                    .ghost()
                                    .xsmall()
                                    .icon(Icon::new(Lucide::MessagesSquare))
                                    .label("토론 과정 보기")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.run_tab = RunTab::Debate;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("report-goto-theory")
                                    .ghost()
                                    .xsmall()
                                    .icon(Icon::new(Lucide::Network))
                                    .label("이론 상세 보기")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.run_tab = RunTab::Theory;
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        if !view.references.entries.is_empty() {
            body = body.child(heading("참고문헌", cx));
            let mut list = v_flex().gap_1p5();
            for (i, (key, b)) in view.references.entries.iter().enumerate() {
                list = list.child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .child(
                            div()
                                .flex_none()
                                .w(rems(1.25))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{}.", i + 1)),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div().text_xs().child(document(
                                        SharedString::from(format!("ref-{rid}-{i}")),
                                        match b.link() {
                                            Some(link) => b
                                                .reference()
                                                .replace(&link, &format!("[{link}]({link})")),
                                            None => b.reference(),
                                        },
                                    )),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("인용 키 @{key}")),
                                ),
                        ),
                );
            }
            body = body.child(list);
        }
        body = body.child(self.followup_section(run, &view, cx));
        if run.state != RunState::Running {
            body = body.child(self.continue_card(run, cx));
        }
        body.into_any_element()
    }
    /// Follow-up questions about this report, answered by the scribe in one call each.
    fn followup_section(&self, run: &Run, view: &ReaderView, cx: &mut Context<Self>) -> AnyElement {
        let mut thread = v_flex().gap_3();
        for (i, f) in run.followups.iter().enumerate() {
            thread = thread.child(
                h_flex().w_full().justify_end().child(
                    div()
                        .max_w(rems(34.))
                        .px_3()
                        .py_2()
                        .rounded(cx.theme().radius_lg)
                        .bg(cx.theme().muted)
                        .text_sm()
                        .child(f.question.clone()),
                ),
            );
            let answer = match &f.error {
                Some(e) => div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(format!("답하지 못했습니다: {}", friendly_error(e))),
                None => div().text_sm().child(self.prose(
                    format!("followup-{}-{i}", run.id),
                    &f.answer,
                    view,
                )),
            };
            let direction = (f.needs_research && !f.suggested_direction.is_empty())
                .then(|| f.suggested_direction.clone());
            thread =
                thread.child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .child(avatar(&f.provider, cx))
                        .child(v_flex().flex_1().min_w_0().gap_2().child(answer).when_some(
                            direction,
                            |v, direction| {
                                v.child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .flex_wrap()
                                        .child(chip(
                                            "토론이 더 필요한 질문",
                                            gpui_kit::component::amber_500(),
                                        ))
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "continue-from-followup-{i}"
                                            )))
                                            .xsmall()
                                            .icon(Icon::new(Lucide::FastForward))
                                            .label("이 방향으로 토론 이어 가기")
                                            .disabled(self.busy())
                                            .on_click(cx.listener(move |this, _, w, cx| {
                                                this.direction.update(cx, |d, cx| {
                                                    d.set_value(direction.clone(), w, cx);
                                                    d.focus(w, cx);
                                                });
                                                cx.notify();
                                            })),
                                        ),
                                )
                            },
                        )),
                );
        }
        let can_ask = run.state != RunState::Running && !run.report.is_empty();
        v_flex()
            .gap_3()
            .child(heading("이어서 묻기", cx))
            .child(muted(
                "보고서에서 이해가 안 되는 부분, 용어, 예시를 물어보세요. 서기가 이 보고서와 토론 기록 안에서 답합니다(호출 1회). 새 근거나 논증이 필요한 질문이면 토론을 이어 가도록 안내합니다.",
                cx,
            ))
            .child(thread)
            .child(
                v_flex()
                    .gap_2()
                    .p_2()
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        Textarea::new(&self.followup)
                            .appearance(false)
                            .aria_label("후속 질문")
                            .accessibility_id("followup-input")
                            .readonly(self.busy() || !can_ask),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().flex_1())
                            .child(div().text_xs().text_color(cx.theme().muted_foreground).child("⌘↵"))
                            .child(
                                Button::new("ask-followup")
                                    .small()
                                    .icon(Icon::new(Lucide::Send))
                                    .label("묻기")
                                    .disabled(
                                        self.busy() || !can_ask || self.followup.read(cx).value().trim().is_empty(),
                                    )
                                    .on_click(cx.listener(|this, _, w, cx| this.ask_followup(w, cx))),
                            ),
                    ),
            )
            .into_any_element()
    }
    /// More debate rounds on this question, optionally with the user's direction.
    pub(super) fn continue_card(&self, run: &Run, cx: &mut Context<Self>) -> AnyElement {
        let open = run
            .critiques
            .iter()
            .filter(|c| c.material() && !c.status.is_settled())
            .count();
        let stop = run
            .deliberation
            .stop
            .map(|s| format!("토론은 ‘{}’로 멈췄습니다. ", s.label()))
            .unwrap_or_default();
        let can = run.plan.is_some() && !run.theory().is_empty();
        let rounds = self.continue_rounds;
        let mut picker = h_flex()
            .gap_0p5()
            .p_0p5()
            .rounded(cx.theme().radius)
            .bg(cx.theme().muted);
        for n in [1u32, 2, 4] {
            picker = picker.child(
                Button::new(SharedString::from(format!("continue-rounds-{n}")))
                    .ghost()
                    .xsmall()
                    .selected(rounds == n)
                    .label(format!("+{n}회"))
                    .tooltip(format!("비판↔수정 {n}회 더 · 호출 최대 {}회", 2 * n + 3))
                    .disabled(self.busy())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.continue_rounds = n;
                        cx.notify();
                    })),
            );
        }
        let id = run.id.clone();
        card(cx)
            .id("continue-debate")
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(Icon::new(Lucide::FastForward).small())
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("토론 이어 가기")),
            )
            .child(muted(
                format!(
                    "{stop}{}회차를 더 주면 두 모델이 같은 작업 이론에서 토론을 계속하고, 끝나면 답변과 보고서를 다시 씁니다. 방향을 적으면 두 모델 모두 그 방향을 따릅니다.",
                    if open > 0 {
                        format!("남은 주요 비판 {open}개를 다시 다룹니다. ")
                    } else {
                        "남은 주요 비판이 없으면 비판 모델이 먼저 이론을 다시 검토합니다. ".into()
                    }
                ),
                cx,
            ))
            .child(
                Textarea::new(&self.direction)
                    .aria_label("토론 방향")
                    .accessibility_id("continue-direction")
                    .readonly(self.busy() || !can),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .flex_wrap()
                    .child(picker)
                    .child(div().flex_1())
                    .child(
                        Button::new("continue-run")
                            .primary()
                            .small()
                            .icon(Icon::new(Lucide::FastForward))
                            .label("토론 이어 가기")
                            .disabled(self.busy() || !can)
                            .on_click(cx.listener(move |this, _, w, cx| this.continue_run(id.clone(), w, cx))),
                    ),
            )
            .when(!can, |v| v.child(muted("이어 갈 작업 이론이 없습니다. 새 질문으로 실행하세요.", cx)))
            .into_any_element()
    }
}
