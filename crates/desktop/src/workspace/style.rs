//! Small presentation vocabulary shared by every page: colors, labels, chips, cards.
//! Spacing and type use the theme's rem scale; colors come from the theme or the
//! component palette so light and dark modes stay consistent.
use super::*;
use gpui_kit::component::{
    amber_500, emerald_400, emerald_600, orange_400, orange_600, sky_500, theme::ActiveTheme as _,
};

/// Each model keeps one identity color everywhere: Claude warm, GPT green.
pub(super) fn provider_color(label: &str, cx: &App) -> Hsla {
    let dark = cx.theme().is_dark();
    match (who(label), dark) {
        ("Claude", false) => orange_600(),
        ("Claude", true) => orange_400(),
        (_, false) => emerald_600(),
        (_, true) => emerald_400(),
    }
}
/// Researcher-facing model family for a stored provider label.
pub(super) fn who(label: &str) -> &'static str {
    ProviderKind::from_label(label).display()
}
pub(super) fn avatar(label: &str, cx: &App) -> Div {
    let color = provider_color(label, cx);
    div()
        .flex_none()
        .size(rems(1.5))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(color.opacity(0.16))
        .text_color(color)
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .child(if who(label) == "Claude" { "C" } else { "G" })
}
/// A compact label. Color carries meaning but the text always states it too.
pub(super) fn chip(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_none()
        .px(rems(0.4))
        .py(rems(0.05))
        .rounded(rems(0.25))
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .bg(color.opacity(0.12))
        .text_color(color)
        .child(text.into())
}
pub(super) fn neutral_chip(text: impl Into<SharedString>, cx: &App) -> Div {
    chip(text, cx.theme().muted_foreground)
}
pub(super) fn card(cx: &App) -> Div {
    v_flex()
        .gap_2()
        .p_4()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
}
pub(super) fn section_title(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .pt_2()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}
pub(super) fn page_header(title: &str, description: &str, cx: &App) -> Div {
    v_flex()
        .gap_1()
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_string()),
        )
        .child(muted(description.to_string(), cx))
}
pub(super) fn dot(color: Hsla) -> Div {
    div().flex_none().size(rems(0.5)).rounded_full().bg(color)
}
pub(super) fn state_color(state: RunState, cx: &App) -> Hsla {
    let t = cx.theme();
    match state {
        RunState::Answered => t.success,
        RunState::EvidenceInsufficient | RunState::Partial | RunState::BudgetExhausted => {
            amber_500()
        }
        RunState::Running => sky_500(),
        RunState::Failed => t.danger,
        _ => t.muted_foreground,
    }
}
pub(super) fn severity_color(severity: &str, cx: &App) -> Hsla {
    match severity {
        "high" => cx.theme().danger,
        "medium" => amber_500(),
        _ => cx.theme().muted_foreground,
    }
}
pub(super) fn severity_label(severity: &str) -> &'static str {
    match severity {
        "high" => "중대",
        "medium" => "중간",
        _ => "경미",
    }
}
pub(super) fn status_color(status: CritiqueStatus, cx: &App) -> Hsla {
    match status {
        CritiqueStatus::Open => amber_500(),
        CritiqueStatus::Contested => cx.theme().danger,
        CritiqueStatus::Addressed | CritiqueStatus::Withdrawn => cx.theme().success,
        CritiqueStatus::Deferred => cx.theme().muted_foreground,
    }
}
pub(super) fn stance_color(stance: Stance, cx: &App) -> Hsla {
    match stance {
        Stance::Accept => cx.theme().success,
        Stance::PartiallyAccept => amber_500(),
        Stance::Rebut => sky_500(),
    }
}
pub(super) fn claim_kind_label(kind: ClaimKind) -> &'static str {
    match kind {
        ClaimKind::ReportedFinding => "보고된 결과",
        ClaimKind::Observation => "관찰",
        ClaimKind::TheoreticalAssumption => "이론적 가정",
        ClaimKind::Deduction => "도출",
        ClaimKind::Speculation => "추측",
    }
}
pub(super) fn claim_kind_color(kind: ClaimKind, cx: &App) -> Hsla {
    match kind {
        ClaimKind::ReportedFinding | ClaimKind::Observation => sky_500(),
        ClaimKind::Deduction => cx.theme().foreground.opacity(0.7),
        ClaimKind::TheoreticalAssumption => amber_500(),
        ClaimKind::Speculation => cx.theme().muted_foreground,
    }
}
pub(super) fn entry_kind_label(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Claim => "주장",
        EntryKind::Assumption => "가정",
        EntryKind::Term => "용어",
        EntryKind::Candidate => "후보",
    }
}
pub(super) fn adoption_label(a: Adoption) -> &'static str {
    match a {
        Adoption::Proposed => "제안",
        Adoption::Adopted => "채택",
        Adoption::Deferred => "보류",
        Adoption::Withdrawn => "철회",
    }
}
pub(super) fn review_label(r: Review) -> &'static str {
    match r {
        Review::Unreviewed => "미검토",
        Review::Reviewed => "비판 검토 통과",
        Review::Contested => "이견 있음",
        Review::NeedsReview => "재검토 필요",
    }
}
pub(super) fn review_color(r: Review, cx: &App) -> Hsla {
    match r {
        Review::Reviewed => cx.theme().success,
        Review::Contested => cx.theme().danger,
        Review::NeedsReview => amber_500(),
        Review::Unreviewed => cx.theme().muted_foreground,
    }
}
pub(super) fn relation_label(r: EvidenceRelation) -> &'static str {
    match r {
        EvidenceRelation::Supports => "지지",
        EvidenceRelation::PartialSupport => "부분 지지",
        EvidenceRelation::Contradicts => "반박",
        EvidenceRelation::Related => "관련",
        EvidenceRelation::Indeterminate => "판단 불가",
    }
}
pub(super) fn relation_color(r: EvidenceRelation, cx: &App) -> Hsla {
    match r {
        EvidenceRelation::Supports => cx.theme().success,
        EvidenceRelation::PartialSupport => amber_500(),
        EvidenceRelation::Contradicts => cx.theme().danger,
        _ => cx.theme().muted_foreground,
    }
}
pub(super) fn access_label(access: Access) -> &'static str {
    match access {
        Access::ProvidedExcerpt => "제공된 구절",
        Access::ProvidedText => "본문",
        Access::AbstractOnly => "초록만",
        Access::MetadataOnly => "서지정보만",
    }
}
pub(super) fn access_color(access: Access, cx: &App) -> Hsla {
    match access {
        Access::ProvidedText | Access::ProvidedExcerpt => sky_500(),
        Access::AbstractOnly => amber_500(),
        Access::MetadataOnly => cx.theme().muted_foreground,
    }
}
pub(super) fn phase_of(a: &Attempt) -> Phase {
    a.phase.unwrap_or(match a.role {
        None => Phase::Plan,
        Some(Role::Critic | Role::EvidenceReviewer) => Phase::Review,
        Some(Role::Synthesizer) => Phase::Synthesis,
        Some(_) => Phase::Construct,
    })
}
/// "Claude · 1차 응답·수정" style step titles.
pub(super) fn step_title(a: &Attempt) -> String {
    let what = match (phase_of(a), a.round) {
        (Phase::Plan, _) => "연구 계획".to_string(),
        (Phase::Construct, _) => a.role.map(|r| r.label()).unwrap_or("분석").to_string(),
        (Phase::Review, 0) => a.role.map(|r| r.label()).unwrap_or("비판 검토").to_string(),
        (Phase::Review, r) => format!("{r}차 재검토"),
        (Phase::Revise, r) => format!("{r}차 응답·수정"),
        (Phase::Synthesis, _) => "답변 종합".into(),
    };
    format!("{} · {what}", who(&a.provider))
}
pub(super) fn duration_label(seconds: u64) -> String {
    if seconds < 60 {
        format!("{seconds}초")
    } else {
        format!("{}분 {}초", seconds / 60, seconds % 60)
    }
}
/// A left-aligned clickable row (component buttons always center their content).
pub(super) fn row_button(
    id: impl Into<ElementId>,
    icon: Lucide,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &App,
) -> Stateful<Div> {
    h_flex()
        .id(id)
        .w_full()
        .gap_2()
        .items_center()
        .px_2()
        .py_1p5()
        .rounded(cx.theme().radius)
        .text_sm()
        .cursor_pointer()
        .when(selected, |v| {
            v.bg(cx.theme().sidebar_accent)
                .text_color(cx.theme().sidebar_accent_foreground)
                .font_weight(FontWeight::MEDIUM)
        })
        .hover(|s| s.bg(cx.theme().sidebar_accent))
        .child(
            Icon::new(icon)
                .small()
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(label.into()),
        )
}
pub(super) fn lines_label(start: u32, end: u32) -> String {
    if start == end {
        format!("행 {start}")
    } else {
        format!("행 {start}–{end}")
    }
}
/// Register a custom clickable element for UI queries (a no-op outside test builds).
pub(super) fn observed(element: Stateful<Div>) -> AnyElement {
    element.test_support().into_any_element()
}
