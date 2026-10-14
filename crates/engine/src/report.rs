//! Reader-facing output for a question: the scribe's report (or a plain fallback for runs
//! made before the scribe existed), citations as pandoc keys backed by a BibTeX file,
//! figures as SVG, a self-contained interactive HTML page, and the debate record kept as
//! a separate appendix. Source documents themselves are never copied into a report.
pub mod figure;
mod html;
use crate::execution::reader::{citation_keys, cited_sources, internal_names, theory_evidence};
use crate::providers::ProviderKind;
pub use figure::{Palette, figure_svg};
pub use html::page as html_page;
use std::collections::HashMap;
use theory_core::*;

/// The bibliography behind a set of reports: key → record, in order of first citation.
#[derive(Clone, Default)]
pub struct References {
    pub entries: Vec<(String, BibInfo)>,
}
impl References {
    /// Records for `keys`, looked up among the project's sources (frozen or provisional).
    pub fn new(p: &Project, keys: &[(String, String)]) -> Self {
        let mut entries: Vec<(String, BibInfo)> = vec![];
        for (source_id, key) in keys {
            if entries.iter().any(|(k, _)| k == key) {
                continue;
            }
            if let Some(s) = p.sources.iter().find(|s| s.id == *source_id) {
                entries.push((key.clone(), s.derived_bib()));
            }
        }
        Self { entries }
    }
    pub fn get(&self, key: &str) -> Option<&BibInfo> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, b)| b)
    }
    /// Only the entries a text actually cites, in order of first citation.
    pub fn cited_in(&self, keys: &[String]) -> Self {
        Self {
            entries: keys
                .iter()
                .filter_map(|k| self.get(k).map(|b| (k.clone(), b.clone())))
                .collect(),
        }
    }
    pub fn bibtex(&self) -> String {
        let mut out = String::from("% Theory Studio — references cited in report.md\n\n");
        for (key, b) in &self.entries {
            out.push_str(&b.bibtex(key));
            out.push('\n');
        }
        out
    }
    /// Replace `[@a; @b, p. 4]` with readable author–year text.
    pub fn display(&self, text: &str, style: CiteStyle) -> String {
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        for (start, end, inner) in citation_groups(text) {
            out.push_str(&text[last..start]);
            let items: Vec<String> = inner
                .split(';')
                .filter_map(|item| {
                    let item = item.trim().trim_start_matches('-');
                    let body = item.strip_prefix('@')?;
                    let end = body
                        .find(|c: char| !(c.is_ascii_alphanumeric() || "_-:./".contains(c)))
                        .unwrap_or(body.len());
                    let key = body[..end].trim_end_matches(['.', ':', '/']);
                    let locator = &body[key.len()..];
                    let b = self.get(key)?;
                    let label = format!("{}{locator}", b.short_cite());
                    Some(match style {
                        CiteStyle::Text => label,
                        CiteStyle::Markdown => match b.link() {
                            Some(url) => format!("[{}]({url})", label.replace(['[', ']'], "")),
                            None => label,
                        },
                        CiteStyle::Html => format!(
                            "<a class=\"cite\" href=\"#ref-{key}\" title=\"{}\">{}</a>",
                            figure::escape(&b.reference().replace('*', "")),
                            figure::escape(&label)
                        ),
                    })
                })
                .collect();
            if items.is_empty() {
                let trimmed = out.trim_end_matches(' ').len();
                out.truncate(trimmed);
            } else {
                out.push_str(&format!("({})", items.join("; ")));
            }
            last = end;
        }
        out.push_str(&text[last..]);
        out
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CiteStyle {
    Text,
    /// Author–year linked to DOI/PubMed where known (for in-app markdown).
    Markdown,
    Html,
}
/// What the reader sees for a run, and whether it is the plain fallback.
pub struct ReaderView {
    pub report: ReaderReport,
    pub fallback: bool,
    pub references: References,
}
/// Citation keys a run's report uses: frozen keys of cited sources, provisional otherwise.
pub fn run_keys(p: &Project, run: &Run) -> Vec<(String, String)> {
    let cited = cited_sources(run);
    let keys = citation_keys(p, &cited);
    cited
        .into_iter()
        .filter_map(|id| keys.get(&id).map(|k| (id, k.clone())))
        .collect()
}
pub fn reader_view(p: &Project, run: &Run) -> ReaderView {
    let keys = run_keys(p, run);
    let all = References::new(p, &keys);
    match &run.reading {
        Some(r) => {
            let mut report = r.report.clone();
            // Reports stored before figures were de-duplicated may repeat a figure.
            report.tidy_figures();
            let cited = report.citations();
            ReaderView {
                references: all.cited_in(&cited),
                report,
                fallback: false,
            }
        }
        None => {
            let report = fallback(p, run, &keys);
            let cited = report.citations();
            ReaderView {
                references: all.cited_in(&cited),
                report,
                fallback: true,
            }
        }
    }
}
fn first_sentence(text: &str, n: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut = flat
        .char_indices()
        .find(|(i, c)| {
            matches!(c, '.' | '。' | '?' | '!')
                && flat[i + c.len_utf8()..].starts_with(' ')
                && *i > 10
        })
        .map(|(i, c)| &flat[..i + c.len_utf8()])
        .unwrap_or(&flat);
    if cut.chars().count() > n {
        format!("{}…", cut.chars().take(n).collect::<String>())
    } else {
        cut.to_string()
    }
}
/// How the answer was produced, in plain words (no model output involved).
pub fn how_made(run: &Run) -> String {
    let d = &run.deliberation;
    let settled = run
        .critiques
        .iter()
        .filter(|c| c.status.is_settled())
        .count();
    let pairing = if d.cross_model() {
        format!(
            "{}가 이론을 세우고 {}가 비판하는",
            ProviderKind::from_label(&d.author).display(),
            ProviderKind::from_label(&d.critic).display()
        )
    } else if d.author.is_empty() {
        "한 모델이 분석하고 별도 문맥에서 검토하는".into()
    } else {
        format!(
            "{}가 이론을 세우고 별도 문맥의 같은 모델이 비판하는",
            ProviderKind::from_label(&d.author).display()
        )
    };
    let mut s = format!(
        "두 AI 문맥이 서로 다른 역할을 맡아, {pairing} 토론을 {}회 수정까지 진행했습니다. 제기된 비판 {}개 중 {settled}개가 정리되었고 {}개는 남아 있습니다.",
        d.rounds,
        run.critiques.len(),
        run.critiques.len() - settled
    );
    if let Some(stop) = d.stop {
        s.push_str(&format!(" 토론은 '{}'로 끝났습니다.", stop.label()));
    }
    // Runs made before the writer role were synthesised by the author.
    let synthesis = if d.writer.is_empty() {
        &d.author
    } else {
        &d.writer
    };
    let report = run.reading.as_ref().map_or("", |r| r.provider.as_str());
    let name = |p: &str| ProviderKind::from_label(p).display();
    if !report.is_empty() && (report == synthesis || synthesis.is_empty()) {
        s.push_str(&format!(" 종합과 이 보고서는 {}가 썼습니다.", name(report)));
    } else if !report.is_empty() {
        s.push_str(&format!(
            " 종합은 {}가, 이 보고서는 {}가 썼습니다.",
            name(synthesis),
            name(report)
        ));
    } else if !synthesis.is_empty() {
        s.push_str(&format!(" 종합은 {}가 썼습니다.", name(synthesis)));
    }
    s.push_str(" 수치 시뮬레이션은 수행하지 않았고, 두 모델의 합의는 실험적 근거가 아닙니다.");
    s
}
/// A plain report assembled from validated results, for runs without a scribe report.
/// Nothing is invented: it reorders the synthesis, the working theory and its citations.
fn fallback(p: &Project, run: &Run, keys: &[(String, String)]) -> ReaderReport {
    let key_of: HashMap<&str, &str> = keys.iter().map(|(s, k)| (s.as_str(), k.as_str())).collect();
    let vocabulary = Vocabulary {
        keys: keys.iter().map(|(_, k)| k.clone()).collect(),
        internal: internal_names(p, run),
    };
    let mut notes = vec![];
    let mut clean = |t: &str| vocabulary.clean(t, &mut notes);
    let output = run.final_output();
    let summary = output
        .map(|o| o.summary.clone())
        .unwrap_or_else(|| run.report.clone());
    let summary = clean(&summary);
    let mut parts = summary.splitn(2, "\n\n");
    let answer = parts.next().unwrap_or_default().trim().to_string();
    let rest = parts.next().unwrap_or_default().trim().to_string();
    let evidence = theory_evidence(run);
    let key_points = run
        .theory()
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut cites: Vec<&str> = vec![];
            for (_, e, _) in evidence.iter().filter(|(h, _, _)| *h == t.id) {
                if let Some(k) = key_of.get(e.source_id.as_str())
                    && !cites.contains(k)
                {
                    cites.push(k);
                }
            }
            let statement = clean(&t.statement);
            let mut body = statement.clone();
            if !cites.is_empty() {
                body.push_str(&format!(
                    " [{}]",
                    cites
                        .iter()
                        .map(|k| format!("@{k}"))
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }
            if !t.scope.trim().is_empty() {
                body.push_str(&format!("\n\n*범위:* {}", clean(&t.scope)));
            }
            record::<KeyPoint>(|k| {
                k.heading = format!("{}. {}", i + 1, first_sentence(&statement, 70));
                k.body = body;
                k.standing = match (t.kind, t.claim_kind, cites.is_empty()) {
                    (EntryKind::Term, _, _) | (_, ClaimKind::Deduction, _) => Standing::Reasoned,
                    (_, ClaimKind::ReportedFinding | ClaimKind::Observation, false) => {
                        Standing::Supported
                    }
                    (_, ClaimKind::Speculation, _) => Standing::Speculation,
                    _ => Standing::Hypothesis,
                };
            })
        })
        .collect();
    let mut sections = vec![];
    if !rest.is_empty() {
        sections.push(record::<Section>(|s| {
            s.heading = "토론 종합".into();
            s.body = rest;
        }));
    }
    if let Some(o) = output
        && !o.next_actions.is_empty()
    {
        sections.push(record::<Section>(|s| {
            s.heading = "토론이 제안한 다음 확인".into();
            s.body = o
                .next_actions
                .iter()
                .map(|a| format!("- {}", clean(a)))
                .collect::<Vec<_>>()
                .join("\n");
        }));
    }
    let open_questions = output
        .map(|o| {
            o.unresolved
                .iter()
                .map(|u| record::<OpenQuestion>(|q| q.question = clean(u)))
                .collect()
        })
        .unwrap_or_default();
    record::<ReaderReport>(|r| {
        r.title = first_sentence(&run.question, 60);
        r.answer = answer;
        r.key_points = key_points;
        r.sections = sections;
        r.open_questions = open_questions;
        r.how_made = how_made(run);
    })
}
fn civil_date(secs: u64) -> String {
    // Howard Hinnant's days → civil date.
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}
pub fn today() -> String {
    civil_date(now())
}
fn yaml_string(s: &str) -> String {
    format!(
        "\"{}\"",
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', " ")
    )
}
fn table_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}
/// Markdown for a table figure (pipe table with caption line).
fn table_markdown(f: &Figure, number: usize) -> String {
    let mut s = format!("**표 {number}. {}**", f.title);
    if !f.caption.is_empty() {
        s.push_str(&format!(" — {}", f.caption));
    }
    s.push_str("\n\n| ");
    s.push_str(
        &f.columns
            .iter()
            .map(|c| table_cell(c))
            .collect::<Vec<_>>()
            .join(" | "),
    );
    s.push_str(" |\n|");
    s.push_str(&" --- |".repeat(f.columns.len()));
    s.push('\n');
    for row in &f.rows {
        s.push_str("| ");
        s.push_str(
            &row.iter()
                .map(|c| table_cell(c))
                .collect::<Vec<_>>()
                .join(" | "),
        );
        s.push_str(" |\n");
    }
    s
}
/// Pandoc markdown for one run's report. Citations stay `[@key]`; `figure_path` names the
/// SVG file written for each diagram.
pub fn run_markdown(
    run: &Run,
    view: &ReaderView,
    level: usize,
    figure_path: &dyn Fn(&Figure) -> String,
) -> String {
    let h = |n: usize| "#".repeat(level + n);
    let r = &view.report;
    let mut s = format!("{} {}\n\n", h(0), r.title.trim());
    s.push_str(&format!("**질문.** {}\n\n", run.question.trim()));
    if view.fallback {
        s.push_str("> *이 질문은 서기(읽기 쉬운 보고서) 단계가 생기기 전에 실행되어, 토론 종합과 작업 이론을 정리해 보여 줍니다. 앱의 보고서 탭에서 ‘보고서 작성’을 누르면 다시 쓸 수 있습니다.*\n\n");
    }
    s.push_str(&format!(
        "> **답.** {}\n\n",
        r.answer.trim().replace('\n', "\n> ")
    ));
    let figure_block = |id: &str| -> String {
        let Some(f) = r.figures.iter().find(|f| f.id == id) else {
            return String::new();
        };
        let n = r.figure_number(id).unwrap_or(0);
        match f.kind {
            FigureKind::Diagram => {
                let mut caption = format!("그림 {n}. {}", f.title);
                if !f.caption.is_empty() {
                    caption.push_str(&format!(" — {}", f.caption));
                }
                let mut s = format!(
                    "![{}]({})\n",
                    caption.replace(['[', ']'], ""),
                    figure_path(f)
                );
                let notes: Vec<String> = f
                    .nodes
                    .iter()
                    .filter(|n| !n.note.is_empty())
                    .map(|n| format!("- **{}**: {}", n.label, n.note))
                    .collect();
                if !notes.is_empty() {
                    s.push_str(&format!("\n{}\n", notes.join("\n")));
                }
                s
            }
            FigureKind::Table => table_markdown(f, n),
        }
    };
    if !r.key_points.is_empty() {
        s.push_str(&format!("{} 핵심 내용\n\n", h(1)));
        for k in &r.key_points {
            s.push_str(&format!(
                "{} {}\n\n*확실성: {}* — {}\n\n{}\n\n",
                h(2),
                k.heading.trim(),
                k.standing.label(),
                k.standing.explain(),
                k.body.trim()
            ));
        }
        for f in r.key_point_figures() {
            s.push_str(&figure_block(&f.id));
            s.push('\n');
        }
    }
    if !r.sections.is_empty() {
        s.push_str(&format!("{} 자세한 설명\n\n", h(1)));
        for sec in &r.sections {
            s.push_str(&format!(
                "{} {}\n\n{}\n\n",
                h(2),
                sec.heading.trim(),
                sec.body.trim()
            ));
            for id in &sec.figure_ids {
                s.push_str(&figure_block(id));
                s.push('\n');
            }
        }
    }
    for f in r.unplaced_figures() {
        s.push_str(&figure_block(&f.id));
        s.push('\n');
    }
    if !r.glossary.is_empty() {
        s.push_str(&format!(
            "{} 용어 풀이\n\n| 용어 | 뜻 |\n| --- | --- |\n",
            h(1)
        ));
        for g in &r.glossary {
            s.push_str(&format!(
                "| {} | {} |\n",
                table_cell(&g.term),
                table_cell(&g.plain)
            ));
        }
        s.push('\n');
    }
    if !r.open_questions.is_empty() {
        s.push_str(&format!("{} 아직 확실하지 않은 것\n\n", h(1)));
        for (i, q) in r.open_questions.iter().enumerate() {
            s.push_str(&format!("{}. **{}**\n", i + 1, q.question.trim()));
            if !q.why_it_matters.is_empty() {
                s.push_str(&format!("   - 왜 중요한가: {}\n", q.why_it_matters.trim()));
            }
            if !q.how_to_settle.is_empty() {
                s.push_str(&format!("   - 판가름하려면: {}\n", q.how_to_settle.trim()));
            }
        }
        s.push('\n');
    }
    if !r.next_steps.is_empty() {
        s.push_str(&format!("{} 다음에 해 볼 수 있는 일\n\n", h(1)));
        for kind in NextKind::ALL {
            let steps: Vec<&NextStep> = r.next_steps.iter().filter(|n| n.kind == kind).collect();
            if steps.is_empty() {
                continue;
            }
            s.push_str(&format!("**{}**\n\n", kind.label()));
            for n in steps {
                s.push_str(&format!("- {}", n.action.trim()));
                if !n.purpose.is_empty() {
                    s.push_str(&format!(" — *{}*", n.purpose.trim()));
                }
                s.push('\n');
            }
            s.push('\n');
        }
    }
    s.push_str(&format!(
        "{} 이 답변은 어떻게 만들어졌나\n\n{}\n\n",
        h(1),
        if r.how_made.trim().is_empty() {
            how_made(run)
        } else {
            r.how_made.trim().to_string()
        }
    ));
    if !run.directions.is_empty() {
        s.push_str("토론을 이어 갈 때 사용자가 준 방향:\n\n");
        for d in &run.directions {
            s.push_str(&format!("- {}\n", d.text.trim()));
        }
        s.push('\n');
    }
    if !run.followups.iter().all(|f| f.error.is_some()) {
        s.push_str(&format!("{} 이어서 물은 것\n\n", h(1)));
        for f in run.followups.iter().filter(|f| f.error.is_none()) {
            s.push_str(&format!(
                "**문.** {}\n\n{}\n\n",
                f.question.trim(),
                f.answer.trim()
            ));
        }
    }
    s
}
/// The process record: debate steps, critique ledger, verified quotes and the claims
/// ledger, with citations as keys. No source text beyond the short quoted passages.
pub fn debate_record(p: &Project, runs: &[&Run], keys: &HashMap<String, String>) -> String {
    let who = |label: &str| ProviderKind::from_label(label).display();
    let cite = |source_id: &str| match keys.get(source_id) {
        Some(k) => format!("[@{k}]"),
        None => p
            .sources
            .iter()
            .find(|s| s.id == source_id)
            .map(|s| format!("“{}”", s.derived_bib().title))
            .unwrap_or_else(|| "(출처 기록 없음)".into()),
    };
    let mut out = format!(
        "---\ntitle: {}\nsubtitle: \"부록 — 토론과 검증 기록\"\ndate: {}\nbibliography: references.bib\n---\n\n이 문서는 보고서(report.md)의 근거가 된 과정 기록입니다. 두 모델의 토론 단계, 비판과 그 처리, 원문과 글자 단위로 대조한 인용, 연구 노트의 항목을 담습니다. 사용자가 제공한 자료의 본문은 담지 않습니다.\n\n",
        yaml_string(&p.title),
        today()
    );
    for r in runs {
        out.push_str(&format!(
            "# {}\n\n상태: {} · 호출 {}/{}\n\n## 토론 종합 (원문)\n\n{}\n\n",
            r.question.trim(),
            r.state.label(),
            r.calls,
            r.call_limit(),
            r.report
        ));
        for l in &r.limitations {
            out.push_str(&format!("- 메모: {l}\n"));
        }
        for d in &r.directions {
            out.push_str(&format!("- 사용자 방향 ({}차부터): {}\n", d.round, d.text));
        }
        let d = &r.deliberation;
        if !d.author.is_empty() {
            out.push_str(&format!(
                "\n## 토론 기록\n\n구성 {} · 비판 {} · 수정 {}회{}\n\n",
                who(&d.author),
                who(&d.critic),
                d.rounds,
                d.stop
                    .map(|s| format!(" · {}", s.label()))
                    .unwrap_or_default()
            ));
            for a in r
                .attempts
                .iter()
                .filter(|a| a.state == TaskState::Succeeded)
            {
                if let (Some(phase), Some(o)) = (a.phase, &a.output)
                    && phase != Phase::Plan
                {
                    let step = if a.round > 0 {
                        format!("{}차 {}", a.round, phase.label())
                    } else {
                        phase.label().to_string()
                    };
                    out.push_str(&format!(
                        "- **{} · {step}**: {}\n",
                        who(&a.provider),
                        o.summary.lines().next().unwrap_or_default()
                    ));
                }
            }
        }
        if !r.critiques.is_empty() {
            out.push_str("\n## 비판 원장\n\n");
            for c in &r.critiques {
                out.push_str(&format!(
                    "- **{}** [{} · {}] {} (대상 `{}`, {} 제기)\n",
                    c.id,
                    c.critique.severity,
                    c.status.label(),
                    c.critique.reasoning,
                    if c.critique.target_id.is_empty() {
                        "초안 전체"
                    } else {
                        &c.critique.target_id
                    },
                    who(&c.provider)
                ));
                for e in &c.events {
                    out.push_str(&format!(
                        "  - {}차 {} {}: {}\n",
                        e.round,
                        who(&e.provider),
                        e.stance
                            .map(|s| s.label())
                            .or(e.status.map(|s| s.label()))
                            .unwrap_or_default(),
                        e.reasoning
                    ));
                }
            }
        }
        let quotes: Vec<&Evidence> = r
            .attempts
            .iter()
            .filter(|a| a.state == TaskState::Succeeded)
            .filter_map(|a| a.output.as_ref())
            .flat_map(|o| o.evidence_refs.iter())
            .collect();
        if !quotes.is_empty() {
            out.push_str("\n## 원문과 대조한 인용\n");
            for e in quotes {
                out.push_str(&format!(
                    "\n> {}\n\n{} 행 {}–{} · {} · 주장 `{}`: {}\n",
                    e.quote.trim().replace('\n', "\n> "),
                    cite(&e.source_id),
                    e.start_line,
                    e.end_line,
                    relation_label(e.relation),
                    e.claim_id,
                    e.reasoning
                ));
            }
        }
        out.push('\n');
    }
    let ids: Vec<&str> = runs.iter().map(|r| r.id.as_str()).collect();
    let entries: Vec<&Entry> = p
        .entries
        .iter()
        .filter(|e| ids.contains(&e.run_id.as_str()))
        .collect();
    if !entries.is_empty() {
        out.push_str("# 연구 노트 항목 (채택 전 제안 포함)\n\n");
        for e in entries {
            out.push_str(&format!(
                "- **{}** ({:?}, {:?}, {:?}): {}\n  - 범위: {}\n  - 근거/도입 이유: {}\n  - 의존: {}\n",
                e.proposal.id,
                e.proposal.kind,
                e.adoption,
                e.review,
                e.proposal.statement,
                e.proposal.scope,
                e.proposal.rationale,
                e.proposal.depends_on.join(", ")
            ));
        }
    }
    out.push_str("\n# 참고문헌\n\n::: {#refs}\n:::\n");
    out
}
pub fn relation_label(r: EvidenceRelation) -> &'static str {
    match r {
        EvidenceRelation::Supports => "지지",
        EvidenceRelation::PartialSupport => "부분 지지",
        EvidenceRelation::Contradicts => "반박",
        EvidenceRelation::Related => "관련",
        EvidenceRelation::Indeterminate => "판정 불가",
    }
}
/// A complete pandoc document for the given runs, with YAML front matter.
pub fn document(
    title: &str,
    runs: &[(&Run, ReaderView)],
    figure_path: &dyn Fn(usize, &Figure) -> String,
) -> String {
    let lang = if title
        .chars()
        .any(|c| ('\u{AC00}'..='\u{D7AF}').contains(&c))
        || runs.iter().any(|(r, _)| {
            r.question
                .chars()
                .any(|c| ('\u{AC00}'..='\u{D7AF}').contains(&c))
        }) {
        "ko"
    } else {
        "en"
    };
    let mut s = format!(
        "---\ntitle: {}\ndate: {}\nlang: {lang}\nbibliography: references.bib\nlink-citations: true\n---\n\n",
        yaml_string(title),
        today()
    );
    for (i, (run, view)) in runs.iter().enumerate() {
        s.push_str(&run_markdown(run, view, 1, &|f| figure_path(i, f)));
        s.push('\n');
    }
    s.push_str("# 참고문헌\n\n::: {#refs}\n:::\n\n---\n\n*Theory Studio에서 내보냄. 토론 과정과 원문 대조 기록은 debate-record.md에 있습니다.*\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dates_and_citation_display() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(1_760_400_000), "2025-10-14");
        let refs = References {
            entries: vec![("chang2023mixed".into(), {
                let mut b = BibInfo::default();
                b.title = "Mixed".into();
                b.authors = vec!["Chang, S".into(), "Chou, T".into()];
                b.year = "2023".into();
                b.doi = "10.1/x".into();
                b
            })],
        };
        assert_eq!(
            refs.display("A [@chang2023mixed, p. 4]. B [@gone].", CiteStyle::Text),
            "A (Chang & Chou, 2023, p. 4). B."
        );
        assert_eq!(
            refs.display("[@chang2023mixed]", CiteStyle::Markdown),
            "([Chang & Chou, 2023](https://doi.org/10.1/x))"
        );
        assert!(
            refs.display("[@chang2023mixed]", CiteStyle::Html)
                .contains("href=\"#ref-chang2023mixed\"")
        );
    }
}
