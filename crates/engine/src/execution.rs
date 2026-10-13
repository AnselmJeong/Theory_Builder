use crate::{
    providers::{Cancel, ModelProvider, ProviderKind, Request, Settings},
    storage::Store,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use theory_core::*;
mod citations;

pub const GENERAL: &str = include_str!("../../../resources/profiles/general-research.json");
pub const NEURO: &str = include_str!("../../../resources/profiles/neurobiology.json");
pub const DYNAMICS: &str = include_str!("../../../resources/profiles/multilevel-dynamics.json");
fn profiles(p: &Project) -> Vec<Value> {
    vec![
        serde_json::from_str(GENERAL).unwrap(),
        json!({"id":"project-research-guidance","goal":p.research_goal,"guidance":p.guidance(),"selection_rule":"Treat these as project preferences, not a fixed checklist. Select only perspectives relevant to the current question, add needed perspectives, and explain the selection in the plan. Never assume a disease or a discipline not requested."}),
    ]
}
fn source_context(s: &Source, query: &str) -> Value {
    // A bounded, contiguous passage selected locally; real line numbers are kept.
    // Lexical retrieval is explicit, not a claim that the whole book was reviewed.
    let all: Vec<_> = s.text.lines().collect();
    let terms: HashSet<_> = query
        .split(|c: char| !c.is_alphanumeric())
        .chain(query.split(|c: char| !c.is_ascii_alphanumeric()))
        .filter(|t| t.chars().count() >= 4)
        .map(str::to_lowercase)
        .collect();
    let best = all
        .chunks(40)
        .enumerate()
        .map(|(i, chunk)| {
            let text = chunk.join(" ").to_lowercase();
            (
                i * 40,
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
            bytes <= 12000
        })
        .collect();
    json!({"id":s.id,"title":s.title,"hash":s.hash,"revision":s.revision,"access":s.access,"locator":s.locator,"included_lines":[start+1,start+lines.len()],"omitted_lines":all.len().saturating_sub(lines.len()),"selection":"bounded lexical passage; not a complete source review","text":lines.iter().enumerate().map(|(i,l)|format!("{}: {}",start+i+1,l)).collect::<Vec<_>>().join("\n")})
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
fn ancestors(run: &Run, task: &TaskSpec) -> HashSet<String> {
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
/// Fresh specialist context. Only explicitly selected source versions and dependency outputs enter it.
pub fn task_context(p: &Project, run: &Run, task: &TaskSpec) -> Value {
    let selected = selected_entries(p, &run.targets);
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
    let sources: Vec<_> = p
        .sources
        .iter()
        .filter(|s| s.active && task.source_ids.contains(&s.id))
        .map(|s| source_context(s, &run.question))
        .collect();
    json!({"question":run.question,"targets":run.targets,"base_revision":run.base_revision,"profiles":profiles(p),"task":task,"ledger":selected,"sources":sources,"dependency_outputs":prior,"contract_version":"research/v1","output_id_prefix":format!("{}_{}",&run.id[..12],task.id),"rules":"New proposal IDs MUST start with output_id_prefix. Existing entries are read-only; revisions are new proposals. All critiques remain open proposals. Include no model attribution or other reviewers' outputs when constructing independent candidates. If task dependencies are missing, state the gap. Numerical execution unavailable. Literature retrieval has already been performed if requested in the plan; only the supplied source text is evidence."})
}
fn planner_context(p: &Project, r: &Run, search_enabled: bool) -> Value {
    json!({"search_enabled":search_enabled,"search_instructions":"When search_enabled, generate up to 2 focused English literature search_queries (3-6 core terms each; avoid stacking every possible mechanism into one AND query) if external evidence is needed. Search is available through PubMed and TinyFish before task execution; retrieved sources are automatically attached to all tasks. For a research/theory question with no usable local sources, search is normally required. Use empty search_queries only when external evidence is unnecessary or the user forbids search. Never assume sources were checked before retrieval.","question":r.question,"target_ids":r.targets,"base_revision":r.base_revision,"profiles":profiles(p),"ledger":selected_entries(p,&r.targets),"sources":p.sources.iter().filter(|s| s.active).map(|s|json!({"id":s.id,"title":s.title,"access":s.access,"lines":s.text.lines().count()})).collect::<Vec<_>>(),"max_tasks":r.budget.calls()-r.calls-1,"rules":"Propose a minimal DAG for this question. Include a critic or evidence reviewer and finish with a synthesizer depending transitively on every task. Use only supplied source IDs and ledger IDs. No allowed_tools (empty arrays). base_only=true requires no dependencies. Independent constructors must be base_only=true and receive the same base. Concept: analyst->critic->synthesizer. Evidence: evidence_reviewer->critic->synthesizer. Compare: two independent analysts->critic->synthesizer. Mechanism: two independent constructors->critic->synthesizer. Formalization: formalizer->critic->synthesizer; no numerical execution. Observation design: analyst->critic->synthesizer. Revision: only affected targets and preserve user constraints. Documentation: analyst->reconstructor->synthesizer plus review. Do not insert clinical critique unless the question asks clinical scope. No disease assumptions from profiles. Scope and requirements must cover all user constraints."})
}
struct RunExecutor<'a> {
    store: &'a mut Store,
    provider: &'a dyn ModelProvider,
    settings: &'a Settings,
    cancel: &'a Cancel,
    notify: &'a mut dyn FnMut(&Project, String),
    deadline: Instant,
    run_ix: usize,
}
impl RunExecutor<'_> {
    fn emit(&mut self, message: String) {
        (self.notify)(&self.store.project, message);
    }
    fn call(
        &mut self,
        task_id: &str,
        role: Option<Role>,
        context: Value,
        output_schema: Value,
        provider: ProviderKind,
    ) -> Result<Value> {
        ensure!(!self.cancel.load(Ordering::SeqCst), "cancelled");
        ensure!(Instant::now() < self.deadline, "budget_exhausted");
        let run = &mut self.store.project.runs[self.run_ix];
        ensure!(run.calls < run.budget.calls(), "budget_exhausted");
        let prompt = serde_json::to_string(&context)?;
        ensure!(
            prompt.len() <= 240_000,
            "선택 자료가 문맥 한도를 넘었습니다. 출처 또는 대상을 줄이세요."
        );
        run.calls += 1;
        run.attempts.push(record::<Attempt>(|a| {
            a.id = id("attempt");
            a.task_id = task_id.into();
            a.role = role;
            a.state = TaskState::Running;
            a.provider = provider.label().into();
            a.model = self.settings.model(provider).into();
            a.context = context;
            a.started = now();
        }));
        let attempt_ix = run.attempts.len() - 1;
        self.store.save("호출 예산 예약 및 요청 기록")?;
        self.emit(format!(
            "{} · 호출 {}/{}",
            role.map(|r| r.label()).unwrap_or("계획 구성"),
            self.store.project.runs[self.run_ix].calls,
            self.store.project.runs[self.run_ix].budget.calls()
        ));
        let request = Request::new(
            prompt,
            output_schema,
            self.settings.model(provider).into(),
            self.deadline.min(Instant::now() + Duration::from_secs(600)),
        );
        let project = &self.store.project;
        let notify = &mut self.notify;
        let result = self
            .provider
            .generate(provider, &request, self.cancel, &mut |status| {
                notify(project, status)
            });
        if self.cancel.load(Ordering::SeqCst) {
            let a = &mut self.store.project.runs[self.run_ix].attempts[attempt_ix];
            a.state = TaskState::Cancelled;
            a.error = Some("취소 뒤 도착한 출력은 적용하지 않음; 원격 상태 확인 불가".into());
            self.store.save("호출 취소")?;
            anyhow::bail!("cancelled");
        }
        match result {
            Ok(response) => {
                let a = &mut self.store.project.runs[self.run_ix].attempts[attempt_ix];
                a.model = response.model;
                a.usage = response.usage;
                a.response = Some(response.value.clone());
                a.state = TaskState::Validating;
                self.store.save("응답 수신; 검증 전 원장 미적용")?;
                Ok(response.value)
            }
            Err(e) => {
                let a = &mut self.store.project.runs[self.run_ix].attempts[attempt_ix];
                a.state = TaskState::Failed;
                a.error = Some(e.to_string());
                self.store.save("호출 실패; 자동 재전송 없음")?;
                Err(e)
            }
        }
    }
    fn mark_validation(&mut self, error: Option<String>) -> Result<()> {
        let a = self.store.project.runs[self.run_ix]
            .attempts
            .last_mut()
            .unwrap();
        a.state = if error.is_some() {
            TaskState::Failed
        } else {
            TaskState::Succeeded
        };
        a.error = error;
        self.store.save("출력 구조 검증")
    }
    fn execute(&mut self) -> Result<()> {
        let p = &self.store.project;
        let r = &p.runs[self.run_ix];
        let context = planner_context(p, r, self.settings.search_enabled);
        let raw = self.call(
            "planner",
            None,
            context,
            schema::<Plan>(),
            self.settings.primary,
        )?;
        let source_ids: HashSet<_> = self
            .store
            .project
            .sources
            .iter()
            .filter(|s| s.active)
            .map(|s| s.id.clone())
            .collect();
        let ledger_ids: HashSet<_> = selected_entries(
            &self.store.project,
            &self.store.project.runs[self.run_ix].targets,
        )
        .iter()
        .map(|e| e.proposal.id.clone())
        .collect();
        let parse = |v: Value, left: u32| -> Result<Plan> {
            let plan: Plan = serde_json::from_value(v)?;
            plan.validate(left, &source_ids, &ledger_ids)?;
            Ok(plan)
        };
        let remaining = self.store.project.runs[self.run_ix].budget.calls()
            - self.store.project.runs[self.run_ix].calls;
        let mut parsed = parse(raw.clone(), remaining);
        self.mark_validation(parsed.as_ref().err().map(ToString::to_string))?;
        if let Err(e) = &parsed
            && remaining >= 4
        {
            let r = &self.store.project.runs[self.run_ix];
            let context = json!({"original":planner_context(&self.store.project,r,self.settings.search_enabled),"invalid_plan":raw,"validation_error":e.to_string(),"max_tasks":remaining-1,"instruction":"Repair once. Return a valid minimal plan under the remaining call budget."});
            let repair = self.call(
                "planner-repair",
                None,
                context,
                schema::<Plan>(),
                self.settings.primary,
            )?;
            parsed = parse(repair, remaining - 1);
            self.mark_validation(parsed.as_ref().err().map(ToString::to_string))?;
        }
        let mut plan = parsed.unwrap_or_else(|e| {
            let r = &mut self.store.project.runs[self.run_ix];
            r.limitations
                .push(format!("계획 검증 실패로 보수적 recipe 사용: {e}"));
            Plan::fallback(&r.question, source_ids.iter().cloned().collect())
        });
        if self.settings.search_enabled && !plan.search_queries.is_empty() {
            self.store.project.runs[self.run_ix].plan = Some(plan.clone());
            self.store.save("문헌 검색 계획 저장")?;
            let mut statuses = vec![];
            let project = &self.store.project;
            let notify = &mut self.notify;
            let found = crate::search::retrieve(
                &plan.search_queries,
                self.cancel,
                self.deadline,
                &mut |s| {
                    notify(project, s.clone());
                    statuses.push(s);
                },
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
            self.store.project.runs[self.run_ix]
                .limitations
                .extend(found.warnings);
            self.store.save("검색 원문과 출처 범위 저장")?;
            self.emit(format!("문헌 검색 완료 · 자료 {}개 확보", retrieved.len()));
        }
        let source_ids: HashSet<_> = self
            .store
            .project
            .sources
            .iter()
            .filter(|s| s.active)
            .map(|s| s.id.clone())
            .collect();
        let run = &mut self.store.project.runs[self.run_ix];
        let order = plan.validate(run.budget.calls() - run.calls, &source_ids, &ledger_ids)?;
        run.plan = Some(plan.clone());
        self.store.save("검증된 계획 고정")?;
        self.emit(plan.scope.clone());
        for ix in order {
            let task = &plan.tasks[ix];
            let r = &self.store.project.runs[self.run_ix];
            let missing = task.depends_on.iter().any(|id| {
                !r.attempts
                    .iter()
                    .any(|a| a.task_id == *id && a.state == TaskState::Succeeded)
            });
            if missing && task.kind != Role::Synthesizer {
                self.store.project.runs[self.run_ix]
                    .attempts
                    .push(record::<Attempt>(|a| {
                        a.id = id("attempt");
                        a.task_id = task.id.clone();
                        a.role = Some(task.kind);
                        a.state = TaskState::Blocked;
                        a.error = Some("필수 선행 작업 실패".into());
                    }));
                self.store.save("선행 작업 실패로 차단")?;
                continue;
            }
            let context = task_context(&self.store.project, r, task);
            let provider = if self.settings.cross_review && task.kind == Role::Critic {
                match self.settings.primary {
                    ProviderKind::Codex => ProviderKind::Claude,
                    ProviderKind::Claude => ProviderKind::Codex,
                }
            } else {
                self.settings.primary
            };
            let raw = self.call(
                &task.id,
                Some(task.kind),
                context.clone(),
                schema::<Envelope>(),
                provider,
            )?;
            let parsed: Result<Envelope> = (|| {
                let mut output: Envelope = serde_json::from_value(raw)?;
                let prefix = context["output_id_prefix"].as_str().unwrap();
                ensure!(
                    output
                        .proposed_changes
                        .iter()
                        .all(|p| p.id.starts_with(prefix)),
                    "제안 ID가 작업 namespace 밖에 있습니다"
                );
                let mut existing = ledger_ids.clone();
                let dependencies = ancestors(&self.store.project.runs[self.run_ix], task);
                for a in &self.store.project.runs[self.run_ix].attempts {
                    if dependencies.contains(&a.task_id)
                        && let Some(o) = &a.output
                    {
                        existing.extend(o.proposed_changes.iter().map(|p| p.id.clone()));
                    }
                }
                let sources: Vec<_> = self
                    .store
                    .project
                    .sources
                    .iter()
                    .filter(|s| s.active && task.source_ids.contains(&s.id))
                    .cloned()
                    .collect();
                let notes = citations::reconcile(&mut output, &sources, &context);
                self.store.project.runs[self.run_ix]
                    .attempts
                    .last_mut()
                    .unwrap()
                    .validation_notes = notes;
                output.validate(&sources, &existing)?;
                // A quote outside the transmitted range cannot be certified even if it happens to exist on disk.
                for e in &output.evidence_refs {
                    let source = context["sources"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|s| s["id"] == e.source_id)
                        .context("전송하지 않은 출처")?;
                    ensure!(
                        e.start_line as u64
                            >= source["included_lines"][0].as_u64().unwrap_or(u64::MAX)
                            && e.end_line as u64
                                <= source["included_lines"][1].as_u64().unwrap_or(0),
                        "전송하지 않은 원문 범위의 인용"
                    );
                }
                Ok(output)
            })();
            self.mark_validation(parsed.as_ref().err().map(ToString::to_string))?;
            if let Ok(output) = parsed {
                let r = &mut self.store.project.runs[self.run_ix];
                r.attempts.last_mut().unwrap().output = Some(output.clone());
                if task.kind == Role::Synthesizer {
                    r.report = render_answer(&output);
                    r.state = if output.evidence_insufficient {
                        RunState::EvidenceInsufficient
                    } else {
                        RunState::Answered
                    };
                }
                self.store.save("검증된 산출물 저장")?;
                self.emit(format!("{} 완료", task.kind.label()));
            }
        }
        Ok(())
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
        r.limitations = vec![
            if settings.cross_review {
                "비판 작업에 다른 구독 provider 사용; 합의는 경험적 근거가 아님"
            } else {
                "동일 provider의 별도 문맥 검토; 독립적인 경험적 검증이 아님"
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
        r.limitations.push(message);
    }
    if matches!(r.state, RunState::Answered | RunState::EvidenceInsufficient)
        && r.attempts
            .iter()
            .any(|a| a.role.is_some() && matches!(a.state, TaskState::Failed | TaskState::Blocked))
    {
        r.state = RunState::Partial;
        r.limitations
            .push("일부 분석 또는 검토 작업이 실패했습니다. 종합 답변은 부분 결과입니다.".into());
    }
    if r.state == RunState::Running {
        r.state = RunState::Failed;
        r.limitations
            .push("종합 작업이 검증을 통과하지 못했습니다".into());
    }
    if r.report.is_empty() {
        r.report = format!(
            "{}\n\n{}",
            r.state.label(),
            r.attempts
                .iter()
                .filter_map(|a| a.output.as_ref().map(|o| format!(
                    "### {}\n\n{}",
                    a.task_id,
                    render_answer(o)
                )))
                .collect::<Vec<_>>()
                .join("\n\n")
        );
    }
    r.finished = Some(now());
    // The user-facing ledger receives only validated proposals, never adopted changes.
    let conflict = p.revision != base_revision;
    if !cancel.load(Ordering::SeqCst) {
        let mut ids: HashSet<_> = p.entries.iter().map(|e| e.proposal.id.clone()).collect();
        for a in &r.attempts {
            if let Some(o) = &a.output {
                for proposal in &o.proposed_changes {
                    if ids.insert(proposal.id.clone()) {
                        p.entries.push(record::<Entry>(|e| {
                            e.proposal = proposal.clone();
                            e.adoption = Adoption::Proposed;
                            e.review = if conflict {
                                Review::NeedsReview
                            } else {
                                Review::Unreviewed
                            };
                            e.revision = p.revision + 1;
                            e.run_id = r.id.clone();
                            e.task_id = a.task_id.clone();
                        }));
                    }
                }
            }
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
