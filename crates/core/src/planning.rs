use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Budget {
    Quick,
    #[default]
    Standard,
    Deep,
}
impl Budget {
    pub fn calls(self) -> u32 {
        match self {
            Self::Quick => 4,
            Self::Standard => 12,
            Self::Deep => 24,
        }
    }
    /// Wall-clock limit. Debate rounds take two calls each, so these exceed the
    /// plan's first proposal (5/15/30 min); a round only starts if it can finish.
    pub fn seconds(self) -> u64 {
        match self {
            Self::Quick => 480,
            Self::Standard => 1500,
            Self::Deep => 3000,
        }
    }
    /// Maximum author-revision ↔ critic-review rounds after the first critique.
    pub fn rounds(self) -> u32 {
        match self {
            Self::Quick => 0,
            Self::Standard => 2,
            Self::Deep => 4,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Quick => "빠르게",
            Self::Standard => "표준",
            Self::Deep => "깊게",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Quick => "빠르게 · 4회",
            Self::Standard => "표준 · 12회",
            Self::Deep => "깊게 · 24회",
        }
    }
    pub fn detail(self) -> &'static str {
        match self {
            Self::Quick => "호출 4회 · 8분 · 비판 1회, 수정 없음",
            Self::Standard => "호출 12회 · 25분 · 비판↔수정 최대 2회",
            Self::Deep => "호출 24회 · 50분 · 비판↔수정 최대 4회",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    #[default]
    Analyst,
    Constructor,
    EvidenceReviewer,
    Critic,
    Formalizer,
    Synthesizer,
    Reconstructor,
}
impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Self::Analyst => "분석",
            Self::Constructor => "후보 구성",
            Self::EvidenceReviewer => "원문 대조",
            Self::Critic => "비판 검토",
            Self::Formalizer => "형식화",
            Self::Synthesizer => "답변 종합",
            Self::Reconstructor => "논증 재구성",
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct TaskSpec {
    pub id: String,
    pub kind: Role,
    pub objective: String,
    pub source_ids: Vec<String>,
    pub target_ids: Vec<String>,
    pub depends_on: Vec<String>,
    pub allowed_tools: Vec<String>,
    pub base_only: bool,
}
impl TaskSpec {
    pub fn new(id: &str, kind: Role, objective: &str) -> Self {
        Self {
            id: id.into(),
            kind,
            objective: objective.into(),
            ..Self::default()
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Plan {
    pub scope: String,
    pub requirements: Vec<String>,
    pub tasks: Vec<TaskSpec>,
    #[serde(default)]
    pub search_queries: Vec<String>,
}
impl Plan {
    pub fn validate(
        &self,
        remaining: u32,
        source_ids: &HashSet<String>,
        ledger_ids: &HashSet<String>,
    ) -> Result<Vec<usize>> {
        ensure!(
            self.search_queries.len() <= 2
                && self
                    .search_queries
                    .iter()
                    .all(|q| !q.trim().is_empty() && q.len() <= 500),
            "검색어는 500자 이내로 최대 2개입니다"
        );
        ensure!(
            !self.scope.trim().is_empty() && !self.requirements.is_empty(),
            "계획의 범위와 요구사항이 비어 있습니다"
        );
        ensure!(
            !self.tasks.is_empty()
                && self.tasks.len() <= 40
                && self.tasks.len() <= remaining as usize,
            "작업 수가 호출 예산을 초과합니다"
        );
        let mut ids = HashSet::new();
        for t in &self.tasks {
            ensure!(
                !t.id.is_empty() && ids.insert(t.id.clone()),
                "중복되거나 빈 작업 ID"
            );
            ensure!(!t.objective.trim().is_empty(), "작업 목적 누락");
            ensure!(
                t.allowed_tools.is_empty(),
                "v0.1은 모델 도구 실행을 허용하지 않습니다"
            );
            ensure!(
                t.source_ids.iter().all(|s| source_ids.contains(s)),
                "알 수 없는 출처 참조"
            );
            ensure!(
                t.target_ids.iter().all(|s| ledger_ids.contains(s)),
                "알 수 없는 원장 참조"
            );
            ensure!(
                !t.base_only || t.depends_on.is_empty(),
                "독립 생성 작업에 선행 출력 노출이 지정되었습니다"
            );
        }
        let mut done = HashSet::new();
        let mut order = vec![];
        while order.len() < self.tasks.len() {
            let before = order.len();
            for (i, t) in self.tasks.iter().enumerate() {
                ensure!(
                    t.depends_on.iter().all(|d| ids.contains(d)),
                    "없는 선행 작업 참조"
                );
                if !done.contains(&t.id) && t.depends_on.iter().all(|d| done.contains(d)) {
                    done.insert(t.id.clone());
                    order.push(i);
                }
            }
            ensure!(order.len() > before, "계획에 순환 의존 관계가 있습니다");
        }
        let last = &self.tasks[*order.last().unwrap()];
        ensure!(
            last.kind == Role::Synthesizer,
            "마지막 작업은 답변 종합이어야 합니다"
        );
        let mut ancestors = HashSet::new();
        fn visit(id: &str, tasks: &[TaskSpec], a: &mut HashSet<String>) {
            if a.insert(id.into()) {
                for d in &tasks.iter().find(|t| t.id == id).unwrap().depends_on {
                    visit(d, tasks, a);
                }
            }
        }
        visit(&last.id, &self.tasks, &mut ancestors);
        ensure!(
            ancestors.len() == self.tasks.len(),
            "종합 결과와 연결되지 않은 작업이 있습니다"
        );
        ensure!(
            self.tasks
                .iter()
                .any(|t| t.kind == Role::Critic || t.kind == Role::EvidenceReviewer),
            "검토 작업이 필요합니다"
        );
        Ok(order)
    }
    pub fn fallback(question: &str, sources: Vec<String>) -> Self {
        let mut analyst = TaskSpec::new(
            "analysis",
            Role::Analyst,
            "질문에 필요한 최소 분석만 수행한다. 새 이론을 요구하지 않으면 만들지 않는다.",
        );
        analyst.source_ids = sources.clone();
        analyst.base_only = true;
        let mut critic = TaskSpec::new(
            "review",
            Role::Critic,
            "분석의 추론과 원문 범위를 검토하고 근거 부족·반례·중요한 이견을 보존한다.",
        );
        critic.depends_on = vec!["analysis".into()];
        critic.source_ids = sources.clone();
        let mut synthesis = TaskSpec::new(
            "answer",
            Role::Synthesizer,
            "직접적인 답변, 근거, 가정, 미해결 사항을 종합한다.",
        );
        synthesis.depends_on = vec!["analysis".into(), "review".into()];
        synthesis.source_ids = sources;
        Self {
            scope: "보수적 분석 → 검토 → 종합".into(),
            requirements: vec![question.into()],
            tasks: vec![analyst, critic, synthesis],
            search_queries: vec![],
        }
    }
}
pub fn schema<T: JsonSchema>() -> serde_json::Value {
    {
        let mut schema = serde_json::to_value(schemars::schema_for!(T)).unwrap();
        // Claude Code's installed validator accepts the shared draft-7 subset, not the 2020-12 meta-schema URL.
        schema.as_object_mut().unwrap().remove("$schema");
        strict_objects(&mut schema);
        schema
    }
}
// Structured Outputs requires every property to be required. Optional values remain
// nullable; serde defaults are only for persisted records, not omitted wire fields.
fn strict_objects(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(properties) = map.get("properties").and_then(|v| v.as_object()) {
                let required: Vec<_> = properties.keys().cloned().collect();
                map.insert("required".into(), serde_json::json!(required));
                map.insert("additionalProperties".into(), serde_json::json!(false));
            }
            map.remove("default");
            for child in map.values_mut() {
                strict_objects(child);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                strict_objects(child);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codex_schema_requires_every_property_including_nullable_replacements() {
        fn check(v: &serde_json::Value) {
            if let Some(properties) = v.get("properties").and_then(|v| v.as_object()) {
                let required = v["required"].as_array().unwrap();
                assert_eq!(properties.len(), required.len());
                for key in properties.keys() {
                    assert!(required.contains(&serde_json::json!(key)));
                }
                assert_eq!(v["additionalProperties"], false);
            }
            match v {
                serde_json::Value::Object(m) => m.values().for_each(check),
                serde_json::Value::Array(a) => a.iter().for_each(check),
                _ => {}
            }
        }
        let s = schema::<crate::Envelope>();
        check(&s);
        assert_eq!(
            s["$defs"]["CritiqueSeverity"]["enum"],
            serde_json::json!(["low", "medium", "high"])
        );
        assert!(
            s["$defs"]["Proposal"]["required"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("replaces_id"))
        );
        assert!(
            s["$defs"]["Proposal"]["properties"]["replaces_id"]["type"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("null"))
        );
    }
    #[test]
    fn invalid_plans_rejected() {
        let p = Plan::fallback("개념 구분", vec![]);
        assert!(p.validate(3, &HashSet::new(), &HashSet::new()).is_ok());
        assert!(p.validate(2, &HashSet::new(), &HashSet::new()).is_err());
        let mut cycle = p.clone();
        cycle.tasks[0].base_only = false;
        cycle.tasks[0].depends_on = vec!["answer".into()];
        assert!(cycle.validate(3, &HashSet::new(), &HashSet::new()).is_err());
        let mut tools = p.clone();
        tools.tasks[0].allowed_tools = vec!["shell".into()];
        assert!(tools.validate(3, &HashSet::new(), &HashSet::new()).is_err());
        let mut missing = p;
        missing.tasks[1].depends_on = vec!["absent".into()];
        assert!(
            missing
                .validate(3, &HashSet::new(), &HashSet::new())
                .is_err()
        );
    }
}
