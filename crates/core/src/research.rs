use crate::{Budget, Plan, Role, hash, id, now};
use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    #[default]
    ProvidedExcerpt,
    ProvidedText,
    AbstractOnly,
    MetadataOnly,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Source {
    pub id: String,
    pub title: String,
    pub locator: String,
    pub text: String,
    pub hash: String,
    pub access: Access,
    pub asset_hash: Option<String>,
    pub extension: String,
    pub revision: u32,
    #[serde(default)]
    pub relative_path: Option<String>,
    #[serde(default = "source_active")]
    pub active: bool,
    #[serde(default)]
    pub import_note: String,
    #[serde(default)]
    pub retrieved_at: Option<u64>,
    #[serde(default)]
    pub search_query: Option<String>,
    /// Bibliographic record for citations; derived on demand when absent.
    #[serde(default)]
    pub bib: Option<crate::BibInfo>,
}
fn source_active() -> bool {
    true
}
impl Source {
    pub fn new(title: String, text: String, locator: String, access: Access) -> Self {
        Self {
            id: id("S"),
            hash: hash(text.as_bytes()),
            title,
            text,
            locator,
            access,
            revision: 1,
            active: true,
            ..Self::default()
        }
    }
    pub fn numbered(&self) -> String {
        self.text
            .lines()
            .enumerate()
            .map(|(i, l)| format!("{}: {}", i + 1, l))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    #[default]
    Claim,
    Assumption,
    Term,
    Candidate,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    ReportedFinding,
    Observation,
    TheoreticalAssumption,
    Deduction,
    #[default]
    Speculation,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Adoption {
    #[default]
    Proposed,
    Adopted,
    Deferred,
    Withdrawn,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Review {
    #[default]
    Unreviewed,
    Reviewed,
    Contested,
    NeedsReview,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRelation {
    Supports,
    PartialSupport,
    Contradicts,
    Related,
    #[default]
    Indeterminate,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Proposal {
    pub id: String,
    pub kind: EntryKind,
    pub statement: String,
    pub claim_kind: ClaimKind,
    pub rationale: String,
    pub scope: String,
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub replaces_id: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Entry {
    pub proposal: Proposal,
    pub adoption: Adoption,
    pub review: Review,
    pub revision: u64,
    pub run_id: String,
    pub task_id: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Evidence {
    pub claim_id: String,
    pub source_id: String,
    pub source_hash: String,
    pub quote: String,
    pub start_line: u32,
    pub end_line: u32,
    pub relation: EvidenceRelation,
    pub reasoning: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Critique {
    pub target_id: String,
    #[schemars(with = "CritiqueSeverity")]
    pub severity: String,
    pub category: String,
    pub reasoning: String,
    pub suggested_action: String,
}
#[derive(JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
enum CritiqueSeverity {
    Low,
    Medium,
    High,
}
/// How the author answers a critique. Answering never closes it; only the critic's review can.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Stance {
    #[default]
    Accept,
    PartiallyAccept,
    Rebut,
}
impl Stance {
    pub fn label(self) -> &'static str {
        match self {
            Self::Accept => "수용",
            Self::PartiallyAccept => "부분 수용",
            Self::Rebut => "반박",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CritiqueStatus {
    #[default]
    Open,
    Addressed,
    Contested,
    Deferred,
    Withdrawn,
}
impl CritiqueStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "미해결",
            Self::Addressed => "해소됨",
            Self::Contested => "이견 유지",
            Self::Deferred => "보류",
            Self::Withdrawn => "비판 철회",
        }
    }
    pub fn is_settled(self) -> bool {
        matches!(self, Self::Addressed | Self::Withdrawn | Self::Deferred)
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct CritiqueResponse {
    pub critique_id: String,
    pub stance: Stance,
    pub reasoning: String,
    pub revised_ids: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct CritiqueReview {
    pub critique_id: String,
    pub status: CritiqueStatus,
    pub reasoning: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Envelope {
    pub summary: String,
    pub proposed_changes: Vec<Proposal>,
    pub evidence_refs: Vec<Evidence>,
    pub critiques: Vec<Critique>,
    #[serde(default)]
    pub critique_responses: Vec<CritiqueResponse>,
    #[serde(default)]
    pub critique_reviews: Vec<CritiqueReview>,
    pub unresolved: Vec<String>,
    pub next_actions: Vec<String>,
    pub evidence_insufficient: bool,
}
/// What an output may legitimately reference. Built by the engine from the exact request.
pub struct Allowed<'a> {
    pub sources: &'a [Source],
    /// Line ranges actually transmitted per source ID. Quotes outside them cannot be certified.
    pub ranges: &'a HashMap<String, (u32, u32)>,
    pub existing: &'a HashSet<String>,
    pub critiques: &'a HashSet<String>,
}
impl Envelope {
    /// Give every new proposal an ID inside the task namespace and rewrite references to it.
    /// Models are unreliable at prefixing IDs; the engine, not the model, owns identity.
    pub fn namespace_ids(&mut self, prefix: &str, existing: &HashSet<String>) -> Vec<String> {
        let mut renamed = HashMap::new();
        let mut used: HashSet<String> = existing.clone();
        for (n, p) in self.proposed_changes.iter_mut().enumerate() {
            if p.id.starts_with(prefix) && used.insert(p.id.clone()) {
                continue;
            }
            let stem: String =
                p.id.chars()
                    .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                    .take(24)
                    .collect();
            let mut candidate = if stem.is_empty() {
                format!("{prefix}_P{}", n + 1)
            } else {
                format!("{prefix}_{stem}")
            };
            let mut k = 2;
            while used.contains(&candidate) {
                candidate = format!("{prefix}_P{}_{k}", n + 1);
                k += 1;
            }
            used.insert(candidate.clone());
            if !p.id.is_empty() {
                renamed.insert(p.id.clone(), candidate.clone());
            }
            p.id = candidate;
        }
        if renamed.is_empty() {
            return vec![];
        }
        let map = |id: &mut String| {
            if let Some(new) = renamed.get(id.as_str()) {
                *id = new.clone();
            }
        };
        for p in &mut self.proposed_changes {
            p.depends_on.iter_mut().for_each(map);
        }
        for e in &mut self.evidence_refs {
            map(&mut e.claim_id);
        }
        for c in &mut self.critiques {
            map(&mut c.target_id);
        }
        for r in &mut self.critique_responses {
            r.revised_ids.iter_mut().for_each(map);
        }
        vec![format!(
            "제안 ID {}개를 작업 namespace로 재부여 (참조도 함께 갱신)",
            renamed.len()
        )]
    }
    /// Quarantine individual invalid items instead of discarding the whole output.
    /// Removed items are reported; nothing invalid is ever admitted to the ledger.
    pub fn sanitize(&mut self, allowed: &Allowed) -> Vec<String> {
        let mut notes = vec![];
        let mut ids = allowed.existing.clone();
        let before = self.proposed_changes.len();
        self.proposed_changes.retain(|p| {
            !p.statement.trim().is_empty() && !p.id.is_empty() && ids.insert(p.id.clone())
        });
        if self.proposed_changes.len() < before {
            notes.push(format!(
                "빈 내용 또는 중복 ID 제안 {}개 제외",
                before - self.proposed_changes.len()
            ));
        }
        for p in &mut self.proposed_changes {
            if let Some(r) = &p.replaces_id
                && (!allowed.existing.contains(r) || r == &p.id)
            {
                notes.push(format!(
                    "{}: 기준 원장에 없는 수정 대상 {r} 연결 해제",
                    p.id
                ));
                p.replaces_id = None;
            }
            let n = p.depends_on.len();
            let own = p.id.clone();
            p.depends_on.retain(|d| ids.contains(d) && *d != own);
            p.depends_on.dedup();
            if p.depends_on.len() < n {
                notes.push(format!(
                    "{}: 존재하지 않는 의존 참조 {}개 제거",
                    p.id,
                    n - p.depends_on.len()
                ));
            }
        }
        // Break dependency cycles among new proposals by dropping the edges inside the cycle.
        let mut resolved = allowed.existing.clone();
        loop {
            let before = resolved.len();
            for p in &self.proposed_changes {
                if p.depends_on.iter().all(|d| resolved.contains(d)) {
                    resolved.insert(p.id.clone());
                }
            }
            if resolved.len() == ids.len() {
                break;
            }
            if resolved.len() == before {
                for p in &mut self.proposed_changes {
                    if !resolved.contains(&p.id) {
                        p.depends_on.retain(|d| resolved.contains(d));
                        notes.push(format!("{}: 순환 의존 관계 제거", p.id));
                    }
                }
            }
        }
        self.evidence_refs.retain(|e| {
            let problem = if !ids.contains(&e.claim_id) {
                Some("존재하지 않는 주장 ID".to_string())
            } else {
                evidence_problem(e, allowed.sources).or_else(|| {
                    match allowed.ranges.get(&e.source_id) {
                        Some((first, last)) if e.start_line >= *first && e.end_line <= *last => {
                            None
                        }
                        _ => Some("요청에 전송하지 않은 원문 범위".to_string()),
                    }
                })
            };
            if let Some(problem) = &problem {
                notes.push(format!(
                    "인용 제외 · {} → {} 행 {}–{}: {problem}",
                    e.claim_id, e.source_id, e.start_line, e.end_line
                ));
            }
            problem.is_none()
        });
        for c in &mut self.critiques {
            c.severity = c.severity.trim().to_lowercase();
            if !["low", "medium", "high"].contains(&c.severity.as_str()) {
                notes.push(format!("비판 심각도 '{}'를 medium으로 처리", c.severity));
                c.severity = "medium".into();
            }
            if !c.target_id.is_empty() && !ids.contains(&c.target_id) {
                notes.push(format!(
                    "비판 대상 {}를 찾지 못해 전체 초안에 대한 비판으로 기록",
                    c.target_id
                ));
                c.target_id.clear();
            }
        }
        self.critiques.retain(|c| !c.reasoning.trim().is_empty());
        let n = self.critique_responses.len() + self.critique_reviews.len();
        self.critique_responses
            .retain(|r| allowed.critiques.contains(&r.critique_id));
        self.critique_reviews
            .retain(|r| allowed.critiques.contains(&r.critique_id));
        for r in &mut self.critique_responses {
            r.revised_ids.retain(|id| ids.contains(id));
        }
        let kept = self.critique_responses.len() + self.critique_reviews.len();
        if kept < n {
            notes.push(format!(
                "존재하지 않는 비판 ID에 대한 응답·판정 {}개 제외",
                n - kept
            ));
        }
        notes
    }
    pub fn validate(&self, sources: &[Source], existing: &HashSet<String>) -> Result<()> {
        ensure!(!self.summary.trim().is_empty(), "답변 요약 누락");
        let mut ids = existing.clone();
        for p in &self.proposed_changes {
            ensure!(
                !p.id.is_empty() && ids.insert(p.id.clone()),
                "중복 또는 빈 제안 ID"
            );
            ensure!(!p.statement.trim().is_empty(), "빈 주장");
        }
        for p in &self.proposed_changes {
            if let Some(id) = &p.replaces_id {
                ensure!(existing.contains(id), "수정 대상이 기준 원장에 없습니다");
            }
            ensure!(
                p.depends_on.iter().all(|d| ids.contains(d) && d != &p.id),
                "없는 의존 항목 또는 자기 참조"
            );
        }
        // Cycles among new propositions are not an argument.
        let mut resolved = existing.clone();
        loop {
            let before = resolved.len();
            for p in &self.proposed_changes {
                if p.depends_on.iter().all(|d| resolved.contains(d)) {
                    resolved.insert(p.id.clone());
                }
            }
            if resolved.len() == ids.len() {
                break;
            }
            ensure!(resolved.len() > before, "제안의 의존 관계가 순환합니다");
        }
        for e in &self.evidence_refs {
            ensure!(
                ids.contains(&e.claim_id),
                "근거가 없는 주장 ID를 가리킵니다"
            );
            if let Some(problem) = evidence_problem(e, sources) {
                anyhow::bail!(problem);
            }
        }
        for c in &self.critiques {
            ensure!(
                c.target_id.is_empty() || ids.contains(&c.target_id),
                "비판 대상이 없습니다"
            );
            ensure!(
                ["low", "medium", "high"].contains(&c.severity.as_str()),
                "비판 심각도 오류"
            );
        }
        Ok(())
    }
}
/// Exact-text check of one citation against the stored source revision.
pub fn evidence_problem(e: &Evidence, sources: &[Source]) -> Option<String> {
    let Some(s) = sources.iter().find(|s| s.id == e.source_id) else {
        return Some("출처가 없습니다".into());
    };
    if s.access == Access::MetadataOnly {
        return Some("메타데이터를 본문 근거로 사용할 수 없습니다".into());
    }
    if e.source_hash != s.hash || hash(s.text.as_bytes()) != s.hash {
        return Some("출처 hash 불일치".into());
    }
    let lines: Vec<_> = s.text.lines().collect();
    if e.start_line == 0 || e.end_line < e.start_line || e.end_line as usize > lines.len() {
        return Some("인용 위치가 범위를 벗어났습니다".into());
    }
    let passage = lines[e.start_line as usize - 1..e.end_line as usize].join("\n");
    let found = !e.quote.trim().is_empty()
        && passage
            .match_indices(&e.quote)
            .any(|(start, _)| quote_boundaries(&passage, start, start + e.quote.len()));
    (!found).then(|| "인용 구절이 지정된 원문 위치에 없습니다".into())
}
/// Prevent a literal substring quote from changing a word by cutting off its prefix/suffix.
pub fn quote_boundaries(text: &str, start: usize, end: usize) -> bool {
    if start >= end
        || end > text.len()
        || !text.is_char_boundary(start)
        || !text.is_char_boundary(end)
    {
        return false;
    }
    let word = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '\u{2010}' | '\u{2011}');
    !(text[..start].chars().next_back().is_some_and(word)
        && text[start..end].chars().next().is_some_and(word)
        || text[end..].chars().next().is_some_and(word)
            && text[start..end].chars().next_back().is_some_and(word))
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    #[default]
    Running,
    Answered,
    EvidenceInsufficient,
    Partial,
    Stalled,
    BudgetExhausted,
    AwaitingDecision,
    Cancelled,
    Failed,
    Interrupted,
}
impl RunState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "실행 중",
            Self::Answered => "답변 작성됨",
            Self::EvidenceInsufficient => "근거 부족",
            Self::Partial => "일부 작업 실패 · 부분 답변",
            Self::Stalled => "진전 없음",
            Self::BudgetExhausted => "한도 도달",
            Self::AwaitingDecision => "결정 대기",
            Self::Cancelled => "중단됨",
            Self::Failed => "실행 실패",
            Self::Interrupted => "재시작으로 중단 확인",
        }
    }
}
/// Where an attempt sits in the author ↔ critic exchange.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Plan,
    #[default]
    Construct,
    Review,
    Revise,
    Synthesis,
    /// The scribe rewrites the outcome for the reader.
    Report,
    /// A follow-up question about a finished report.
    Ask,
}
impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Plan => "계획",
            Self::Construct => "초안",
            Self::Review => "비판",
            Self::Revise => "응답·수정",
            Self::Synthesis => "종합",
            Self::Report => "보고서 작성",
            Self::Ask => "후속 질문",
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// No open medium/high critique remains in the reviewed scope.
    Converged,
    RoundLimit,
    Stalled,
    Budget,
    Failed,
    Cancelled,
}
impl StopReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::Converged => "검토 범위의 주요 비판 해소",
            Self::RoundLimit => "토론 회차 한도 도달",
            Self::Stalled => "진전 없음으로 토론 종료",
            Self::Budget => "호출 예산 부족으로 토론 종료",
            Self::Failed => "응답·검토 실패로 토론 종료",
            Self::Cancelled => "중단됨",
        }
    }
}
/// The recursive exchange between an author model and a critic model.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Deliberation {
    pub author: String,
    pub critic: String,
    pub max_rounds: u32,
    pub rounds: u32,
    pub stop: Option<StopReason>,
    /// Round count when the user last continued the debate; dissent counted from here.
    #[serde(default)]
    pub resumed_at: u32,
    #[serde(default)]
    pub continuations: u32,
}
impl Deliberation {
    pub fn cross_model(&self) -> bool {
        !self.critic.is_empty() && self.author != self.critic
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CritiqueEvent {
    pub round: u32,
    pub provider: String,
    pub stance: Option<Stance>,
    pub status: Option<CritiqueStatus>,
    pub reasoning: String,
    #[serde(default)]
    pub revised_ids: Vec<String>,
}
/// A critique with an engine-assigned identity and its full response/review history.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CritiqueRecord {
    pub id: String,
    pub round: u32,
    pub task_id: String,
    pub provider: String,
    pub critique: Critique,
    pub status: CritiqueStatus,
    pub events: Vec<CritiqueEvent>,
}
impl CritiqueRecord {
    pub fn material(&self) -> bool {
        matches!(self.critique.severity.as_str(), "medium" | "high")
    }
    pub fn times_contested(&self) -> usize {
        self.times_contested_after(0)
    }
    pub fn times_contested_after(&self, round: u32) -> usize {
        self.events
            .iter()
            .filter(|e| e.round > round && e.status == Some(CritiqueStatus::Contested))
            .count()
    }
    /// Still worth another revision round: open, or contested once (one more chance).
    pub fn actionable(&self) -> bool {
        self.actionable_since(0)
    }
    /// After the user continues a debate, earlier dissent gets another chance.
    pub fn actionable_since(&self, resumed_at: u32) -> bool {
        self.material()
            && match self.status {
                CritiqueStatus::Open => true,
                CritiqueStatus::Contested => self.times_contested_after(resumed_at) < 2,
                _ => false,
            }
    }
    /// "비판 3" — what readers see instead of the engine ID.
    pub fn label(&self) -> String {
        critique_label(&self.id)
    }
}
/// "비판 3" for an engine critique ID ("K3").
pub fn critique_label(id: &str) -> String {
    match id.strip_prefix('K') {
        Some(n) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => format!("비판 {n}"),
        _ => id.to_string(),
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    #[default]
    Queued,
    Running,
    Validating,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
    Blocked,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Attempt {
    pub id: String,
    pub task_id: String,
    pub role: Option<Role>,
    pub state: TaskState,
    pub provider: String,
    pub model: String,
    pub context: Value,
    pub response: Option<Value>,
    pub output: Option<Envelope>,
    pub error: Option<String>,
    #[serde(default)]
    pub validation_notes: Vec<String>,
    pub usage: Option<Value>,
    pub started: u64,
    #[serde(default)]
    pub phase: Option<Phase>,
    #[serde(default)]
    pub round: u32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Run {
    pub id: String,
    pub question: String,
    pub targets: Vec<String>,
    pub base_revision: u64,
    pub revision: u64,
    pub budget: Budget,
    pub calls: u32,
    pub state: RunState,
    pub plan: Option<Plan>,
    pub attempts: Vec<Attempt>,
    pub report: String,
    pub started: u64,
    pub finished: Option<u64>,
    pub profile_versions: Vec<String>,
    pub limitations: Vec<String>,
    #[serde(default)]
    pub deliberation: Deliberation,
    #[serde(default)]
    pub critiques: Vec<CritiqueRecord>,
    /// Calls granted after the original budget (continued debate, report, follow-ups).
    #[serde(default)]
    pub extra_calls: u32,
    /// Directions the user added when continuing the debate.
    #[serde(default)]
    pub directions: Vec<crate::Direction>,
    /// The reader-facing report written by the scribe.
    #[serde(default)]
    pub reading: Option<crate::Reading>,
    #[serde(default)]
    pub followups: Vec<crate::FollowUp>,
}
impl Run {
    pub fn call_limit(&self) -> u32 {
        self.budget.calls() + self.extra_calls
    }
    /// Reader numbering of the working theory ("작업 이론 3"), shared by every view.
    pub fn theory_labels(&self) -> Vec<(String, String)> {
        self.theory()
            .into_iter()
            .enumerate()
            .map(|(i, p)| (p.id, format!("작업 이론 {}", i + 1)))
            .collect()
    }
    /// Latest version of every proposal made in this run: revisions supersede their drafts.
    pub fn theory(&self) -> Vec<Proposal> {
        let all: Vec<&Proposal> = self
            .attempts
            .iter()
            .filter(|a| a.state == TaskState::Succeeded)
            .filter_map(|a| a.output.as_ref())
            .flat_map(|o| o.proposed_changes.iter())
            .collect();
        let local: HashSet<&str> = all.iter().map(|p| p.id.as_str()).collect();
        let mut next: HashMap<&str, &str> = HashMap::new();
        for p in &all {
            if let Some(r) = p.replaces_id.as_deref()
                && local.contains(r)
            {
                next.insert(r, p.id.as_str());
            }
        }
        fn head<'a>(next: &HashMap<&'a str, &'a str>, mut id: &'a str, limit: usize) -> String {
            let mut guard = 0;
            while let Some(n) = next.get(id) {
                id = n;
                guard += 1;
                if guard > limit {
                    break;
                }
            }
            id.to_string()
        }
        let origin = |p: &Proposal| {
            let mut r = p.replaces_id.clone();
            let mut guard = 0;
            while let Some(id) = r.as_deref() {
                if !local.contains(id) || guard > all.len() {
                    break;
                }
                r = all
                    .iter()
                    .find(|q| q.id == id)
                    .and_then(|q| q.replaces_id.clone());
                guard += 1;
            }
            r
        };
        all.iter()
            .filter(|p| !next.contains_key(p.id.as_str()))
            .map(|p| {
                let mut q = (*p).clone();
                let mut deps = vec![];
                for d in &p.depends_on {
                    let h = if local.contains(d.as_str()) {
                        head(&next, d, all.len())
                    } else {
                        d.clone()
                    };
                    if h != q.id && !deps.contains(&h) {
                        deps.push(h);
                    }
                }
                q.depends_on = deps;
                q.replaces_id = origin(p);
                q
            })
            .collect()
    }
    pub fn final_output(&self) -> Option<&Envelope> {
        self.attempts
            .iter()
            .rev()
            .find(|a| a.phase == Some(Phase::Synthesis) && a.state == TaskState::Succeeded)
            .or_else(|| {
                self.attempts
                    .iter()
                    .rev()
                    .find(|a| a.role == Some(Role::Synthesizer) && a.state == TaskState::Succeeded)
            })
            .and_then(|a| a.output.as_ref())
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Revision {
    pub number: u64,
    pub parent: u64,
    pub reason: String,
    pub entries: Vec<Entry>,
    pub timestamp: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Project {
    pub id: String,
    pub title: String,
    pub revision: u64,
    pub profiles: Vec<String>,
    #[serde(default)]
    pub workspace_folder: Option<String>,
    #[serde(default)]
    pub research_goal: String,
    #[serde(default)]
    pub research_guidance: Option<String>,
    pub draft: String,
    pub sources: Vec<Source>,
    pub entries: Vec<Entry>,
    pub runs: Vec<Run>,
    pub history: Vec<Revision>,
}
impl Project {
    pub fn new(title: &str) -> Self {
        Self {
            id: id("project"),
            title: title.into(),
            revision: 0,
            profiles: vec!["general-research".into()],
            workspace_folder: None,
            research_goal: String::new(),
            research_guidance: Some(String::new()),
            draft: String::new(),
            sources: vec![],
            entries: vec![],
            runs: vec![],
            history: vec![],
        }
    }
    pub fn guidance(&self) -> String {
        self.research_guidance.clone().unwrap_or_else(|| {
            self.profiles
                .iter()
                .filter_map(|p| match p.as_str() {
                    "neurobiology" => Some("종, 표본, 측정법, 시간 척도와 관찰 수준을 구분한다."),
                    "multilevel-dynamics" => {
                        Some("여러 수준의 연결 기제와 시간 척도, 동역학적 가정을 검토한다.")
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
    }
    pub fn selection_key(&self) -> String {
        self.workspace_folder
            .as_ref()
            .map(|p| format!("folder:{p}"))
            .unwrap_or_else(|| self.id.clone())
    }
    pub fn checkpoint(&mut self, reason: &str) {
        let parent = self.revision;
        self.revision += 1;
        self.history.push(Revision {
            number: self.revision,
            parent,
            reason: reason.into(),
            entries: self.entries.clone(),
            timestamp: now(),
        });
    }
    pub fn withdraw(&mut self, id: &str) -> Result<Vec<String>> {
        ensure!(
            self.entries.iter().any(|e| e.proposal.id == id),
            "철회할 항목을 찾지 못했습니다"
        );
        let mut affected = HashSet::from([id.to_string()]);
        loop {
            let n = affected.len();
            for e in &self.entries {
                if e.proposal.depends_on.iter().any(|d| affected.contains(d)) {
                    affected.insert(e.proposal.id.clone());
                }
            }
            if n == affected.len() {
                break;
            }
        }
        for e in &mut self.entries {
            if e.proposal.id == id {
                e.adoption = Adoption::Withdrawn;
            }
            if affected.contains(&e.proposal.id) {
                e.review = Review::NeedsReview;
            }
        }
        self.checkpoint(&format!("{id} 철회 — 관련 항목 재검토"));
        Ok(affected.into_iter().collect())
    }
    pub fn recover(&mut self) -> bool {
        let mut changed = false;
        for r in &mut self.runs {
            if r.state == RunState::Running {
                r.state = RunState::Interrupted;
                r.finished = Some(now());
                r.limitations.push(
                    "이전 호출의 원격 완료 여부는 확인할 수 없습니다. 자동 재전송하지 않았습니다."
                        .into(),
                );
                changed = true;
                for a in &mut r.attempts {
                    if matches!(a.state, TaskState::Running | TaskState::Validating) {
                        a.state = TaskState::Interrupted;
                    }
                }
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_must_target_an_existing_entry() {
        let mut output = Envelope {
            summary: "Revision proposal".into(),
            proposed_changes: vec![Proposal {
                id: "new".into(),
                statement: "Revised scope".into(),
                replaces_id: Some("old".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(output.validate(&[], &HashSet::new()).is_err());
        assert!(output.validate(&[], &HashSet::from(["old".into()])).is_ok());
        output.proposed_changes[0].replaces_id = Some("new".into());
        assert!(
            output
                .validate(&[], &HashSet::from(["old".into()]))
                .is_err()
        );
    }
    #[test]
    fn evidence_rejects_fabrication_and_wrong_location() {
        let s = Source::new(
            "Paper".into(),
            "One\nTwo\nThree".into(),
            "Methods".into(),
            Access::ProvidedExcerpt,
        );
        let p = Proposal {
            id: "C1".into(),
            statement: "Claim".into(),
            ..Default::default()
        };
        let e = Evidence {
            claim_id: "C1".into(),
            source_id: s.id.clone(),
            source_hash: s.hash.clone(),
            quote: "Two".into(),
            start_line: 2,
            end_line: 2,
            ..Default::default()
        };
        let mut result = Envelope {
            summary: "Answer".into(),
            proposed_changes: vec![p],
            evidence_refs: vec![e],
            ..Default::default()
        };
        assert!(
            result
                .validate(std::slice::from_ref(&s), &HashSet::new())
                .is_ok()
        );
        result.evidence_refs[0].start_line = 1;
        result.evidence_refs[0].end_line = 1;
        assert!(
            result
                .validate(std::slice::from_ref(&s), &HashSet::new())
                .is_err()
        );
        result.evidence_refs[0].quote = "Made up".into();
        assert!(result.validate(&[s], &HashSet::new()).is_err());
    }
    #[test]
    fn sanitize_quarantines_items_and_keeps_valid_work() {
        let s = Source::new(
            "Paper".into(),
            "One\nTwo\nThree".into(),
            "Methods".into(),
            Access::ProvidedText,
        );
        let mut out = Envelope {
            summary: "Draft".into(),
            proposed_changes: vec![
                Proposal {
                    id: "A".into(),
                    statement: "Claim A".into(),
                    depends_on: vec!["B".into(), "ghost".into()],
                    replaces_id: Some("missing".into()),
                    ..Default::default()
                },
                Proposal {
                    id: "B".into(),
                    statement: "Claim B".into(),
                    depends_on: vec!["A".into()],
                    ..Default::default()
                },
            ],
            evidence_refs: vec![
                Evidence {
                    claim_id: "A".into(),
                    source_id: s.id.clone(),
                    source_hash: s.hash.clone(),
                    quote: "Two".into(),
                    start_line: 2,
                    end_line: 2,
                    ..Default::default()
                },
                Evidence {
                    claim_id: "B".into(),
                    source_id: s.id.clone(),
                    source_hash: s.hash.clone(),
                    quote: "Invented".into(),
                    start_line: 1,
                    end_line: 1,
                    ..Default::default()
                },
                Evidence {
                    claim_id: "B".into(),
                    source_id: s.id.clone(),
                    source_hash: s.hash.clone(),
                    quote: "Three".into(),
                    start_line: 3,
                    end_line: 3,
                    ..Default::default()
                },
            ],
            critiques: vec![Critique {
                target_id: "nowhere".into(),
                severity: "HIGH".into(),
                reasoning: "Gap".into(),
                ..Default::default()
            }],
            critique_reviews: vec![CritiqueReview {
                critique_id: "K9".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let ranges = HashMap::from([(s.id.clone(), (1, 2))]);
        let notes = out.sanitize(&Allowed {
            sources: std::slice::from_ref(&s),
            ranges: &ranges,
            existing: &HashSet::new(),
            critiques: &HashSet::new(),
        });
        assert!(notes.len() >= 5, "{notes:?}");
        assert_eq!(out.proposed_changes.len(), 2);
        assert_eq!(out.proposed_changes[0].replaces_id, None);
        // Exact quote kept; invented and untransmitted (line 3) quotes removed.
        assert_eq!(out.evidence_refs.len(), 1);
        assert_eq!(out.evidence_refs[0].quote, "Two");
        assert_eq!(out.critiques[0].severity, "high");
        assert!(out.critiques[0].target_id.is_empty());
        assert!(out.critique_reviews.is_empty());
        assert!(out.validate(&[s], &HashSet::new()).is_ok());
    }
    #[test]
    fn namespace_ids_rewrites_references() {
        let mut out = Envelope {
            summary: "x".into(),
            proposed_changes: vec![
                Proposal {
                    id: "C1".into(),
                    statement: "a".into(),
                    ..Default::default()
                },
                Proposal {
                    id: "run_t_ok".into(),
                    statement: "b".into(),
                    depends_on: vec!["C1".into(), "LEDGER".into()],
                    ..Default::default()
                },
            ],
            evidence_refs: vec![Evidence {
                claim_id: "C1".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        out.namespace_ids("run_t", &HashSet::from(["LEDGER".to_string()]));
        assert_eq!(out.proposed_changes[0].id, "run_t_C1");
        assert_eq!(out.proposed_changes[1].id, "run_t_ok");
        assert_eq!(out.proposed_changes[1].depends_on, ["run_t_C1", "LEDGER"]);
        assert_eq!(out.evidence_refs[0].claim_id, "run_t_C1");
    }
    #[test]
    fn run_theory_keeps_revision_heads_only() {
        let mut run = Run::default();
        let attempt = |task: &str, ps: Vec<Proposal>| Attempt {
            task_id: task.into(),
            state: TaskState::Succeeded,
            output: Some(Envelope {
                summary: "s".into(),
                proposed_changes: ps,
                ..Default::default()
            }),
            ..Default::default()
        };
        let p = |id: &str, deps: &[&str], replaces: Option<&str>| Proposal {
            id: id.into(),
            statement: id.into(),
            depends_on: deps.iter().map(|d| d.to_string()).collect(),
            replaces_id: replaces.map(String::from),
            ..Default::default()
        };
        run.attempts.push(attempt(
            "draft",
            vec![p("a1", &[], Some("LEDGER")), p("b1", &["a1"], None)],
        ));
        run.attempts
            .push(attempt("r1", vec![p("a2", &[], Some("a1"))]));
        run.attempts
            .push(attempt("r2", vec![p("a3", &["b1"], Some("a2"))]));
        let ids: Vec<_> = run
            .theory()
            .into_iter()
            .map(|p| (p.id, p.depends_on, p.replaces_id))
            .collect();
        assert_eq!(
            ids,
            [
                ("b1".to_string(), vec!["a3".to_string()], None),
                (
                    "a3".to_string(),
                    vec!["b1".to_string()],
                    Some("LEDGER".to_string())
                ),
            ]
        );
    }
    #[test]
    fn withdrawal_propagates_without_rewriting_history() {
        let mut p = Project::new("Test");
        for (id, deps) in [("A1", vec![]), ("C1", vec!["A1".into()]), ("C2", vec![])] {
            p.entries.push(Entry {
                proposal: Proposal {
                    id: id.into(),
                    depends_on: deps,
                    ..Default::default()
                },
                ..Default::default()
            });
        }
        p.checkpoint("base");
        p.withdraw("A1").unwrap();
        assert_eq!(p.entries[1].review, Review::NeedsReview);
        assert_eq!(p.entries[2].review, Review::Unreviewed);
        assert_eq!(p.history[0].entries[0].adoption, Adoption::Proposed);
    }
}
