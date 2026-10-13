//! One question → plan → initial construction → author ↔ critic deliberation → synthesis.
//!
//! The planner proposes a small DAG for the initial work. After it, the engine (not a model)
//! drives a bounded recursive exchange: the critic model reviews the working theory, the
//! author model answers every material critique and revises, and the critic judges those
//! answers and raises new problems. It stops when material critiques are settled, when no
//! progress is made, or when the round/call/time budget runs out. Only the critic can change
//! a critique's status; models never adopt anything into the project ledger.
use crate::{
    providers::{Cancel, ModelProvider, ProviderKind, Request, Settings},
    storage::Store,
};
use anyhow::{Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use theory_core::*;
mod citations;
mod context;
use context::*;
pub use context::{selected_entries, task_context};

pub const GENERAL: &str = include_str!("../../../resources/profiles/general-research.json");
pub const NEURO: &str = include_str!("../../../resources/profiles/neurobiology.json");
pub const DYNAMICS: &str = include_str!("../../../resources/profiles/multilevel-dynamics.json");

pub(crate) fn ancestors(run: &Run, task: &TaskSpec) -> HashSet<String> {
    let mut ids: HashSet<String> = task.depends_on.iter().cloned().collect();
    if let Some(plan) = &run.plan {
        loop {
            let n = ids.len();
            for t in &plan.tasks {
                if ids.contains(&t.id) {
                    ids.extend(t.depends_on.clone());
                }
            }
            if n == ids.len() {
                break;
            }
        }
    }
    ids
}
fn planner_context(p: &Project, r: &Run, search_enabled: bool) -> Value {
    let remaining = r.budget.calls().saturating_sub(r.calls + 1);
    let rounds = r.budget.rounds();
    let max_tasks = remaining.saturating_sub(2 * rounds).max(3);
    json!({"search_enabled":search_enabled,"search_instructions":"When search_enabled, generate up to 2 focused English literature search_queries (3-6 core terms each; avoid stacking every possible mechanism into one AND query) if external evidence is needed. Search is available through PubMed and TinyFish before task execution; retrieved sources are automatically attached to all tasks. For a research/theory question with no usable local sources, search is normally required. Use empty search_queries only when external evidence is unnecessary or the user forbids search. Never assume sources were checked before retrieval.","question":r.question,"target_ids":r.targets,"base_revision":r.base_revision,"profiles":profiles(p),"ledger":ledger_entries(p,&r.targets,&r.question).iter().map(|e| json!({"id":e.proposal.id,"kind":e.proposal.kind,"statement":e.proposal.statement,"adoption":e.adoption})).collect::<Vec<_>>(),"sources":p.sources.iter().filter(|s| s.active).map(|s|json!({"id":s.id,"title":s.title,"access":s.access,"lines":s.text.lines().count()})).collect::<Vec<_>>(),"max_tasks":max_tasks,
        "deliberation":{"max_revision_rounds":rounds,"how":"After your initial tasks finish, the engine itself runs critique → author response and revision → critic re-review rounds between two separate model contexts until material critiques are settled or limits are reached. Plan only the initial analysis/construction, one initial critic or evidence_reviewer, and the final synthesizer. Never add revision or second-round tasks yourself."},
        "rules":"Propose a minimal DAG for this question. Include a critic or evidence reviewer and finish with a synthesizer depending transitively on every task. Use only supplied source IDs and ledger IDs. No allowed_tools (empty arrays). base_only=true requires no dependencies. Independent constructors must be base_only=true and receive the same base. Concept: analyst->critic->synthesizer. Evidence: evidence_reviewer->critic->synthesizer. Compare: two independent analysts->critic->synthesizer. Mechanism or theory building: two independent constructors->critic->synthesizer. Formalization: formalizer->critic->synthesizer; no numerical execution. Observation design: analyst->critic->synthesizer. Revision: only affected targets and preserve user constraints. Documentation: analyst->reconstructor->synthesizer plus review. Do not insert clinical critique unless the question asks clinical scope. No disease assumptions from profiles. Scope and requirements must cover all user constraints. Write scope in the user's language as a short plan description (for example: 개념 정리 → 비판 검토 → 종합)."})
}
/// Errors where the same request may succeed shortly: the server refused it before work.
fn is_transient(message: &str) -> bool {
    let m = message.to_lowercase();
    [
        "serveroverloaded",
        "overloaded",
        "at capacity",
        "rate limit",
        "rate_limit",
        "ratelimit",
        "too many requests",
        " 429",
        " 529",
        " 503",
        "temporarily unavailable",
        "try again later",
    ]
    .iter()
    .any(|k| m.contains(k))
}
/// Errors that no later call to the same provider can fix within this run.
fn is_fatal(message: &str) -> bool {
    let m = message.to_lowercase();
    m == "cancelled"
        || m == "budget_exhausted"
        || [
            "로그인이 필요",
            "authentication",
            "not logged in",
            "cli를 찾지 못했습니다",
            "사용량을 확인할 수 없거나 소진",
            "사용량이 소진",
            "지출 제한",
            "api 키",
        ]
        .iter()
        .any(|k| m.contains(k))
}
const CALL_BUDGET: &str = "호출 예산이 남지 않아 이 단계를 건너뜀";
const CONTEXT_LIMIT: &str = "선택 자료가 문맥 한도를 넘었습니다. 출처 또는 대상을 줄이세요.";
#[derive(Clone, Copy)]
struct Step<'s> {
    task_id: &'s str,
    role: Option<Role>,
    phase: Phase,
    round: u32,
    provider: ProviderKind,
    /// Calls that must stay available after this one (later critic review, final synthesis).
    reserve: u32,
}
impl Step<'_> {
    fn describe(&self) -> String {
        let what = match (self.phase, self.round) {
            (Phase::Plan, _) => "연구 계획".to_string(),
            (Phase::Construct, _) => self.role.map(|r| r.label()).unwrap_or("분석").into(),
            (Phase::Review, 0) => "비판 검토".into(),
            (Phase::Review, r) => format!("{r}차 재검토"),
            (Phase::Revise, r) => format!("{r}차 응답·수정"),
            (Phase::Synthesis, _) => "답변 종합".into(),
        };
        format!("{} · {what}", self.provider.display())
    }
}
struct RunExecutor<'a> {
    store: &'a mut Store,
    provider: &'a dyn ModelProvider,
    settings: &'a Settings,
    cancel: &'a Cancel,
    notify: &'a mut dyn FnMut(&Project, String),
    deadline: Instant,
    run_ix: usize,
    author: ProviderKind,
    critic: ProviderKind,
    /// Set when the critic provider cannot be used any more in this run (login, quota).
    critic_down: Option<String>,
}
impl RunExecutor<'_> {
    fn run(&self) -> &Run {
        &self.store.project.runs[self.run_ix]
    }
    fn run_mut(&mut self) -> &mut Run {
        &mut self.store.project.runs[self.run_ix]
    }
    fn remaining(&self) -> u32 {
        let r = self.run();
        r.budget.calls().saturating_sub(r.calls)
    }
    fn emit(&mut self, message: String) {
        (self.notify)(&self.store.project, message);
    }
    /// Wait without ignoring cancellation or the run deadline.
    fn pause(&self, duration: Duration) -> Result<()> {
        let until = Instant::now() + duration;
        while Instant::now() < until {
            ensure!(!self.cancel.load(Ordering::SeqCst), "cancelled");
            ensure!(Instant::now() < self.deadline, "budget_exhausted");
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }
    /// One model request. Each attempt (including retries) is reserved and recorded before it
    /// is sent. Only refusals before work (overload, rate limit) are retried, at most twice.
    fn call(&mut self, step: Step, context: Value, output_schema: Value) -> Result<Value> {
        let prompt = serde_json::to_string(&context)?;
        ensure!(prompt.len() <= 240_000, CONTEXT_LIMIT);
        let mut retries = 0;
        loop {
            ensure!(!self.cancel.load(Ordering::SeqCst), "cancelled");
            ensure!(Instant::now() < self.deadline, "budget_exhausted");
            // Running out of calls skips this step; running out of time ends the run.
            ensure!(self.remaining() > step.reserve, CALL_BUDGET);
            let model = self.settings.model(step.provider).to_string();
            let run = self.run_mut();
            run.calls += 1;
            run.attempts.push(record::<Attempt>(|a| {
                a.id = id("attempt");
                a.task_id = step.task_id.into();
                a.role = step.role;
                a.phase = Some(step.phase);
                a.round = step.round;
                a.state = TaskState::Running;
                a.provider = step.provider.label().into();
                a.model = model.clone();
                a.context = context.clone();
                a.started = now();
            }));
            let attempt_ix = run.attempts.len() - 1;
            self.store.save("호출 예산 예약 및 요청 기록")?;
            let (calls, budget) = (self.run().calls, self.run().budget.calls());
            self.emit(format!("{} · 호출 {calls}/{budget}", step.describe()));
            let request = Request::new(
                prompt.clone(),
                output_schema.clone(),
                model,
                self.deadline.min(Instant::now() + Duration::from_secs(600)),
            );
            let project = &self.store.project;
            let notify = &mut self.notify;
            let result =
                self.provider
                    .generate(step.provider, &request, self.cancel, &mut |status| {
                        notify(project, status)
                    });
            if self.cancel.load(Ordering::SeqCst) {
                let a = &mut self.run_mut().attempts[attempt_ix];
                a.state = TaskState::Cancelled;
                a.error = Some("취소 뒤 도착한 출력은 적용하지 않음; 원격 상태 확인 불가".into());
                self.store.save("호출 취소")?;
                bail!("cancelled");
            }
            match result {
                Ok(response) => {
                    let a = &mut self.run_mut().attempts[attempt_ix];
                    a.model = response.model;
                    a.usage = response.usage;
                    a.response = Some(response.value.clone());
                    a.state = TaskState::Validating;
                    self.store.save("응답 수신; 검증 전 원장 미적용")?;
                    return Ok(response.value);
                }
                Err(e) => {
                    let message = e.to_string();
                    let a = &mut self.run_mut().attempts[attempt_ix];
                    a.state = TaskState::Failed;
                    a.error = Some(message.clone());
                    self.store.save("호출 실패")?;
                    if is_transient(&message) && retries < 2 && self.remaining() > step.reserve {
                        retries += 1;
                        let wait = 5 * 3u64.pow(retries - 1);
                        self.emit(format!(
                            "{} 서버 혼잡 · {wait}초 후 다시 요청 ({retries}/2)",
                            step.provider.display()
                        ));
                        self.pause(Duration::from_secs(wait))?;
                        continue;
                    }
                    return Err(e);
                }
            }
        }
    }
    fn finish_attempt(&mut self, outcome: &Result<Envelope>, notes: Vec<String>) -> Result<()> {
        let a = self.run_mut().attempts.last_mut().unwrap();
        a.validation_notes = notes;
        match outcome {
            Ok(output) => {
                a.state = TaskState::Succeeded;
                a.error = None;
                a.output = Some(output.clone());
            }
            Err(e) => {
                a.state = TaskState::Failed;
                a.error = Some(e.to_string());
            }
        }
        self.store.save("출력 구조 검증")
    }
    /// IDs an output may reference: the transmitted ledger slice plus run proposals it could see.
    fn known_ids(&self, step: &Step, task: &TaskSpec) -> HashSet<String> {
        let p = &self.store.project;
        let run = self.run();
        let mut ids: HashSet<String> = ledger_entries(p, &run.targets, &run.question)
            .iter()
            .map(|e| e.proposal.id.clone())
            .collect();
        let visible: Option<HashSet<String>> = match step.phase {
            Phase::Construct | Phase::Synthesis if step.round == 0 => Some(if task.base_only {
                HashSet::new()
            } else {
                ancestors(run, task)
            }),
            _ => None,
        };
        for a in &run.attempts {
            if a.state == TaskState::Succeeded
                && visible.as_ref().is_none_or(|v| v.contains(&a.task_id))
                && let Some(o) = &a.output
            {
                ids.extend(o.proposed_changes.iter().map(|p| p.id.clone()));
            }
        }
        if step.phase == Phase::Synthesis {
            ids.extend(run.theory().into_iter().map(|p| p.id));
        }
        ids
    }
    /// Parse, normalize and quarantine. Invalid quotes or references are removed item by item
    /// and reported; the remaining output must then pass the strict ledger validation.
    fn accept(
        &self,
        step: &Step,
        task: &TaskSpec,
        context: &Value,
        raw: Value,
        notes: &mut Vec<String>,
    ) -> Result<Envelope> {
        let mut output: Envelope = serde_json::from_value(raw)?;
        ensure!(!output.summary.trim().is_empty(), "답변 요약 누락");
        let existing = self.known_ids(step, task);
        notes.extend(output.namespace_ids(&prefix(self.run(), &task.id), &existing));
        let sent: HashSet<&str> = context["sources"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s["id"].as_str())
            .collect();
        let sources: Vec<Source> = self
            .store
            .project
            .sources
            .iter()
            .filter(|s| s.active && sent.contains(s.id.as_str()))
            .cloned()
            .collect();
        notes.extend(citations::reconcile(&mut output, &sources, context));
        if step.phase != Phase::Review && !output.critique_reviews.is_empty() {
            notes.push("저자 측 출력의 비판 판정은 적용하지 않음".into());
            output.critique_reviews.clear();
        }
        if step.phase != Phase::Revise && !output.critique_responses.is_empty() {
            output.critique_responses.clear();
        }
        let critiques: HashSet<String> =
            self.run().critiques.iter().map(|c| c.id.clone()).collect();
        let ranges = transmitted_ranges(context);
        notes.extend(output.sanitize(&Allowed {
            sources: &sources,
            ranges: &ranges,
            existing: &existing,
            critiques: &critiques,
        }));
        output.validate(&sources, &existing)?;
        Ok(output)
    }
    /// Apply responses and reviews to the critique ledger, then register new critiques.
    fn register(&mut self, step: &Step, output: &Envelope) {
        let provider = step.provider.label().to_string();
        let run = self.run_mut();
        if step.phase == Phase::Revise {
            for r in &output.critique_responses {
                if let Some(c) = run.critiques.iter_mut().find(|c| c.id == r.critique_id) {
                    c.events.push(record::<CritiqueEvent>(|e| {
                        e.round = step.round;
                        e.provider = provider.clone();
                        e.stance = Some(r.stance);
                        e.reasoning = r.reasoning.clone();
                        e.revised_ids = r.revised_ids.clone();
                    }));
                }
            }
        }
        if step.phase == Phase::Review {
            for r in &output.critique_reviews {
                if let Some(c) = run.critiques.iter_mut().find(|c| c.id == r.critique_id) {
                    c.status = r.status;
                    c.events.push(record::<CritiqueEvent>(|e| {
                        e.round = step.round;
                        e.provider = provider.clone();
                        e.status = Some(r.status);
                        e.reasoning = r.reasoning.clone();
                    }));
                }
            }
            for critique in &output.critiques {
                let n = run.critiques.len() + 1;
                run.critiques.push(record::<CritiqueRecord>(|c| {
                    c.id = format!("K{n}");
                    c.round = step.round;
                    c.task_id = step.task_id.into();
                    c.provider = provider.clone();
                    c.critique = critique.clone();
                    c.status = CritiqueStatus::Open;
                }));
            }
        }
    }
    /// Run one step to a validated output. `Ok(None)` means this step failed but the run can
    /// continue; errors that make every later call futile are returned.
    fn step(&mut self, step: Step, task: &TaskSpec, context: Value) -> Result<Option<Envelope>> {
        if step.provider == self.critic
            && self.critic != self.author
            && let Some(reason) = self.critic_down.clone()
        {
            self.block(task, step, format!("비판 모델 사용 불가: {reason}"))?;
            return Ok(None);
        }
        let raw = match self.call(step, context.clone(), schema::<Envelope>()) {
            Ok(raw) => raw,
            Err(e) => {
                let message = e.to_string();
                if message == "cancelled" || message == "budget_exhausted" {
                    return Err(e);
                }
                if message == CALL_BUDGET || message == CONTEXT_LIMIT {
                    // Nothing was sent; record why this step has no output.
                    self.block(task, step, message)?;
                    return Ok(None);
                }
                if is_fatal(&message) {
                    if step.provider == self.author {
                        return Err(e);
                    }
                    self.critic_down = Some(message.clone());
                    self.run_mut().limitations.push(format!(
                        "비판 모델({}) 사용 불가로 토론을 멈춤: {message}",
                        step.provider.display()
                    ));
                }
                return Ok(None);
            }
        };
        let mut notes = vec![];
        let outcome = self.accept(&step, task, &context, raw, &mut notes);
        self.finish_attempt(&outcome, notes)?;
        match outcome {
            Ok(output) => {
                self.register(&step, &output);
                self.store.save("검증된 산출물 저장")?;
                self.emit(format!("{} 완료", step.describe()));
                Ok(Some(output))
            }
            Err(_) => Ok(None),
        }
    }
    fn block(&mut self, task: &TaskSpec, step: Step, reason: String) -> Result<()> {
        self.run_mut().attempts.push(record::<Attempt>(|a| {
            a.id = id("attempt");
            a.task_id = task.id.clone();
            a.role = Some(task.kind);
            a.phase = Some(step.phase);
            a.round = step.round;
            a.provider = step.provider.label().into();
            a.state = TaskState::Blocked;
            a.error = Some(reason);
        }));
        self.store.save("선행 작업 실패로 차단")
    }
    fn plan(&mut self) -> Result<Plan> {
        let step = Step {
            task_id: "planner",
            role: None,
            phase: Phase::Plan,
            round: 0,
            provider: self.author,
            reserve: 2,
        };
        let source_ids: HashSet<_> = self
            .store
            .project
            .sources
            .iter()
            .filter(|s| s.active)
            .map(|s| s.id.clone())
            .collect();
        let ledger_ids: HashSet<_> = ledger_entries(
            &self.store.project,
            &self.run().targets,
            &self.run().question,
        )
        .iter()
        .map(|e| e.proposal.id.clone())
        .collect();
        let parse = |v: Value, left: u32| -> Result<Plan> {
            let plan: Plan = serde_json::from_value(v)?;
            plan.validate(left, &source_ids, &ledger_ids)?;
            Ok(plan)
        };
        let context = planner_context(
            &self.store.project,
            self.run(),
            self.settings.search_enabled,
        );
        let remaining = self.remaining().saturating_sub(1);
        let parsed = match self.call(step, context, schema::<Plan>()) {
            Ok(raw) => {
                let parsed = parse(raw.clone(), remaining);
                self.mark_plan(&parsed)?;
                if let Err(e) = &parsed
                    && remaining >= 4
                {
                    let context = json!({"original":planner_context(&self.store.project,self.run(),self.settings.search_enabled),"invalid_plan":raw,"validation_error":e.to_string(),"max_tasks":remaining-1,"instruction":"Repair once. Return a valid minimal plan under the remaining call budget."});
                    let repair = Step {
                        task_id: "planner-repair",
                        ..step
                    };
                    match self.call(repair, context, schema::<Plan>()) {
                        Ok(raw) => {
                            let again = parse(raw, remaining - 1);
                            self.mark_plan(&again)?;
                            again
                        }
                        Err(e) if is_fatal(&e.to_string()) => return Err(e),
                        Err(e) => Err(e),
                    }
                } else {
                    parsed
                }
            }
            Err(e) if is_fatal(&e.to_string()) => return Err(e),
            Err(e) => Err(e),
        };
        if self.remaining() < 3 && parsed.is_err() {
            bail!("budget_exhausted");
        }
        let mut plan = match parsed {
            Ok(plan) => plan,
            Err(e) => {
                let r = self.run_mut();
                r.limitations
                    .push(format!("계획 검증 실패로 보수적 recipe 사용: {e}"));
                Plan::fallback(&r.question, source_ids.iter().cloned().collect())
            }
        };
        if self.settings.search_enabled && !plan.search_queries.is_empty() {
            self.run_mut().plan = Some(plan.clone());
            self.store.save("문헌 검색 계획 저장")?;
            let project = &self.store.project;
            let notify = &mut self.notify;
            let found = crate::search::retrieve(
                &plan.search_queries,
                self.cancel,
                self.deadline,
                &mut |s| notify(project, s),
            )?;
            let mut retrieved = vec![];
            for source in found.sources {
                if let Some(existing) = self.store.project.sources.iter().find(|s| {
                    s.active
                        && s.locator == source.locator
                        && s.hash == source.hash
                        && s.access == source.access
                }) {
                    retrieved.push(existing.id.clone());
                } else {
                    retrieved.push(source.id.clone());
                    self.store.project.sources.push(source);
                }
            }
            for t in &mut plan.tasks {
                for id in &retrieved {
                    if !t.source_ids.contains(id) {
                        t.source_ids.push(id.clone());
                    }
                }
            }
            self.run_mut().limitations.extend(found.warnings);
            self.store.save("검색 원문과 출처 범위 저장")?;
            self.emit(format!("문헌 검색 완료 · 자료 {}개 확보", retrieved.len()));
        }
        Ok(plan)
    }
    fn mark_plan(&mut self, parsed: &Result<Plan>) -> Result<()> {
        let a = self.run_mut().attempts.last_mut().unwrap();
        match parsed {
            Ok(_) => {
                a.state = TaskState::Succeeded;
                a.error = None;
            }
            Err(e) => {
                a.state = TaskState::Failed;
                a.error = Some(e.to_string());
            }
        }
        self.store.save("계획 구조 검증")
    }
    fn initial_task(&mut self, task: &TaskSpec) -> Result<()> {
        let review = matches!(task.kind, Role::Critic | Role::EvidenceReviewer);
        let step = Step {
            task_id: &task.id,
            role: Some(task.kind),
            phase: if review {
                Phase::Review
            } else {
                Phase::Construct
            },
            round: 0,
            provider: if task.kind == Role::Critic {
                self.critic
            } else {
                self.author
            },
            reserve: 1,
        };
        let run = self.run();
        let missing = task.depends_on.iter().any(|id| {
            !run.attempts
                .iter()
                .any(|a| a.task_id == *id && a.state == TaskState::Succeeded)
        });
        if missing {
            return self.block(task, step, "필수 선행 작업 실패".into());
        }
        let context = task_context(&self.store.project, self.run(), task);
        self.step(step, task, context)?;
        Ok(())
    }
    fn round_task(&self, plan: &Plan, id: String, kind: Role, objective: &str) -> TaskSpec {
        let mut task = TaskSpec::new(&id, kind, objective);
        for t in &plan.tasks {
            for s in &t.source_ids {
                if !task.source_ids.contains(s) {
                    task.source_ids.push(s.clone());
                }
            }
        }
        task
    }
    fn review(&mut self, plan: &Plan, round: u32) -> Result<bool> {
        let task = self.round_task(
            plan,
            format!("review-r{round}"),
            Role::Critic,
            "작업 이론을 독립적으로 비판 검토하고 이전 비판에 대한 저자 응답을 판정한다.",
        );
        let focus: Vec<String> = self
            .run()
            .critiques
            .iter()
            .filter(|c| !c.status.is_settled())
            .map(|c| c.id.clone())
            .collect();
        let context = deliberation_context(
            &self.store.project,
            self.run(),
            &task,
            Phase::Review,
            round,
            &focus,
        );
        let step = Step {
            task_id: &task.id,
            role: Some(Role::Critic),
            phase: Phase::Review,
            round,
            provider: self.critic,
            reserve: 1,
        };
        Ok(self.step(step, &task, context)?.is_some())
    }
    fn revise(&mut self, plan: &Plan, round: u32, focus: &[String]) -> Result<bool> {
        let task = self.round_task(
            plan,
            format!("revise-r{round}"),
            Role::Constructor,
            "비판 하나하나에 응답하고, 수용한 부분은 작업 이론을 수정한다.",
        );
        let context = deliberation_context(
            &self.store.project,
            self.run(),
            &task,
            Phase::Revise,
            round,
            focus,
        );
        let step = Step {
            task_id: &task.id,
            role: Some(Role::Constructor),
            phase: Phase::Revise,
            round,
            provider: self.author,
            reserve: 2,
        };
        Ok(self.step(step, &task, context)?.is_some())
    }
    /// Observed time per model call in this run (at least a minute before any data).
    fn per_call(&self) -> Duration {
        let run = self.run();
        let elapsed = now().saturating_sub(run.started);
        Duration::from_secs((elapsed / u64::from(run.calls.max(1))).max(60))
    }
    fn reviewed(&self) -> bool {
        self.run()
            .attempts
            .iter()
            .any(|a| a.phase == Some(Phase::Review) && a.state == TaskState::Succeeded)
    }
    fn stop(&mut self, reason: StopReason) -> Result<()> {
        self.run_mut().deliberation.stop = Some(reason);
        self.store.save("토론 종료 사유 기록")
    }
    /// The recursive author ↔ critic exchange.
    fn deliberate(&mut self, plan: &Plan) -> Result<()> {
        let max = self.run().budget.rounds();
        self.run_mut().deliberation.max_rounds = max;
        let drafted = !self.run().theory().is_empty()
            || self
                .run()
                .attempts
                .iter()
                .any(|a| a.state == TaskState::Succeeded && a.phase == Some(Phase::Construct));
        if !self.reviewed() && drafted && self.critic_down.is_none() && self.remaining() >= 2 {
            self.review(plan, 0)?;
        }
        loop {
            if self.critic_down.is_some() {
                return self.stop(StopReason::Failed);
            }
            if !self.reviewed() {
                return self.stop(StopReason::Failed);
            }
            let focus: Vec<String> = self
                .run()
                .critiques
                .iter()
                .filter(|c| c.actionable())
                .map(|c| c.id.clone())
                .collect();
            if focus.is_empty() {
                let unsettled = self
                    .run()
                    .critiques
                    .iter()
                    .any(|c| c.material() && !c.status.is_settled());
                return self.stop(if unsettled {
                    StopReason::Stalled
                } else {
                    StopReason::Converged
                });
            }
            let done = self.run().deliberation.rounds;
            if done >= max {
                return self.stop(StopReason::RoundLimit);
            }
            if self.remaining() < 3 {
                return self.stop(StopReason::Budget);
            }
            // A round is revise + review, and synthesis must still fit afterwards.
            if self.deadline.saturating_duration_since(Instant::now()) < self.per_call() * 3 {
                return self.stop(StopReason::Budget);
            }
            let round = done + 1;
            let statuses = |r: &Run| -> Vec<(String, CritiqueStatus)> {
                r.critiques
                    .iter()
                    .map(|c| (c.id.clone(), c.status))
                    .collect()
            };
            let before = statuses(self.run());
            let theory_before: HashSet<String> =
                self.run().theory().into_iter().map(|p| p.id).collect();
            if !self.revise(plan, round, &focus)? {
                return self.stop(StopReason::Failed);
            }
            if !self.review(plan, round)? {
                self.run_mut().deliberation.rounds = round;
                return self.stop(StopReason::Failed);
            }
            self.run_mut().deliberation.rounds = round;
            self.store.save("토론 회차 완료")?;
            let theory_after: HashSet<String> =
                self.run().theory().into_iter().map(|p| p.id).collect();
            let after = statuses(self.run());
            let unchanged_statuses = after.iter().take(before.len()).eq(before.iter());
            let new_critiques = after.len() > before.len();
            if unchanged_statuses && !new_critiques && theory_after == theory_before {
                return self.stop(StopReason::Stalled);
            }
        }
    }
    fn synthesize(&mut self, task: &TaskSpec) -> Result<()> {
        let step = Step {
            task_id: &task.id,
            role: Some(Role::Synthesizer),
            phase: Phase::Synthesis,
            round: self.run().deliberation.rounds,
            provider: self.author,
            reserve: 0,
        };
        let context = synthesis_context(&self.store.project, self.run(), task);
        if let Some(output) = self.step(step, task, context)? {
            let r = self.run_mut();
            r.report = render_answer(&output);
            r.state = if output.evidence_insufficient {
                RunState::EvidenceInsufficient
            } else {
                RunState::Answered
            };
            self.store.save("최종 답변 저장")?;
        }
        Ok(())
    }
    fn execute(&mut self) -> Result<()> {
        let plan = self.plan()?;
        let source_ids: HashSet<_> = self
            .store
            .project
            .sources
            .iter()
            .filter(|s| s.active)
            .map(|s| s.id.clone())
            .collect();
        let ledger_ids: HashSet<_> = ledger_entries(
            &self.store.project,
            &self.run().targets,
            &self.run().question,
        )
        .iter()
        .map(|e| e.proposal.id.clone())
        .collect();
        let order = plan.validate(self.remaining(), &source_ids, &ledger_ids)?;
        self.run_mut().plan = Some(plan.clone());
        self.store.save("검증된 계획 고정")?;
        self.emit(plan.scope.clone());
        let (last, initial) = order.split_last().expect("validated plan has tasks");
        for ix in initial {
            self.initial_task(&plan.tasks[*ix])?;
        }
        if self.cancel.load(Ordering::SeqCst) {
            bail!("cancelled");
        }
        self.deliberate(&plan)?;
        self.synthesize(&plan.tasks[*last])
    }
}
pub struct RunInput {
    question: String,
    targets: Vec<String>,
    budget: Budget,
}
impl RunInput {
    pub fn new(question: impl Into<String>, budget: Budget) -> Self {
        Self {
            question: question.into(),
            targets: vec![],
            budget,
        }
    }
    pub fn with_targets(mut self, targets: Vec<String>) -> Self {
        self.targets = targets;
        self
    }
}
pub fn run(
    store: &mut Store,
    input: RunInput,
    settings: &Settings,
    provider: &dyn ModelProvider,
    cancel: &Cancel,
    notify: &mut dyn FnMut(&Project, String),
) -> Result<()> {
    let RunInput {
        question,
        targets,
        budget,
    } = input;
    ensure!(!question.trim().is_empty(), "질문을 입력하세요");
    ensure!(question.len() < 30000, "질문이 너무 깁니다");
    ensure!(
        targets
            .iter()
            .all(|id| store.project.entries.iter().any(|e| e.proposal.id == *id)),
        "질문 대상이 없습니다"
    );
    let author = settings.primary;
    let critic = if settings.cross_review {
        author.other()
    } else {
        author
    };
    let base_revision = store.project.revision;
    store.project.runs.push(record::<Run>(|r| {
        r.id = id("run");
        r.question = question;
        r.targets = targets;
        r.base_revision = base_revision;
        r.budget = budget;
        r.started = now();
        r.profile_versions = store
            .project
            .profiles
            .iter()
            .map(|p| format!("{p}/v1"))
            .collect();
        r.profile_versions.push(format!(
            "project-guidance/{}",
            hash(
                format!(
                    "{}\n{}",
                    store.project.research_goal,
                    store.project.guidance()
                )
                .as_bytes()
            )
        ));
        r.deliberation = record::<Deliberation>(|d| {
            d.author = author.label().into();
            d.critic = critic.label().into();
            d.max_rounds = budget.rounds();
        });
        r.limitations = vec![
            if author != critic {
                "구성과 비판에 서로 다른 구독 모델 사용; 두 모델의 합의는 경험적 근거가 아님"
            } else {
                "동일 모델의 별도 문맥 검토; 독립적인 경험적 검증이 아님"
            }
            .into(),
        ];
    }));
    let run_ix = store.project.runs.len() - 1;
    store.save("새 질문 실행")?;
    let mut executor = RunExecutor {
        store,
        provider,
        settings,
        cancel,
        notify,
        deadline: Instant::now() + Duration::from_secs(budget.seconds()),
        run_ix,
        author,
        critic,
        critic_down: None,
    };
    let result = executor.execute();
    let p = &mut executor.store.project;
    let r = &mut p.runs[run_ix];
    if let Err(e) = result {
        let message = e.to_string();
        r.state = if cancel.load(Ordering::SeqCst) || message == "cancelled" {
            RunState::Cancelled
        } else if Instant::now() >= executor.deadline || message == "budget_exhausted" {
            RunState::BudgetExhausted
        } else {
            RunState::Failed
        };
        if r.deliberation.stop.is_none() && r.state == RunState::Cancelled {
            r.deliberation.stop = Some(StopReason::Cancelled);
        }
        r.limitations.push(message);
    }
    let initial_failed = r.attempts.iter().any(|a| {
        a.role.is_some()
            && a.round == 0
            && a.phase != Some(Phase::Synthesis)
            && matches!(a.state, TaskState::Failed | TaskState::Blocked)
            && !r
                .attempts
                .iter()
                .any(|b| b.task_id == a.task_id && b.state == TaskState::Succeeded)
    });
    let debate_failed = r.deliberation.stop == Some(StopReason::Failed);
    if matches!(r.state, RunState::Answered | RunState::EvidenceInsufficient)
        && (initial_failed || debate_failed)
    {
        r.state = RunState::Partial;
        r.limitations.push(
            "일부 분석·비판 작업이 실패해 토론이 완결되지 않았습니다. 종합 답변은 부분 결과입니다."
                .into(),
        );
    }
    if r.state == RunState::Running {
        r.state = RunState::Failed;
        r.limitations
            .push("종합 작업이 검증을 통과하지 못했습니다".into());
    }
    if r.report.is_empty() {
        r.report = fallback_report(r);
    }
    if let Some(stop) = r.deliberation.stop {
        r.limitations.push(format!(
            "토론: {} → {} · {}회 수정 · {}",
            ProviderKind::from_label(&r.deliberation.author).display(),
            ProviderKind::from_label(&r.deliberation.critic).display(),
            r.deliberation.rounds,
            stop.label()
        ));
    }
    r.finished = Some(now());
    // The user-facing ledger receives only the latest validated proposals, never adoptions.
    let conflict = p.revision != base_revision;
    if !cancel.load(Ordering::SeqCst) {
        let last_review = r
            .attempts
            .iter()
            .rposition(|a| a.phase == Some(Phase::Review) && a.state == TaskState::Succeeded);
        let origin: HashMap<&str, (usize, &str)> = r
            .attempts
            .iter()
            .enumerate()
            .filter_map(|(i, a)| a.output.as_ref().map(|o| (i, a, o)))
            .flat_map(|(i, a, o)| {
                o.proposed_changes
                    .iter()
                    .map(move |p| (p.id.as_str(), (i, a.task_id.as_str())))
            })
            .collect();
        let disputed: HashSet<&str> = r
            .critiques
            .iter()
            .filter(|c| c.material() && !c.status.is_settled())
            .map(|c| c.critique.target_id.as_str())
            .collect();
        let mut ids: HashSet<_> = p.entries.iter().map(|e| e.proposal.id.clone()).collect();
        for proposal in r.theory() {
            if !ids.insert(proposal.id.clone()) {
                continue;
            }
            let (at, task_id) = origin
                .get(proposal.id.as_str())
                .copied()
                .unwrap_or((usize::MAX, ""));
            let review = if conflict {
                Review::NeedsReview
            } else if disputed.contains(proposal.id.as_str()) {
                Review::Contested
            } else if last_review.is_some_and(|l| at < l) {
                Review::Reviewed
            } else {
                Review::Unreviewed
            };
            p.entries.push(record::<Entry>(|e| {
                e.proposal = proposal.clone();
                e.adoption = Adoption::Proposed;
                e.review = review;
                e.revision = p.revision + 1;
                e.run_id = r.id.clone();
                e.task_id = task_id.into();
            }));
        }
    }
    if conflict {
        r.limitations.push(
            "기준 판본 이후 편집 감지: 채택 상태를 변경하지 않고 재검토 제안으로 보존".into(),
        );
    }
    p.checkpoint("질문 실행 산출물 — 채택 전 제안");
    p.runs[run_ix].revision = p.revision;
    executor.store.save("실행 종료 및 원장 제안 기록")?;
    executor.emit(executor.store.project.runs[run_ix].state.label().into());
    Ok(())
}
fn render_answer(o: &Envelope) -> String {
    let mut s = o.summary.clone();
    if !o.unresolved.is_empty() {
        s.push_str("\n\n### 미해결 사항\n");
        for u in &o.unresolved {
            s.push_str(&format!("\n- {u}"));
        }
    }
    if !o.next_actions.is_empty() {
        s.push_str("\n\n### 다음 확인\n");
        for a in &o.next_actions {
            s.push_str(&format!("\n- {a}"));
        }
    }
    s
}
/// When synthesis did not complete, assemble what was validated — never invent an answer.
fn fallback_report(r: &Run) -> String {
    let mut s = format!(
        "**{}** — 최종 종합이 완료되지 않아 검증을 통과한 중간 결과만 정리했습니다.",
        r.state.label()
    );
    if let Some((a, o)) = r
        .attempts
        .iter()
        .rev()
        .filter(|a| a.state == TaskState::Succeeded && a.phase != Some(Phase::Plan))
        .find_map(|a| a.output.as_ref().map(|o| (a, o)))
    {
        s.push_str(&format!(
            "\n\n### 마지막 검증 단계 · {}\n\n{}",
            ProviderKind::from_label(&a.provider).display(),
            o.summary
        ));
    }
    let theory = r.theory();
    if !theory.is_empty() {
        s.push_str("\n\n### 작업 이론 (채택 전 제안)\n");
        for p in &theory {
            s.push_str(&format!("\n- {}", p.statement));
        }
    }
    let open: Vec<_> = r
        .critiques
        .iter()
        .filter(|c| !c.status.is_settled())
        .collect();
    if !open.is_empty() {
        s.push_str("\n\n### 남은 비판\n");
        for c in open {
            s.push_str(&format!(
                "\n- [{}] {}",
                c.status.label(),
                c.critique.reasoning
            ));
        }
    }
    s
}
