use super::*;
impl Workspace {
    pub(super) fn activity_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.busy() && self.page != Page::Inquiry {
            return div().into_any_element();
        }
        let active = self.snapshot.as_ref().and_then(|s| {
            s.project
                .runs
                .iter()
                .rev()
                .find(|r| r.state == RunState::Running)
        });
        let run = active.or_else(|| self.current_run());
        if !self.busy()
            && !run.is_some_and(|r| {
                matches!(
                    r.state,
                    RunState::Failed
                        | RunState::Cancelled
                        | RunState::BudgetExhausted
                        | RunState::Partial
                        | RunState::Interrupted
                )
            })
        {
            return div().into_any_element();
        }
        let mut panel = v_flex()
            .gap_2()
            .px_4()
            .py_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted);
        let mut actions = h_flex().gap_2();
        if self.busy() {
            let detail = if let Some(r) = active {
                let completed = r
                    .attempts
                    .iter()
                    .filter(|a| a.state == TaskState::Succeeded && a.role.is_some())
                    .count();
                let all = r.plan.as_ref().map(|p| p.tasks.len()).unwrap_or(0);
                let seconds = now().saturating_sub(r.started);
                let current = r
                    .attempts
                    .iter()
                    .rev()
                    .find(|a| matches!(a.state, TaskState::Running | TaskState::Validating));
                let phase = if self.status.starts_with("문헌 검색") {
                    Some("문헌 검색")
                } else {
                    None
                };
                let completion = if all == 0 {
                    "작업 계획 준비 중".into()
                } else {
                    format!("작업 {completed}/{all} 완료")
                };
                format!(
                    "{} · {}분 {}초 경과 · {} · 호출 {}/{}",
                    phase.unwrap_or_else(|| current
                        .and_then(|a| a.role)
                        .map(|r| r.label())
                        .unwrap_or("계획 구성")),
                    seconds / 60,
                    seconds % 60,
                    completion,
                    r.calls,
                    r.budget.calls()
                )
            } else {
                "준비 중 · 연결 또는 자료를 확인하고 있습니다".into()
            };
            panel = panel
                .child(div().font_weight(FontWeight::SEMIBOLD).child(detail))
                .child(muted(self.status.clone(), cx));
            if let Some(r) = active
                && let Some(plan) = &r.plan
            {
                let steps = plan
                    .tasks
                    .iter()
                    .map(|t| {
                        let state = r
                            .attempts
                            .iter()
                            .rev()
                            .find(|a| a.task_id == t.id)
                            .map(|a| task_state(a.state))
                            .unwrap_or("대기");
                        format!("{}: {state}", t.kind.label())
                    })
                    .collect::<Vec<_>>()
                    .join("  →  ");
                panel = panel.child(muted(steps, cx));
            }
            actions = actions.child(Button::new("inline-cancel").label("실행 중단").on_click(
                cx.listener(|this, _, _, cx| {
                    this.service.cancel();
                    this.status = "중단 요청됨 · 완료한 기록 보존 중".into();
                    cx.notify();
                }),
            ));
        } else if let Some(r) = run {
            let reason = r
                .limitations
                .last()
                .map(|s| friendly_error(s))
                .unwrap_or_else(|| r.state.label().into());
            panel = panel
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(r.state.label()),
                )
                .child(muted(reason, cx));
            let question = r.question.clone();
            let targets = r.targets.clone();
            actions = actions.child(
                Button::new("retry-question")
                    .label("이 질문 다시 실행")
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.prompt
                            .update(cx, |p, cx| p.set_value(question.clone(), w, cx));
                        this.targets = targets.clone();
                        this.start_run(cx);
                    })),
            );
        }
        actions = actions.child(
            Button::new("inline-progress-details")
                .ghost()
                .label(if self.show_tasks {
                    "상세 기록 닫기"
                } else {
                    "진행 상황 보기"
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.show_tasks = !this.show_tasks;
                    this.page = Page::Inquiry;
                    cx.notify();
                })),
        );
        panel.child(actions).into_any_element()
    }
    pub(super) fn direction_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body=v_flex().gap_4().child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("연구 방향"))
            .child(muted("프로젝트의 목적과 관점을 자유롭게 작성하세요. 각 질문에서는 필요한 관점을 골라 검토하며, 특정 분야로 고정하지 않습니다.",cx));
        if let Some(s) = &self.snapshot {
            body = body.child(
                v_flex()
                    .gap_2()
                    .child("프로젝트 폴더")
                    .child(muted(
                        s.project
                            .workspace_folder
                            .clone()
                            .unwrap_or_else(|| "이전 버전의 앱 내부 저장소".into()),
                        cx,
                    ))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("direction-open-folder")
                                    .label("프로젝트 폴더 열기…")
                                    .disabled(self.busy())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.choose_path("project", cx)
                                    })),
                            )
                            .when(s.project.workspace_folder.is_none(), |v| {
                                v.child(
                                    Button::new("share-current-project")
                                        .label("이 연구를 ScholarPen 폴더로 저장")
                                        .disabled(self.busy())
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.service.cancel.store(false, Ordering::SeqCst);
                                            this.submit(Command::ShareProject, cx);
                                        })),
                                )
                            }),
                    ),
            );
        }
        body.child(v_flex().gap_2().child("설명하려는 현상과 연구 목적").child(Textarea::new(&self.research_goal).aria_label("연구 목적").accessibility_id("research-goal").readonly(self.busy())))
            .child(v_flex().gap_2().child("검토 관점 · 범위 · 제약").child(Textarea::new(&self.research_guidance).aria_label("연구 관점").accessibility_id("research-guidance").readonly(self.busy())))
            .child(muted("예: 수용체 수준의 작용과 회로 수준의 결과를 구분한다. 약물 간 작용 차이를 비교하고 대안 설명과 반례를 함께 검토한다.",cx))
            .child(h_flex().child(Button::new("save-research-direction").primary().label("연구 방향 저장").disabled(self.busy()).on_click(cx.listener(|this,_,_,cx|this.submit(Command::ResearchDirection{goal:this.research_goal.read(cx).value().to_string(),guidance:this.research_guidance.read(cx).value().to_string()},cx)))))
            .into_any_element()
    }
}
pub(super) fn task_state(state: TaskState) -> &'static str {
    match state {
        TaskState::Running => "응답 대기 중",
        TaskState::Validating => "응답 검증 중",
        TaskState::Succeeded => "완료",
        TaskState::Failed => "실패",
        TaskState::Cancelled => "중단",
        TaskState::Interrupted => "중단됨 · 재시작 복구",
        TaskState::Blocked => "선행 작업 실패로 보류",
        _ => "대기",
    }
}
pub(super) fn friendly_error(error: &str) -> String {
    if error.contains("invalid_json_schema") {
        "앱의 응답 형식 오류로 모델 요청이 거부됐습니다. 질문은 저장돼 있습니다. 업데이트 후 다시 실행하세요. 기술 상세는 요청·응답 기록에서 확인할 수 있습니다.".into()
    } else if error.contains("serverOverloaded") || error.contains("at capacity") {
        "선택한 모델의 서버가 혼잡해 실행이 멈췄습니다. 완료된 분석은 저장했습니다. 잠시 후 다시 실행하거나 설정에서 다른 모델을 선택하세요.".into()
    } else if error.contains("cancelled") || error.contains("취소 뒤") {
        "실행을 중단했습니다. 완료한 기록과 질문은 보존했습니다.".into()
    } else if error.chars().count() > 500 {
        format!(
            "{}… 상세 기록을 확인하세요.",
            error.chars().take(400).collect::<String>()
        )
    } else {
        error.into()
    }
}
