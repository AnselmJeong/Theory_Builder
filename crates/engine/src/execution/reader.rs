//! What the scribe and follow-up answers see. The scribe did not take part in the debate:
//! it receives the conclusion, the working theory, the critiques and the verified evidence,
//! with every internal identifier already translated into words a reader can follow.
use super::context::profiles;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use theory_core::*;

/// Internal identifiers that may appear in model text, with what a reader should see.
pub fn internal_names(p: &Project, run: &Run) -> Vec<(String, String)> {
    let labels: HashMap<String, String> = run.theory_labels().into_iter().collect();
    let all: Vec<&Proposal> = run
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
    let mut out: Vec<(String, String)> = vec![];
    let mut push = |id: &str, label: &str| {
        if !id.is_empty() && !out.iter().any(|(i, _)| i == id) {
            out.push((id.to_string(), label.to_string()));
        }
    };
    for p in &all {
        let mut head = p.id.as_str();
        for _ in 0..=all.len() {
            match next.get(head) {
                Some(n) => head = n,
                None => break,
            }
        }
        let label = labels
            .get(head)
            .cloned()
            .unwrap_or_else(|| "이전 초안의 한 항목".into());
        push(&p.id, &label);
        // Models often drop the run prefix: "revise-r1-P3".
        if let Some((_, short)) = p.id.split_once('_')
            && short.len() >= 6
            && short.contains(['-', '_'])
        {
            push(short, &label);
        }
    }
    for e in &p.entries {
        push(&e.proposal.id, "기존 연구 노트의 한 항목");
    }
    for c in &run.critiques {
        push(&c.id, &c.label());
    }
    for s in &p.sources {
        push(&s.id, &s.derived_bib().short_cite());
    }
    out
}
/// Replace internal identifiers in text a model will read.
pub fn friendly(text: &str, names: &[(String, String)]) -> String {
    let mut out = text.to_string();
    let mut ids: Vec<&(String, String)> = names
        .iter()
        .filter(|(id, _)| out.contains(id.as_str()))
        .collect();
    ids.sort_by_key(|(id, _)| std::cmp::Reverse(id.len()));
    for (id, label) in ids {
        out = replace_token(&out, id, label);
    }
    out
}
fn clip(text: &str, n: usize) -> String {
    if text.chars().count() <= n {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(n).collect::<String>())
    }
}
/// Map every proposal of the run to the working-theory head it became.
fn heads(run: &Run) -> HashMap<String, String> {
    let theory: HashSet<String> = run.theory().into_iter().map(|p| p.id).collect();
    let all: Vec<&Proposal> = run
        .attempts
        .iter()
        .filter(|a| a.state == TaskState::Succeeded)
        .filter_map(|a| a.output.as_ref())
        .flat_map(|o| o.proposed_changes.iter())
        .collect();
    let mut next: HashMap<&str, &str> = HashMap::new();
    for p in &all {
        if let Some(r) = p.replaces_id.as_deref() {
            next.insert(r, p.id.as_str());
        }
    }
    let mut map = HashMap::new();
    for p in &all {
        let mut head = p.id.as_str();
        for _ in 0..=all.len() {
            match next.get(head) {
                Some(n) => head = n,
                None => break,
            }
        }
        if theory.contains(head) {
            map.insert(p.id.clone(), head.to_string());
        }
    }
    map
}
/// Verified evidence for each working-theory head: (claim head, evidence, attached to an
/// earlier version of the claim).
pub fn theory_evidence(run: &Run) -> Vec<(String, Evidence, bool)> {
    let heads = heads(run);
    let mut out: Vec<(String, Evidence, bool)> = vec![];
    for e in run
        .attempts
        .iter()
        .filter(|a| a.state == TaskState::Succeeded)
        .filter_map(|a| a.output.as_ref())
        .flat_map(|o| o.evidence_refs.iter())
    {
        if let Some(head) = heads.get(&e.claim_id)
            && !out.iter().any(|(h, x, _)| {
                h == head
                    && x.source_id == e.source_id
                    && x.start_line == e.start_line
                    && x.quote == e.quote
            })
        {
            out.push((head.clone(), e.clone(), *head != e.claim_id));
        }
    }
    out
}
/// Sources with verified evidence behind the working theory, in order of first use.
pub fn cited_sources(run: &Run) -> Vec<String> {
    let mut ids: Vec<String> = vec![];
    for (_, e, _) in theory_evidence(run) {
        if !ids.contains(&e.source_id) {
            ids.push(e.source_id);
        }
    }
    ids
}
/// Citation keys for the run's cited sources: frozen keys stay; others are provisional.
pub fn citation_keys(p: &Project, cited: &[String]) -> HashMap<String, String> {
    let sources: Vec<&Source> = cited
        .iter()
        .filter_map(|id| p.sources.iter().find(|s| s.id == *id))
        .collect();
    let elsewhere: HashSet<String> = p
        .sources
        .iter()
        .filter(|s| !cited.contains(&s.id))
        .filter_map(|s| s.bib.as_ref().map(|b| b.key.clone()))
        .filter(|k| !k.is_empty())
        .collect();
    assign_keys(&sources, &elsewhere)
}
fn access_words(a: Access) -> &'static str {
    match a {
        Access::ProvidedText => "본문",
        Access::ProvidedExcerpt => "발췌",
        Access::AbstractOnly => "초록만",
        Access::MetadataOnly => "서지 정보만",
    }
}
/// The material the scribe works from, with reader numbering instead of engine IDs.
fn material(p: &Project, run: &Run, keys: &HashMap<String, String>) -> Value {
    let names = internal_names(p, run);
    let f = |t: &str| friendly(t, &names);
    let labels: HashMap<String, String> = run.theory_labels().into_iter().collect();
    let evidence = theory_evidence(run);
    let disputed: HashMap<&str, CritiqueStatus> = run
        .critiques
        .iter()
        .filter(|c| c.material() && !c.status.is_settled())
        .map(|c| (c.critique.target_id.as_str(), c.status))
        .collect();
    let theory: Vec<Value> = run
        .theory()
        .iter()
        .map(|t| {
            let ev: Vec<Value> = evidence
                .iter()
                .filter(|(h, _, _)| *h == t.id)
                .filter_map(|(_, e, earlier)| {
                    let key = keys.get(&e.source_id)?;
                    Some(json!({"cite":format!("@{key}"),"relation":e.relation,"quote":clip(&e.quote,320),"why":clip(&f(&e.reasoning),240),"attached_to_earlier_version":earlier}))
                })
                .collect();
            json!({"ref":labels.get(&t.id).cloned().unwrap_or_default(),"kind":t.kind,"claim_kind":t.claim_kind,"statement":f(&t.statement),"scope":clip(&f(&t.scope),500),"rationale":clip(&f(&t.rationale),500),
                "builds_on":t.depends_on.iter().filter_map(|d| labels.get(d)).collect::<Vec<_>>(),
                "evidence":ev,"open_objection":disputed.get(t.id.as_str()).map(|s| s.label())})
        })
        .collect();
    let critiques: Vec<Value> = run
        .critiques
        .iter()
        .map(|c| {
            let reply = c.events.iter().rev().find(|e| e.stance.is_some());
            let judgement = c.events.iter().rev().find(|e| e.status.is_some());
            json!({"ref":c.label(),"severity":c.critique.severity,"status":c.status.label(),"about":labels.get(&c.critique.target_id).cloned().unwrap_or_else(|| "초안 전체".into()),
                "objection":clip(&f(&c.critique.reasoning),700),"suggested":clip(&f(&c.critique.suggested_action),300),
                "latest_reply":reply.map(|e| json!({"stance":e.stance.map(|s| s.label()),"text":clip(&f(&e.reasoning),400)})),
                "latest_judgement":judgement.map(|e| clip(&f(&e.reasoning),400))})
        })
        .collect();
    let conclusion = match run.final_output() {
        Some(o) => {
            json!({"summary":f(&o.summary),"unresolved":o.unresolved.iter().map(|u| f(u)).collect::<Vec<_>>(),"next_checks":o.next_actions.iter().map(|u| f(u)).collect::<Vec<_>>(),"evidence_insufficient":o.evidence_insufficient})
        }
        None => {
            json!({"summary":"The final synthesis did not complete; work from the working theory and critiques.","unresolved":[],"next_checks":[],"evidence_insufficient":true})
        }
    };
    let settled = run
        .critiques
        .iter()
        .filter(|c| c.status.is_settled())
        .count();
    let references: Vec<Value> = keys
        .iter()
        .filter_map(|(id, key)| {
            let s = p.sources.iter().find(|s| s.id == *id)?;
            let b = s.derived_bib();
            Some(json!({"key":key,"short":b.short_cite(),"title":b.title,"year":b.year,"what_was_read":access_words(s.access)}))
        })
        .collect();
    json!({"question":run.question,"research_goal":p.research_goal,"user_direction":run.directions.iter().map(|d| &d.text).collect::<Vec<_>>(),
        "conclusion":conclusion,"working_theory":theory,"critiques":critiques,
        "debate":{"rounds":run.deliberation.rounds,"ended_because":run.deliberation.stop.map(|s| s.label()),"objections_total":run.critiques.len(),"objections_settled":settled,"objections_open":run.critiques.len()-settled,
            "models":if run.deliberation.cross_model(){"two different AI models (one built, one criticised)"}else{"one AI model in two separate contexts"},"numerical_execution":"none"},
        "references":references})
}
const SCRIBE_RULES: &str = "You are the SCRIBE. Two AI contexts debated the question; you did not take part and take no side. Your reader is the person who asked: a researcher who knows their own field but did not watch the debate and does not know its vocabulary. A theory the reader cannot follow is useless to them, so being understood comes first. Write the whole report in the language of the question.\n\
\n\
HOW TO WRITE SO THE READER UNDERSTANDS\n\
- Point first, then the reason, then an example. One idea per paragraph, 2-4 sentences each.\n\
- Short sentences with a clear subject and verb: say who or what does what to what, and with what result. Do not stack nouns or abstract nominalisations; unpack them into a sentence. In Korean, '스트레스 호르몬 조절이 늦게 회복되면 증상이 오래 간다' instead of '조절 축 재보정 지연 기반 증상 지속성'.\n\
- Natural prose in the reader's language, not translated English. Give an English technical term in parentheses at most once, the first time it appears.\n\
- Every technical term is explained in plain words where it first appears (a short clause or parenthesis), and listed in glossary.\n\
- Make mechanisms concrete: tell them as a sequence in time (first … then … so …), walk through one concrete case, or use an everyday analogy and say where it stops working.\n\
- State uncertainty once, plainly, where it applies ('아직 모른다', '쥐에서만 확인되었다'); do not pile up hedges. Keep species, sample, measurement, time scale and level explicit when they limit a point.\n\
- No LaTeX; write x(t), ẋ, ε ≪ 1, → in Unicode. Markdown lists and **bold** are fine; no tables inside text (use a table figure).\n\
\n\
STRUCTURE: ONE STORY, NOTHING SAID TWICE\n\
- title: a short plain title for the question's topic.\n\
- answer: the direct answer in 2-4 plain sentences, as if the reader had asked you in person.\n\
- key_points: 3-5 things the reader should remember. heading: the point itself as a short full sentence. body: 1-3 sentences on why it holds and what it rests on. standing: its honest standing. Do not repeat the answer. figure_ids: [] (figures belong to sections) unless there are no sections.\n\
- sections: 2-4 sections that explain the reasoning in order, each building on the one before, for example: (1) the facts we start from and what makes the question hard; (2) the proposed mechanism, step by step; (3) what it predicts, worked through on one concrete case; (4) where it may fail and what competes with it. Each at most ~300 words. Sections deepen the key points; they do not restate them. When something was already said, refer back to it ('앞에서 본 것처럼') instead of saying it again.\n\
\n\
Never show internal labels: no '작업 이론 n', '비판 n', IDs, task names, field names, 'author', 'critic', 'proposal' or 'ledger'. Refer to ideas by their content. Do not narrate who said what in the debate.\n\
Faithfulness: use only the supplied conclusion, working_theory, critiques and references; add no new claims, data or sources. Give each key point its honest `standing`: supported only when its claim has evidence; reasoned for deductions; hypothesis for proposed but untested; speculation otherwise. Agreement between models is not evidence.\n\
Citations: only pandoc syntax [@key] or [@key1; @key2] with keys from `references`, placed after statements that the cited evidence supports. Never invent keys.\n\
\n\
FIGURES (1-3, each showing something the others do not)\n\
- Usually one diagram of the core mechanism (a causal chain, or levels such as molecule → cell → circuit → behaviour) and, when it helps, one table comparing alternatives, predictions or evidence. Never make two diagrams of the same structure or with mostly the same boxes.\n\
- Show each figure exactly once: put its id in figure_ids of the one section that explains it, and have that section's text walk the reader through it ('아래 그림에서 …'). Figures are numbered automatically; do not write figure numbers.\n\
- Diagram: 4-10 nodes. label: a short noun phrase (at most ~16 characters) that makes sense without the note. note: one plain sentence for every node. Edges read as a sentence from → to with a short verb label ('높인다', '늦춘다', '억제'); dashed = hypothetical link. groups: ordered levels or stages, drawn top to bottom; leave [] when there are none.\n\
- caption: one sentence telling the reader how to read the figure (what arrows, dashes and bands mean, and what to notice).\n\
- Table: 2-5 columns, at most 8 rows, short cells.\n\
- Leave unused figure fields as empty arrays.\n\
\n\
open_questions: what is still uncertain, as plain questions the reader would care about, why it matters, and what evidence or argument would settle it. Merge overlapping open objections; do not mention objection numbers.\n\
next_steps: concrete actions with kind experiment (a new measurement: say what to record in whom/what and what result would count for or against), data_analysis (analysis of existing or collectable data), theory (modelling or argument work), reading, or ask_here (a follow-up question the reader can ask this app; write `action` as that question). `purpose` says what the reader would learn. Never write vague steps such as 'verify X' without saying how.\n\
how_made: 2-3 plain sentences on how the answer was produced (from `debate`), including that no numerical simulation was run when that is so.\n\
bibliography: one item per entry of bib_needed only: copy title, authors ('Family, Given'), year, container (journal or book), publisher and doi exactly as printed in its header; leave a field empty when it is not printed. Never guess. Use [] when bib_needed is empty.";
/// The scribe's request. `bib_needed` holds sources whose metadata is only a file name.
pub(super) fn report_context(
    p: &Project,
    run: &Run,
    keys: &HashMap<String, String>,
    bib_needed: &[String],
) -> Value {
    let mut context = material(p, run, keys);
    context["request"] = json!("reader_report");
    context["bib_needed"] = json!(
        bib_needed
            .iter()
            .filter_map(|id| {
                let s = p.sources.iter().find(|s| s.id == *id)?;
                Some(json!({"key":keys.get(id)?,"file":s.title,"header":s.header(2500)}))
            })
            .collect::<Vec<_>>()
    );
    context["profiles"] = json!(profiles(p));
    context["rules"] = json!(SCRIBE_RULES);
    context
}
const ASK_RULES: &str = "You are the SCRIBE answering the reader's follow-up question about the report on their question. Answer in the reader's language in at most ~300 words: the direct answer first, then the reason, then an example when it helps. Short sentences with a clear subject and verb; explain any technical term in plain words where it first appears; do not repeat what the report already says unless the question asks for it. Use only the supplied report, working_theory, critiques and references; if the question needs new evidence or argument that this material does not contain, say so plainly, set needs_research true and give suggested_direction: one sentence the reader could use to continue the debate. Otherwise needs_research is false and suggested_direction is empty. Cite only with [@key] from references. Never show internal labels ('작업 이론 n', '비판 n', IDs, 'author', 'critic'). No LaTeX; use Unicode math.";
pub(super) fn ask_context(p: &Project, run: &Run, question: &str) -> Value {
    let keys: HashMap<String, String> = cited_sources(run)
        .into_iter()
        .filter_map(|id| {
            let key = p
                .sources
                .iter()
                .find(|s| s.id == id)?
                .bib
                .as_ref()?
                .key
                .clone();
            (!key.is_empty()).then_some((id, key))
        })
        .collect();
    let mut context = material(p, run, &keys);
    context["request"] = json!("follow_up");
    context["report"] = json!(run.reading.as_ref().map(|r| &r.report));
    context["earlier_follow_ups"] = json!(
        run.followups
            .iter()
            .rev()
            .filter(|f| f.error.is_none())
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|f| json!({"question":f.question,"answer":clip(&f.answer,1200)}))
            .collect::<Vec<_>>()
    );
    context["follow_up_question"] = json!(question);
    context["rules"] = json!(ASK_RULES);
    context
}
/// Vocabulary for cleaning a stored text: known citation keys and internal names.
pub fn vocabulary(p: &Project, run: &Run, keys: impl IntoIterator<Item = String>) -> Vocabulary {
    Vocabulary {
        keys: keys.into_iter().collect(),
        internal: internal_names(p, run),
    }
}
