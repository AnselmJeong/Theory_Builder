//! What each model call sees. Every context is built fresh from pinned project state;
//! nothing is carried over in a provider-side conversation.
use super::{GENERAL, ancestors};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use theory_core::*;

/// Byte budget shared by all source passages of one request (the request cap is 240 KB).
const SOURCE_BUDGET: usize = 140_000;
const SOURCE_MAX: usize = 12_000;
const SOURCE_MIN: usize = 2_000;
/// Untargeted questions see adopted entries plus the most relevant recent proposals.
const LEDGER_LIMIT: usize = 40;

pub(super) fn profiles(p: &Project) -> Vec<Value> {
    vec![
        serde_json::from_str(GENERAL).unwrap(),
        json!({"id":"project-research-guidance","goal":p.research_goal,"guidance":p.guidance(),"selection_rule":"Treat these as project preferences, not a fixed checklist. Select only perspectives relevant to the current question, add needed perspectives, and explain the selection in the plan. Never assume a disease or a discipline not requested."}),
    ]
}
fn terms(query: &str) -> HashSet<String> {
    query
        .split(|c: char| !c.is_alphanumeric())
        .chain(query.split(|c: char| !c.is_ascii_alphanumeric()))
        .filter(|t| t.chars().count() >= 4)
        .map(str::to_lowercase)
        .collect()
}
fn relevance(text: &str, terms: &HashSet<String>) -> usize {
    let head: String = text
        .chars()
        .take(400_000)
        .collect::<String>()
        .to_lowercase();
    terms.iter().filter(|t| head.contains(t.as_str())).count()
}
/// A bounded, contiguous passage selected locally; real line numbers are kept.
/// Lexical retrieval is explicit, not a claim that the whole book was reviewed.
pub(super) fn source_context(s: &Source, query: &str, max_bytes: usize) -> Value {
    let all: Vec<_> = s.text.lines().collect();
    let terms = terms(query);
    let window = 40;
    let best = all
        .chunks(window)
        .enumerate()
        .map(|(i, chunk)| {
            let text = chunk.join(" ").to_lowercase();
            (
                i * window,
                terms.iter().filter(|t| text.contains(t.as_str())).count(),
            )
        })
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)));
    let start = best
        .filter(|(_, score)| *score > 0)
        .map(|(i, _)| i.saturating_sub(20))
        .unwrap_or(0);
    let mut bytes = 0;
    let lines: Vec<_> = all
        .iter()
        .skip(start)
        .take_while(|l| {
            bytes += l.len() + 1;
            bytes <= max_bytes
        })
        .collect();
    json!({"id":s.id,"title":s.title,"hash":s.hash,"revision":s.revision,"access":s.access,"locator":s.locator,"included_lines":[start+1,start+lines.len()],"omitted_lines":all.len().saturating_sub(lines.len()),"selection":"bounded lexical passage; not a complete source review","text":lines.iter().enumerate().map(|(i,l)|format!("{}: {}",start+i+1,l)).collect::<Vec<_>>().join("\n")})
}
/// Fit the selected sources into one request: shrink windows evenly, drop the least
/// relevant only when even minimal windows do not fit, and say so in the context.
pub(super) fn pack_sources(p: &Project, ids: &[String], query: &str) -> (Vec<Value>, Vec<Value>) {
    let selected: Vec<&Source> = p
        .sources
        .iter()
        .filter(|s| s.active && ids.contains(&s.id))
        .collect();
    if selected.is_empty() {
        return (vec![], vec![]);
    }
    let terms = terms(query);
    let mut ranked: Vec<(usize, &Source)> = selected
        .iter()
        .map(|s| (relevance(&format!("{}\n{}", s.title, s.text), &terms), *s))
        .collect();
    ranked.sort_by_key(|a| std::cmp::Reverse(a.0));
    let keep = (SOURCE_BUDGET / SOURCE_MIN).max(1);
    let omitted: Vec<Value> = ranked
        .iter()
        .skip(keep)
        .map(|(_, s)| json!({"id":s.id,"title":s.title,"reason":"request size limit; least lexically relevant"}))
        .collect();
    ranked.truncate(keep);
    let n = ranked.len();
    let mut cap = (SOURCE_BUDGET / n).clamp(SOURCE_MIN, SOURCE_MAX);
    // Short sources (abstracts) leave room that longer ones may use.
    let short: usize = ranked
        .iter()
        .map(|(_, s)| s.text.len() + s.text.lines().count() * 7)
        .filter(|len| *len < cap)
        .sum();
    let long = ranked
        .iter()
        .filter(|(_, s)| s.text.len() + s.text.lines().count() * 7 >= cap)
        .count();
    if long > 0 {
        let slack = SOURCE_BUDGET.saturating_sub(short + long * cap);
        cap = (cap + slack / long).min(SOURCE_MAX);
    }
    // Keep the caller's order so prompts stay stable between runs.
    let order: HashMap<&str, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();
    ranked.sort_by_key(|(_, s)| order.get(s.id.as_str()).copied().unwrap_or(usize::MAX));
    (
        ranked
            .into_iter()
            .map(|(_, s)| source_context(s, query, cap))
            .collect(),
        omitted,
    )
}
/// Line ranges actually transmitted, used to refuse quotes the model could not have seen.
pub(super) fn transmitted_ranges(context: &Value) -> HashMap<String, (u32, u32)> {
    context["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            Some((
                s["id"].as_str()?.to_string(),
                (
                    s["included_lines"][0].as_u64()? as u32,
                    s["included_lines"][1].as_u64()? as u32,
                ),
            ))
        })
        .collect()
}
pub fn selected_entries(p: &Project, targets: &[String]) -> Vec<Entry> {
    if targets.is_empty() {
        return p.entries.clone();
    }
    let mut ids: HashSet<_> = targets.iter().cloned().collect();
    // First collect affected descendants. Do not expand from their shared ancestors:
    // that would pull unrelated sibling branches into a targeted follow-up.
    loop {
        let n = ids.len();
        for e in &p.entries {
            if e.proposal.depends_on.iter().any(|d| ids.contains(d)) {
                ids.insert(e.proposal.id.clone());
            }
        }
        if n == ids.len() {
            break;
        }
    }
    loop {
        let n = ids.len();
        for e in &p.entries {
            if ids.contains(&e.proposal.id) {
                ids.extend(e.proposal.depends_on.clone());
            }
        }
        if n == ids.len() {
            break;
        }
    }
    p.entries
        .iter()
        .filter(|e| ids.contains(&e.proposal.id))
        .cloned()
        .collect()
}
/// The ledger slice a request may read and reference.
pub(super) fn ledger_entries(p: &Project, targets: &[String], question: &str) -> Vec<Entry> {
    if !targets.is_empty() {
        return selected_entries(p, targets);
    }
    let terms = terms(question);
    let mut live: Vec<(usize, usize, &Entry)> = p
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.adoption != Adoption::Withdrawn)
        .map(|(i, e)| {
            let adopted = usize::from(e.adoption == Adoption::Adopted);
            (
                adopted * 1000 + relevance(&e.proposal.statement, &terms),
                i,
                e,
            )
        })
        .collect();
    live.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut ids: HashSet<String> = live
        .iter()
        .take(LEDGER_LIMIT)
        .map(|(_, _, e)| e.proposal.id.clone())
        .collect();
    // Keep the slice closed under dependencies so references stay resolvable.
    loop {
        let n = ids.len();
        for e in &p.entries {
            if ids.contains(&e.proposal.id) {
                ids.extend(e.proposal.depends_on.clone());
            }
        }
        if n == ids.len() {
            break;
        }
    }
    p.entries
        .iter()
        .filter(|e| ids.contains(&e.proposal.id))
        .cloned()
        .collect()
}
fn ledger_view(entries: &[Entry]) -> Vec<Value> {
    entries
        .iter()
        .map(|e| {
            let p = &e.proposal;
            json!({"id":p.id,"kind":p.kind,"claim_kind":p.claim_kind,"statement":p.statement,"scope":p.scope,"rationale":p.rationale.chars().take(600).collect::<String>(),"depends_on":p.depends_on,"replaces_id":p.replaces_id,"adoption":e.adoption,"review":e.review})
        })
        .collect()
}
/// Fresh specialist context. Only explicitly selected source versions and dependency outputs enter it.
pub fn task_context(p: &Project, run: &Run, task: &TaskSpec) -> Value {
    let dependencies = ancestors(run, task);
    let prior: Vec<_> = if task.base_only {
        vec![]
    } else {
        run.attempts
            .iter()
            .filter(|a| a.state == TaskState::Succeeded && dependencies.contains(&a.task_id))
            .filter_map(|a| {
                a.output
                    .as_ref()
                    .map(|o| json!({"task_id":a.task_id,"output":o}))
            })
            .collect()
    };
    let (sources, omitted) = pack_sources(p, &task.source_ids, &run.question);
    json!({"question":run.question,"targets":run.targets,"base_revision":run.base_revision,"profiles":profiles(p),"task":task,"ledger":ledger_view(&ledger_entries(p,&run.targets,&run.question)),"sources":sources,"omitted_sources":omitted,"dependency_outputs":prior,"contract_version":"research/v2","output_id_prefix":prefix(run,&task.id),"rules":"Existing entries are read-only; revisions are new proposals with replaces_id. Critiques you raise receive engine IDs and are judged later by the critic; you never close critiques. Leave critique_responses and critique_reviews empty in this task. Include no model attribution or other reviewers' outputs when constructing independent candidates. If task dependencies are missing, state the gap. Numerical execution unavailable. Literature retrieval has already been performed if requested in the plan; only the supplied source text is evidence."})
}
pub(super) fn prefix(run: &Run, task_id: &str) -> String {
    format!("{}_{}", &run.id[..run.id.len().min(12)], task_id)
}
fn critique_view(c: &CritiqueRecord) -> Value {
    json!({"id":c.id,"target_id":c.critique.target_id,"severity":c.critique.severity,"category":c.critique.category,"reasoning":c.critique.reasoning,"suggested_action":c.critique.suggested_action,"status":c.status,"raised_in_round":c.round,
        "history":c.events.iter().map(|e| json!({"round":e.round,"by":if e.stance.is_some(){"author"}else{"critic"},"stance":e.stance,"status":e.status,"reasoning":e.reasoning,"revised_ids":e.revised_ids})).collect::<Vec<_>>()})
}
fn round_summaries(run: &Run) -> Vec<Value> {
    run.attempts
        .iter()
        .filter(|a| a.state == TaskState::Succeeded && a.phase != Some(Phase::Plan))
        .filter_map(|a| {
            a.output.as_ref().map(|o| {
                json!({"task_id":a.task_id,"round":a.round,"phase":a.phase,"by":if a.phase==Some(Phase::Review){"critic"}else{"author"},"summary":o.summary.chars().take(2000).collect::<String>()})
            })
        })
        .collect()
}
const REVISE_RULES: &str = "You are the AUTHOR of the working theory. The critic (a separate model context) raised the critiques listed in critiques_to_answer. For EVERY one of them add a critique_responses item: stance accept, partially_accept or rebut, with reasoning grounded in supplied sources or explicit logic. For accepted or partially accepted points, write revised proposals with replaces_id set to the current_theory ID they revise and list those new IDs in revised_ids. A rebuttal must say why the critique fails; do not concede to agreement alone and do not hide weaknesses. You may add a missing assumption only with its reason and dependents; never reintroduce withdrawn or prohibited assumptions. Cite only supplied source passages with exact text and line ranges; otherwise mark the claim as assumption or speculation. Leave critiques and critique_reviews empty: you cannot judge critiques of your own work. Summary: what changed in this revision and what remains weak.";
const REVIEW_RULES: &str = "You are the CRITIC. Judge the current_theory independently; agreement with the author is not evidence. First, for EVERY critique in critiques_to_judge add a critique_reviews item: addressed (the revision or response resolves it), withdrawn (the author's rebuttal is correct), contested (you maintain the objection — say precisely why the response fails), deferred (cannot be settled with the available sources or is out of the question's scope), or open (not answered). Then add NEW critiques only for material problems not already listed: logical gaps, unsupported level bridges, correlation read as causation, quotes that do not support the claim's scope, ignored alternatives or counter-evidence. Target current_theory IDs (or empty target_id for the whole draft). Do not pad with minor objections. You may attach counter-evidence as evidence_refs to existing claim IDs (relation contradicts or partial_support) with exact quotes. Leave proposed_changes and critique_responses empty. Summary: your overall verdict on the current draft.";
/// One step of the author ↔ critic exchange. Model identities are not disclosed to each other.
pub(super) fn deliberation_context(
    p: &Project,
    run: &Run,
    task: &TaskSpec,
    phase: Phase,
    round: u32,
    focus: &[String],
) -> Value {
    let (sources, omitted) = pack_sources(p, &task.source_ids, &run.question);
    let theory = run.theory();
    let focus_set: HashSet<&String> = focus.iter().collect();
    let (focus_list, others): (Vec<_>, Vec<_>) = run
        .critiques
        .iter()
        .partition(|c| focus_set.contains(&c.id));
    let (focus_key, rules) = match phase {
        Phase::Revise => ("critiques_to_answer", REVISE_RULES),
        _ => ("critiques_to_judge", REVIEW_RULES),
    };
    let mut context = json!({"question":run.question,"targets":run.targets,"base_revision":run.base_revision,"profiles":profiles(p),"task":task,
        "deliberation":{"phase":phase,"round":round,"max_rounds":run.deliberation.max_rounds,"your_role":if phase==Phase::Revise{"author"}else{"critic"}},
        "current_theory":theory,
        "other_critiques":others.iter().map(|c| critique_view(c)).collect::<Vec<_>>(),
        "previous_steps":round_summaries(run),
        "ledger":ledger_view(&ledger_entries(p,&run.targets,&run.question)),
        "sources":sources,"omitted_sources":omitted,
        "contract_version":"research/v2","output_id_prefix":prefix(run,&task.id),"rules":rules});
    context[focus_key] = json!(
        focus_list
            .iter()
            .map(|c| critique_view(c))
            .collect::<Vec<_>>()
    );
    context
}
/// Final answer: the converged theory and the full critique ledger, including dissent.
pub(super) fn synthesis_context(p: &Project, run: &Run, task: &TaskSpec) -> Value {
    let mut context = task_context(p, run, task);
    context["final_theory"] = json!(run.theory());
    context["critique_ledger"] = json!(run.critiques.iter().map(critique_view).collect::<Vec<_>>());
    context["deliberation"] = json!({"rounds":run.deliberation.rounds,"max_rounds":run.deliberation.max_rounds,"stop":run.deliberation.stop,"steps":round_summaries(run)});
    context["rules"] = json!(
        "Write the final answer for the user in their language. Start with the direct conclusion. Then present the theory as a short ordered argument that refers to final_theory IDs, separating reported findings with exact citations from assumptions, deductions and speculation. State faithfully which critiques were settled and which remain contested or open, giving both positions; agreement between models is not evidence. End with distinguishing predictions or observations and next checks in next_actions, and remaining gaps in unresolved. Refer to existing IDs instead of cloning proposals; add a new proposal only if the synthesis itself needs one. Leave critique_responses and critique_reviews empty. Set evidence_insufficient when the supplied sources cannot support the central claims."
    );
    context
}
