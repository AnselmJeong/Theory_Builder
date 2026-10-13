use super::*;
impl Workspace {
    pub(super) fn direction_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = v_flex().gap_5().child(page_header(
            "연구 방향",
            "프로젝트의 목적과 관점을 자유롭게 적으세요. 모든 질문의 계획과 비판에 참고되지만, 각 질문에서는 필요한 관점만 골라 씁니다.",
            cx,
        ));
        if let Some(s) = &self.snapshot {
            body = body.child(
                card(cx)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(Icon::new(Lucide::Folder).small())
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .child(
                                        s.project
                                            .workspace_folder
                                            .clone()
                                            .unwrap_or_else(|| "앱 내부 저장소".into()),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(
                                Button::new("direction-open-folder")
                                    .small()
                                    .label("다른 연구 폴더 열기…")
                                    .disabled(self.busy())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.choose_path("project", cx)
                                    })),
                            )
                            .when(s.project.workspace_folder.is_none(), |v| {
                                v.child(
                                    Button::new("share-current-project")
                                        .small()
                                        .ghost()
                                        .label("ScholarPen 폴더로 옮기기")
                                        .tooltip("기록을 ScholarPen 프로젝트 폴더로 옮깁니다. 원본은 백업으로 남습니다.")
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
        body.child(
            v_flex()
                .gap_2()
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("설명하려는 현상과 연구 목적"))
                .child(
                    Textarea::new(&self.research_goal)
                        .aria_label("연구 목적")
                        .accessibility_id("research-goal")
                        .readonly(self.busy()),
                ),
        )
        .child(
            v_flex()
                .gap_2()
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("검토 관점 · 범위 · 제약"))
                .child(
                    Textarea::new(&self.research_guidance)
                        .aria_label("연구 관점")
                        .accessibility_id("research-guidance")
                        .readonly(self.busy()),
                )
                .child(muted(
                    "예: 수용체 수준의 작용과 회로 수준의 결과를 구분한다. 종·표본·시간 척도를 명시하고, 대안 설명과 반례를 함께 검토한다.",
                    cx,
                )),
        )
        .child(
            h_flex().child(
                Button::new("save-research-direction")
                    .primary()
                    .small()
                    .label("연구 방향 저장")
                    .disabled(self.busy())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.submit(
                            Command::ResearchDirection {
                                goal: this.research_goal.read(cx).value().to_string(),
                                guidance: this.research_guidance.read(cx).value().to_string(),
                            },
                            cx,
                        )
                    })),
            ),
        )
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
        TaskState::Interrupted => "재시작으로 중단",
        TaskState::Blocked => "선행 작업 실패로 보류",
        _ => "대기",
    }
}
pub(super) fn friendly_error(error: &str) -> String {
    let lower = error.to_lowercase();
    if error.contains("invalid_json_schema") {
        "앱의 응답 형식 오류로 모델 요청이 거부됐습니다. 질문은 저장돼 있습니다. 업데이트 후 다시 실행하세요. 기술 상세는 요청·응답 기록에서 확인할 수 있습니다.".into()
    } else if lower.contains("serveroverloaded")
        || lower.contains("at capacity")
        || lower.contains("overloaded")
    {
        "모델 서버가 혼잡해 요청이 거절됐습니다. 자동 재시도 후에도 실패한 경우입니다. 완료된 분석은 저장했습니다. 잠시 후 다시 실행하거나 설정에서 다른 모델을 선택하세요.".into()
    } else if error == "budget_exhausted" {
        "호출 또는 시간 한도에 도달했습니다. 완료한 결과는 보존했습니다. 범위를 ‘깊게’로 바꾸거나 질문을 좁혀 다시 실행하세요.".into()
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
