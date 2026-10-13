use crate::{Budget, Plan, Role, hash, id, now};
use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

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
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Envelope {
    pub summary: String,
    pub proposed_changes: Vec<Proposal>,
    pub evidence_refs: Vec<Evidence>,
    pub critiques: Vec<Critique>,
    pub unresolved: Vec<String>,
    pub next_actions: Vec<String>,
    pub evidence_insufficient: bool,
}
impl Envelope {
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
            let s = sources
                .iter()
                .find(|s| s.id == e.source_id)
                .ok_or_else(|| anyhow::anyhow!("출처가 없습니다"))?;
            ensure!(
                s.access != Access::MetadataOnly,
                "메타데이터를 본문 근거로 사용할 수 없습니다"
            );
            ensure!(
                e.source_hash == s.hash && hash(s.text.as_bytes()) == s.hash,
                "출처 hash 불일치"
            );
            let lines: Vec<_> = s.text.lines().collect();
            ensure!(
                e.start_line > 0
                    && e.end_line >= e.start_line
                    && e.end_line as usize <= lines.len(),
                "인용 위치가 범위를 벗어났습니다"
            );
            let passage = lines[e.start_line as usize - 1..e.end_line as usize].join("\n");
            ensure!(
                !e.quote.trim().is_empty()
                    && passage
                        .match_indices(&e.quote)
                        .any(|(start, _)| quote_boundaries(&passage, start, start + e.quote.len())),
                "인용 구절이 지정된 원문 위치에 없습니다"
            );
        }
        for c in &self.critiques {
            ensure!(ids.contains(&c.target_id), "비판 대상이 없습니다");
            ensure!(
                ["low", "medium", "high"].contains(&c.severity.as_str()),
                "비판 심각도 오류"
            );
        }
        Ok(())
    }
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
