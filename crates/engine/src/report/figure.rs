//! Diagrams as standalone SVG: ordered groups become horizontal bands (levels or stages),
//! nodes are laid out per band, and edges are drawn as curves with short labels. The same
//! SVG is shown in the app, embedded in the HTML report and written next to report.md.
use std::collections::HashMap;
use theory_core::{Figure, FigureKind};

#[derive(Clone, Copy)]
pub struct Palette {
    /// Draw a rounded background panel (standalone files); off when embedded in a card.
    pub frame: bool,
    pub background: &'static str,
    pub text: &'static str,
    pub muted: &'static str,
    pub line: &'static str,
    pub band: &'static str,
    pub fills: [&'static str; 6],
    pub strokes: [&'static str; 6],
}
impl Palette {
    pub const LIGHT: Self = Self {
        frame: true,
        background: "#ffffff",
        text: "#1f2328",
        muted: "#59636e",
        line: "#6e7781",
        band: "#f3f5f8",
        fills: [
            "#ddf4ff", "#dafbe1", "#fff1e5", "#fbefff", "#fff8c5", "#ffebe9",
        ],
        strokes: [
            "#0969da", "#1a7f37", "#bc4c00", "#8250df", "#9a6700", "#cf222e",
        ],
    };
    /// For drawing inside an app card that already has a background.
    pub fn embedded(self) -> Self {
        Self {
            frame: false,
            ..self
        }
    }
    pub const DARK: Self = Self {
        frame: true,
        background: "#0d1117",
        text: "#e6edf3",
        muted: "#9198a1",
        line: "#8b949e",
        band: "#161b22",
        fills: [
            "#0c2d6b", "#033a16", "#3d1d00", "#271052", "#3b2300", "#490202",
        ],
        strokes: [
            "#4493f8", "#3fb950", "#f0883e", "#a371f7", "#d29922", "#f85149",
        ],
    };
}
const FONT: f32 = 13.0;
const LINE: f32 = 18.0;
const PAD_X: f32 = 12.0;
const PAD_Y: f32 = 9.0;
const NODE_MAX: f32 = 168.0;
const GAP_X: f32 = 26.0;
const GAP_Y: f32 = 64.0;
const MARGIN: f32 = 18.0;
const BAND_HEAD: f32 = 24.0;
const FAMILY: &str = "'Apple SD Gothic Neo','Noto Sans CJK KR','Noto Sans KR','Malgun Gothic','Segoe UI',system-ui,sans-serif";

fn char_width(c: char, size: f32) -> f32 {
    let wide = matches!(c as u32,
        0x1100..=0x11FF | 0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF00..=0xFF60);
    size * if wide {
        1.0
    } else if c.is_ascii_uppercase() || c == 'm' || c == 'w' {
        0.68
    } else if c == ' ' || c == 'i' || c == 'l' || c == '.' || c == ',' {
        0.3
    } else {
        0.56
    }
}
pub fn text_width(s: &str, size: f32) -> f32 {
    s.chars().map(|c| char_width(c, size)).sum()
}
/// Greedy wrap by words; long words (or CJK runs) break between characters.
pub fn wrap(text: &str, max: f32, size: f32) -> Vec<String> {
    let mut lines: Vec<String> = vec![];
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if text_width(&candidate, size) <= max {
            line = candidate;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        if text_width(word, size) <= max {
            line = word.to_string();
            continue;
        }
        for c in word.chars() {
            if text_width(&format!("{line}{c}"), size) > max && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            line.push(c);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.len() > 4 {
        lines.truncate(4);
        if let Some(last) = lines.last_mut() {
            last.push('…');
        }
    }
    lines
}
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
struct Placed {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    lines: Vec<String>,
    color: usize,
}
/// Layer of every node: its group's position, or longest-path depth when there are no groups.
fn layers(fig: &Figure) -> Vec<usize> {
    let index: HashMap<&str, usize> = fig
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    if !fig.groups.is_empty() {
        let ungrouped = fig.groups.len();
        return fig
            .nodes
            .iter()
            .map(|n| {
                fig.groups
                    .iter()
                    .position(|g| *g == n.group)
                    .unwrap_or(ungrouped)
            })
            .collect();
    }
    let mut layer = vec![0usize; fig.nodes.len()];
    for _ in 0..fig.nodes.len() {
        let mut changed = false;
        for e in &fig.edges {
            let (Some(&a), Some(&b)) = (index.get(e.from.as_str()), index.get(e.to.as_str()))
            else {
                continue;
            };
            if layer[b] <= layer[a] && layer[a] + 1 < fig.nodes.len() {
                layer[b] = layer[a] + 1;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    layer
}
/// The SVG for a diagram figure with its width and height. Tables are not drawn here.
pub fn figure_svg(fig: &Figure, palette: &Palette) -> Option<(String, f32, f32)> {
    if fig.kind != FigureKind::Diagram || fig.nodes.is_empty() {
        return None;
    }
    let layer = layers(fig);
    let bands = layer.iter().max().copied().unwrap_or(0) + 1;
    let index: HashMap<&str, usize> = fig
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    // Order within each band: given order, then two barycenter sweeps against the band above.
    let mut rows: Vec<Vec<usize>> = vec![vec![]; bands];
    for (i, l) in layer.iter().enumerate() {
        rows[*l].push(i);
    }
    for _ in 0..2 {
        for b in 1..bands {
            let above: HashMap<usize, usize> = rows[b - 1]
                .iter()
                .enumerate()
                .map(|(pos, n)| (*n, pos))
                .collect();
            let mut keyed: Vec<(f32, usize, usize)> = rows[b]
                .iter()
                .enumerate()
                .map(|(pos, n)| {
                    let parents: Vec<f32> = fig
                        .edges
                        .iter()
                        .filter_map(|e| {
                            let (a, t) = (index.get(e.from.as_str())?, index.get(e.to.as_str())?);
                            if *t == *n {
                                above.get(a).map(|p| *p as f32)
                            } else if *a == *n {
                                above.get(t).map(|p| *p as f32)
                            } else {
                                None
                            }
                        })
                        .collect();
                    let key = if parents.is_empty() {
                        pos as f32
                    } else {
                        parents.iter().sum::<f32>() / parents.len() as f32
                    };
                    (key, pos, *n)
                })
                .collect();
            keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            rows[b] = keyed.into_iter().map(|(_, _, n)| n).collect();
        }
    }
    let banded = !fig.groups.is_empty();
    let sizes: Vec<(f32, f32, Vec<String>)> = fig
        .nodes
        .iter()
        .map(|n| {
            let lines = wrap(&n.label, NODE_MAX - 2.0 * PAD_X, FONT);
            let w = lines
                .iter()
                .map(|l| text_width(l, FONT))
                .fold(0.0, f32::max)
                + 2.0 * PAD_X;
            (w.max(72.0), lines.len() as f32 * LINE + 2.0 * PAD_Y, lines)
        })
        .collect();
    let row_width = |row: &Vec<usize>| -> f32 {
        row.iter().map(|n| sizes[*n].0).sum::<f32>() + GAP_X * row.len().saturating_sub(1) as f32
    };
    let band_label_width = if banded {
        fig.groups
            .iter()
            .map(|g| text_width(g, 11.5) + 24.0)
            .fold(0.0, f32::max)
    } else {
        0.0
    };
    let content = rows
        .iter()
        .map(row_width)
        .fold(0.0, f32::max)
        .max(band_label_width);
    let width = content + 2.0 * MARGIN + if banded { 2.0 * 12.0 } else { 0.0 };
    let mut placed: Vec<Option<Placed>> = (0..fig.nodes.len()).map(|_| None).collect();
    let mut band_boxes = vec![];
    let mut y = MARGIN;
    for (b, row) in rows.iter().enumerate() {
        let head = if banded { BAND_HEAD } else { 0.0 };
        let h = row.iter().map(|n| sizes[*n].1).fold(0.0, f32::max);
        let top = y;
        let mut x = (width - row_width(row)) / 2.0;
        for n in row {
            let (w, nh, lines) = sizes[*n].clone();
            placed[*n] = Some(Placed {
                x,
                y: top + head + 10.0 + (h - nh) / 2.0,
                w,
                h: nh,
                lines,
                color: if banded { b } else { layer[*n] } % 6,
            });
            x += w + GAP_X;
        }
        let band_h = head + h + 20.0;
        band_boxes.push((top, band_h));
        y = top + band_h + GAP_Y - 20.0;
    }
    let height = y - GAP_Y + 20.0 + MARGIN;
    let id = fig
        .id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>();
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width:.0} {height:.0}\" width=\"{width:.0}\" height=\"{height:.0}\" role=\"img\" aria-label=\"{}\" font-family=\"{FAMILY}\">\n<defs><marker id=\"arrow-{id}\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 z\" fill=\"{}\"/></marker></defs>\n{}",
        escape(&fig.title),
        palette.line,
        if palette.frame {
            format!(
                "<rect width=\"100%\" height=\"100%\" rx=\"12\" fill=\"{}\"/>\n",
                palette.background
            )
        } else {
            String::new()
        }
    );
    if banded {
        for (b, (top, h)) in band_boxes.iter().enumerate() {
            let label = fig.groups.get(b).map(String::as_str).unwrap_or("기타");
            svg.push_str(&format!(
                "<rect x=\"{:.1}\" y=\"{top:.1}\" width=\"{:.1}\" height=\"{h:.1}\" rx=\"10\" fill=\"{}\"/>\n<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"11.5\" font-weight=\"600\" fill=\"{}\">{}</text>\n",
                MARGIN - 6.0,
                width - 2.0 * (MARGIN - 6.0),
                palette.band,
                MARGIN + 6.0,
                top + 17.0,
                palette.muted,
                escape(label)
            ));
        }
    }
    // Edges under nodes.
    for e in &fig.edges {
        let (Some(a), Some(b)) = (index.get(e.from.as_str()), index.get(e.to.as_str())) else {
            continue;
        };
        let (Some(pa), Some(pb)) = (&placed[*a], &placed[*b]) else {
            continue;
        };
        let (ax, bx) = (pa.x + pa.w / 2.0, pb.x + pb.w / 2.0);
        let (p0, p1, p2, p3) = if layer[*a] < layer[*b] {
            let (y0, y3) = (pa.y + pa.h, pb.y - 1.5);
            let d = (y3 - y0) / 2.0;
            ((ax, y0), (ax, y0 + d), (bx, y3 - d), (bx, y3))
        } else if layer[*a] > layer[*b] {
            let (y0, y3) = (pa.y, pb.y + pb.h + 1.5);
            let d = (y0 - y3) / 2.0;
            ((ax, y0), (ax, y0 - d), (bx, y3 + d), (bx, y3))
        } else {
            let y0 = pa.y + pa.h;
            let y3 = pb.y + pb.h + 1.5;
            let dip = 26.0 + (ax - bx).abs() * 0.08;
            ((ax, y0), (ax, y0 + dip), (bx, y3 + dip), (bx, y3))
        };
        svg.push_str(&format!(
            "<path d=\"M{:.1},{:.1} C{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.4\"{} marker-end=\"url(#arrow-{id})\"/>\n",
            p0.0, p0.1, p1.0, p1.1, p2.0, p2.1, p3.0, p3.1,
            palette.line,
            if e.dashed { " stroke-dasharray=\"5 4\"" } else { "" }
        ));
        if !e.label.trim().is_empty() {
            let mx = (p0.0 + 3.0 * p1.0 + 3.0 * p2.0 + p3.0) / 8.0;
            let my = (p0.1 + 3.0 * p1.1 + 3.0 * p2.1 + p3.1) / 8.0;
            let lines = wrap(&e.label, 130.0, 11.0);
            let lw = lines
                .iter()
                .map(|l| text_width(l, 11.0))
                .fold(0.0, f32::max)
                + 10.0;
            let lh = lines.len() as f32 * 14.0 + 4.0;
            svg.push_str(&format!(
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{lw:.1}\" height=\"{lh:.1}\" rx=\"4\" fill=\"{}\" opacity=\"0.92\"/>\n",
                mx - lw / 2.0,
                my - lh / 2.0,
                palette.background
            ));
            for (i, l) in lines.iter().enumerate() {
                svg.push_str(&format!(
                    "<text x=\"{mx:.1}\" y=\"{:.1}\" font-size=\"11\" text-anchor=\"middle\" fill=\"{}\">{}</text>\n",
                    my - lh / 2.0 + 13.0 + i as f32 * 14.0,
                    palette.muted,
                    escape(l)
                ));
            }
        }
    }
    for (n, p) in fig.nodes.iter().zip(&placed) {
        let Some(p) = p else { continue };
        svg.push_str(&format!(
            "<g class=\"node\" data-node=\"{}\" tabindex=\"0\"><title>{}</title><rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"9\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.3\"/>",
            escape(&n.id),
            escape(if n.note.is_empty() { &n.label } else { &n.note }),
            p.x, p.y, p.w, p.h,
            palette.fills[p.color],
            palette.strokes[p.color]
        ));
        for (i, l) in p.lines.iter().enumerate() {
            svg.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"{FONT}\" text-anchor=\"middle\" fill=\"{}\">{}</text>",
                p.x + p.w / 2.0,
                p.y + PAD_Y + 13.5 + i as f32 * LINE,
                palette.text,
                escape(l)
            ));
        }
        svg.push_str("</g>\n");
    }
    svg.push_str("</svg>\n");
    Some((svg, width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use theory_core::{DiagramEdge, DiagramNode, record};
    fn node(id: &str, label: &str, group: &str) -> DiagramNode {
        record::<DiagramNode>(|n| {
            n.id = id.into();
            n.label = label.into();
            n.group = group.into();
            n.note = format!("{label} 설명 <&>");
        })
    }
    fn edge(a: &str, b: &str, label: &str) -> DiagramEdge {
        record::<DiagramEdge>(|e| {
            e.from = a.into();
            e.to = b.into();
            e.label = label.into();
        })
    }
    #[test]
    fn banded_diagram_places_every_node_and_escapes_text() {
        let fig = record::<Figure>(|f| {
            f.id = "F1".into();
            f.title = "수준 간 연결".into();
            f.groups = vec!["뉴런 집단".into(), "센터".into(), "기분".into()];
            f.nodes = vec![
                node("n", "발화율 벡터 x(t)", "뉴런 집단"),
                node("c", "센터 상태 y = h(x)", "센터"),
                node("m", "기분 점수", "기분"),
                node("r", "일주기 위상 θ", "센터"),
            ];
            f.edges = vec![
                edge("n", "c", "관측·축약"),
                edge("c", "m", "평균화"),
                edge("r", "c", ""),
            ];
        });
        let (svg, w, h) = figure_svg(&fig, &Palette::LIGHT).unwrap();
        assert!(w > 100.0 && h > 200.0);
        assert_eq!(svg.matches("class=\"node\"").count(), 4);
        assert_eq!(svg.matches("marker-end=").count(), 3);
        assert!(svg.contains("설명 &lt;&amp;&gt;"));
        assert!(svg.contains(">기분</text>"));
        // Well-formed enough for an XML parser.
        roxmltree::Document::parse(&svg).unwrap();
    }
    #[test]
    fn plain_diagram_layers_by_depth_and_tables_are_not_svg() {
        let fig = record::<Figure>(|f| {
            f.nodes = vec![node("a", "A", ""), node("b", "B", ""), node("c", "C", "")];
            f.edges = vec![
                edge("a", "b", "x"),
                edge("b", "c", "y"),
                edge("c", "a", "cycle"),
            ];
        });
        assert_eq!(layers(&fig).len(), 3);
        assert!(figure_svg(&fig, &Palette::DARK).is_some());
        let table = record::<Figure>(|f| f.kind = FigureKind::Table);
        assert!(figure_svg(&table, &Palette::LIGHT).is_none());
        assert_eq!(
            wrap("아주 긴 한국어 라벨을 여러 줄로 나눕니다", 60.0, 13.0).len(),
            4
        );
    }
}
