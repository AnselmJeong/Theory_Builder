use super::{Inspector, RunTab, Workspace};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Entity, TestAppContext, WindowHandle, component::Root, px, size};
use std::sync::{Arc, atomic::AtomicBool};
use theory_core::*;
use theory_engine::{execution, providers::*, storage::Store};

fn open(cx: &mut TestAppContext, dir: &std::path::Path) -> (WindowHandle<Root>, Entity<Workspace>) {
    open_sized(cx, dir, 900.)
}
fn open_sized(
    cx: &mut TestAppContext,
    dir: &std::path::Path,
    height: f32,
) -> (WindowHandle<Root>, Entity<Workspace>) {
    cx.update(gpui_kit::init);
    let mut entity = None;
    let path = dir.to_path_buf();
    let window = cx.open_window(size(px(1360.), px(height)), |window, cx| {
        let view = cx.new(|cx| Workspace::with_directory(path, window, cx));
        entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = entity.unwrap();
    settle(cx, window, &view, |v| v.snapshot.is_some());
    (window, view)
}
fn settle(
    cx: &mut TestAppContext,
    window: WindowHandle<Root>,
    view: &Entity<Workspace>,
    done: impl Fn(&Workspace) -> bool,
) {
    // Up to 10 s for the service thread (SQLite with full sync) under a parallel test run;
    // returns as soon as `done` holds.
    for _ in 0..1000 {
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.update_window(window.into(), |_, w, cx| {
            view.update(cx, |v, cx| v.poll(w, cx))
        })
        .unwrap();
        if cx.update(|cx| done(view.read(cx))) {
            break;
        }
    }
}

#[gpui_kit::test]
fn project_dialog_examples_inspector_and_draft_survive_switching(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (window, view) = open(cx, dir.path());
    cx.update_window(window.into(), |_, w, cx| {
        w.render_frame(cx);
        w.click("project-switcher", cx);
    })
    .unwrap();
    // Let the dialog's open animation settle: clicks during it land on stale bounds.
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(600));
    cx.run_until_parked();
    cx.update_window(window.into(), |_, w, cx| {
        w.render_frame(cx);
        assert!(
            w.try_find("project-title").is_some(),
            "project dialog opens"
        );
        w.click("project-title", cx);
        w.input("시냅스 가소성 연구", cx);
        w.click("create-project", cx);
        assert_eq!(view.read(cx).pending, 1, "create submits one command");
    })
    .unwrap();
    settle(cx, window, &view, |v| {
        v.pending == 0
            && v.snapshot
                .as_ref()
                .is_some_and(|s| s.project.title == "시냅스 가소성 연구")
    });
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).snapshot.as_ref().unwrap().project.title,
            "시냅스 가소성 연구"
        )
    });
    let first_id = cx.update(|cx| view.read(cx).snapshot.as_ref().unwrap().project.id.clone());
    cx.update_window(window.into(), |_, w, cx| {
        w.render_frame(cx);
        w.click("example-concept", cx);
        assert!(
            view.read(cx)
                .prompt
                .read(cx)
                .value()
                .contains("Metastability")
        );
        view.update(cx, |v, cx| {
            v.prompt
                .update(cx, |s, cx| s.set_value("세포와 회로를 구분해 줘", w, cx));
            v.open_inspector(Inspector::None, cx);
        });
        w.render_frame(cx);
        assert!(view.read(cx).show_inspector);
        w.click("close-inspector", cx);
        assert!(!view.read(cx).show_inspector);
    })
    .unwrap();
    // Switch immediately, before the autosave debounce. The original question must survive.
    cx.update_window(window.into(), |_, _, cx| {
        view.update(cx, |v, cx| {
            v.submit(
                theory_engine::service::Command::NewProject("Another project".into()),
                cx,
            )
        });
    })
    .unwrap();
    settle(cx, window, &view, |v| v.pending == 0);
    let saved = theory_engine::storage::Store::open(dir.path(), &first_id).unwrap();
    assert!(saved.project.draft.contains("세포와 회로"));
}

/// Real keystrokes, including Korean text; a plain Enter is a newline, never a model call.
#[gpui_kit::test]
fn korean_typing_and_plain_enter_never_submit(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (window, view) = open(cx, dir.path());
    cx.update_window(window.into(), |_, w, cx| {
        w.render_frame(cx);
        let prompt_id = view.read(cx).prompt.entity_id();
        w.click(("input", prompt_id), cx);
        w.input("세포와 회로를 구분해 줘", cx);
        w.press("enter", cx);
        assert!(
            view.read(cx)
                .prompt
                .read(cx)
                .value()
                .contains("세포와 회로")
        );
        assert!(
            view.read(cx)
                .snapshot
                .as_ref()
                .unwrap()
                .project
                .runs
                .is_empty(),
            "ordinary Enter must not submit a model call"
        );
    })
    .unwrap();
}

/// Scripted provider: one draft, one material critique, one revision, one judgement.
struct Script;
impl ModelProvider for Script {
    fn generate(
        &self,
        _: ProviderKind,
        r: &Request,
        _: &Cancel,
        _: &mut dyn FnMut(String),
    ) -> anyhow::Result<Response> {
        let c: serde_json::Value = serde_json::from_str(&r.prompt)?;
        let mut o = record::<Envelope>(|o| o.summary = "fixture step".into());
        let value = if c.get("request").is_some() {
            serde_json::to_value(record::<ReaderReport>(|r| {
                r.title = "Plain answer".into();
                r.answer = "The revised claim holds within its scope.".into();
                r.key_points.push(record::<KeyPoint>(|k| {
                    k.heading = "What changed".into();
                    k.body = "The causal step was narrowed.".into();
                    k.standing = Standing::Reasoned;
                    k.figure_ids = vec!["F1".into()];
                }));
                r.figures.push(record::<Figure>(|f| {
                    f.id = "F1".into();
                    f.title = "Two levels".into();
                    f.groups = vec!["Cells".into(), "Behaviour".into()];
                    f.nodes = ["a", "b"]
                        .iter()
                        .zip(["Cells", "Behaviour"])
                        .map(|(id, g)| {
                            record::<DiagramNode>(|n| {
                                n.id = id.to_string();
                                n.label = format!("node {id}");
                                n.group = g.into();
                                n.note = "note".into();
                            })
                        })
                        .collect();
                    f.edges = vec![record::<DiagramEdge>(|e| {
                        e.from = "a".into();
                        e.to = "b".into();
                    })];
                }));
                r.next_steps.push(record::<NextStep>(|n| {
                    n.kind = NextKind::AskHere;
                    n.action = "What would falsify it?".into();
                }));
            }))?
        } else if c.get("final_theory").is_some() {
            serde_json::to_value(o)?
        } else if let Some(d) = c.get("deliberation").filter(|d| d.get("phase").is_some()) {
            let k = c[if d["phase"] == "revise" {
                "critiques_to_answer"
            } else {
                "critiques_to_judge"
            }][0]["id"]
                .as_str()
                .unwrap()
                .to_string();
            if d["phase"] == "revise" {
                o.critique_responses.push(record::<CritiqueResponse>(|r| {
                    r.critique_id = k;
                    r.reasoning = "accepted".into();
                }));
                let current = c["current_theory"][0]["id"].as_str().unwrap().to_string();
                o.proposed_changes.push(record::<Proposal>(|p| {
                    p.id = "revised".into();
                    p.statement = "Revised claim".into();
                    p.replaces_id = Some(current);
                }));
            } else {
                o.critique_reviews.push(record::<CritiqueReview>(|r| {
                    r.critique_id = k;
                    r.status = CritiqueStatus::Addressed;
                    r.reasoning = "resolved".into();
                }));
            }
            serde_json::to_value(o)?
        } else if let Some(t) = c.get("task") {
            if t["id"] == "analysis" {
                o.proposed_changes.push(record::<Proposal>(|p| {
                    p.id = "draft".into();
                    p.statement = "Draft claim".into();
                }));
            } else {
                let target = c["dependency_outputs"][0]["output"]["proposed_changes"][0]["id"]
                    .as_str()
                    .unwrap()
                    .to_string();
                o.critiques.push(record::<Critique>(|k| {
                    k.target_id = target;
                    k.severity = "high".into();
                    k.reasoning = "Unsupported causal step".into();
                }));
            }
            serde_json::to_value(o)?
        } else {
            serde_json::to_value(Plan::fallback("q", vec![]))?
        };
        let mut response = Response::default();
        response.value = value;
        response.model = "fixture-only".into();
        Ok(response)
    }
}

#[gpui_kit::test]
fn debate_timeline_shows_both_models_and_opens_critique_history(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    {
        let mut store = Store::create(dir.path(), "Debate").unwrap();
        let mut settings = Settings::default();
        settings.primary = ProviderKind::Claude;
        settings.cross_review = true;
        settings.search_enabled = false;
        execution::run(
            &mut store,
            execution::RunInput::new("question", Budget::Standard),
            &settings,
            &Script,
            &Arc::new(AtomicBool::new(false)),
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(store.project.runs[0].deliberation.rounds, 1);
    }
    let (window, view) = open(cx, dir.path());
    let (review_attempt, run) = cx.update(|cx| {
        let run = view.read(cx).current_run().unwrap().clone();
        let review = run
            .attempts
            .iter()
            .find(|a| a.task_id == "review" && a.state == TaskState::Succeeded)
            .unwrap()
            .id
            .clone();
        (review, run)
    });
    assert_eq!(run.critiques[0].status, CritiqueStatus::Addressed);
    cx.update_window(window.into(), |_, w, cx| {
        view.update(cx, |v, cx| {
            v.run_tab = RunTab::Debate;
            cx.notify();
        });
        w.render_frame(cx);
        // Every step of the exchange is rendered, critic and author alike.
        for a in run.attempts.iter().filter(|a| a.task_id != "planner") {
            assert!(
                w.try_find(gpui_kit::SharedString::from(format!("step-log-{}", a.id)))
                    .is_some(),
                "missing step {}",
                a.task_id
            );
        }
        w.click(
            gpui_kit::SharedString::from(format!("new-critique-{review_attempt}-0")),
            cx,
        );
        assert!(view.read(cx).show_inspector);
        assert!(matches!(view.read(cx).inspector, Inspector::Critique(_, ref k) if k == "K1"));
    })
    .unwrap();
}

fn debated(dir: &std::path::Path) {
    let mut store = Store::create(dir, "Report").unwrap();
    let mut settings = Settings::default();
    settings.primary = ProviderKind::Claude;
    settings.cross_review = true;
    settings.search_enabled = false;
    execution::run(
        &mut store,
        execution::RunInput::new("question", Budget::Standard),
        &settings,
        &Script,
        &Arc::new(AtomicBool::new(false)),
        &mut |_, _| {},
    )
    .unwrap();
    assert!(store.project.runs[0].reading.is_some());
}

/// The report tab is what the reader sees first: the answer, figures, suggested follow-up
/// questions that fill the question box, and a way to continue the debate.
#[gpui_kit::test]
fn report_tab_offers_follow_up_and_continuation(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    debated(dir.path());
    // Tall enough to show the whole report without scrolling.
    let (window, view) = open_sized(cx, dir.path(), 4000.);
    cx.update_window(window.into(), |_, w, cx| {
        assert!(matches!(view.read(cx).run_tab, RunTab::Report));
        w.render_frame(cx);
        assert!(w.try_find("write-report").is_none(), "report exists");
        assert!(w.try_find("continue-run").is_some());
        assert!(w.try_find("ask-followup").is_some());
        w.click("ask-suggested-0", cx);
        assert_eq!(
            view.read(cx).followup.read(cx).value().to_string(),
            "What would falsify it?"
        );
        w.click("continue-rounds-4", cx);
        assert_eq!(view.read(cx).continue_rounds, 4);
    })
    .unwrap();
    // The diagram was rendered once and cached for this theme.
    cx.update(|cx| assert_eq!(view.read(cx).figure_cache.borrow().len(), 1));
}

/// Runs made before the scribe existed show the plain fallback and offer to write a report.
#[gpui_kit::test]
fn runs_without_a_report_offer_to_write_one(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    debated(dir.path());
    let (id, _) = theory_engine::storage::projects(dir.path())
        .unwrap()
        .remove(0);
    {
        let mut store = Store::open(dir.path(), &id).unwrap();
        store.project.runs[0].reading = None;
        store.save("legacy run").unwrap();
    }
    let (window, view) = open(cx, dir.path());
    cx.update_window(window.into(), |_, w, cx| {
        assert!(view.read(cx).current_run().unwrap().reading.is_none());
        w.render_frame(cx);
        // Not clicked: that would make a real subscription call where one is logged in.
        assert!(w.try_find("write-report").is_some());
        assert!(w.try_find("continue-run").is_some());
    })
    .unwrap();
}
