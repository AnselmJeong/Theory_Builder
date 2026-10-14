//! The reader-facing report. After the debate, a scribe (a separate model context that took
//! no side) rewrites the outcome for the person who asked: a direct answer, the key points
//! with how firm each one is, figures, plain definitions, what is still uncertain and what
//! the reader could do next. Internal identifiers never reach the reader; citations use
//! pandoc keys (`[@key]`) that resolve to the exported BibTeX file.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// How firm a point is, in words a reader can use.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Standing {
    // Backed by supplied source passages.
    Supported,
    // Follows by argument from definitions or supported points.
    Reasoned,
    // Proposed and testable, not yet tested. (Plain comments: variant doc comments would
    // turn the schema enum into `oneOf`, which strict structured output rejects.)
    #[default]
    Hypothesis,
    Speculation,
}
impl Standing {
    pub fn label(self) -> &'static str {
        match self {
            Self::Supported => "문헌 근거 있음",
            Self::Reasoned => "논리적 도출",
            Self::Hypothesis => "검증 전 가설",
            Self::Speculation => "추측",
        }
    }
    pub fn explain(self) -> &'static str {
        match self {
            Self::Supported => "제공·검색된 문헌의 구절이 직접 뒷받침합니다.",
            Self::Reasoned => "정의와 논증으로 따라 나옵니다. 실험 결과는 아닙니다.",
            Self::Hypothesis => "검증할 수 있는 형태로 제안했지만 아직 검증되지 않았습니다.",
            Self::Speculation => "가능성으로만 제시합니다.",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FigureKind {
    // Boxes and arrows; optional ordered groups are drawn as horizontal bands (levels).
    #[default]
    Diagram,
    // A comparison table.
    Table,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct DiagramNode {
    pub id: String,
    pub label: String,
    /// One of the figure's groups, or empty.
    pub group: String,
    /// One sentence shown when the reader selects the node.
    pub note: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct DiagramEdge {
    pub from: String,
    pub to: String,
    pub label: String,
    /// Hypothetical or weaker relation.
    pub dashed: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Figure {
    pub id: String,
    pub kind: FigureKind,
    pub title: String,
    pub caption: String,
    /// Diagram: ordered groups, drawn top to bottom.
    pub groups: Vec<String>,
    pub nodes: Vec<DiagramNode>,
    pub edges: Vec<DiagramEdge>,
    /// Table: header and rows.
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct KeyPoint {
    pub heading: String,
    pub body: String,
    pub standing: Standing,
    pub figure_ids: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Section {
    pub heading: String,
    pub body: String,
    pub figure_ids: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct GlossaryTerm {
    pub term: String,
    pub plain: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct OpenQuestion {
    pub question: String,
    pub why_it_matters: String,
    pub how_to_settle: String,
}
/// What kind of action a next step is, so the reader knows who would do it and how.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NextKind {
    Experiment,
    DataAnalysis,
    Theory,
    Reading,
    // A follow-up question the reader can ask in this app.
    #[default]
    AskHere,
}
impl NextKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Experiment => "새 실험·측정",
            Self::DataAnalysis => "자료 분석",
            Self::Theory => "이론·모형 작업",
            Self::Reading => "더 읽을 것",
            Self::AskHere => "이 앱에서 이어서 묻기",
        }
    }
    pub const ALL: [Self; 5] = [
        Self::AskHere,
        Self::Theory,
        Self::DataAnalysis,
        Self::Experiment,
        Self::Reading,
    ];
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct NextStep {
    pub kind: NextKind,
    pub action: String,
    pub purpose: String,
}
/// Bibliographic fields the scribe copied from a source's printed header. The engine keeps
/// only fields whose text is actually present in that source.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct BibCandidate {
    pub key: String,
    pub entry_type: String,
    pub title: String,
    pub authors: Vec<String>,
    pub year: String,
    pub container: String,
    pub publisher: String,
    pub doi: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ReaderReport {
    pub title: String,
    /// The direct answer, two to four sentences.
    pub answer: String,
    pub key_points: Vec<KeyPoint>,
    pub sections: Vec<Section>,
    pub figures: Vec<Figure>,
    pub glossary: Vec<GlossaryTerm>,
    pub open_questions: Vec<OpenQuestion>,
    pub next_steps: Vec<NextStep>,
    pub how_made: String,
    pub bibliography: Vec<BibCandidate>,
}
/// A stored reader report and how it was produced.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Reading {
    pub report: ReaderReport,
    pub provider: String,
    pub model: String,
    pub written: u64,
    /// Debate round the report describes.
    pub round: u32,
    /// What the engine removed or corrected before storing it.
    pub notes: Vec<String>,
}
/// A direction the user gave when continuing a debate. It states task intent, like the
/// question itself, and is shown to both models and the scribe.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Direction {
    pub round: u32,
    pub text: String,
    pub given: u64,
}
/// The model contract for a follow-up question about a finished report.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct FollowUpAnswer {
    pub answer: String,
    /// The question needs new argument or evidence, not just explanation.
    pub needs_research: bool,
    /// When needs_research: a one-sentence direction for continuing the debate.
    pub suggested_direction: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FollowUp {
    pub question: String,
    pub answer: String,
    pub needs_research: bool,
    pub suggested_direction: String,
    pub provider: String,
    pub asked: u64,
    pub error: Option<String>,
}

/// Internal names a model might copy into reader text, and what a reader should see instead.
/// An empty replacement means "remove".
pub struct Vocabulary {
    pub keys: HashSet<String>,
    pub internal: Vec<(String, String)>,
}
impl Vocabulary {
    /// Remove internal identifiers and keep only citations whose keys exist.
    pub fn clean(&self, text: &str, notes: &mut Vec<String>) -> String {
        let text = self.strip_internal(text, notes);
        clean_citations(&text, &self.keys, notes)
    }
    fn strip_internal(&self, text: &str, notes: &mut Vec<String>) -> String {
        let mut ids: Vec<&(String, String)> = self
            .internal
            .iter()
            .filter(|(id, _)| !id.is_empty() && text.contains(id.as_str()))
            .collect();
        if ids.is_empty() {
            return text.to_string();
        }
        ids.sort_by_key(|(id, _)| std::cmp::Reverse(id.len()));
        // Parenthetical references that hold only identifiers disappear entirely.
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(open) = rest.find(['(', '（']) {
            let close_rel = rest[open..].find([')', '）']);
            let Some(close_rel) = close_rel else { break };
            let inner_start = open + rest[open..].chars().next().unwrap().len_utf8();
            let inner = &rest[inner_start..open + close_rel];
            let mut residue = inner.to_string();
            let mut had = false;
            for (id, _) in &ids {
                if contains_token(&residue, id) {
                    residue = replace_token(&residue, id, "");
                    had = true;
                }
            }
            let only_ids = had
                && residue
                    .chars()
                    .all(|c| c.is_whitespace() || matches!(c, ',' | ';' | '·' | '/' | '、'));
            let end =
                open + close_rel + rest[open + close_rel..].chars().next().unwrap().len_utf8();
            if only_ids {
                out.push_str(rest[..open].trim_end_matches([' ', '\u{a0}']));
            } else {
                out.push_str(&rest[..end]);
            }
            rest = &rest[end..];
        }
        out.push_str(rest);
        for (id, friendly) in &ids {
            if contains_token(&out, id) {
                out = replace_token(&out, id, friendly);
            }
        }
        notes.push(format!(
            "내부 ID {}종을 독자용 표현으로 바꾸거나 제거",
            ids.len()
        ));
        out
    }
}
fn token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}
fn boundary_ok(text: &str, start: usize, end: usize) -> bool {
    !text[..start].chars().next_back().is_some_and(token_char)
        && !text[end..].chars().next().is_some_and(token_char)
}
fn contains_token(text: &str, token: &str) -> bool {
    text.match_indices(token)
        .any(|(i, _)| boundary_ok(text, i, i + token.len()))
}
/// Replace whole-token occurrences only: `K1` must not touch `K13`.
pub fn replace_token(text: &str, token: &str, with: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (i, _) in text.match_indices(token) {
        if i < last || !boundary_ok(text, i, i + token.len()) {
            continue;
        }
        out.push_str(&text[last..i]);
        out.push_str(with);
        last = i + token.len();
    }
    out.push_str(&text[last..]);
    out
}
fn key_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':' | '.' | '/')
}
/// One pandoc citation item: `@key` (or `-@key`, author suppressed) plus an optional
/// locator (`, p. 4`).
fn split_item(item: &str) -> Option<(&str, &str)> {
    let item = item.trim();
    let body = item.strip_prefix('@').or_else(|| item.strip_prefix("-@"))?;
    let end = body.find(|c: char| !key_char(c)).unwrap_or(body.len());
    let key = body[..end].trim_end_matches(['.', ':', '/']);
    (!key.is_empty()).then(|| (key, &body[key.len()..]))
}
fn suppressed(item: &str) -> bool {
    item.trim_start().starts_with('-')
}
/// Every key cited with pandoc bracket syntax.
pub fn cited_keys(text: &str) -> Vec<String> {
    let mut keys = vec![];
    for group in citation_groups(text) {
        for item in group.2.split(';') {
            if let Some((key, _)) = split_item(item)
                && !keys.iter().any(|k| k == key)
            {
                keys.push(key.to_string());
            }
        }
    }
    keys
}
/// (start, end, inner) of each `[@…]` group.
pub fn citation_groups(text: &str) -> Vec<(usize, usize, &str)> {
    let mut out = vec![];
    let mut from = 0;
    while let Some(rel) = text[from..].find("[@").or_else(|| text[from..].find("[-@")) {
        let start = from + rel;
        let Some(close) = text[start..].find(']') else {
            break;
        };
        let inner = &text[start + 1..start + close];
        if !inner.contains('[') && !inner.contains('\n') {
            out.push((start, start + close + 1, inner));
        }
        from = start + close + 1;
    }
    out
}
/// Keep only citations whose key exists; a group with no valid key disappears.
pub fn clean_citations(text: &str, keys: &HashSet<String>, notes: &mut Vec<String>) -> String {
    let groups = citation_groups(text);
    if groups.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (start, end, inner) in groups {
        let kept: Vec<String> = inner
            .split(';')
            .filter_map(|item| {
                let (key, locator) = split_item(item)?;
                if keys.contains(key) {
                    let dash = if suppressed(item) { "-" } else { "" };
                    Some(format!("{dash}@{key}{locator}"))
                } else {
                    notes.push(format!("확인할 수 없는 인용 키 @{key} 제거"));
                    None
                }
            })
            .collect();
        if kept.is_empty() {
            out.push_str(text[last..start].trim_end_matches(' '));
        } else {
            out.push_str(&text[last..start]);
            out.push_str(&format!("[{}]", kept.join("; ")));
        }
        last = end;
    }
    out.push_str(&text[last..]);
    out
}
impl ReaderReport {
    /// Apply `f` to every reader-visible string.
    pub fn map_text(&mut self, mut f: impl FnMut(&str) -> String) {
        let mut apply = |s: &mut String| *s = f(s);
        apply(&mut self.title);
        apply(&mut self.answer);
        apply(&mut self.how_made);
        for k in &mut self.key_points {
            apply(&mut k.heading);
            apply(&mut k.body);
        }
        for s in &mut self.sections {
            apply(&mut s.heading);
            apply(&mut s.body);
        }
        for fig in &mut self.figures {
            apply(&mut fig.title);
            apply(&mut fig.caption);
            for g in &mut fig.groups {
                apply(g);
            }
            for n in &mut fig.nodes {
                apply(&mut n.label);
                apply(&mut n.note);
            }
            for e in &mut fig.edges {
                apply(&mut e.label);
            }
            for c in &mut fig.columns {
                apply(c);
            }
            for row in &mut fig.rows {
                for c in row {
                    apply(c);
                }
            }
        }
        for g in &mut self.glossary {
            apply(&mut g.term);
            apply(&mut g.plain);
        }
        for q in &mut self.open_questions {
            apply(&mut q.question);
            apply(&mut q.why_it_matters);
            apply(&mut q.how_to_settle);
        }
        for n in &mut self.next_steps {
            apply(&mut n.action);
            apply(&mut n.purpose);
        }
    }
    /// Every citation key used anywhere in the report, in order of first use.
    pub fn citations(&self) -> Vec<String> {
        let mut copy = self.clone();
        let mut keys: Vec<String> = vec![];
        copy.map_text(|s| {
            for k in cited_keys(s) {
                if !keys.contains(&k) {
                    keys.push(k);
                }
            }
            s.to_string()
        });
        keys
    }
    /// Make the report safe to show: no internal IDs, no unknown citations, consistent
    /// figure references, bounded sizes. Returns what was changed.
    pub fn sanitize(&mut self, vocabulary: &Vocabulary) -> Vec<String> {
        let mut notes = vec![];
        self.map_text(|s| vocabulary.clean(s, &mut notes));
        notes.sort();
        notes.dedup();
        self.key_points.retain(|k| !k.body.trim().is_empty());
        self.sections.retain(|s| !s.body.trim().is_empty());
        self.glossary
            .retain(|g| !g.term.trim().is_empty() && !g.plain.trim().is_empty());
        self.open_questions
            .retain(|q| !q.question.trim().is_empty());
        self.next_steps.retain(|n| !n.action.trim().is_empty());
        // Figures: unique IDs, nodes referenced by edges must exist, bounded size.
        let mut seen = HashSet::new();
        let before = self.figures.len();
        self.figures.retain_mut(|f| {
            if f.id.trim().is_empty() {
                f.id = format!("F{}", seen.len() + 1);
            }
            if !seen.insert(f.id.clone()) {
                return false;
            }
            match f.kind {
                FigureKind::Diagram => {
                    f.nodes.truncate(16);
                    let mut ids = HashSet::new();
                    f.nodes.retain(|n| {
                        !n.id.is_empty() && !n.label.trim().is_empty() && ids.insert(n.id.clone())
                    });
                    f.edges
                        .retain(|e| ids.contains(&e.from) && ids.contains(&e.to) && e.from != e.to);
                    f.edges.truncate(30);
                    for n in &mut f.nodes {
                        if !n.group.is_empty() && !f.groups.contains(&n.group) {
                            f.groups.push(n.group.clone());
                        }
                    }
                    f.nodes.len() >= 2
                }
                FigureKind::Table => {
                    f.columns.truncate(8);
                    let width = f.columns.len();
                    f.rows.truncate(14);
                    for row in &mut f.rows {
                        row.resize(width, String::new());
                    }
                    width >= 2 && !f.rows.is_empty()
                }
            }
        });
        if self.figures.len() < before {
            notes.push(format!(
                "형식이 맞지 않는 그림 {}개 제외",
                before - self.figures.len()
            ));
        }
        let figures: HashSet<String> = self.figures.iter().map(|f| f.id.clone()).collect();
        for list in self
            .key_points
            .iter_mut()
            .map(|k| &mut k.figure_ids)
            .chain(self.sections.iter_mut().map(|s| &mut s.figure_ids))
        {
            list.retain(|id| figures.contains(id));
            list.dedup();
        }
        notes
    }
    /// Figures no key point or section shows; the reader still sees them, after the sections.
    pub fn unplaced_figures(&self) -> Vec<&Figure> {
        let placed: HashSet<&String> = self
            .key_points
            .iter()
            .flat_map(|k| &k.figure_ids)
            .chain(self.sections.iter().flat_map(|s| &s.figure_ids))
            .collect();
        self.figures
            .iter()
            .filter(|f| !placed.contains(&f.id))
            .collect()
    }
    /// The number readers see for a figure ("그림 2" or "표 1"): diagrams and tables are
    /// numbered separately, by order of first appearance.
    pub fn figure_number(&self, id: &str) -> Option<usize> {
        let kind = self.figures.iter().find(|f| f.id == id)?.kind;
        let mut order: Vec<&String> = vec![];
        for id in self
            .key_points
            .iter()
            .flat_map(|k| &k.figure_ids)
            .chain(self.sections.iter().flat_map(|s| &s.figure_ids))
            .chain(self.unplaced_figures().into_iter().map(|f| &f.id))
        {
            let same = self.figures.iter().any(|f| f.id == *id && f.kind == kind);
            if same && !order.contains(&id) {
                order.push(id);
            }
        }
        order.iter().position(|f| *f == id).map(|i| i + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record;
    fn vocab() -> Vocabulary {
        Vocabulary {
            keys: HashSet::from(["chang2023mixed".to_string(), "kim2021".to_string()]),
            internal: vec![
                ("run-7009ed87_revise-r1-P3".into(), "작업 이론 3".into()),
                ("revise-r1-P3".into(), "작업 이론 3".into()),
                ("K1".into(), "비판 1".into()),
                ("K13".into(), "비판 13".into()),
            ],
        }
    }
    #[test]
    fn internal_ids_are_removed_or_replaced_by_reader_words() {
        let mut notes = vec![];
        let v = vocab();
        assert_eq!(
            v.clean(
                "영역–내부좌표안은 양립합니다(run-7009ed87_revise-r1-P3).",
                &mut notes
            ),
            "영역–내부좌표안은 양립합니다."
        );
        assert_eq!(
            v.clean("K13은 열려 있고 K1은 해소됨, K1x는 그대로", &mut notes),
            "비판 13은 열려 있고 비판 1은 해소됨, K1x는 그대로"
        );
        assert_eq!(
            v.clean("근거 (revise-r1-P3, K1; 원문 참조)", &mut notes),
            "근거 (작업 이론 3, 비판 1; 원문 참조)"
        );
        assert!(!notes.is_empty());
    }
    #[test]
    fn unknown_citation_keys_are_dropped_and_groups_rebuilt() {
        let mut notes = vec![];
        let keys = vocab().keys;
        assert_eq!(
            clean_citations(
                "A [@chang2023mixed; @ghost, p. 3]. B [@ghost]. C [@kim2021, p. 4].",
                &keys,
                &mut notes
            ),
            "A [@chang2023mixed]. B. C [@kim2021, p. 4]."
        );
        assert_eq!(
            clean_citations("Kim [-@kim2021] showed", &keys, &mut notes),
            "Kim [-@kim2021] showed"
        );
        assert_eq!(notes.len(), 2);
        assert_eq!(
            cited_keys("[@a; @b] then [@a]. Email x@y.z is not a citation."),
            ["a", "b"]
        );
    }
    #[test]
    fn sanitize_keeps_figures_consistent() {
        let mut r = record::<ReaderReport>(|r| {
            r.answer = "답 [@nope]".into();
            r.key_points.push(record::<KeyPoint>(|k| {
                k.body = "본문".into();
                k.figure_ids = vec!["F1".into(), "F9".into()];
            }));
            r.figures.push(record::<Figure>(|f| {
                f.id = "F1".into();
                f.nodes = vec![
                    record::<DiagramNode>(|n| {
                        n.id = "a".into();
                        n.label = "뉴런".into();
                        n.group = "미시".into();
                    }),
                    record::<DiagramNode>(|n| {
                        n.id = "b".into();
                        n.label = "센터".into();
                    }),
                ];
                f.edges = vec![
                    record::<DiagramEdge>(|e| {
                        e.from = "a".into();
                        e.to = "b".into();
                    }),
                    record::<DiagramEdge>(|e| {
                        e.from = "a".into();
                        e.to = "missing".into();
                    }),
                ];
            }));
            r.figures.push(record::<Figure>(|f| {
                f.id = "F2".into();
                f.kind = FigureKind::Table;
                f.columns = vec!["안".into(), "내용".into()];
                f.rows = vec![vec!["1".into()]];
            }));
        });
        let notes = r.sanitize(&vocab());
        assert_eq!(r.answer, "답");
        assert_eq!(r.key_points[0].figure_ids, ["F1"]);
        assert_eq!(r.figures[0].edges.len(), 1);
        assert_eq!(r.figures[0].groups, ["미시"]);
        assert_eq!(r.figures[1].rows[0], ["1", ""]);
        assert_eq!(r.figure_number("F2"), Some(1));
        assert_eq!(r.unplaced_figures().len(), 1);
        assert!(notes.iter().any(|n| n.contains("@nope")));
    }
}
