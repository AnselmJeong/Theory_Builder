use super::*;
impl Workspace {
    pub(super) fn content(&self, cx: &mut Context<Self>) -> AnyElement {
        let body = match self.page {
            Page::Inquiry => self.inquiry(cx),
            Page::Claims => self.ledger(false, cx),
            Page::Assumptions => self.ledger(true, cx),
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
                    .p_6()
                    .w_full()
                    .max_w(rems(61.25))
                    .mx_auto()
                    .child(body),
            )
            .into_any_element()
    }
    fn inquiry(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = v_flex().gap_5();
        if let Some(run) = self.current_run() {
            body = body
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(run.question.clone()),
                )
                .child(muted(
                    format!(
                        "{} · 호출 {}/{} · 기준 r{}",
                        run.state.label(),
                        run.calls,
                        run.budget.calls(),
                        run.base_revision
                    ),
                    cx,
                ));
            if let Some(plan) = &run.plan {
                body = body.child(muted(plan.scope.clone(), cx));
            }
            if self.show_tasks {
                if let Some(plan) = &run.plan {
                    for task in &plan.tasks {
                        body = body.child(muted(
                            format!("{} · {}", task.kind.label(), task.objective),
                            cx,
                        ));
                    }
                }
                for a in &run.attempts {
                    let id = a.id.clone();
                    body = body.child(
                        v_flex()
                            .gap_2()
                            .pb_4()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(div().flex_1().font_weight(FontWeight::MEDIUM).child(
                                        format!(
                                            "{} · {}",
                                            a.role.map(|r| r.label()).unwrap_or("계획 구성"),
                                            task_state(a.state)
                                        ),
                                    ))
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "attempt-{}",
                                            a.id
                                        )))
                                        .small()
                                        .label("요청·응답 보기")
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.inspector = Inspector::Task(id.clone());
                                                this.show_inspector = true;
                                                cx.notify();
                                            }),
                                        ),
                                    ),
                            )
                            .child(muted(
                                format!(
                                    "{} · {}",
                                    a.provider,
                                    if a.model.is_empty() {
                                        "계정 기본 모델"
                                    } else {
                                        &a.model
                                    }
                                ),
                                cx,
                            ))
                            .when_some(a.error.clone(), |v, e| {
                                v.child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().danger)
                                        .child(friendly_error(&e)),
                                )
                            }),
                    );
                }
            }
            if !run.report.is_empty() {
                body = body.child(document(
                    SharedString::from(format!("report-{}", run.id)),
                    run.report.clone(),
                ));
            } else {
                body=body.child(muted(if run.state == RunState::Running { "질문을 검토하고 있습니다. 현재 단계와 대기 상태는 입력창 바로 위에 표시됩니다." } else { "이 실행에서는 최종 답변을 만들지 못했습니다. 아래 실행 상태에서 원인을 확인하고 질문을 다시 실행할 수 있습니다." },cx));
            }
            if let Some(output) = run
                .attempts
                .iter()
                .rev()
                .find(|a| {
                    a.role == Some(theory_core::Role::Synthesizer)
                        && a.state == TaskState::Succeeded
                })
                .and_then(|a| a.output.as_ref())
            {
                for proposal in &output.proposed_changes {
                    body = body
                        .child(div().font_weight(FontWeight::SEMIBOLD).child("검토할 제안"))
                        .child(document(
                            SharedString::from(format!("proposal-{}", proposal.id)),
                            format!(
                                "{}\n\n**도입 이유**\n\n{}\n\n**적용 범위**\n\n{}",
                                proposal.statement, proposal.rationale, proposal.scope
                            ),
                        ));
                    for id in std::iter::once(&proposal.id).chain(proposal.depends_on.iter()) {
                        let target = id.clone();
                        let label = if *id == proposal.id {
                            "이 제안의 근거·비판 검토".into()
                        } else {
                            self.snapshot
                                .as_ref()
                                .and_then(|s| {
                                    s.project.entries.iter().find(|e| e.proposal.id == *id)
                                })
                                .map(|e| {
                                    format!("연결된 전제 · {}", short(&e.proposal.statement, 55))
                                })
                                .unwrap_or_else(|| "연결된 전제 검토".into())
                        };
                        body = body.child(
                            Button::new(SharedString::from(format!(
                                "proposal-link-{}-{id}",
                                proposal.id
                            )))
                            .ghost()
                            .justify_start()
                            .label(label)
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.inspector = Inspector::Entry(target.clone());
                                    this.show_inspector = true;
                                    cx.notify();
                                },
                            )),
                        );
                    }
                }
            }
            let evidence: Vec<_> = run
                .attempts
                .iter()
                .filter_map(|a| a.output.as_ref())
                .flat_map(|o| o.evidence_refs.iter())
                .collect();
            if !evidence.is_empty() {
                body = body.child(div().font_weight(FontWeight::SEMIBOLD).child("대조한 원문"));
            }
            for (ix, e) in evidence.into_iter().enumerate() {
                let evidence = e.clone();
                let title = self
                    .snapshot
                    .as_ref()
                    .and_then(|s| s.project.sources.iter().find(|s| s.id == e.source_id))
                    .map(|s| s.title.as_str())
                    .unwrap_or("출처");
                body = body.child(
                    v_flex()
                        .gap_2()
                        .child(div().text_sm().child(format!("“{}”", e.quote)))
                        .child(
                            Button::new(SharedString::from(format!("evidence-{}-{ix}", run.id)))
                                .ghost()
                                .small()
                                .justify_start()
                                .label(format!(
                                    "{} · 행 {}–{} · {:?}",
                                    short(title, 30),
                                    e.start_line,
                                    e.end_line,
                                    e.relation
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.inspector = Inspector::Source(
                                        evidence.source_id.clone(),
                                        Some(evidence.clone()),
                                    );
                                    this.show_inspector = true;
                                    cx.notify();
                                })),
                        ),
                );
            }
            for l in &run.limitations {
                body = body.child(muted(friendly_error(l), cx));
            }
            let text = run.report.clone();
            body = body.child(
                Button::new("copy-answer")
                    .small()
                    .label("답변 복사")
                    .disabled(text.is_empty())
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
                    }),
            );
        } else {
            body=body.child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child("어떤 현상을 설명하고 싶나요?")).child(muted("질문을 따라 연구 작업을 구성하고, 근거·가정·이견을 함께 남깁니다.",cx)).child(muted("ScholarPen 폴더의 자료를 사용하고, 문헌 검색이 켜져 있으면 PubMed와 TinyFish에서 필요한 자료를 찾습니다. 자료 없이도 질문부터 시작할 수 있습니다.",cx));
            for (id, title, question) in [
                (
                    "example-concept",
                    "개념 구분",
                    "Metastability와 multistability를 회로 설명에서 구분해 줘. 새로운 이론은 만들지 말고 정의와 적용 조건을 분리해 줘.",
                ),
                (
                    "example-evidence",
                    "주장 검토",
                    "제공한 자료로 ‘세포 수준의 변화가 행동 상태 전환을 일으킨다’고 써도 될까? 실제 구절과 인과 추론의 범위를 대조해 줘.",
                ),
                (
                    "example-mechanism",
                    "기제 후보 구성",
                    "활동 의존적 가소성과 느린 조절 과정이 함께 장기적인 행동 변화를 만들 수 있는 서로 다른 기제 후보를 제안해 줘. 새 가정과 문헌 근거를 구분해 줘.",
                ),
            ] {
                body = body.child(
                    v_flex()
                        .gap_2()
                        .py_3()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            Button::new(id)
                                .ghost()
                                .justify_start()
                                .label(title)
                                .disabled(self.busy())
                                .on_click(cx.listener(move |this, _, w, cx| {
                                    this.prompt.update(cx, |p, cx| {
                                        p.set_value(question, w, cx);
                                        p.focus(w, cx);
                                    });
                                    cx.notify();
                                })),
                        )
                        .child(muted(question, cx)),
                );
            }
            body = body.child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("empty-add-source")
                            .label("자료 추가…")
                            .disabled(self.busy())
                            .on_click(cx.listener(|this, _, _, cx| this.choose_path("source", cx))),
                    )
                    .child(
                        Button::new("empty-connect")
                            .label("구독 연결 설정")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_settings(cx);
                            })),
                    ),
            );
        }
        body.into_any_element()
    }
    fn ledger(&self, assumptions: bool, cx: &mut Context<Self>) -> AnyElement {
        let mut body=v_flex().gap_4().child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child(if assumptions{"용어·가정"}else{"후보·주장"})).child(muted("모델이 만든 항목은 제안으로 남습니다. 채택은 연구자의 선택이며, 과학적 확정을 뜻하지 않습니다.",cx));
        body = body.child(
            h_flex()
                .gap_2()
                .child(
                    Button::new("notes-claims")
                        .small()
                        .label("후보·주장")
                        .selected(!assumptions)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.page = Page::Claims;
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("notes-assumptions")
                        .small()
                        .label("용어·가정")
                        .selected(assumptions)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.page = Page::Assumptions;
                            cx.notify();
                        })),
                ),
        );
        let mut count = 0;
        if let Some(s) = &self.snapshot {
            for e in &s.project.entries {
                if matches!(e.proposal.kind, EntryKind::Assumption | EntryKind::Term) != assumptions
                {
                    continue;
                }
                count += 1;
                let id = e.proposal.id.clone();
                body = body.child(
                    v_flex()
                        .gap_2()
                        .py_3()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(muted(
                            format!("{:?} · {:?} · {:?}", e.proposal.kind, e.adoption, e.review),
                            cx,
                        ))
                        .child(div().child(e.proposal.statement.clone()))
                        .child(
                            Button::new(SharedString::from(format!("entry-{}", e.proposal.id)))
                                .ghost()
                                .small()
                                .justify_start()
                                .label("근거·의존 관계 검토")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.inspector = Inspector::Entry(id.clone());
                                    this.show_inspector = true;
                                    cx.notify();
                                })),
                        ),
                );
            }
        }
        if count == 0 {
            body = body.child(muted(
                "아직 기록된 항목이 없습니다. 질문 실행에서 검증을 통과한 제안이 여기에 쌓입니다.",
                cx,
            ));
        }
        body.into_any_element()
    }
    fn sources(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.search.read(cx).value().to_lowercase();
        let mut body=v_flex().gap_4().child(h_flex().gap_3().child(div().flex_1().text_xl().font_weight(FontWeight::SEMIBOLD).child("자료")).child(Button::new("import-source").label("파일 추가…").disabled(self.busy()).on_click(cx.listener(|this,_,_,cx|this.choose_path("source",cx))))).child(Input::new(&self.search).id("source-search")).child(muted("resources / resource의 논문과 책을 자동으로 읽습니다. PDF·EPUB·DOCX·Markdown·TXT 지원. 스캔 PDF는 OCR이 필요하며, 읽지 못한 자료는 아래에 이유를 표시합니다.",cx));
        if let Some(s) = &self.snapshot {
            let ready = s
                .project
                .sources
                .iter()
                .filter(|s| s.active && !s.text.trim().is_empty())
                .count();
            let unread = s
                .project
                .sources
                .iter()
                .filter(|s| s.active && s.text.trim().is_empty())
                .count();
            body = body.child(muted(
                format!("본문 읽기 완료 {ready}개 · 본문 확인 필요 {unread}개"),
                cx,
            ));
            if let Some(folder) = &s.project.workspace_folder {
                body = body.child(muted(folder.clone(),cx)).child(h_flex().child(Button::new("refresh-resources").small().label("자료 다시 확인").disabled(self.busy()).on_click(cx.listener(|this,_,_,cx| {this.service.cancel.store(false,Ordering::SeqCst);this.submit(Command::SyncResources,cx);}))))
                    .child(muted("앱을 열어 두면 15초마다 변경을 확인하고, 질문 실행 전에도 동기화합니다. 이전 인용 원문은 보존합니다.",cx));
            }
            for source in s.project.sources.iter().filter(|s| s.active) {
                if !query.is_empty()
                    && !source.title.to_lowercase().contains(&query)
                    && !source.text.to_lowercase().contains(&query)
                {
                    continue;
                }
                let id = source.id.clone();
                body = body.child(
                    v_flex()
                        .gap_2()
                        .py_3()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            Button::new(SharedString::from(format!("source-{}", source.id)))
                                .ghost()
                                .justify_start()
                                .label(source.title.clone())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.inspector = Inspector::Source(id.clone(), None);
                                    this.show_inspector = true;
                                    cx.notify();
                                })),
                        )
                        .child(muted(
                            format!(
                                "{} · {}행 · {} · {}",
                                access_label(source.access),
                                source.text.lines().count(),
                                source.locator,
                                source.import_note
                            ),
                            cx,
                        ))
                        .when(!query.is_empty(), |v| {
                            v.child(muted(
                                source
                                    .text
                                    .lines()
                                    .enumerate()
                                    .filter(|(_, l)| l.to_lowercase().contains(&query))
                                    .take(3)
                                    .map(|(i, l)| format!("{}: {}", i + 1, short(l, 100)))
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                                cx,
                            ))
                        }),
                );
            }
        }
        body = body
            .child(
                div()
                    .pt_4()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("원문 구절 붙여 넣기"),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child("출처")
                    .child(Input::new(&self.source_title).id("source-title")),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child("원문 위치")
                    .child(Input::new(&self.source_locator).id("source-location")),
            )
            .child(
                v_flex().gap_2().child("구절").child(
                    Textarea::new(&self.source_text)
                        .aria_label("원문 구절")
                        .accessibility_id("source-excerpt"),
                ),
            )
            .child(
                Button::new("save-excerpt")
                    .label("구절 추가")
                    .disabled(self.busy())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.submit(
                            Command::Excerpt {
                                title: this.source_title.read(cx).value().to_string(),
                                text: this.source_text.read(cx).value().to_string(),
                                locator: this.source_locator.read(cx).value().to_string(),
                                access: Access::ProvidedExcerpt,
                            },
                            cx,
                        );
                    })),
            );
        body.into_any_element()
    }
    fn history(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body=v_flex().gap_4().child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("판본과 변경 이력")).child(muted("과거 판본을 복원하면 현재 기록을 지우지 않고 새 작업안이 생깁니다. 자료와 실행 이력은 유지됩니다.",cx));
        if let Some(s) = &self.snapshot {
            for r in s.project.history.iter().rev() {
                let number = r.number;
                body = body.child(
                    h_flex()
                        .gap_3()
                        .py_2()
                        .child(
                            div()
                                .flex_1()
                                .child(format!("r{} ← r{}  {}", r.number, r.parent, r.reason)),
                        )
                        .child(
                            Button::new(SharedString::from(format!("revision-{number}")))
                                .small()
                                .label("차이 보기")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.selected_revision = Some(number);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new(SharedString::from(format!("restore-{number}")))
                                .small()
                                .label("새 작업안으로 복원")
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
                body = body.child(
                    div()
                        .pt_4()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("r{} → 현재 r{}", r.number, s.project.revision)),
                );
                for e in &s.project.entries {
                    let old = r
                        .entries
                        .iter()
                        .find(|old| old.proposal.id == e.proposal.id);
                    let change = match old {
                        None => "추가",
                        Some(old)
                            if old.adoption != e.adoption
                                || old.review != e.review
                                || old.proposal.statement != e.proposal.statement =>
                        {
                            "변경"
                        }
                        _ => continue,
                    };
                    body = body.child(muted(
                        format!(
                            "{change} · {}\n이전: {:?} / {:?}\n현재: {:?} / {:?}",
                            e.proposal.statement,
                            old.map(|e| e.adoption),
                            old.map(|e| e.review),
                            e.adoption,
                            e.review
                        ),
                        cx,
                    ));
                }
                for e in &r.entries {
                    if !s
                        .project
                        .entries
                        .iter()
                        .any(|current| current.proposal.id == e.proposal.id)
                    {
                        body = body.child(muted(
                            format!("현재 작업안에서 제외 · {}", e.proposal.statement),
                            cx,
                        ));
                    }
                }
            }
        }
        body.into_any_element()
    }
    pub(super) fn inspector(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = v_flex().gap_4().child(
            h_flex()
                .child(
                    div()
                        .flex_1()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("근거 / 변경 제안"),
                )
                .child(
                    Button::new("close-inspector")
                        .ghost()
                        .small()
                        .label("닫기")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_inspector = false;
                            cx.notify();
                        })),
                ),
        );
        if let Some(s) = &self.snapshot {
            match &self.inspector {
            Inspector::Entry(id)=>if let Some(e)=s.project.entries.iter().find(|e|e.proposal.id==*id){
                body=body.child(muted(format!("{:?} · {:?}",e.proposal.kind,e.proposal.claim_kind),cx)).child(document("entry-statement",e.proposal.statement.clone())).child(muted(format!("범위: {}\n도입 이유: {}\n채택: {:?}\n검토: {:?}",e.proposal.scope,e.proposal.rationale,e.adoption,e.review),cx));
                for d in &e.proposal.depends_on{let dep=d.clone();body=body.child(Button::new(SharedString::from(format!("dep-{d}"))).ghost().small().justify_start().label(format!("의존: {}",s.project.entries.iter().find(|e|e.proposal.id==*d).map(|e|short(&e.proposal.statement,30)).unwrap_or_else(||d.clone()))).on_click(cx.listener(move|this,_,_,cx|{this.inspector=Inspector::Entry(dep.clone());cx.notify();})));}
                let adopt=id.clone();let withdraw=id.clone();let target=id.clone();body=body.child(h_flex().gap_2().child(Button::new("adopt-entry").label("채택").disabled(self.busy()||e.adoption==Adoption::Withdrawn||e.adoption==Adoption::Adopted).on_click(cx.listener(move|this,_,_,cx|this.submit(Command::Adopt(adopt.clone()),cx)))).child(Button::new("withdraw-entry").label("철회").disabled(self.busy()||e.adoption==Adoption::Withdrawn).on_click(cx.listener(move|this,_,_,cx|this.submit(Command::Withdraw(withdraw.clone()),cx))))).child(Button::new("target-entry").label("이 항목에 후속 질문").on_click(cx.listener(move|this,_,w,cx|{this.targets=vec![target.clone()];this.page=Page::Inquiry;this.prompt.update(cx,|p,cx|{p.set_value("이 항목의 가정과 근거를 다시 검토해 줘.",w,cx);p.focus(w,cx);});cx.notify();})));
                for run in &s.project.runs{for a in &run.attempts{if let Some(o)=&a.output{for (ix,evidence) in o.evidence_refs.iter().filter(|ev|ev.claim_id==*id).enumerate(){let e=evidence.clone();body=body.child(document(SharedString::from(format!("quote-{}-{ix}",a.id)),format!("> {}\n\n{:?}: {}",e.quote,e.relation,e.reasoning))).child(Button::new(SharedString::from(format!("open-evidence-{}-{ix}",a.id))).small().label(format!("원문 행 {}–{} 열기",e.start_line,e.end_line)).on_click(cx.listener(move|this,_,_,cx|{this.inspector=Inspector::Source(e.source_id.clone(),Some(e.clone()));cx.notify();})));}for critique in o.critiques.iter().filter(|c|c.target_id==*id){body=body.child(muted(format!("미해결 비판 · {}\n{}\n제안: {}",critique.severity,critique.reasoning,critique.suggested_action),cx));}}}}
            },
            Inspector::Source(id,evidence)=>if let Some(source)=s.project.sources.iter().find(|s|s.id==*id){
                body=body.child(div().font_weight(FontWeight::SEMIBOLD).child(source.title.clone())).child(muted(format!("{} · r{}\n{}",access_label(source.access),source.revision,source.locator),cx));
                if let Some(e)=evidence{body=body.child(document("selected-evidence",format!("### 대조한 구절 · 행 {}–{}\n\n> {}\n\n{}\n\n문자열 대조 통과 · 의미 판정: {:?}",e.start_line,e.end_line,e.quote,e.reasoning,e.relation)));}
                if source.asset_hash.is_some() || theory_engine::search::safe_public_url(&source.locator){let id=id.clone();body=body.child(Button::new("open-original").label("원문 열기…").disabled(self.busy()).on_click(cx.listener(move|this,_,_,cx|this.submit(Command::OpenSource(id.clone()),cx))));}
                if source.text.is_empty(){body=body.child(muted("본문 미확보. 이 파일을 근거로 원문 대조를 수행하지 않습니다. 관련 구절을 자료 화면에 추가하세요.",cx));}else{body=body.child(document("source-full-text",format!("```text\n{}\n```",source.numbered())));}body=body.child(muted(format!("출처 ID: {}\nSHA-256: {}",source.id,source.hash),cx));
            },
            Inspector::Task(id) => if let Some(a) = s.project.runs.iter().flat_map(|r| r.attempts.iter()).find(|a| a.id == *id) {
                body = body.child(document("task-validation", format!("### 검증 기록\n\n{}\n\n{}", a.error.as_deref().unwrap_or(""), if a.validation_notes.is_empty() { "인용 위치 보정 없음".into() } else { a.validation_notes.join("\n\n") })));
                body = body.child(document("task-details", format!("### {}\n\n{} · {} · {:?}\n\n#### 실제 입력\n```json\n{}\n```\n\n#### 모델 원본 응답\n```json\n{}\n```\n\n#### 검증을 통과한 출력\n```json\n{}\n```\n\n#### 사용량\n```json\n{}\n```", a.task_id, a.provider, a.model, a.state, serde_json::to_string_pretty(&a.context).unwrap_or_default(), serde_json::to_string_pretty(&a.response).unwrap_or_default(), serde_json::to_string_pretty(&a.output).unwrap_or_default(), serde_json::to_string_pretty(&a.usage).unwrap_or_default())));
            },
            Inspector::None=>body=body.child(muted("답변의 원문 링크나 원장 항목을 선택하면 출처 구절, 접근 범위와 의존 가정을 볼 수 있습니다.",cx)),
        }
        }
        div()
            .size_full()
            .min_w_0()
            .overflow_y_scrollbar()
            .child(div().p_4().child(body))
            .into_any_element()
    }
}
fn access_label(access: Access) -> &'static str {
    match access {
        Access::ProvidedExcerpt => "제공된 구절",
        Access::ProvidedText => "제공된 텍스트",
        Access::AbstractOnly => "초록만 확보",
        Access::MetadataOnly => "본문 미확보",
    }
}
