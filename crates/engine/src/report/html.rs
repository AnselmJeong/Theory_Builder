//! A self-contained, offline HTML report: no external scripts, fonts or images. Diagrams
//! are inline SVG (light and dark versions), nodes explain themselves when selected, terms
//! from the glossary show their meaning in place, and key points can be filtered by how
//! firm they are.
use super::figure::{Palette, escape, figure_svg};
use super::{CiteStyle, ReaderView, how_made, today};
use std::collections::HashMap;
use theory_core::*;

fn md(text: &str, view: &ReaderView) -> String {
    let html = markdown::to_html_with_options(text, &markdown::Options::gfm())
        .unwrap_or_else(|_| format!("<p>{}</p>", escape(text)));
    view.references.display(&html, CiteStyle::Html)
}
fn inline(text: &str, view: &ReaderView) -> String {
    let html = md(text, view);
    let t = html.trim();
    match t.strip_prefix("<p>").and_then(|t| t.strip_suffix("</p>")) {
        Some(inner) if !inner.contains("<p>") => inner.to_string(),
        _ => html,
    }
}
fn json_script(value: &serde_json::Value) -> String {
    value.to_string().replace("</", "<\\/")
}
fn standing_class(s: Standing) -> &'static str {
    match s {
        Standing::Supported => "supported",
        Standing::Reasoned => "reasoned",
        Standing::Hypothesis => "hypothesis",
        Standing::Speculation => "speculation",
    }
}
fn figure_html(q: usize, f: &Figure, number: usize, view: &ReaderView) -> String {
    let anchor = format!(
        "q{q}-{}",
        f.id.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
    );
    let caption = format!(
        "<figcaption><b>{} {number}.</b> {}{}</figcaption>",
        if f.kind == FigureKind::Table {
            "표"
        } else {
            "그림"
        },
        escape(&f.title),
        if f.caption.is_empty() {
            String::new()
        } else {
            format!(" <span>{}</span>", inline(&f.caption, view))
        }
    );
    match f.kind {
        FigureKind::Table => {
            let head: String = f
                .columns
                .iter()
                .map(|c| format!("<th>{}</th>", inline(c, view)))
                .collect();
            let rows: String = f
                .rows
                .iter()
                .map(|r| {
                    format!(
                        "<tr>{}</tr>",
                        r.iter()
                            .map(|c| format!("<td>{}</td>", inline(c, view)))
                            .collect::<String>()
                    )
                })
                .collect();
            format!(
                "<figure class=\"fig table\" id=\"{anchor}\">{caption}<div class=\"scroll\"><table><thead><tr>{head}</tr></thead><tbody>{rows}</tbody></table></div></figure>\n"
            )
        }
        FigureKind::Diagram => {
            let svg = |palette: &Palette, theme: &str| {
                figure_svg(f, palette)
                    .map(|(s, _, _)| s.replace("arrow-", &format!("arrow-q{q}-{theme}-")))
                    .unwrap_or_default()
            };
            let notes: serde_json::Map<String, serde_json::Value> = f
                .nodes
                .iter()
                .map(|n| {
                    (
                        n.id.clone(),
                        serde_json::json!({"label":n.label,"note":n.note}),
                    )
                })
                .collect();
            format!(
                "<figure class=\"fig diagram\" id=\"{anchor}\">{caption}<div class=\"svg only-light\">{}</div><div class=\"svg only-dark\">{}</div><p class=\"hint\">상자를 누르면 설명이 나옵니다.</p><div class=\"node-detail\" hidden></div><script type=\"application/json\" class=\"notes\">{}</script></figure>\n",
                svg(&Palette::LIGHT, "l"),
                svg(&Palette::DARK, "d"),
                json_script(&serde_json::Value::Object(notes))
            )
        }
    }
}
fn article(q: usize, run: &Run, view: &ReaderView) -> String {
    let r = &view.report;
    let figure = |id: &str| -> String {
        r.figures
            .iter()
            .find(|f| f.id == id)
            .map(|f| figure_html(q, f, r.figure_number(id).unwrap_or(0), view))
            .unwrap_or_default()
    };
    let mut s = format!(
        "<article class=\"question\" id=\"q{q}\">\n<h1 class=\"title\">{}</h1>\n<p class=\"asked\"><span>질문</span>{}</p>\n",
        escape(r.title.trim()),
        escape(run.question.trim())
    );
    if view.fallback {
        s.push_str("<p class=\"notice\">이 질문은 서기(읽기 쉬운 보고서) 단계가 생기기 전에 실행되어, 토론 종합과 작업 이론을 정리해 보여 줍니다. 앱에서 ‘보고서 작성’을 누르면 다시 쓸 수 있습니다.</p>\n");
    }
    s.push_str(&format!(
        "<section class=\"answer\"><p class=\"label\">답</p><div class=\"prose\">{}</div></section>\n",
        md(&r.answer, view)
    ));
    if !r.key_points.is_empty() {
        s.push_str(&format!(
            "<section id=\"q{q}-points\"><div class=\"section-head\"><h2>핵심 내용</h2><div class=\"filter\" role=\"group\" aria-label=\"확실성으로 거르기\"><button type=\"button\" data-f=\"all\" aria-pressed=\"true\">모두</button>{}</div></div>\n<ol class=\"points\">\n",
            [Standing::Supported, Standing::Reasoned, Standing::Hypothesis, Standing::Speculation]
                .iter()
                .filter(|st| r.key_points.iter().any(|k| k.standing == **st))
                .map(|st| format!(
                    "<button type=\"button\" data-f=\"{}\" aria-pressed=\"false\">{}</button>",
                    standing_class(*st),
                    st.label()
                ))
                .collect::<String>()
        ));
        for k in &r.key_points {
            s.push_str(&format!(
                "<li class=\"point\" data-standing=\"{}\"><span class=\"badge {}\" title=\"{}\">{}</span><h3>{}</h3><div class=\"prose\">{}</div>",
                standing_class(k.standing),
                standing_class(k.standing),
                escape(k.standing.explain()),
                k.standing.label(),
                escape(k.heading.trim()),
                md(&k.body, view)
            ));
            for id in &k.figure_ids {
                s.push_str(&figure(id));
            }
            s.push_str("</li>\n");
        }
        s.push_str("</ol></section>\n");
    }
    if !r.sections.is_empty() {
        s.push_str(&format!(
            "<section id=\"q{q}-explain\"><h2>자세한 설명</h2>\n"
        ));
        for sec in &r.sections {
            s.push_str(&format!(
                "<div class=\"sub\"><h3>{}</h3><div class=\"prose\">{}</div>",
                escape(sec.heading.trim()),
                md(&sec.body, view)
            ));
            for id in &sec.figure_ids {
                s.push_str(&figure(id));
            }
            s.push_str("</div>\n");
        }
        s.push_str("</section>\n");
    }
    let unplaced = r.unplaced_figures();
    if !unplaced.is_empty() {
        s.push_str(&format!("<section id=\"q{q}-figures\"><h2>그림</h2>"));
        for f in unplaced {
            s.push_str(&figure(&f.id));
        }
        s.push_str("</section>\n");
    }
    if !r.glossary.is_empty() {
        s.push_str(&format!(
            "<section id=\"q{q}-terms\"><h2>용어 풀이</h2><p class=\"muted\">본문에서 밑줄 친 용어에 마우스를 올리거나 누르면 뜻이 보입니다.</p><dl class=\"glossary\">"
        ));
        for g in &r.glossary {
            s.push_str(&format!(
                "<div><dt>{}</dt><dd>{}</dd></div>",
                escape(&g.term),
                inline(&g.plain, view)
            ));
        }
        s.push_str("</dl></section>\n");
    }
    if !r.open_questions.is_empty() {
        s.push_str(&format!(
            "<section id=\"q{q}-open\"><h2>아직 확실하지 않은 것</h2><div class=\"cards\">"
        ));
        for oq in &r.open_questions {
            s.push_str(&format!(
                "<div class=\"card\"><p class=\"q\">{}</p>",
                inline(&oq.question, view)
            ));
            if !oq.why_it_matters.is_empty() {
                s.push_str(&format!(
                    "<p><span class=\"k\">왜 중요한가</span>{}</p>",
                    inline(&oq.why_it_matters, view)
                ));
            }
            if !oq.how_to_settle.is_empty() {
                s.push_str(&format!(
                    "<p><span class=\"k\">판가름하려면</span>{}</p>",
                    inline(&oq.how_to_settle, view)
                ));
            }
            s.push_str("</div>");
        }
        s.push_str("</div></section>\n");
    }
    if !r.next_steps.is_empty() {
        s.push_str(&format!(
            "<section id=\"q{q}-next\"><h2>다음에 해 볼 수 있는 일</h2>"
        ));
        for kind in NextKind::ALL {
            let steps: Vec<&NextStep> = r.next_steps.iter().filter(|n| n.kind == kind).collect();
            if steps.is_empty() {
                continue;
            }
            s.push_str(&format!(
                "<h3 class=\"kind\">{}</h3><ul class=\"steps\">",
                kind.label()
            ));
            for n in steps {
                s.push_str(&format!(
                    "<li><p>{}</p>{}</li>",
                    inline(&n.action, view),
                    if n.purpose.is_empty() {
                        String::new()
                    } else {
                        format!("<p class=\"muted\">{}</p>", inline(&n.purpose, view))
                    }
                ));
            }
            s.push_str("</ul>");
        }
        s.push_str("</section>\n");
    }
    let settled = run
        .critiques
        .iter()
        .filter(|c| c.status.is_settled())
        .count();
    let made = if r.how_made.trim().is_empty() {
        how_made(run)
    } else {
        r.how_made.clone()
    };
    s.push_str(&format!(
        "<section id=\"q{q}-how\"><h2>이 답변은 어떻게 만들어졌나</h2><div class=\"prose\">{}</div><ul class=\"stats\"><li><b>{}</b>회 수정</li><li><b>{}</b>개 비판</li><li><b>{settled}</b>개 정리</li><li><b>{}</b>개 남음</li><li><b>{}</b>개 문헌 인용</li></ul>",
        md(&made, view),
        run.deliberation.rounds,
        run.critiques.len(),
        run.critiques.len() - settled,
        view.references.entries.len()
    ));
    if !run.directions.is_empty() {
        s.push_str("<p class=\"muted\">토론을 이어 갈 때 사용자가 준 방향:</p><ul>");
        for d in &run.directions {
            s.push_str(&format!("<li>{}</li>", escape(d.text.trim())));
        }
        s.push_str("</ul>");
    }
    s.push_str("<p class=\"muted\">토론 단계별 기록과 원문 대조 인용은 함께 내보낸 debate-record.md에 있습니다.</p></section>\n");
    let answered: Vec<&FollowUp> = run.followups.iter().filter(|f| f.error.is_none()).collect();
    if !answered.is_empty() {
        s.push_str(&format!(
            "<section id=\"q{q}-asked\"><h2>이어서 물은 것</h2>"
        ));
        for f in answered {
            s.push_str(&format!(
                "<details class=\"followup\"><summary>{}</summary><div class=\"prose\">{}</div></details>",
                escape(f.question.trim()),
                md(&f.answer, view)
            ));
        }
        s.push_str("</section>\n");
    }
    let terms: Vec<serde_json::Value> = r
        .glossary
        .iter()
        .map(|g| serde_json::json!({"term":g.term,"plain":g.plain}))
        .collect();
    s.push_str(&format!(
        "<script type=\"application/json\" class=\"terms\">{}</script>\n</article>\n",
        json_script(&serde_json::Value::Array(terms))
    ));
    s
}
/// The full page for one or more questions.
pub fn page(title: &str, runs: &[(&Run, ReaderView)]) -> String {
    let mut toc = String::new();
    let mut body = String::new();
    let mut references: Vec<(String, BibInfo)> = vec![];
    for (i, (run, view)) in runs.iter().enumerate() {
        let q = i + 1;
        let r = &view.report;
        toc.push_str(&format!(
            "<li><a href=\"#q{q}\">{}</a><ul>{}{}{}{}{}</ul></li>",
            escape(r.title.trim()),
            if r.key_points.is_empty() {
                String::new()
            } else {
                format!("<li><a href=\"#q{q}-points\">핵심 내용</a></li>")
            },
            if r.sections.is_empty() {
                String::new()
            } else {
                format!("<li><a href=\"#q{q}-explain\">자세한 설명</a></li>")
            },
            if r.open_questions.is_empty() {
                String::new()
            } else {
                format!("<li><a href=\"#q{q}-open\">확실하지 않은 것</a></li>")
            },
            if r.next_steps.is_empty() {
                String::new()
            } else {
                format!("<li><a href=\"#q{q}-next\">다음 할 일</a></li>")
            },
            format_args!("<li><a href=\"#q{q}-how\">만들어진 과정</a></li>"),
        ));
        body.push_str(&article(q, run, view));
        for (k, b) in &view.references.entries {
            if !references.iter().any(|(x, _)| x == k) {
                references.push((k.clone(), b.clone()));
            }
        }
    }
    if !references.is_empty() {
        toc.push_str("<li><a href=\"#refs\">참고문헌</a></li>");
        let mut by_key: HashMap<&str, usize> = HashMap::new();
        body.push_str("<section id=\"refs\" class=\"refs\"><h2>참고문헌</h2><ol>");
        for (i, (k, b)) in references.iter().enumerate() {
            by_key.insert(k, i);
            let line = escape(&b.reference());
            // *Journal* → <i>Journal</i>, links clickable.
            let mut html = String::new();
            for (n, part) in line.split('*').enumerate() {
                if n % 2 == 1 {
                    html.push_str(&format!("<i>{part}</i>"));
                } else {
                    html.push_str(part);
                }
            }
            if let Some(link) = b.link() {
                let esc = escape(&link);
                html = html.replace(
                    &esc,
                    &format!("<a href=\"{esc}\" rel=\"noopener\">{esc}</a>"),
                );
            }
            body.push_str(&format!("<li id=\"ref-{k}\">{html} <code>@{k}</code></li>"));
        }
        body.push_str("</ol></section>\n");
    }
    let lang = if title
        .chars()
        .chain(runs.iter().flat_map(|(r, _)| r.question.chars()))
        .any(|c| ('\u{AC00}'..='\u{D7AF}').contains(&c))
    {
        "ko"
    } else {
        "en"
    };
    format!(
        "<!doctype html>\n<html lang=\"{lang}\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<header class=\"top\"><div class=\"wrap\"><p class=\"eyebrow\">Theory Studio · 연구 보고서 · {}</p><p class=\"doc\">{}</p></div></header>\n<div class=\"wrap layout\">\n<nav class=\"toc\" aria-label=\"목차\"><ol>{toc}</ol></nav>\n<main>\n{body}</main>\n</div>\n<footer class=\"wrap\"><p class=\"muted\">두 AI 모델의 토론을 바탕으로 작성한 보고서입니다. 모델 간 합의는 실험적 근거가 아니며, 각 핵심 내용의 확실성 표시를 함께 보세요.</p></footer>\n<script>{SCRIPT}</script>\n</body>\n</html>\n",
        escape(title),
        today(),
        escape(title)
    )
}
const STYLE: &str = r#"
:root{--bg:#fbfbfa;--surface:#ffffff;--text:#1f2328;--muted:#59636e;--line:#d8dee4;--accent:#0969da;--accent-soft:#ddf4ff;
--supported:#1a7f37;--supported-bg:#dafbe1;--reasoned:#0969da;--reasoned-bg:#ddf4ff;--hypothesis:#9a6700;--hypothesis-bg:#fff8c5;--speculation:#6e7781;--speculation-bg:#eef1f4;color-scheme:light}
@media (prefers-color-scheme:dark){:root{--bg:#0d1117;--surface:#161b22;--text:#e6edf3;--muted:#9198a1;--line:#30363d;--accent:#4493f8;--accent-soft:#0c2d6b;
--supported:#3fb950;--supported-bg:#033a16;--reasoned:#4493f8;--reasoned-bg:#0c2d6b;--hypothesis:#d29922;--hypothesis-bg:#3b2300;--speculation:#9198a1;--speculation-bg:#21262d;color-scheme:dark}
.only-light{display:none}.only-dark{display:block}}
@media (prefers-color-scheme:light){.only-dark{display:none}}
*{box-sizing:border-box}
html{-webkit-text-size-adjust:100%}
body{margin:0;background:var(--bg);color:var(--text);font:16px/1.7 "Apple SD Gothic Neo","Noto Sans KR","Pretendard","Malgun Gothic",system-ui,-apple-system,sans-serif;word-break:keep-all;overflow-wrap:anywhere}
.wrap{max-width:1120px;margin:0 auto;padding:0 16px}
.top{border-bottom:1px solid var(--line);background:var(--surface)}
.top .wrap{padding-top:14px;padding-bottom:14px}
.eyebrow{margin:0;font-size:12px;color:var(--muted);letter-spacing:.02em}
.doc{margin:2px 0 0;font-weight:600}
.layout{display:grid;grid-template-columns:220px minmax(0,1fr);gap:40px;padding-top:28px}
.toc{position:sticky;top:20px;align-self:start;font-size:13px;max-height:calc(100vh - 40px);overflow:auto}
.toc ol,.toc ul{list-style:none;margin:0;padding:0}
.toc>ol>li{margin-bottom:14px}.toc>ol>li>a{font-weight:600}
.toc ul{margin-top:4px;padding-left:10px;border-left:1px solid var(--line)}
.toc a{color:var(--text);text-decoration:none;display:block;padding:2px 0}.toc ul a{color:var(--muted)}
.toc a:hover{color:var(--accent)}
main{min-width:0}
article.question{margin-bottom:64px}
h1.title{font-size:30px;line-height:1.3;margin:0 0 10px;letter-spacing:-.01em}
h2{font-size:21px;margin:44px 0 14px;line-height:1.35}
h3{font-size:17px;margin:0 0 6px;line-height:1.45}
.asked{color:var(--muted);margin:0 0 18px;font-size:14.5px}
.asked span{display:inline-block;font-size:11px;font-weight:700;border:1px solid var(--line);border-radius:999px;padding:0 8px;margin-right:8px;color:var(--muted)}
.notice{background:var(--hypothesis-bg);border-radius:10px;padding:10px 14px;font-size:14px}
.answer{background:var(--surface);border:1px solid var(--line);border-left:4px solid var(--accent);border-radius:12px;padding:16px 20px;margin:12px 0 8px}
.answer .label{margin:0 0 4px;font-size:12px;font-weight:700;color:var(--accent);letter-spacing:.04em}
.answer .prose{font-size:17.5px}
.prose p{margin:0 0 .8em}.prose p:last-child{margin-bottom:0}
.prose ul,.prose ol{padding-left:1.3em;margin:.3em 0 .8em}
.section-head{display:flex;flex-wrap:wrap;align-items:baseline;justify-content:space-between;gap:8px 16px}
.section-head h2{margin-bottom:8px}
.filter{display:flex;flex-wrap:wrap;gap:6px}
.filter button{font:inherit;font-size:12.5px;border:1px solid var(--line);background:var(--surface);color:var(--text);border-radius:999px;padding:3px 11px;cursor:pointer}
.filter button[aria-pressed=true]{background:var(--text);color:var(--bg);border-color:var(--text)}
ol.points{list-style:none;margin:0;padding:0;counter-reset:pt}
.point{counter-increment:pt;background:var(--surface);border:1px solid var(--line);border-radius:12px;padding:16px 20px 16px 52px;margin:0 0 12px;position:relative}
.point::before{content:counter(pt);position:absolute;left:18px;top:16px;width:24px;height:24px;border-radius:50%;background:var(--accent-soft);color:var(--accent);font-weight:700;font-size:13px;display:grid;place-items:center}
.badge{display:inline-block;font-size:11.5px;font-weight:700;border-radius:6px;padding:1px 8px;margin-bottom:6px;cursor:help}
.badge.supported{color:var(--supported);background:var(--supported-bg)}.badge.reasoned{color:var(--reasoned);background:var(--reasoned-bg)}
.badge.hypothesis{color:var(--hypothesis);background:var(--hypothesis-bg)}.badge.speculation{color:var(--speculation);background:var(--speculation-bg)}
.sub{margin-bottom:22px}
.fig{margin:16px 0;background:var(--surface);border:1px solid var(--line);border-radius:12px;padding:12px}
.fig figcaption{font-size:14px;margin:0 4px 10px}.fig figcaption span{color:var(--muted)}
.svg svg{display:block;max-width:100%;height:auto;margin:0 auto}
.fig .hint{font-size:12px;color:var(--muted);margin:6px 4px 0}
.node-detail{margin:8px 4px 0;padding:10px 12px;border-radius:8px;background:var(--accent-soft);font-size:14.5px}
g.node{cursor:pointer;outline:none}g.node:focus rect,g.node.active rect{stroke-width:3}
.scroll{overflow-x:auto}
table{border-collapse:collapse;width:100%;font-size:14px}
th,td{border-bottom:1px solid var(--line);padding:8px 10px;text-align:left;vertical-align:top}
th{font-weight:700;background:color-mix(in srgb,var(--line) 35%,transparent)}
dl.glossary{display:grid;grid-template-columns:repeat(auto-fill,minmax(260px,1fr));gap:10px;margin:0}
dl.glossary div{background:var(--surface);border:1px solid var(--line);border-radius:10px;padding:10px 14px}
dt{font-weight:700}dd{margin:2px 0 0;color:var(--muted);font-size:14.5px}
.term{border-bottom:1.5px dotted var(--accent);cursor:help;position:relative}
.term:hover::after,.term:focus::after{content:attr(data-def);position:absolute;left:0;top:calc(100% + 6px);z-index:5;width:max-content;max-width:min(300px,80vw);background:var(--text);color:var(--bg);font-size:13px;line-height:1.5;padding:8px 10px;border-radius:8px;font-weight:400;white-space:normal}
.cards{display:grid;gap:10px}
.card{background:var(--surface);border:1px solid var(--line);border-radius:12px;padding:14px 18px}
.card p{margin:0 0 6px}.card p:last-child{margin:0}.card .q{font-weight:700}
.k{display:inline-block;font-size:11.5px;font-weight:700;color:var(--muted);margin-right:8px}
h3.kind{font-size:14px;color:var(--muted);margin:18px 0 6px;text-transform:none}
ul.steps{list-style:none;padding:0;margin:0}
ul.steps li{border-left:3px solid var(--accent);background:var(--surface);border-radius:0 10px 10px 0;padding:8px 14px;margin-bottom:8px}
ul.steps p{margin:0}
.muted{color:var(--muted);font-size:14px}
ul.stats{list-style:none;display:flex;flex-wrap:wrap;gap:8px;padding:0;margin:12px 0}
ul.stats li{border:1px solid var(--line);border-radius:999px;padding:2px 12px;font-size:13px;background:var(--surface)}
details.followup{background:var(--surface);border:1px solid var(--line);border-radius:10px;padding:10px 14px;margin-bottom:8px}
details.followup summary{cursor:pointer;font-weight:600}
a{color:var(--accent)}a.cite{text-decoration:none;border-bottom:1px solid color-mix(in srgb,var(--accent) 40%,transparent)}
.refs ol{padding-left:1.4em;font-size:14.5px}.refs li{margin-bottom:8px}.refs code{font-size:12px;color:var(--muted)}
footer{padding-bottom:40px}
@media (max-width:860px){.layout{grid-template-columns:1fr;gap:0}.toc{position:static;max-height:none;border-bottom:1px solid var(--line);padding-bottom:12px;margin-bottom:8px}
h1.title{font-size:24px}.point{padding:14px 14px 14px 46px}.point::before{left:12px}}
@media print{.toc,.filter,.hint{display:none}.layout{display:block}.only-dark{display:none}.only-light{display:block}body{background:#fff;color:#000}}
"#;
const SCRIPT: &str = r#"
(function(){
  document.querySelectorAll('article.question').forEach(function(art){
    var buttons=art.querySelectorAll('.filter button');
    buttons.forEach(function(b){b.addEventListener('click',function(){
      buttons.forEach(function(x){x.setAttribute('aria-pressed',String(x===b));});
      var f=b.dataset.f;
      art.querySelectorAll('.point').forEach(function(p){p.hidden=!(f==='all'||p.dataset.standing===f);});
    });});
    var data=art.querySelector('script.terms');var terms=[];
    try{terms=JSON.parse(data?data.textContent:'[]');}catch(e){}
    terms.forEach(function(t){
      if(!t.term)return;
      var walker=document.createTreeWalker(art,NodeFilter.SHOW_TEXT,{acceptNode:function(n){
        var p=n.parentElement;
        if(!p||!p.closest('.prose')||p.closest('.term,a,code,dl.glossary,h1,h2,h3,figure'))return NodeFilter.FILTER_REJECT;
        return NodeFilter.FILTER_ACCEPT;}});
      var n;
      while((n=walker.nextNode())){
        var i=n.data.indexOf(t.term);
        if(i<0)continue;
        var r=document.createRange();r.setStart(n,i);r.setEnd(n,i+t.term.length);
        var span=document.createElement('span');span.className='term';span.tabIndex=0;span.dataset.def=t.plain;
        r.surroundContents(span);break;
      }
    });
  });
  document.querySelectorAll('figure.diagram').forEach(function(fig){
    var notes={};try{notes=JSON.parse(fig.querySelector('script.notes').textContent);}catch(e){}
    var box=fig.querySelector('.node-detail');
    fig.querySelectorAll('g.node').forEach(function(g){
      g.setAttribute('role','button');
      var show=function(){
        var n=notes[g.dataset.node];if(!n)return;
        box.hidden=false;box.textContent='';
        var b=document.createElement('b');b.textContent=n.label;box.appendChild(b);
        if(n.note){box.appendChild(document.createTextNode(' — '+n.note));}
        fig.querySelectorAll('g.node').forEach(function(x){x.classList.toggle('active',x.dataset.node===g.dataset.node);});
      };
      g.addEventListener('click',show);
      g.addEventListener('keydown',function(e){if(e.key==='Enter'||e.key===' '){e.preventDefault();show();}});
    });
  });
})();
"#;
