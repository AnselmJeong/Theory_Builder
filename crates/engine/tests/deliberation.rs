//! The recursive author ↔ critic exchange, driven by a deterministic fixture provider.
//! Fixture outputs are synthetic and say nothing about neuroscience.
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use theory_core::*;
use theory_engine::{
    execution::{RunInput, run},
    providers::*,
    storage::*,
};

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Converge,
    Stall,
    OverloadOnce,
    OverloadDraftOnce,
    CriticAuth,
    AuthorReviews,
}
struct Debate {
    mode: Mode,
    calls: Mutex<Vec<(ProviderKind, String)>>,
    overloaded: AtomicBool,
}
impl Debate {
    fn new(mode: Mode) -> Self {
        Self {
            mode,
            calls: Mutex::new(vec![]),
            overloaded: AtomicBool::new(false),
        }
    }
    fn log(&self) -> Vec<(ProviderKind, String)> {
        self.calls.lock().unwrap().clone()
    }
}
fn envelope(summary: &str) -> Envelope {
    record::<Envelope>(|o| o.summary = summary.into())
}
fn ok(v: Envelope) -> Result<Response> {
    let mut r = Response::default();
    r.value = serde_json::to_value(v)?;
    r.model = "fixture-only".into();
    Ok(r)
}
impl ModelProvider for Debate {
    fn generate(
        &self,
        provider: ProviderKind,
        r: &Request,
        _: &Cancel,
        _: &mut dyn FnMut(String),
    ) -> Result<Response> {
        let c: Value = serde_json::from_str(&r.prompt)?;
        let phase = if c.get("final_theory").is_some() {
            "synthesis".to_string()
        } else if let Some(d) = c.get("deliberation").filter(|d| d.get("phase").is_some()) {
            format!("{}-r{}", d["phase"].as_str().unwrap(), d["round"])
        } else if let Some(t) = c.get("task") {
            t["id"].as_str().unwrap().to_string()
        } else {
            "planner".into()
        };
        self.calls.lock().unwrap().push((provider, phase.clone()));
        if phase == "planner" {
            let sources = c["sources"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["id"].as_str().unwrap().into())
                .collect();
            let mut r = Response::default();
            r.value =
                serde_json::to_value(Plan::fallback(c["question"].as_str().unwrap(), sources))?;
            return Ok(r);
        }
        if self.mode == Mode::CriticAuth && provider == ProviderKind::Claude {
            bail!("Claude Code 구독 로그인이 필요합니다. API 키로 전환하지 않았습니다.");
        }
        let overloaded_phase = match self.mode {
            Mode::OverloadOnce => "review",
            Mode::OverloadDraftOnce => "analysis",
            _ => "",
        };
        if phase == overloaded_phase && !self.overloaded.swap(true, Ordering::SeqCst) {
            bail!("Codex 작업 미완료: serverOverloaded Selected model is at capacity");
        }
        let prefix = c["output_id_prefix"].as_str().unwrap_or("");
        match phase.as_str() {
            "analysis" => {
                let mut o = envelope("Initial draft with one mechanism claim.");
                let source = &c["sources"][0];
                o.proposed_changes.push(record::<Proposal>(|p| {
                    // Deliberately outside the namespace: the engine must re-assign it.
                    p.id = "C1".into();
                    p.statement = "Synthetic change causes the state transition.".into();
                    p.claim_kind = ClaimKind::TheoreticalAssumption;
                }));
                o.evidence_refs.push(record::<Evidence>(|e| {
                    e.claim_id = "C1".into();
                    e.source_id = source["id"].as_str().unwrap().into();
                    e.source_hash = source["hash"].as_str().unwrap().into();
                    e.quote = "A change was observed.".into();
                    e.start_line = 1;
                    e.end_line = 1;
                    e.relation = EvidenceRelation::PartialSupport;
                }));
                if self.mode == Mode::AuthorReviews {
                    o.critique_reviews.push(record::<CritiqueReview>(|r| {
                        r.critique_id = "K1".into();
                        r.status = CritiqueStatus::Addressed;
                    }));
                }
                ok(o)
            }
            "review" => {
                let target = c["dependency_outputs"][0]["output"]["proposed_changes"][0]["id"]
                    .as_str()
                    .unwrap()
                    .to_string();
                assert!(target.starts_with(prefix.split('_').next().unwrap()));
                let mut o = envelope("The causal step is not supported by the quoted observation.");
                o.critiques.push(record::<Critique>(|k| {
                    k.target_id = target.clone();
                    k.severity = "high".into();
                    k.category = "causal_gap".into();
                    k.reasoning = "An observed change does not show causation.".into();
                    k.suggested_action = "Restrict the claim to association.".into();
                }));
                o.critiques.push(record::<Critique>(|k| {
                    k.target_id = target;
                    k.severity = "low".into();
                    k.reasoning = "Wording could be tighter.".into();
                }));
                ok(o)
            }
            p if p.starts_with("revise") => {
                let theory = c["current_theory"].as_array().unwrap();
                let current = theory[0]["id"].as_str().unwrap().to_string();
                let to_answer = c["critiques_to_answer"].as_array().unwrap();
                assert!(!to_answer.is_empty());
                // The critic's identity is never disclosed to the author.
                assert!(!r.prompt.contains("Claude") && !r.prompt.contains("Codex"));
                let mut o = envelope("Revision");
                for k in to_answer {
                    o.critique_responses
                        .push(record::<CritiqueResponse>(|resp| {
                            resp.critique_id = k["id"].as_str().unwrap().into();
                            resp.stance = if self.mode == Mode::Stall {
                                Stance::Rebut
                            } else {
                                Stance::Accept
                            };
                            resp.reasoning = "Response".into();
                            if self.mode != Mode::Stall {
                                resp.revised_ids = vec![format!("{prefix}_C1b")];
                            }
                        }));
                }
                if self.mode != Mode::Stall {
                    o.proposed_changes.push(record::<Proposal>(|p| {
                        p.id = format!("{prefix}_C1b");
                        p.statement =
                            "The synthetic change is associated with the transition.".into();
                        p.claim_kind = ClaimKind::ReportedFinding;
                        p.replaces_id = Some(current.clone());
                    }));
                }
                // An author may not judge critiques; the engine must ignore this.
                o.critique_reviews.push(record::<CritiqueReview>(|r| {
                    r.critique_id = "K1".into();
                    r.status = CritiqueStatus::Withdrawn;
                }));
                ok(o)
            }
            p if p.starts_with("review-r") => {
                let mut o = envelope("Re-review");
                for k in c["critiques_to_judge"].as_array().unwrap() {
                    o.critique_reviews.push(record::<CritiqueReview>(|r| {
                        r.critique_id = k["id"].as_str().unwrap().into();
                        r.status = match (self.mode, k["severity"].as_str()) {
                            (Mode::Stall, Some("high")) => CritiqueStatus::Contested,
                            (_, Some("low")) => CritiqueStatus::Deferred,
                            _ => CritiqueStatus::Addressed,
                        };
                        r.reasoning = "Judgement".into();
                    }));
                }
                ok(o)
            }
            "synthesis" => {
                assert!(c["critique_ledger"].as_array().is_some());
                ok(envelope(
                    "Final answer: association only; causation untested.",
                ))
            }
            other => bail!("unexpected fixture phase {other}"),
        }
    }
}
fn setup() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::create(dir.path(), "Debate").unwrap();
    store.project.sources.push(Source::new(
        "Synthetic fixture".into(),
        "A change was observed.\nNo causal test was performed.".into(),
        "fixture lines".into(),
        Access::ProvidedText,
    ));
    store.save("fixture").unwrap();
    (dir, store)
}
fn cross() -> Settings {
    let mut s = Settings::default();
    s.primary = ProviderKind::Codex;
    s.cross_review = true;
    s.search_enabled = false;
    s
}
fn execute(store: &mut Store, provider: &Debate, budget: Budget) {
    run(
        store,
        RunInput::new("Does the change cause the transition?", budget),
        &cross(),
        provider,
        &Arc::new(AtomicBool::new(false)),
        &mut |_, _| {},
    )
    .unwrap();
}

#[test]
fn author_and_critic_alternate_until_material_critiques_are_settled() {
    let (_dir, mut store) = setup();
    let provider = Debate::new(Mode::Converge);
    execute(&mut store, &provider, Budget::Standard);
    let run = &store.project.runs[0];
    let phases: Vec<_> = provider.log();
    assert_eq!(
        phases
            .iter()
            .map(|(p, s)| format!("{}:{s}", p.display()))
            .collect::<Vec<_>>(),
        [
            "GPT:planner",
            "GPT:analysis",
            "Claude:review",
            "GPT:revise-r1",
            "Claude:review-r1",
            "GPT:synthesis"
        ]
    );
    assert_eq!(run.state, RunState::Answered);
    assert_eq!(run.deliberation.rounds, 1);
    assert_eq!(run.deliberation.stop, Some(StopReason::Converged));
    assert!(run.deliberation.cross_model());
    let k1 = &run.critiques[0];
    assert_eq!(
        (k1.id.as_str(), k1.status),
        ("K1", CritiqueStatus::Addressed)
    );
    // Response from the author, then the critic's judgement — the author's own
    // attempt to withdraw the critique was ignored.
    assert_eq!(k1.events.len(), 2);
    assert_eq!(k1.events[0].stance, Some(Stance::Accept));
    assert_eq!(k1.events[1].status, Some(CritiqueStatus::Addressed));
    assert_eq!(run.critiques[1].status, CritiqueStatus::Deferred);
    // Only the revised head enters the ledger; the superseded draft stays in the audit trail.
    assert_eq!(store.project.entries.len(), 1);
    let entry = &store.project.entries[0];
    assert!(entry.proposal.statement.contains("associated"));
    assert_eq!(entry.proposal.replaces_id, None);
    assert_eq!(entry.review, Review::Reviewed);
    assert_eq!(entry.adoption, Adoption::Proposed);
    // The model's out-of-namespace ID was re-assigned together with its evidence reference.
    let analysis = run
        .attempts
        .iter()
        .find(|a| a.task_id == "analysis")
        .unwrap();
    let out = analysis.output.as_ref().unwrap();
    assert_ne!(out.proposed_changes[0].id, "C1");
    assert_eq!(out.evidence_refs[0].claim_id, out.proposed_changes[0].id);
    assert!(run.report.starts_with("Final answer"));
    // The exported report keeps the whole exchange, not just the conclusion.
    let md = markdown(&store.project);
    assert!(md.contains("### 토론 기록") && md.contains("### 비판 원장"));
    assert!(md.contains("K1") && md.contains("1차 GPT 수용") && md.contains("1차 Claude 해소됨"));
}

#[test]
fn persistent_dissent_stops_and_is_preserved_not_resolved() {
    let (_dir, mut store) = setup();
    let provider = Debate::new(Mode::Stall);
    execute(&mut store, &provider, Budget::Standard);
    let run = &store.project.runs[0];
    assert_eq!(run.deliberation.rounds, 2);
    assert_eq!(run.deliberation.stop, Some(StopReason::Stalled));
    assert_eq!(run.critiques[0].status, CritiqueStatus::Contested);
    assert_eq!(run.critiques[0].times_contested(), 2);
    assert_eq!(run.state, RunState::Answered);
    assert_eq!(store.project.entries[0].review, Review::Contested);
    assert!(run.limitations.iter().any(|l| l.contains("토론")));
}

#[test]
fn quick_budget_reviews_once_without_revision() {
    let (_dir, mut store) = setup();
    let provider = Debate::new(Mode::Converge);
    execute(&mut store, &provider, Budget::Quick);
    let run = &store.project.runs[0];
    assert_eq!(run.calls, 4);
    assert_eq!(run.deliberation.rounds, 0);
    assert_eq!(run.deliberation.stop, Some(StopReason::RoundLimit));
    assert_eq!(run.critiques[0].status, CritiqueStatus::Open);
    assert_eq!(store.project.entries[0].review, Review::Contested);
}

#[test]
fn overloaded_server_is_retried_within_budget() {
    let (_dir, mut store) = setup();
    let provider = Debate::new(Mode::OverloadOnce);
    execute(&mut store, &provider, Budget::Standard);
    let run = &store.project.runs[0];
    let reviews: Vec<_> = run
        .attempts
        .iter()
        .filter(|a| a.task_id == "review")
        .map(|a| a.state)
        .collect();
    assert_eq!(reviews, [TaskState::Failed, TaskState::Succeeded]);
    assert_eq!(run.state, RunState::Answered);
    assert_eq!(run.calls, 7);
}

#[test]
fn unavailable_critic_stops_debate_without_silent_fallback() {
    let (_dir, mut store) = setup();
    let provider = Debate::new(Mode::CriticAuth);
    execute(&mut store, &provider, Budget::Standard);
    let run = &store.project.runs[0];
    // Exactly one failed critic request; no Claude call is replaced by GPT.
    let log = provider.log();
    assert_eq!(
        log.iter()
            .filter(|(p, _)| *p == ProviderKind::Claude)
            .count(),
        1
    );
    assert!(
        log.iter()
            .all(|(p, s)| *p == ProviderKind::Claude || !s.starts_with("review"))
    );
    assert_eq!(run.deliberation.stop, Some(StopReason::Failed));
    assert_eq!(run.state, RunState::Partial);
    assert!(run.limitations.iter().any(|l| l.contains("비판 모델")));
    assert!(!run.report.is_empty());
}

#[test]
fn authors_cannot_judge_critiques() {
    let (_dir, mut store) = setup();
    let provider = Debate::new(Mode::AuthorReviews);
    execute(&mut store, &provider, Budget::Standard);
    let analysis = store.project.runs[0]
        .attempts
        .iter()
        .find(|a| a.task_id == "analysis")
        .unwrap();
    assert!(
        analysis
            .output
            .as_ref()
            .unwrap()
            .critique_reviews
            .is_empty()
    );
    assert!(
        json!(analysis.validation_notes)
            .to_string()
            .contains("비판 판정")
    );
}

#[test]
fn a_retry_that_uses_up_calls_skips_a_step_but_still_answers() {
    let (_dir, mut store) = setup();
    let provider = Debate::new(Mode::OverloadDraftOnce);
    execute(&mut store, &provider, Budget::Quick);
    let run = &store.project.runs[0];
    assert_eq!(run.calls, 4);
    let review = run.attempts.iter().find(|a| a.task_id == "review").unwrap();
    assert_eq!(review.state, TaskState::Blocked);
    assert!(review.error.as_deref().unwrap().contains("호출 예산"));
    assert_eq!(run.state, RunState::Partial);
    assert!(run.report.starts_with("Final answer"));
}
