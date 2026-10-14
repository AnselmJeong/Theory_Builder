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
    execution::{ContinueInput, RunInput, ask, resume, run, write_report},
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
    BrokenReport,
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
        let phase = if let Some(request) = c.get("request").and_then(|r| r.as_str()) {
            request.to_string()
        } else if c.get("final_theory").is_some() {
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
                let to_judge = c["critiques_to_judge"].as_array().unwrap();
                // Continued with a user direction and nothing left to judge: a new objection.
                if to_judge.is_empty() && c.get("user_direction").is_some() {
                    assert!(c["rules"].as_str().unwrap().contains("USER"));
                    o.critiques.push(record::<Critique>(|k| {
                        k.target_id = c["current_theory"][0]["id"].as_str().unwrap().into();
                        k.severity = "high".into();
                        k.reasoning =
                            "The direction asks for the time scale; it is missing.".into();
                    }));
                }
                for k in to_judge {
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
            "reader_report" => {
                assert!(c["rules"].as_str().unwrap().contains("SCRIBE"));
                // The scribe never sees engine IDs.
                let theory = c["working_theory"].to_string();
                assert!(!theory.contains("run-") && theory.contains("작업 이론 1"));
                assert!(!c["critiques"].to_string().contains("\"K1\""));
                if self.mode == Mode::BrokenReport {
                    let mut r = Response::default();
                    r.value = json!({"answer": 3});
                    return Ok(r);
                }
                let key = c["references"][0]["key"].as_str().unwrap().to_string();
                // Asked only while the source's metadata is just a title.
                if let Some(needed) = c["bib_needed"][0]["key"].as_str() {
                    assert_eq!(needed, key);
                }
                let leaked = c["working_theory"][0]["ref"].as_str().unwrap();
                let report = record::<ReaderReport>(|r| {
                    r.title = "Association, not causation".into();
                    r.answer = format!(
                        "The change goes with the transition, but no test shows it causes it [@{key}; @invented]. See K1 (K1)."
                    );
                    r.key_points.push(record::<KeyPoint>(|k| {
                        k.heading = "Observed together".into();
                        k.body = format!("{leaked} rests on one observation [@{key}].");
                        k.standing = Standing::Supported;
                        k.figure_ids = vec!["F1".into()];
                    }));
                    r.figures.push(record::<Figure>(|f| {
                        f.id = "F1".into();
                        f.title = "What was observed".into();
                        f.groups = vec!["Observation".into(), "Claim".into()];
                        f.nodes = vec![
                            record::<DiagramNode>(|n| {
                                n.id = "obs".into();
                                n.label = "Change observed".into();
                                n.group = "Observation".into();
                                n.note = "Reported in the fixture source.".into();
                            }),
                            record::<DiagramNode>(|n| {
                                n.id = "claim".into();
                                n.label = "Causes transition?".into();
                                n.group = "Claim".into();
                            }),
                        ];
                        f.edges = vec![record::<DiagramEdge>(|e| {
                            e.from = "obs".into();
                            e.to = "claim".into();
                            e.label = "untested".into();
                            e.dashed = true;
                        })];
                    }));
                    r.next_steps.push(record::<NextStep>(|n| {
                        n.kind = NextKind::Experiment;
                        n.action = "Manipulate the change and record the transition.".into();
                        n.purpose = "Separates cause from co-occurrence.".into();
                    }));
                    r.bibliography.push(record::<BibCandidate>(|b| {
                        b.key = key.clone();
                        b.title = "A change was observed".into();
                        b.year = "1999".into();
                    }));
                });
                let mut r = Response::default();
                r.value = serde_json::to_value(report)?;
                Ok(r)
            }
            "follow_up" => {
                assert_eq!(c["follow_up_question"], "Why is K1 settled?");
                let mut r = Response::default();
                r.value = serde_json::to_value(record::<FollowUpAnswer>(|a| {
                    a.answer = "Because K1 was narrowed [@ghost].".into();
                    a.needs_research = true;
                    a.suggested_direction = "Ask for a manipulation test.".into();
                }))?;
                Ok(r)
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
            // The writer (Claude unless chosen otherwise) writes the outcome.
            "Claude:synthesis",
            "Claude:reader_report"
        ]
    );
    assert_eq!(run.state, RunState::Answered);
    assert_eq!(run.deliberation.writer, "Claude");
    assert_eq!(run.reading.as_ref().unwrap().provider, "Claude");
    assert!(
        run.limitations
            .iter()
            .any(|l| l.starts_with("토론: GPT → Claude") && l.ends_with("정리 Claude"))
    );
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
    // The scribe's report: internal IDs and unknown citations removed, keys renamed to the
    // verified printed title and frozen on the source.
    let reading = run.reading.as_ref().unwrap();
    let key = store.project.sources[0].bib.as_ref().unwrap().key.clone();
    assert_eq!(key, "changeobserved");
    assert_eq!(
        reading.report.answer,
        "The change goes with the transition, but no test shows it causes it [@changeobserved]. See 비판 1."
    );
    assert!(
        reading.report.key_points[0]
            .body
            .starts_with("작업 이론 1 rests")
    );
    assert!(reading.notes.iter().any(|n| n.contains("@invented")));
    let bib = store.project.sources[0].bib.as_ref().unwrap();
    assert_eq!(
        (bib.title.as_str(), bib.year.as_str()),
        ("A change was observed", "")
    );
}
#[test]
fn report_export_is_reader_friendly_and_leaves_documents_out() {
    let (dir, mut store) = setup();
    let provider = Debate::new(Mode::Converge);
    execute(&mut store, &provider, Budget::Standard);
    let id = store.project.runs[0].id.clone();
    let folder = store
        .export_report(&dir.path().join("out"), ReportScope::Run(id))
        .unwrap();
    let read = |f: &str| std::fs::read_to_string(folder.join(f)).unwrap();
    let md = read("report.md");
    assert!(md.starts_with("---\ntitle: \"Association, not causation\""));
    assert!(md.contains("bibliography: references.bib"));
    assert!(md.contains("[@changeobserved]") && md.contains("::: {#refs}"));
    assert!(md.contains("![그림 1. What was observed](figures/q1-F1.svg)"));
    assert!(md.contains("**새 실험·측정**"));
    assert!(!md.contains("No causal test was performed") && !md.contains("run-"));
    let bib = read("references.bib");
    assert!(
        bib.contains("@misc{changeobserved,") && bib.contains("title = {A change was observed}")
    );
    let html = read("report.html");
    assert!(html.contains("href=\"#ref-changeobserved\"") && html.contains("class=\"node\""));
    assert!(!html.contains("No causal test was performed") && !html.contains("<script src"));
    assert!(read("figures/q1-F1.svg").starts_with("<svg"));
    // The exchange is kept, separately, as an appendix.
    let record = read("debate-record.md");
    assert!(record.contains("## 토론 기록") && record.contains("## 비판 원장"));
    assert!(record.contains("1차 GPT 수용") && record.contains("1차 Claude 해소됨"));
    assert!(record.contains("[@changeobserved] 행 1–1"));
    assert!(!record.contains("No causal test was performed"));
}
#[test]
fn continuing_a_converged_debate_follows_the_user_direction() {
    let (_dir, mut store) = setup();
    let provider = Debate::new(Mode::Converge);
    execute(&mut store, &provider, Budget::Standard);
    let id = store.project.runs[0].id.clone();
    let calls_before = store.project.runs[0].calls;
    resume(
        &mut store,
        &id,
        ContinueInput::new(2, "Discuss the time scale of the transition."),
        &cross(),
        &provider,
        &Arc::new(AtomicBool::new(false)),
        &mut |_, _| {},
    )
    .unwrap();
    let run = &store.project.runs[0];
    let tail: Vec<String> = provider.log()[7..]
        .iter()
        .map(|(p, s)| format!("{}:{s}", p.display()))
        .collect();
    assert_eq!(
        tail,
        [
            "Claude:review-r2",
            "GPT:revise-r2",
            "Claude:review-r2",
            "Claude:synthesis",
            "Claude:reader_report"
        ]
    );
    assert_eq!(run.state, RunState::Answered);
    assert_eq!(run.deliberation.rounds, 2);
    assert_eq!(run.deliberation.resumed_at, 1);
    assert_eq!(run.deliberation.stop, Some(StopReason::Converged));
    assert_eq!(
        run.directions[0].text,
        "Discuss the time scale of the transition."
    );
    assert_eq!(run.critiques.len(), 3);
    assert_eq!(run.critiques[2].status, CritiqueStatus::Addressed);
    assert_eq!(run.calls, calls_before + 5);
    assert_eq!(run.extra_calls, 2 * 2 + 3);
    assert_eq!(run.reading.as_ref().unwrap().round, 2);
    assert_eq!(
        run.limitations
            .iter()
            .filter(|l| l.starts_with("토론: "))
            .count(),
        1
    );
    // The revised claim joins the ledger as a new proposal.
    assert_eq!(store.project.entries.len(), 2);
}
#[test]
fn a_debate_started_long_ago_can_still_be_continued() {
    let (_dir, mut store) = setup();
    let provider = Debate::new(Mode::Stall);
    execute(&mut store, &provider, Budget::Standard);
    // As if the run had been made yesterday: time per call must come from this session.
    store.project.runs[0].started -= 86_400;
    let id = store.project.runs[0].id.clone();
    resume(
        &mut store,
        &id,
        ContinueInput::new(1, ""),
        &cross(),
        &provider,
        &Arc::new(AtomicBool::new(false)),
        &mut |_, _| {},
    )
    .unwrap();
    let run = &store.project.runs[0];
    // The twice-contested objection got another chance: one more revise + review.
    assert!(provider.log().iter().any(|(_, s)| s == "revise-r3"));
    assert_eq!(run.deliberation.rounds, 3);
    assert_ne!(run.deliberation.stop, Some(StopReason::Budget));
}
#[test]
fn continuing_without_new_objections_keeps_the_answer() {
    let (_dir, mut store) = setup();
    let provider = Debate::new(Mode::Converge);
    execute(&mut store, &provider, Budget::Standard);
    let id = store.project.runs[0].id.clone();
    let report = store.project.runs[0].report.clone();
    resume(
        &mut store,
        &id,
        ContinueInput::new(1, ""),
        &cross(),
        &provider,
        &Arc::new(AtomicBool::new(false)),
        &mut |_, _| {},
    )
    .unwrap();
    let run = &store.project.runs[0];
    assert_eq!(provider.log().len(), 8);
    assert_eq!(run.state, RunState::Answered);
    assert_eq!(run.report, report);
    assert!(run.limitations.iter().any(|l| l.contains("바뀌지 않아")));
}
#[test]
fn follow_up_answers_are_cleaned_and_can_suggest_more_debate() {
    let (_dir, mut store) = setup();
    let provider = Debate::new(Mode::Converge);
    execute(&mut store, &provider, Budget::Standard);
    let id = store.project.runs[0].id.clone();
    ask(
        &mut store,
        &id,
        "Why is K1 settled?",
        &cross(),
        &provider,
        &Arc::new(AtomicBool::new(false)),
        &mut |_, _| {},
    )
    .unwrap();
    let run = &store.project.runs[0];
    let f = &run.followups[0];
    assert_eq!(f.answer, "Because 비판 1 was narrowed.");
    assert!(f.needs_research && f.error.is_none());
    assert_eq!(f.provider, "Claude");
    assert_eq!(provider.log().last().unwrap().0, ProviderKind::Claude);
    assert_eq!(run.attempts.last().unwrap().phase, Some(Phase::Ask));
    assert_eq!(run.state, RunState::Answered);
}
#[test]
fn a_broken_report_keeps_the_answer_and_can_be_rewritten() {
    let (_dir, mut store) = setup();
    execute(
        &mut store,
        &Debate::new(Mode::BrokenReport),
        Budget::Standard,
    );
    let run = &store.project.runs[0];
    assert_eq!(run.state, RunState::Answered);
    assert!(run.reading.is_none());
    assert!(
        run.limitations
            .iter()
            .any(|l| l.starts_with("읽기 쉬운 보고서"))
    );
    let id = run.id.clone();
    write_report(
        &mut store,
        &id,
        &cross(),
        &Debate::new(Mode::Converge),
        &Arc::new(AtomicBool::new(false)),
        &mut |_, _| {},
    )
    .unwrap();
    let run = &store.project.runs[0];
    assert!(run.reading.is_some());
    assert!(
        !run.limitations
            .iter()
            .any(|l| l.starts_with("읽기 쉬운 보고서"))
    );
}

#[test]
fn the_writer_is_chosen_separately_and_can_rewrite_a_report() {
    let (_dir, mut store) = setup();
    let provider = Debate::new(Mode::Converge);
    let mut settings = cross();
    settings.writer = ProviderKind::Codex;
    run(
        &mut store,
        RunInput::new("Does the change cause the transition?", Budget::Standard),
        &settings,
        &provider,
        &Arc::new(AtomicBool::new(false)),
        &mut |_, _| {},
    )
    .unwrap();
    let log = provider.log();
    assert!(log.ends_with(&[
        (ProviderKind::Codex, "synthesis".into()),
        (ProviderKind::Codex, "reader_report".into())
    ]));
    // The same debate, rewritten for the reader by the other model.
    let id = store.project.runs[0].id.clone();
    settings.writer = ProviderKind::Claude;
    write_report(
        &mut store,
        &id,
        &settings,
        &provider,
        &Arc::new(AtomicBool::new(false)),
        &mut |_, _| {},
    )
    .unwrap();
    let run = &store.project.runs[0];
    assert_eq!(
        provider.log().last().unwrap(),
        &(ProviderKind::Claude, "reader_report".into())
    );
    assert_eq!(run.reading.as_ref().unwrap().provider, "Claude");
    assert_eq!(run.deliberation.writer, "Codex");
    assert!(theory_engine::report::how_made(run).contains("종합은 GPT가, 이 보고서는 Claude가"));
}
#[test]
fn settings_saved_before_the_writer_role_give_the_write_up_to_claude() {
    let old = r#"{"primary":"codex","cross_review":true,"codex_model":"","claude_model":"","codex_path":"","claude_path":"","dark":false,"zoom":0,"selected_project":""}"#;
    let mut s: Settings = serde_json::from_str(old).unwrap();
    assert_eq!(
        (s.author(), s.critic(), s.writer),
        (
            ProviderKind::Codex,
            ProviderKind::Claude,
            ProviderKind::Claude
        )
    );
    assert_eq!(s.roles(), [ProviderKind::Codex, ProviderKind::Claude]);
    // Changing the author keeps the chosen critic, which may make it a same-model critique.
    s.set_author(ProviderKind::Claude);
    assert_eq!(
        (s.author(), s.critic()),
        (ProviderKind::Claude, ProviderKind::Claude)
    );
    assert!(!s.cross_review);
    s.set_critic(ProviderKind::Codex);
    assert!(s.cross_review);
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
    assert_eq!(run.calls, 5);
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
    assert_eq!(run.calls, 8);
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
    // Claude was also the writer: the author writes the outcome instead, and says so.
    assert!(log.ends_with(&[
        (ProviderKind::Codex, "synthesis".into()),
        (ProviderKind::Codex, "reader_report".into())
    ]));
    assert_eq!(run.deliberation.writer, "Codex");
    assert!(
        run.limitations
            .iter()
            .any(|l| l.starts_with("정리 모델(Claude) 사용 불가로 GPT가"))
    );
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
    assert_eq!(run.calls, 5);
    let review = run.attempts.iter().find(|a| a.task_id == "review").unwrap();
    assert_eq!(review.state, TaskState::Blocked);
    assert!(review.error.as_deref().unwrap().contains("호출 예산"));
    assert_eq!(run.state, RunState::Partial);
    assert!(run.report.starts_with("Final answer"));
}
