//! Create a demonstration project for UI development and screenshots.
//!
//! The responses below are scripted. They run through the real engine (planning, quarantine,
//! critique ledger, deliberation rounds, ledger commit) but are NOT model output and NOT
//! scientific claims. The project title and model names say so.
//!
//!     cargo run -p theory-engine --example demo_project -- <data directory>
//!     THEORY_STUDIO_DATA_DIR=<data directory> cargo run -p theory-studio
use anyhow::{Result, bail};
use serde_json::Value;
use std::sync::{Arc, atomic::AtomicBool};
use theory_core::*;
use theory_engine::{execution, providers::*, storage::Store};

const SOURCE_A: &str = "[데모용 합성 발췌 — 실제 논문 아님]
Chronic restraint stress for 21 days reduced dendritic spine density in hippocampal CA3 neurons of adult male rats.
Spine loss was accompanied by impaired long-term potentiation in the Schaffer collateral pathway.
Corticosterone levels remained elevated for 6 days after the stress protocol ended.
Spine density recovered within 3 weeks after stress cessation.";
const SOURCE_B: &str = "[데모용 합성 발췌 — 실제 논문 아님]
In a cohort of 212 patients with major depressive disorder, a blunted cortisol awakening response predicted episode duration.
The association remained after adjusting for age and medication.
Hippocampal volume was not measured in this cohort.";

struct Script;
fn out(summary: &str) -> Envelope {
    record::<Envelope>(|o| o.summary = summary.into())
}
fn claim(id: &str, kind: ClaimKind, statement: &str, rationale: &str, deps: &[&str]) -> Proposal {
    record::<Proposal>(|p| {
        p.id = id.into();
        p.kind = if kind == ClaimKind::TheoreticalAssumption {
            EntryKind::Assumption
        } else {
            EntryKind::Claim
        };
        p.claim_kind = kind;
        p.statement = statement.into();
        p.rationale = rationale.into();
        p.scope = "성체 설치류 및 주요우울장애 성인 코호트; 종 간 일반화는 가정".into();
        p.depends_on = deps.iter().map(|d| d.to_string()).collect();
    })
}
fn quote(
    c: &Value,
    source_title: &str,
    claim_id: &str,
    line: u32,
    rel: EvidenceRelation,
) -> Evidence {
    let s = c["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["title"] == source_title)
        .unwrap();
    let text = s["text"].as_str().unwrap();
    let wanted = format!("{line}: ");
    let body = text
        .lines()
        .find_map(|l| l.strip_prefix(&wanted))
        .unwrap()
        .to_string();
    record::<Evidence>(|e| {
        e.claim_id = claim_id.into();
        e.source_id = s["id"].as_str().unwrap().into();
        e.source_hash = s["hash"].as_str().unwrap().into();
        e.quote = body;
        e.start_line = line;
        e.end_line = line;
        e.relation = rel;
        e.reasoning = "데모 판정: 문자열 대조만 확인됨".into();
    })
}
fn critique(
    target: &str,
    severity: &str,
    category: &str,
    reasoning: &str,
    action: &str,
) -> Critique {
    record::<Critique>(|k| {
        k.target_id = target.into();
        k.severity = severity.into();
        k.category = category.into();
        k.reasoning = reasoning.into();
        k.suggested_action = action.into();
    })
}
fn theory_id(c: &Value, needle: &str) -> String {
    c["current_theory"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["statement"].as_str().unwrap().contains(needle))
        .map(|p| p["id"].as_str().unwrap().to_string())
        .unwrap_or_default()
}
fn critique_id(c: &Value, key: &str, needle: &str) -> String {
    c[key]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["reasoning"].as_str().unwrap().contains(needle))
        .map(|k| k["id"].as_str().unwrap().to_string())
        .unwrap_or_default()
}
impl ModelProvider for Script {
    fn generate(
        &self,
        _: ProviderKind,
        r: &Request,
        _: &Cancel,
        progress: &mut dyn FnMut(String),
    ) -> Result<Response> {
        progress("데모 스크립트 응답".into());
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
        let value = match phase.as_str() {
            "planner" => {
                let sources: Vec<String> = c["sources"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s["id"].as_str().unwrap().into())
                    .collect();
                let mut a = TaskSpec::new(
                    "mechanism-a",
                    Role::Constructor,
                    "시냅스 가소성 저하를 중심으로 삽화 지속 기제를 구성한다",
                );
                a.base_only = true;
                a.source_ids = sources.clone();
                let mut b = TaskSpec::new(
                    "mechanism-b",
                    Role::Constructor,
                    "HPA축 되먹임 둔화를 중심으로 대안 기제를 독립적으로 구성한다",
                );
                b.base_only = true;
                b.source_ids = sources.clone();
                let mut k = TaskSpec::new(
                    "critique",
                    Role::Critic,
                    "두 후보의 연결 가정과 시간 척도를 비판한다",
                );
                k.depends_on = vec!["mechanism-a".into(), "mechanism-b".into()];
                k.source_ids = sources.clone();
                let mut s = TaskSpec::new(
                    "answer",
                    Role::Synthesizer,
                    "근거·가정·이견을 구분해 답한다",
                );
                s.depends_on = vec![
                    "mechanism-a".into(),
                    "mechanism-b".into(),
                    "critique".into(),
                ];
                s.source_ids = sources;
                serde_json::to_value(record::<Plan>(|p| {
                    p.scope = "독립 기제 후보 2개 → 비판 ↔ 수정 → 종합".into();
                    p.requirements = vec![
                        "가소성과 느린 조절의 결합 기제".into(),
                        "근거와 가정 구분".into(),
                    ];
                    p.tasks = vec![a, b, k, s];
                }))?
            }
            "mechanism-a" => {
                let mut o = out(
                    "만성 스트레스가 해마 수상돌기 가시를 줄이고 LTP를 손상시킨다는 동물 자료를 출발점으로, 가소성 저하가 기분 상태를 부정적 끌개에 오래 머물게 한다는 후보를 세웠습니다. 인간 자료와의 연결은 가정으로 남겼습니다.",
                );
                o.proposed_changes = vec![
                    claim(
                        "A1",
                        ClaimKind::ReportedFinding,
                        "21일 만성 구속 스트레스는 성체 수컷 쥐 CA3의 수상돌기 가시 밀도를 감소시켰다.",
                        "제공 발췌의 직접 보고",
                        &[],
                    ),
                    claim(
                        "A2",
                        ClaimKind::TheoreticalAssumption,
                        "해마 가소성 저하는 정서 맥락의 갱신 속도를 낮춰 부정적 기분 상태의 탈출을 늦춘다.",
                        "회로 수준 결과를 기분 경과로 잇는 연결 가정",
                        &["A1"],
                    ),
                    claim(
                        "A3",
                        ClaimKind::Deduction,
                        "따라서 가소성 저하의 지속 기간이 우울 삽화의 지속 기간을 결정한다.",
                        "A1과 A2에서 도출",
                        &["A1", "A2"],
                    ),
                ];
                o.evidence_refs = vec![
                    quote(
                        &c,
                        "합성 발췌 A · 스트레스와 가시 밀도",
                        "A1",
                        2,
                        EvidenceRelation::Supports,
                    ),
                    quote(
                        &c,
                        "합성 발췌 A · 스트레스와 가시 밀도",
                        "A1",
                        3,
                        EvidenceRelation::PartialSupport,
                    ),
                ];
                o.unresolved =
                    vec!["인간에서 가시 밀도 변화의 시간 경과는 제공 자료에 없음".into()];
                serde_json::to_value(o)?
            }
            "mechanism-b" => {
                let mut o = out(
                    "코르티솔 각성 반응 둔화가 삽화 지속을 예측한다는 코호트 자료를 바탕으로, 느린 HPA 되먹임이 회복 신호를 지연시킨다는 대안 후보를 세웠습니다.",
                );
                o.proposed_changes = vec![
                    claim(
                        "B1",
                        ClaimKind::ReportedFinding,
                        "주요우울장애 212명 코호트에서 둔화된 코르티솔 각성 반응이 삽화 지속 기간을 예측했다.",
                        "제공 발췌의 직접 보고",
                        &[],
                    ),
                    claim(
                        "B2",
                        ClaimKind::Speculation,
                        "HPA축 되먹임의 느린 회복이 가소성 회복보다 먼저 삽화 종료를 제한한다.",
                        "두 시간 척도의 순서에 대한 추측",
                        &["B1"],
                    ),
                ];
                o.evidence_refs = vec![quote(
                    &c,
                    "합성 발췌 B · 코르티솔 코호트",
                    "B1",
                    2,
                    EvidenceRelation::Supports,
                )];
                serde_json::to_value(o)?
            }
            "critique" => {
                let a3 = c["dependency_outputs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|d| d["output"]["proposed_changes"].as_array().unwrap().iter())
                    .find(|p| p["statement"].as_str().unwrap().contains("결정한다"))
                    .map(|p| p["id"].as_str().unwrap().to_string())
                    .unwrap();
                let a2 = c["dependency_outputs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|d| d["output"]["proposed_changes"].as_array().unwrap().iter())
                    .find(|p| p["statement"].as_str().unwrap().contains("갱신 속도"))
                    .map(|p| p["id"].as_str().unwrap().to_string())
                    .unwrap();
                let mut o = out(
                    "후보 A의 결론은 시간 척도가 맞지 않습니다. 동물에서 가시 밀도는 3주 안에 회복되지만 삽화는 수개월 지속될 수 있습니다. 또한 회로→기분 연결은 근거 없이 도입되었습니다.",
                );
                o.critiques = vec![
                    critique(
                        &a3,
                        "high",
                        "시간 척도 불일치",
                        "가시 밀도는 스트레스 종료 후 3주 안에 회복되었다는 보고가 있어, 가소성 저하의 지속 기간이 수개월의 삽화 지속을 '결정'한다는 결론과 충돌한다.",
                        "결정 관계를 기여 요인으로 낮추거나, 회복을 막는 추가 과정을 명시하라.",
                    ),
                    critique(
                        &a2,
                        "medium",
                        "수준 연결 근거 부족",
                        "해마 가소성 저하가 '정서 맥락 갱신 속도'를 낮춘다는 연결은 제공 자료 어디에도 없으며, 측정 변수도 정의되지 않았다.",
                        "연결 가정의 변수와 관찰 가능한 대응물을 정의하라.",
                    ),
                    critique(
                        "",
                        "low",
                        "표현",
                        "'부정적 끌개'라는 용어가 정의 없이 쓰였다.",
                        "용어를 정의하거나 제거하라.",
                    ),
                ];
                serde_json::to_value(o)?
            }
            "revise-r1" => {
                let k_time = critique_id(&c, "critiques_to_answer", "3주");
                let k_bridge = critique_id(&c, "critiques_to_answer", "갱신 속도");
                let a3 = theory_id(&c, "결정한다");
                let a1 = theory_id(&c, "CA3");
                let a2 = theory_id(&c, "갱신 속도");
                let b1 = theory_id(&c, "212명");
                let prefix = c["output_id_prefix"].as_str().unwrap();
                let mut o = out(
                    "시간 척도 비판을 수용해 결론을 '기여'로 낮추고, 느린 HPA 회복이 가소성 회복을 지연시킨다는 결합 가정을 명시했습니다. 수준 연결 비판에는 반박했습니다: 연결 가정은 처음부터 가정으로 표시되었고 결론의 근거로 쓰지 않았습니다.",
                );
                let mut revised = claim(
                    &format!("{prefix}_A3"),
                    ClaimKind::Deduction,
                    "가소성 저하는 삽화 지속에 기여하지만, 지속 기간을 단독으로 결정하지 않는다. 지속은 느린 HPA 회복이 가소성 회복을 지연시킬 때 길어진다.",
                    "K1 수용: 동물 자료의 3주 회복과 맞추기 위해 결합 조건을 추가",
                    &[&a1, &a2, &b1],
                );
                revised.replaces_id = Some(a3);
                o.proposed_changes = vec![revised];
                o.evidence_refs = vec![quote(
                    &c,
                    "합성 발췌 A · 스트레스와 가시 밀도",
                    &format!("{prefix}_A3"),
                    5,
                    EvidenceRelation::PartialSupport,
                )];
                o.critique_responses = vec![
                    record::<CritiqueResponse>(|r| {
                        r.critique_id = k_time;
                        r.stance = Stance::Accept;
                        r.reasoning = "3주 회복 보고와 충돌한다는 지적이 맞습니다. 결정 관계를 철회하고 결합 조건을 명시했습니다.".into();
                        r.revised_ids = vec![format!("{prefix}_A3")];
                    }),
                    record::<CritiqueResponse>(|r| {
                        r.critique_id = k_bridge;
                        r.stance = Stance::Rebut;
                        r.reasoning = "해당 연결은 이론적 가정으로 표시되어 있고 결론의 경험적 근거로 쓰지 않았습니다. 근거 부족은 사실이지만 가정으로서의 지위는 유지됩니다.".into();
                    }),
                ];
                serde_json::to_value(o)?
            }
            "review-r1" => {
                let k_time = critique_id(&c, "critiques_to_judge", "3주");
                let k_bridge = critique_id(&c, "critiques_to_judge", "갱신 속도");
                let k_word = critique_id(&c, "critiques_to_judge", "끌개");
                let a3 = theory_id(&c, "단독으로 결정하지");
                let mut o = out(
                    "결정→기여 수정으로 시간 척도 문제는 해소되었습니다. 그러나 연결 가정에 대한 반박은 불충분합니다. 결론이 여전히 그 가정에 의존합니다. 또 두 과정의 순서를 뒤집은 대안을 배제하지 않았습니다.",
                );
                o.critique_reviews = vec![
                    record::<CritiqueReview>(|r| {
                        r.critique_id = k_time;
                        r.status = CritiqueStatus::Addressed;
                        r.reasoning = "수정된 결론이 3주 회복 보고와 양립한다.".into();
                    }),
                    record::<CritiqueReview>(|r| {
                        r.critique_id = k_bridge;
                        r.status = CritiqueStatus::Contested;
                        r.reasoning = "가정으로 표시했더라도 수정 결론이 그 가정에 의존하므로, 관찰 가능한 대응물이 필요하다.".into();
                    }),
                    record::<CritiqueReview>(|r| {
                        r.critique_id = k_word;
                        r.status = CritiqueStatus::Deferred;
                        r.reasoning = "표현 문제로 이번 범위에서 보류.".into();
                    }),
                ];
                o.critiques = vec![critique(
                    &a3,
                    "medium",
                    "대안 설명 누락",
                    "가소성 저하가 HPA 회복을 늦추는 역방향 결합도 같은 관찰을 낳을 수 있다. 두 방향을 구별할 관찰이 제시되지 않았다.",
                    "역방향 대안을 명시하고 구별 예측을 제시하라.",
                )];
                serde_json::to_value(o)?
            }
            "revise-r2" => {
                let k_bridge = critique_id(&c, "critiques_to_answer", "대응물");
                let k_bridge = if k_bridge.is_empty() {
                    critique_id(&c, "critiques_to_answer", "갱신 속도")
                } else {
                    k_bridge
                };
                let k_alt = critique_id(&c, "critiques_to_answer", "역방향");
                let a2 = theory_id(&c, "갱신 속도");
                let prefix = c["output_id_prefix"].as_str().unwrap();
                let mut o = out(
                    "연결 가정에 관찰 가능한 대응물(맥락 전환 과제에서의 학습률)을 붙였고, 역방향 결합을 별도 대안 가정으로 기록하고 구별 예측을 추가했습니다.",
                );
                let mut bridge = claim(
                    &format!("{prefix}_A2"),
                    ClaimKind::TheoreticalAssumption,
                    "해마 가소성 저하는 맥락 전환 과제의 학습률 저하로 관찰되며, 이것이 부정적 기분 상태의 지속과 연관된다.",
                    "K2 부분 수용: 변수와 측정 대응물을 정의",
                    &[],
                );
                bridge.replaces_id = Some(a2);
                let alt = claim(
                    &format!("{prefix}_ALT"),
                    ClaimKind::Speculation,
                    "역방향 대안: 가소성 저하가 해마의 HPA 억제 신호를 약화시켜 HPA 회복을 늦출 수 있다.",
                    "K4 수용: 같은 관찰을 낳는 대안",
                    &[],
                );
                let pred = claim(
                    &format!("{prefix}_PRED"),
                    ClaimKind::Deduction,
                    "구별 예측: HPA 회복이 선행한다면 코르티솔 정상화가 학습률 회복보다 먼저 나타나고, 역방향이라면 순서가 반대다.",
                    "두 방향의 시간 순서 차이",
                    &[&format!("{prefix}_A2"), &format!("{prefix}_ALT")],
                );
                o.proposed_changes = vec![bridge, alt, pred];
                o.critique_responses = vec![
                    record::<CritiqueResponse>(|r| {
                        r.critique_id = k_bridge;
                        r.stance = Stance::PartiallyAccept;
                        r.reasoning =
                            "결론이 가정에 의존한다는 점을 인정하고 측정 대응물을 정의했습니다."
                                .into();
                        r.revised_ids = vec![format!("{prefix}_A2")];
                    }),
                    record::<CritiqueResponse>(|r| {
                        r.critique_id = k_alt;
                        r.stance = Stance::Accept;
                        r.reasoning =
                            "역방향 대안을 기록하고 시간 순서로 구별하는 예측을 추가했습니다."
                                .into();
                        r.revised_ids = vec![format!("{prefix}_ALT"), format!("{prefix}_PRED")];
                    }),
                ];
                serde_json::to_value(o)?
            }
            "review-r2" => {
                let mut o = out(
                    "측정 대응물과 구별 예측이 추가되어 주요 비판은 이번 검토 범위에서 해소되었습니다. 다만 인간에서 학습률과 기분 지속의 연관은 여전히 검증되지 않은 가정입니다.",
                );
                o.critique_reviews = c["critiques_to_judge"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|k| k["status"] != "deferred")
                    .map(|k| {
                        record::<CritiqueReview>(|r| {
                            r.critique_id = k["id"].as_str().unwrap().into();
                            r.status = CritiqueStatus::Addressed;
                            r.reasoning = "수정과 예측으로 이번 범위에서 해소.".into();
                        })
                    })
                    .collect();
                serde_json::to_value(o)?
            }
            "synthesis" => {
                let mut o = out(
                    "**결론: 가능하지만 조건부입니다.** 제공된 (합성) 자료만으로는 해마 가소성 저하가 삽화 지속을 *결정*한다고 말할 수 없습니다. 동물 자료에서 가시 밀도는 3주 안에 회복되기 때문입니다. 대신 **느린 HPA 회복이 가소성 회복을 지연시킬 때 삽화가 길어진다**는 결합 가설이 두 자료와 양립합니다.\n\n1. 보고된 결과: 만성 스트레스 후 CA3 가시 밀도 감소와 LTP 손상(동물), 코르티솔 각성 반응 둔화와 삽화 지속의 연관(인간 코호트).\n2. 연결 가정: 가소성 저하는 맥락 전환 학습률 저하로 관찰된다 — 아직 근거 없음.\n3. 대안: 가소성 저하가 HPA 회복을 늦추는 역방향 결합도 같은 관찰을 낳는다.\n\n두 모델은 세 차례의 비판과 두 차례의 수정 끝에 시간 척도·대안 문제에 합의했지만, 이 합의는 경험적 근거가 아닙니다.",
                );
                o.unresolved = vec![
                    "인간에서 해마 가소성의 시간 경과는 측정되지 않았다".into(),
                    "코호트에서 해마 부피를 측정하지 않아 두 수준을 같은 표본에서 잇지 못한다"
                        .into(),
                ];
                o.next_actions = vec!["코르티솔 정상화와 맥락 전환 학습률 회복의 시간 순서를 같은 표본에서 추적한 연구 확인".into(), "'부정적 끌개' 용어 정의 또는 제거".into()];
                serde_json::to_value(o)?
            }
            "reader_report" => serde_json::to_value(reader_report(&c))?,
            "follow_up" => serde_json::to_value(record::<FollowUpAnswer>(|a| {
                a.answer = "(데모 응답) 가시 밀도는 쥐에서 스트레스가 끝나고 약 3주 안에 회복되었다고 보고되었습니다. 그래서 가소성 저하 *하나만으로*는 몇 달씩 이어지는 삽화를 설명하기 어렵고, 회복을 늦추는 다른 느린 과정 — 여기서는 코르티솔 조절 — 이 함께 필요하다고 본 것입니다.".into();
                a.needs_research = false;
            }))?,
            other => bail!("demo script has no step {other}"),
        };
        let mut response = Response::default();
        response.value = value;
        response.model = "demo-script (가상 응답)".into();
        Ok(response)
    }
}
/// A scripted scribe report for the demo question (not model output).
fn reader_report(c: &Value) -> ReaderReport {
    let key = |needle: &str| {
        c["references"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["title"].as_str().unwrap_or_default().contains(needle))
            .map(|r| format!("@{}", r["key"].as_str().unwrap()))
            .unwrap_or_default()
    };
    let (a, b) = (key("가시 밀도"), key("코르티솔"));
    let node = |id: &str, label: &str, group: &str, note: &str| {
        record::<DiagramNode>(|n| {
            n.id = id.into();
            n.label = label.into();
            n.group = group.into();
            n.note = note.into();
        })
    };
    let edge = |from: &str, to: &str, label: &str, dashed: bool| {
        record::<DiagramEdge>(|e| {
            e.from = from.into();
            e.to = to.into();
            e.label = label.into();
            e.dashed = dashed;
        })
    };
    record::<ReaderReport>(|r| {
        r.title = "스트레스 가소성과 느린 호르몬 회복의 결합 가설".into();
        r.answer = format!(
            "(데모용 가상 보고서) **가능하지만 조건부입니다.** 해마의 시냅스 가소성이 떨어지는 것만으로는 삽화가 길어지는 이유를 설명하기 어렵습니다. 쥐에서는 몇 주 만에 회복되기 때문입니다 [{a}]. 대신 *스트레스 호르몬 조절이 느리게 회복되면 가소성 회복도 늦어져 삽화가 길어진다*는 결합 가설이 두 자료 모두와 어긋나지 않습니다 [{b}]."
        );
        r.key_points = vec![
            record::<KeyPoint>(|k| {
                k.heading = "동물에서 가소성 저하는 실제로 관찰되었다".into();
                k.body = format!(
                    "21일 동안 스트레스를 받은 쥐의 해마 CA3 뉴런에서 가시(시냅스가 붙는 돌기) 밀도가 줄고 장기강화(LTP)가 손상되었습니다 [{a}]. 다만 이 변화는 스트레스가 끝난 뒤 3주 안에 회복되었습니다."
                );
                k.standing = Standing::Supported;
                k.figure_ids = vec!["F1".into()];
            }),
            record::<KeyPoint>(|k| {
                k.heading = "사람에서는 호르몬 반응이 삽화 길이와 함께 움직였다".into();
                k.body = format!(
                    "주요우울장애 환자 212명에서 아침 코르티솔 반응이 둔한 사람일수록 삽화가 길었습니다 [{b}]. 하지만 이 코호트에서는 해마를 측정하지 않았습니다."
                );
                k.standing = Standing::Supported;
            }),
            record::<KeyPoint>(|k| {
                k.heading = "둘을 잇는 고리는 아직 가설이다".into();
                k.body = "‘호르몬 회복이 늦으면 가소성 회복도 늦다’는 연결은 같은 사람이나 동물에서 두 시간 경과를 함께 잰 적이 없어 검증되지 않았습니다. 반대 방향(가소성 저하가 호르몬 회복을 늦춤)도 같은 관찰을 낳을 수 있습니다.".into();
                k.standing = Standing::Hypothesis;
                k.figure_ids = vec!["F2".into()];
            }),
        ];
        r.sections = vec![record::<Section>(|s| {
            s.heading = "왜 ‘시간 척도’가 핵심인가".into();
            s.body = "빨리 회복되는 변화는 오래가는 상태를 혼자 설명하지 못합니다. 욕조에 비유하면, 물이 빨리 빠지는 욕조는 수도꼭지를 잠그면 곧 비워집니다. 삽화가 몇 달 지속되려면 수도꼭지(스트레스 호르몬)가 천천히 잠기거나, 배수구(가소성 회복)가 함께 막혀 있어야 합니다.".into();
        })];
        r.figures = vec![
            record::<Figure>(|f| {
                f.id = "F1".into();
                f.title = "세 수준과 그 사이의 연결".into();
                f.caption =
                    "실선은 자료로 보고된 관계, 점선은 아직 검증되지 않은 연결입니다.".into();
                f.groups = vec![
                    "세포·시냅스 (쥐)".into(),
                    "신경내분비".into(),
                    "임상 (사람)".into(),
                ];
                f.nodes = vec![
                    node(
                        "spine",
                        "CA3 가시 밀도 감소",
                        "세포·시냅스 (쥐)",
                        "21일 스트레스 뒤 관찰, 3주 안에 회복.",
                    ),
                    node(
                        "ltp",
                        "LTP 손상",
                        "세포·시냅스 (쥐)",
                        "가시 감소와 함께 관찰된 기능 변화.",
                    ),
                    node(
                        "hpa",
                        "느린 코르티솔 회복",
                        "신경내분비",
                        "스트레스 종료 뒤에도 6일간 높게 유지(쥐).",
                    ),
                    node(
                        "car",
                        "각성 반응 둔화",
                        "신경내분비",
                        "사람 코호트에서 측정된 아침 코르티솔 반응.",
                    ),
                    node(
                        "episode",
                        "삽화 지속 기간",
                        "임상 (사람)",
                        "212명 코호트에서 각성 반응과 연관.",
                    ),
                ];
                f.edges = vec![
                    edge("spine", "ltp", "함께 관찰", false),
                    edge("hpa", "spine", "회복 지연?", true),
                    edge("car", "episode", "연관", false),
                    edge("ltp", "episode", "가설", true),
                ];
            }),
            record::<Figure>(|f| {
                f.id = "F2".into();
                f.kind = FigureKind::Table;
                f.title = "경쟁하는 두 설명".into();
                f.columns = vec!["설명".into(), "방향".into(), "구별하려면".into()];
                f.rows = vec![
                    vec![
                        "호르몬 → 가소성".into(),
                        "코르티솔이 먼저 정상화되고 그 뒤 학습률이 회복".into(),
                        "같은 표본에서 두 시간 경과의 순서를 잰다".into(),
                    ],
                    vec![
                        "가소성 → 호르몬".into(),
                        "학습률이 먼저 회복되고 코르티솔이 뒤따름".into(),
                        "가소성만 조작했을 때 호르몬이 바뀌는지 본다".into(),
                    ],
                ];
            }),
        ];
        r.glossary = vec![
            record::<GlossaryTerm>(|g| {
                g.term = "장기강화(LTP)".into();
                g.plain =
                    "자극이 반복될 때 시냅스 연결이 오래 강해지는 현상. 학습의 세포 수준 모형."
                        .into();
            }),
            record::<GlossaryTerm>(|g| {
                g.term = "코르티솔 각성 반응".into();
                g.plain = "잠에서 깬 뒤 30–45분 동안 코르티솔이 오르는 정도.".into();
            }),
        ];
        r.open_questions = vec![record::<OpenQuestion>(|q| {
            q.question = "사람의 해마 가소성은 얼마나 빨리 회복될까?".into();
            q.why_it_matters =
                "회복이 쥐처럼 빠르다면 가소성만으로는 긴 삽화를 설명할 수 없습니다.".into();
            q.how_to_settle =
                "같은 환자에서 회복기 동안 학습 과제 수행과 코르티솔을 반복 측정한 종단 자료."
                    .into();
        })];
        r.next_steps = vec![
            record::<NextStep>(|n| {
                n.kind = NextKind::AskHere;
                n.action = "가시 밀도 회복이 3주라는 것이 왜 가설을 약하게 만드나요?".into();
                n.purpose = "시간 척도 논증을 예와 함께 다시 설명받습니다.".into();
            }),
            record::<NextStep>(|n| {
                n.kind = NextKind::DataAnalysis;
                n.action = "코르티솔과 기분을 함께 추적한 기존 종단 자료에서 정상화 시점의 순서를 비교한다.".into();
                n.purpose = "두 방향 가운데 어느 쪽이 먼저인지 첫 단서를 얻습니다.".into();
            }),
            record::<NextStep>(|n| {
                n.kind = NextKind::Experiment;
                n.action =
                    "동물에서 코르티솔 회복만 앞당겼을 때 가시 밀도 회복이 빨라지는지 측정한다."
                        .into();
                n.purpose = "빨라지면 ‘호르몬 → 가소성’ 방향을 지지합니다.".into();
            }),
        ];
        r.how_made =
            "(데모) 스크립트로 만든 가상 응답입니다. 실제 모델 출력이나 과학적 주장이 아닙니다."
                .into();
    })
}
fn main() -> Result<()> {
    let root = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .expect("usage: demo_project <data directory>");
    let mut store = Store::create(&root, "UI 데모 · 가상 응답")?;
    store.project.research_goal = "스트레스 관련 가소성 변화와 느린 내분비 조절이 기분 삽화의 지속을 어떻게 함께 설명할 수 있는지".into();
    for (title, text) in [
        ("합성 발췌 A · 스트레스와 가시 밀도", SOURCE_A),
        ("합성 발췌 B · 코르티솔 코호트", SOURCE_B),
    ] {
        let mut s = Source::new(
            title.into(),
            text.into(),
            "데모 발췌 (실제 논문 아님)".into(),
            Access::ProvidedExcerpt,
        );
        s.import_note = "UI 데모용 합성 텍스트".into();
        store.project.sources.push(s);
    }
    store.save("데모 자료")?;
    let mut settings = Settings::default();
    settings.primary = ProviderKind::Claude;
    settings.cross_review = true;
    settings.search_enabled = false;
    execution::run(
        &mut store,
        execution::RunInput::new(
            "만성 스트레스에서 해마 시냅스 가소성 저하와 느린 HPA축 조절이 결합해 우울 삽화의 지속을 설명할 수 있을까?",
            Budget::Standard,
        ),
        &settings,
        &Script,
        &Arc::new(AtomicBool::new(false)),
        &mut |_, s| eprintln!("{s}"),
    )?;
    let run = store.project.runs.last().unwrap();
    println!(
        "{} · 상태 {:?} · 토론 {}회 · {:?} · 비판 {} · 원장 {}",
        store.project.title,
        run.state,
        run.deliberation.rounds,
        run.deliberation.stop,
        run.critiques.len(),
        store.project.entries.len()
    );
    for a in &run.attempts {
        if !a.validation_notes.is_empty() || a.error.is_some() {
            println!(
                "{} {:?} {:?} {:?}",
                a.task_id, a.state, a.error, a.validation_notes
            );
        }
    }
    let run_id = run.id.clone();
    execution::ask(
        &mut store,
        &run_id,
        "가시 밀도 회복이 3주라는 것이 왜 가설을 약하게 만드나요?",
        &settings,
        &Script,
        &Arc::new(AtomicBool::new(false)),
        &mut |_, s| eprintln!("{s}"),
    )?;
    let folder = store.export_report(
        &root.join("exports"),
        theory_engine::storage::ReportScope::Run(run_id),
    )?;
    println!("보고서 내보냄: {}", folder.display());
    let mut settings_file = Settings::default();
    settings_file.primary = ProviderKind::Claude;
    settings_file.cross_review = true;
    settings_file.selected_project = store.project.selection_key();
    std::fs::write(
        root.join("settings.json"),
        serde_json::to_vec_pretty(&settings_file)?,
    )?;
    Ok(())
}
