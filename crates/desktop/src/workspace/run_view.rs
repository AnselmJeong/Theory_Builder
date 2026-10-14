//! One question: the reader report, the working theory behind it, how the two models
//! argued their way there, which text supports it, and the raw request record.
use super::*;
use gpui_kit::component::tab::{Tab, TabBar};
use std::collections::HashMap;

impl Workspace {
    pub(super) fn run_view(&self, run: &Run, cx: &mut Context<Self>) -> AnyElement {
        let body = match self.run_tab {
            RunTab::Report => self.report_tab(run, cx),
            RunTab::Theory => self.theory_tab(run, cx),
            RunTab::Debate => self.debate_tab(run, cx),
            RunTab::Evidence => self.evidence_tab(run, cx),
            RunTab::Log => self.log_tab(run, cx),
        };
        v_flex()
            .gap_5()
            .child(self.run_header(run, cx))
            .child(body)
            .into_any_element()
    }
    fn run_header(&self, run: &Run, cx: &mut Context<Self>) -> impl IntoElement {
        let d = &run.deliberation;
        let elapsed = run.finished.unwrap_or_else(now).saturating_sub(run.started);
        // Who wrote what the reader sees: the report's writer, else the synthesis writer.
        let writer = run
            .reading
            .as_ref()
            .map(|r| r.provider.clone())
            .filter(|p| !p.is_empty())
            .or_else(|| Some(d.writer.clone()).filter(|p| !p.is_empty()));
        let mut meta = h_flex()
            .flex_wrap()
            .gap_2()
            .items_center()
            .child(chip(run.state.label(), state_color(run.state, cx)));
        if !d.author.is_empty() {
            meta = meta.child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .text_xs()
                    .child(dot(provider_color(&d.author, cx)))
                    .child(format!("{} 구성", who(&d.author)))
                    .when(d.cross_model(), |v| {
                        v.child(
                            Icon::new(Lucide::ArrowLeftRight)
                                .xsmall()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(dot(provider_color(&d.critic, cx)))
                        .child(format!("{} 비판", who(&d.critic)))
                    })
                    .when(!d.cross_model(), |v| v.child("· 같은 모델 별도 문맥 비판"))
                    .when_some(writer, |v, writer| {
                        v.child(div().text_color(cx.theme().muted_foreground).child("·"))
                            .child(dot(provider_color(&writer, cx)))
                            .child(format!("{} 정리", who(&writer)))
                    }),
            );
        }
        if d.rounds > 0 || d.stop.is_some() {
            meta = meta.child(neutral_chip(
                match d.stop {
                    Some(stop) => format!("수정 {}회 · {}", d.rounds, stop.label()),
                    None => format!("수정 {}회 진행", d.rounds),
                },
                cx,
            ));
        }
        meta = meta.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!(
                    "호출 {}/{} · {} · {}",
                    run.calls,
                    run.budget.calls(),
                    duration_label(elapsed),
                    run.budget.name()
                )),
        );
        let copy_id = run.id.clone();
        let finished = run.state != RunState::Running && !run.report.is_empty();
        let open_id = run.id.clone();
        let export_id = run.id.clone();
        let question = run.question.clone();
        let targets = run.targets.clone();
        let critiques = run.critiques.len();
        let evidence = run
            .attempts
            .iter()
            .filter(|a| a.state == TaskState::Succeeded)
            .filter_map(|a| a.output.as_ref())
            .map(|o| o.evidence_refs.len())
            .sum::<usize>();
        let steps = run
            .attempts
            .iter()
            .filter(|a| phase_of(a) != Phase::Plan)
            .count();
        let tabs = TabBar::new("run-tabs")
            .underline()
            .selected_index(match self.run_tab {
                RunTab::Report => 0,
                RunTab::Theory => 1,
                RunTab::Debate => 2,
                RunTab::Evidence => 3,
                RunTab::Log => 4,
            })
            .on_click(cx.listener(|this, ix: &usize, _, cx| {
                this.run_tab = match ix {
                    0 => RunTab::Report,
                    1 => RunTab::Theory,
                    2 => RunTab::Debate,
                    3 => RunTab::Evidence,
                    _ => RunTab::Log,
                };
                cx.notify();
            }))
            .child(Tab::new().label("보고서"))
            .child(Tab::new().label("이론 상세"))
            .child(Tab::new().label(if critiques > 0 {
                format!("토론 · 비판 {critiques}")
            } else {
                format!("토론 · {steps}단계")
            }))
            .child(Tab::new().label(format!("근거 {evidence}")))
            .child(Tab::new().label("작업 기록"));
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_start()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .line_height(rems(1.75))
                            .child(run.question.clone()),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .flex_none()
                            .child(
                                Button::new("copy-answer")
                                    .ghost()
                                    .small()
                                    .icon(Icon::new(Lucide::Copy))
                                    .tooltip("보고서를 Markdown으로 복사")
                                    .disabled(!finished)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        // Built on click: the whole report as readable Markdown.
                                        let Some(s) = &this.snapshot else { return };
                                        let Some(run) =
                                            s.project.runs.iter().find(|r| r.id == copy_id)
                                        else {
                                            return;
                                        };
                                        let view =
                                            theory_engine::report::reader_view(&s.project, run);
                                        let md = theory_engine::report::run_markdown(
                                            run,
                                            &view,
                                            1,
                                            &|f| format!("{}.svg", f.id),
                                        );
                                        let text = view
                                            .references
                                            .display(&md, theory_engine::report::CiteStyle::Text);
                                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                                    })),
                            )
                            .child(
                                Button::new("open-report-browser")
                                    .ghost()
                                    .small()
                                    .icon(Icon::new(Lucide::ExternalLink))
                                    .tooltip("브라우저에서 인터랙티브 보고서 열기")
                                    .disabled(!finished || self.busy())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.submit(Command::OpenReport(open_id.clone()), cx)
                                    })),
                            )
                            .child(
                                Button::new("export-report")
                                    .ghost()
                                    .small()
                                    .icon(Icon::new(Lucide::FileDown))
                                    .tooltip("이 질문의 보고서 내보내기 (report.md · references.bib · report.html)")
                                    .disabled(!finished || self.busy())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.choose_path(&format!("report:{export_id}"), cx)
                                    })),
                            )
                            .child(
                                Button::new("retry-question")
                                    .ghost()
                                    .small()
                                    .icon(Icon::new(Lucide::RotateCcw))
                                    .tooltip("이 질문을 입력창에 다시 불러오기")
                                    .disabled(self.busy())
                                    .on_click(cx.listener(move |this, _, w, cx| {
                                        this.prompt.update(cx, |p, cx| {
                                            p.set_value(question.clone(), w, cx);
                                            p.focus(w, cx);
                                        });
                                        this.targets = targets.clone();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(meta)
            .when(run.state == RunState::Running, |v| {
                v.child(self.live_progress(run, cx))
            })
            .child(tabs)
    }
    /// Where the run is now: the current step and the chain of completed steps.
    fn live_progress(&self, run: &Run, cx: &mut Context<Self>) -> impl IntoElement {
        let mut chain = h_flex().flex_wrap().gap_1().items_center();
        for (i, a) in run
            .attempts
            .iter()
            .filter(|a| phase_of(a) != Phase::Plan)
            .enumerate()
        {
            if i > 0 {
                chain = chain.child(
                    Icon::new(Lucide::ChevronRight)
                        .xsmall()
                        .text_color(cx.theme().muted_foreground),
                );
            }
            let color = match a.state {
                TaskState::Succeeded => provider_color(&a.provider, cx),
                TaskState::Failed | TaskState::Blocked => cx.theme().danger,
                _ => cx.theme().muted_foreground,
            };
            chain = chain.child(chip(step_title(a), color));
        }
        h_flex()
            .gap_3()
            .items_start()
            .p_3()
            .rounded(cx.theme().radius_lg)
            .bg(cx.theme().muted)
            .child(Spinner::new().small())
            .child(
                v_flex()
                    .gap_2()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.status.clone()),
                    )
                    .child(chain),
            )
    }
    fn evidence_count(run: &Run) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for e in run
            .attempts
            .iter()
            .filter(|a| a.state == TaskState::Succeeded)
            .filter_map(|a| a.output.as_ref())
            .flat_map(|o| &o.evidence_refs)
        {
            *counts.entry(e.claim_id.clone()).or_insert(0) += 1;
        }
        counts
    }
    /// The material behind the report: the synthesis as the author model wrote it, the
    /// numbered working theory, and objections that are still open.
    fn theory_tab(&self, run: &Run, cx: &mut Context<Self>) -> AnyElement {
        let names = self
            .snapshot
            .as_ref()
            .map(|s| theory_engine::execution::reader::internal_names(&s.project, run))
            .unwrap_or_default();
        let friendly = |t: &str| theory_engine::execution::reader::friendly(t, &names);
        let mut body = v_flex().gap_5().child(muted(
            "보고서의 재료입니다. 두 모델이 토론하며 만들고 고친 가설 조각(작업 이론)과, 구성 모델이 쓴 토론 종합 원문을 보여 줍니다. 아직 아무것도 채택되지 않았으며, 주장·가정 페이지에서 항목을 채택하거나 철회할 수 있습니다.",
            cx,
        ));
        if run.report.is_empty() {
            body = body.child(
                card(cx)
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child("두 모델이 이론을 다듬고 있습니다"),
                    )
                    .child(muted(
                        "초안 → 비판 → 응답·수정 → 재검토가 반복된 뒤 답변이 작성됩니다. 진행 중인 의견 교환은 토론 탭에서 볼 수 있습니다.",
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
                    ),
            );
        } else {
            body = body
                .child(section_title("토론 종합 (구성 모델의 원문)", cx))
                .child(card(cx).child(div().text_sm().child(document(
                    SharedString::from(format!("report-{}", run.id)),
                    friendly(&run.report),
                ))));
        }
        let theory = run.theory();
        if !theory.is_empty() {
            let counts = Self::evidence_count(run);
            let disputed: HashMap<&str, CritiqueStatus> = run
                .critiques
                .iter()
                .filter(|c| c.material() && !c.status.is_settled())
                .map(|c| (c.critique.target_id.as_str(), c.status))
                .collect();
            let mut list = v_flex().gap_1();
            for (i, p) in theory.iter().enumerate() {
                let id = p.id.clone();
                let n = counts.get(&p.id).copied().unwrap_or(0);
                list = list.child(observed(
                    h_flex()
                        .id(SharedString::from(format!("theory-{}", p.id)))
                        .gap_3()
                        .items_start()
                        .px_3()
                        .py_2()
                        .rounded(cx.theme().radius)
                        .cursor_pointer()
                        .hover(|s| s.bg(cx.theme().muted))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_inspector(Inspector::Entry(id.clone()), cx)
                        }))
                        .child(
                            div()
                                .flex_none()
                                .w(rems(1.25))
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{}", i + 1)),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_1()
                                .child(div().text_sm().child(friendly(&p.statement)))
                                .child(
                                    h_flex()
                                        .gap_1()
                                        .flex_wrap()
                                        .child(chip(
                                            claim_kind_label(p.claim_kind),
                                            claim_kind_color(p.claim_kind, cx),
                                        ))
                                        .when(n > 0, |v| {
                                            v.child(chip(
                                                format!("원문 근거 {n}"),
                                                cx.theme().success,
                                            ))
                                        })
                                        .when(p.replaces_id.is_some(), |v| {
                                            v.child(neutral_chip("기존 항목 수정안", cx))
                                        })
                                        .when_some(disputed.get(p.id.as_str()), |v, s| {
                                            v.child(chip(s.label(), status_color(*s, cx)))
                                        }),
                                ),
                        ),
                ));
            }
            body = body
                .child(section_title(
                    format!(
                        "작업 이론 · 항목 {}개 (채택 전 제안 · 번호는 ‘작업 이론 n’)",
                        theory.len()
                    ),
                    cx,
                ))
                .child(list);
        }
        let open: Vec<_> = run
            .critiques
            .iter()
            .filter(|c| !c.status.is_settled())
            .collect();
        if !open.is_empty() {
            body = body.child(section_title(
                format!("남은 이견과 미해결 비판 · {}개", open.len()),
                cx,
            ));
            for c in open {
                body = body.child(self.critique_card(run, c, cx));
            }
        }
        if !run.limitations.is_empty() {
            let mut notes = v_flex().gap_1();
            for l in &run.limitations {
                notes = notes.child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .child(
                            Icon::new(Lucide::Info)
                                .xsmall()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(friendly_error(l)),
                        ),
                );
            }
            body = body.child(section_title("실행 메모", cx)).child(notes);
        }
        body.into_any_element()
    }
    /// A critique, its target, and the latest exchange about it.
    pub(super) fn critique_card(
        &self,
        run: &Run,
        c: &CritiqueRecord,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let target = run
            .theory()
            .into_iter()
            .chain(
                run.attempts
                    .iter()
                    .filter_map(|a| a.output.as_ref())
                    .flat_map(|o| o.proposed_changes.iter().cloned()),
            )
            .find(|p| p.id == c.critique.target_id)
            .map(|p| short(&p.statement, 90))
            .unwrap_or_else(|| "초안 전체".into());
        let last_response = c.events.iter().rev().find(|e| e.stance.is_some());
        let run_id = run.id.clone();
        let id = c.id.clone();
        let card = card(cx)
            .id(SharedString::from(format!("critique-card-{}", c.id)))
            .cursor_pointer()
            .hover(|s| s.border_color(cx.theme().ring))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_inspector(Inspector::Critique(run_id.clone(), id.clone()), cx)
            }))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .flex_wrap()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(c.label()),
                    )
                    .child(chip(
                        severity_label(&c.critique.severity),
                        severity_color(&c.critique.severity, cx),
                    ))
                    .child(chip(c.status.label(), status_color(c.status, cx)))
                    .when(!c.critique.category.is_empty(), |v| {
                        v.child(neutral_chip(c.critique.category.clone(), cx))
                    })
                    .child(div().flex_1())
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(dot(provider_color(&c.provider, cx)))
                            .child(format!("{} 제기", who(&c.provider))),
                    ),
            )
            .child(div().text_sm().child(c.critique.reasoning.clone()))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("대상: {target}")),
            )
            .when_some(last_response, |v, e| {
                v.child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .pl_2()
                        .border_l_2()
                        .border_color(provider_color(&e.provider, cx).opacity(0.5))
                        .child(chip(
                            e.stance.map(|s| s.label()).unwrap_or(""),
                            stance_color(e.stance.unwrap_or_default(), cx),
                        ))
                        .child(div().flex_1().min_w_0().text_xs().child(format!(
                            "{}: {}",
                            who(&e.provider),
                            short(&e.reasoning, 220)
                        ))),
                )
            });
        observed(card)
    }
    /// The recursive exchange, round by round: author on the left, critic on the right.
    fn debate_tab(&self, run: &Run, cx: &mut Context<Self>) -> AnyElement {
        let steps: Vec<&Attempt> = run
            .attempts
            .iter()
            .filter(|a| !matches!(phase_of(a), Phase::Plan | Phase::Ask))
            .filter(|a| {
                // Show a failed attempt only when it was not retried successfully.
                a.state == TaskState::Succeeded
                    || matches!(a.state, TaskState::Running | TaskState::Validating)
                    || !run.attempts.iter().any(|b| {
                        b.task_id == a.task_id
                            && b.round == a.round
                            && b.state == TaskState::Succeeded
                    })
            })
            .collect();
        let mut body = v_flex().gap_3();
        if let Some(plan) = &run.plan {
            body = body.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(Icon::new(Lucide::ListTree).small())
                    .child(format!("계획 · {}", plan.scope)),
            );
        }
        if steps.is_empty() {
            return body
                .child(muted("아직 모델 응답이 없습니다. 계획과 문헌 검색이 끝나면 첫 초안이 여기에 나타납니다.", cx))
                .into_any_element();
        }
        let mut last_group: Option<String> = None;
        let mut shown_directions = 0;
        for a in steps {
            let group = match (phase_of(a), a.round) {
                (Phase::Synthesis | Phase::Report, r)
                    if r > 0 && run.deliberation.continuations > 0 =>
                {
                    format!("최종 종합 · {r}차까지")
                }
                (Phase::Synthesis | Phase::Report, _) => "최종 종합".to_string(),
                (_, 0) => "초기 구성과 첫 비판".to_string(),
                (_, r) => format!("{r}차 토론 — 응답·수정과 재검토"),
            };
            // The user's direction opens the round it was given for.
            for d in run.directions.iter().skip(shown_directions) {
                if a.round < d.round || phase_of(a) == Phase::Report {
                    break;
                }
                shown_directions += 1;
                body = body.child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .p_3()
                        .mt_2()
                        .rounded(cx.theme().radius_lg)
                        .border_1()
                        .border_color(report_view::accent(cx).opacity(0.4))
                        .child(
                            Icon::new(Lucide::User)
                                .small()
                                .text_color(report_view::accent(cx)),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(report_view::accent(cx))
                                        .child(format!("사용자 방향 · {}차부터", d.round)),
                                )
                                .child(div().text_sm().child(d.text.clone())),
                        ),
                );
            }
            if last_group.as_ref() != Some(&group) {
                body = body.child(
                    h_flex()
                        .pt_3()
                        .gap_3()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(cx.theme().muted_foreground)
                                .child(group.clone()),
                        )
                        .child(div().flex_1().h(px(1.)).bg(cx.theme().border)),
                );
                last_group = Some(group);
            }
            body = body.child(self.debate_step(run, a, cx));
        }
        if let Some(stop) = run.deliberation.stop {
            let settled = run
                .critiques
                .iter()
                .filter(|c| c.status.is_settled())
                .count();
            body = body.child(
                h_flex()
                    .mt_2()
                    .gap_2()
                    .items_center()
                    .p_3()
                    .rounded(cx.theme().radius_lg)
                    .bg(cx.theme().muted)
                    .child(Icon::new(match stop {
                        StopReason::Converged => Lucide::CircleCheck,
                        StopReason::Stalled => Lucide::Scale,
                        _ => Lucide::CircleAlert,
                    }))
                    .child(div().flex_1().text_sm().child(format!(
                        "토론 종료 · {} · 수정 {}회 · 비판 {}개 중 {}개 정리",
                        stop.label(),
                        run.deliberation.rounds,
                        run.critiques.len(),
                        settled
                    ))),
            );
        }
        if run.state != RunState::Running && run.deliberation.stop.is_some() {
            body = body.child(self.continue_card(run, cx));
        }
        body.into_any_element()
    }
    fn debate_step(&self, run: &Run, a: &Attempt, cx: &mut Context<Self>) -> impl IntoElement {
        let phase = phase_of(a);
        let critic_side = phase == Phase::Review;
        let color = provider_color(&a.provider, cx);
        let id = a.id.clone();
        let mut bubble = v_flex()
            .gap_2()
            .p_3()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(color.opacity(0.25))
            .bg(color.opacity(0.04))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(avatar(&a.provider, cx))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(step_title(a)),
                    )
                    .child(div().flex_1())
                    .child(match a.state {
                        TaskState::Succeeded => div(),
                        TaskState::Running | TaskState::Validating => h_flex()
                            .gap_1()
                            .items_center()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(Spinner::new().xsmall())
                            .child(progress::task_state(a.state)),
                        s => chip(progress::task_state(s), cx.theme().danger),
                    })
                    .child(
                        Button::new(SharedString::from(format!("step-log-{}", a.id)))
                            .ghost()
                            .xsmall()
                            .icon(Icon::new(Lucide::FileText))
                            .tooltip(if a.model.is_empty() {
                                "요청·응답 기록".to_string()
                            } else {
                                format!("요청·응답 기록 · {}", a.model)
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_inspector(Inspector::Task(id.clone()), cx)
                            })),
                    ),
            );
        if let Some(e) = &a.error
            && a.state != TaskState::Succeeded
        {
            bubble = bubble.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(friendly_error(e)),
            );
        }
        let objective = run
            .plan
            .as_ref()
            .and_then(|p| p.tasks.iter().find(|t| t.id == a.task_id))
            .filter(|_| a.round == 0 && phase != Phase::Synthesis)
            .map(|t| t.objective.clone());
        if let Some(objective) = objective {
            bubble = bubble.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("과제: {objective}")),
            );
        }
        if phase == Phase::Report && a.state == TaskState::Succeeded {
            bubble = bubble
                .child(muted(
                    "서기가 토론에 참여하지 않은 별도 문맥에서 결론·작업 이론·비판을 읽고, 그림과 용어 풀이를 갖춘 읽기 쉬운 보고서로 정리했습니다.",
                    cx,
                ))
                .child(
                    h_flex().child(
                        Button::new(SharedString::from(format!("debate-open-report-{}", a.id)))
                            .ghost()
                            .xsmall()
                            .icon(Icon::new(Lucide::ArrowRight))
                            .label("보고서 보기")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.run_tab = RunTab::Report;
                                cx.notify();
                            })),
                    ),
                );
            return h_flex().w_full().child(bubble.w_full());
        }
        if let Some(o) = &a.output {
            if phase == Phase::Synthesis {
                let first = o
                    .summary
                    .split("\n\n")
                    .next()
                    .unwrap_or_default()
                    .to_string();
                bubble = bubble
                    .child(div().text_sm().child(document(
                        SharedString::from(format!("step-summary-{}", a.id)),
                        first,
                    )))
                    .child(
                        h_flex().child(
                            Button::new("debate-open-answer")
                                .ghost()
                                .xsmall()
                                .icon(Icon::new(Lucide::ArrowRight))
                                .label("이론 상세 보기")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.run_tab = RunTab::Theory;
                                    cx.notify();
                                })),
                        ),
                    );
                return h_flex().w_full().child(bubble.w_full());
            }
            bubble = bubble.child(div().text_sm().child(document(
                SharedString::from(format!("step-summary-{}", a.id)),
                o.summary.clone(),
            )));
            if !o.critique_responses.is_empty() {
                let mut list = v_flex().gap_1p5();
                for r in &o.critique_responses {
                    list = list.child(
                        h_flex()
                            .gap_2()
                            .items_start()
                            .child(
                                div()
                                    .flex_none()
                                    .w(rems(3.))
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(critique_label(&r.critique_id)),
                            )
                            .child(chip(r.stance.label(), stance_color(r.stance, cx)))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .child(r.reasoning.clone()),
                            ),
                    );
                }
                bubble = bubble
                    .child(section_title("비판에 대한 응답", cx))
                    .child(list);
            }
            if !o.critique_reviews.is_empty() {
                let mut list = v_flex().gap_1p5();
                for r in &o.critique_reviews {
                    list = list.child(
                        h_flex()
                            .gap_2()
                            .items_start()
                            .child(
                                div()
                                    .flex_none()
                                    .w(rems(3.))
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(critique_label(&r.critique_id)),
                            )
                            .child(chip(r.status.label(), status_color(r.status, cx)))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .child(r.reasoning.clone()),
                            ),
                    );
                }
                bubble = bubble
                    .child(section_title("이전 비판 판정", cx))
                    .child(list);
            }
            if !o.critiques.is_empty() {
                let ids: Vec<&CritiqueRecord> = run
                    .critiques
                    .iter()
                    .filter(|c| c.task_id == a.task_id && c.round == a.round)
                    .collect();
                let mut list = v_flex().gap_1p5();
                for (i, k) in o.critiques.iter().enumerate() {
                    let record = ids.get(i).copied();
                    let critique_id = record.map(|c| c.id.clone()).unwrap_or_default();
                    let label = record.map(|c| c.label()).unwrap_or_default();
                    let run_id = run.id.clone();
                    list = list.child(observed(
                        h_flex()
                            .id(SharedString::from(format!("new-critique-{}-{i}", a.id)))
                            .gap_2()
                            .items_start()
                            .rounded(cx.theme().radius)
                            .when(record.is_some(), |v| {
                                v.cursor_pointer()
                                    .hover(|s| s.bg(cx.theme().muted))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_inspector(
                                            Inspector::Critique(
                                                run_id.clone(),
                                                critique_id.clone(),
                                            ),
                                            cx,
                                        )
                                    }))
                            })
                            .child(
                                div()
                                    .flex_none()
                                    .w(rems(3.))
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(label),
                            )
                            .child(chip(
                                severity_label(&k.severity),
                                severity_color(&k.severity, cx),
                            ))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_0p5()
                                    .child(div().text_xs().child(k.reasoning.clone()))
                                    .when(!k.suggested_action.is_empty(), |v| {
                                        v.child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!("→ {}", k.suggested_action)),
                                        )
                                    }),
                            )
                            .when_some(record, |v, r| {
                                v.child(chip(
                                    format!("현재 {}", r.status.label()),
                                    status_color(r.status, cx),
                                ))
                            }),
                    ));
                }
                bubble = bubble.child(section_title("새 비판", cx)).child(list);
            }
            if !o.proposed_changes.is_empty() && phase != Phase::Review {
                let counts = Self::evidence_count(run);
                let mut list = v_flex().gap_1();
                for p in &o.proposed_changes {
                    let n = counts.get(&p.id).copied().unwrap_or(0);
                    list = list.child(
                        h_flex()
                            .gap_2()
                            .items_start()
                            .child(chip(
                                claim_kind_label(p.claim_kind),
                                claim_kind_color(p.claim_kind, cx),
                            ))
                            .child(div().flex_1().min_w_0().text_xs().child(
                                if p.replaces_id.is_some() {
                                    format!("수정 · {}", p.statement)
                                } else {
                                    p.statement.clone()
                                },
                            ))
                            .when(n > 0, |v| {
                                v.child(chip(format!("근거 {n}"), cx.theme().success))
                            }),
                    );
                }
                bubble = bubble
                    .child(section_title(
                        if phase == Phase::Revise {
                            "수정·추가한 항목"
                        } else {
                            "제안한 항목"
                        },
                        cx,
                    ))
                    .child(list);
            }
            let removed = a
                .validation_notes
                .iter()
                .filter(|n| n.contains("인용 제외"))
                .count();
            if removed > 0 {
                bubble = bubble.child(
                    h_flex()
                        .gap_1()
                        .items_center()
                        .text_xs()
                        .text_color(amber_text(cx))
                        .child(Icon::new(Lucide::ShieldCheck).xsmall())
                        .child(format!(
                            "원문과 일치하지 않은 인용 {removed}개를 제외했습니다 (기록 아이콘에서 확인)"
                        )),
                );
            }
        }
        h_flex()
            .w_full()
            .when(critic_side, |v| v.pl(rems(4.)))
            .when(!critic_side && phase != Phase::Synthesis, |v| {
                v.pr(rems(4.))
            })
            .child(bubble.w_full())
    }
    fn evidence_tab(&self, run: &Run, cx: &mut Context<Self>) -> AnyElement {
        let Some(snapshot) = &self.snapshot else {
            return div().into_any_element();
        };
        let claims: HashMap<String, String> = run
            .attempts
            .iter()
            .filter_map(|a| a.output.as_ref())
            .flat_map(|o| o.proposed_changes.iter())
            .map(|p| (p.id.clone(), p.statement.clone()))
            .collect();
        let mut by_source: Vec<(String, Vec<Evidence>)> = vec![];
        for e in run
            .attempts
            .iter()
            .filter(|a| a.state == TaskState::Succeeded)
            .filter_map(|a| a.output.as_ref())
            .flat_map(|o| o.evidence_refs.iter())
        {
            match by_source.iter_mut().find(|(s, _)| *s == e.source_id) {
                Some((_, list)) => list.push(e.clone()),
                None => by_source.push((e.source_id.clone(), vec![e.clone()])),
            }
        }
        let mut body = v_flex().gap_4().child(muted(
            "모델이 인용한 구절 중 저장된 원문과 글자 그대로 일치하고 실제로 전달된 범위에 있는 것만 남깁니다. 일치는 문자열 검증이며, 그 구절이 주장을 실제로 뒷받침하는지는 각 판정과 원문을 함께 확인하세요.",
            cx,
        ));
        if by_source.is_empty() {
            body = body.child(muted(
                "이 질문에서는 검증된 원문 인용이 없습니다. 자료를 추가하거나 문헌 검색을 켜고 다시 실행하세요.",
                cx,
            ));
        }
        for (source_id, list) in by_source {
            let source = snapshot.project.sources.iter().find(|s| s.id == source_id);
            let title = source
                .map(|s| s.title.clone())
                .unwrap_or_else(|| source_id.clone());
            let mut group = card(cx).child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(Icon::new(Lucide::BookOpen).small())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .when_some(source, |v, s| {
                        v.child(chip(access_label(s.access), access_color(s.access, cx)))
                    }),
            );
            for (i, e) in list.iter().enumerate() {
                let evidence = e.clone();
                let claim_id = e.claim_id.clone();
                group = group.child(
                    v_flex()
                        .gap_1()
                        .pl_3()
                        .border_l_2()
                        .border_color(relation_color(e.relation, cx).opacity(0.6))
                        .child(div().text_sm().italic().child(format!("“{}”", e.quote)))
                        .child(
                            h_flex()
                                .gap_2()
                                .flex_wrap()
                                .items_center()
                                .child(chip(
                                    relation_label(e.relation),
                                    relation_color(e.relation, cx),
                                ))
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "ev-src-{source_id}-{i}"
                                    )))
                                    .ghost()
                                    .xsmall()
                                    .label(format!(
                                        "{} 원문 보기",
                                        lines_label(e.start_line, e.end_line)
                                    ))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_inspector(
                                            Inspector::Source(
                                                evidence.source_id.clone(),
                                                Some(evidence.clone()),
                                            ),
                                            cx,
                                        )
                                    })),
                                )
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "ev-claim-{source_id}-{i}"
                                    )))
                                    .ghost()
                                    .xsmall()
                                    .label(format!(
                                        "주장: {}",
                                        short(
                                            claims
                                                .get(&e.claim_id)
                                                .map(String::as_str)
                                                .unwrap_or("?"),
                                            48
                                        )
                                    ))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_inspector(Inspector::Entry(claim_id.clone()), cx)
                                    })),
                                ),
                        )
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
            body = body.child(group);
        }
        let excluded: Vec<String> = run
            .attempts
            .iter()
            .flat_map(|a| a.validation_notes.iter())
            .filter(|n| n.contains("인용 제외"))
            .cloned()
            .collect();
        if !excluded.is_empty() {
            body = body.child(section_title(
                format!("검증에서 제외된 인용 · {}개", excluded.len()),
                cx,
            ));
            for n in excluded {
                body = body.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(n),
                );
            }
        }
        body.into_any_element()
    }
    fn log_tab(&self, run: &Run, cx: &mut Context<Self>) -> AnyElement {
        let mut body = v_flex().gap_3();
        if let Some(plan) = &run.plan {
            let mut tasks = v_flex().gap_1();
            for t in &plan.tasks {
                tasks = tasks.child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .child(neutral_chip(t.kind.label(), cx))
                        .child(div().flex_1().text_xs().child(t.objective.clone())),
                );
            }
            body = body.child(
                card(cx)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("계획 · {}", plan.scope)),
                    )
                    .when(!plan.search_queries.is_empty(), |v| {
                        v.child(muted(
                            format!("문헌 검색어: {}", plan.search_queries.join(" | ")),
                            cx,
                        ))
                    })
                    .child(tasks),
            );
        }
        for a in &run.attempts {
            let id = a.id.clone();
            body = body.child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .py_1p5()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(dot(provider_color(&a.provider, cx)))
                    .child(
                        div()
                            .w(rems(13.))
                            .flex_none()
                            .text_sm()
                            .child(step_title(a)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(if a.model.is_empty() {
                                "계정 기본 모델".to_string()
                            } else {
                                a.model.clone()
                            }),
                    )
                    .child(chip(
                        progress::task_state(a.state),
                        match a.state {
                            TaskState::Succeeded => cx.theme().success,
                            TaskState::Failed | TaskState::Blocked => cx.theme().danger,
                            _ => cx.theme().muted_foreground,
                        },
                    ))
                    .child(
                        Button::new(SharedString::from(format!("attempt-{}", a.id)))
                            .ghost()
                            .xsmall()
                            .label("요청·응답")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_inspector(Inspector::Task(id.clone()), cx)
                            })),
                    ),
            );
        }
        body.into_any_element()
    }
}
fn amber_text(_cx: &App) -> Hsla {
    gpui_kit::component::amber_600()
}
