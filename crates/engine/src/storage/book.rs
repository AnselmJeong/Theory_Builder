use anyhow::{Context, Result, ensure};
use std::{
    collections::HashMap,
    io::{Cursor, Read},
    path::{Component, Path},
};
use zip::ZipArchive;
const LIMIT: usize = 2_000_000;
fn entry(zip: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<String> {
    let file = zip
        .by_name(name)
        .with_context(|| format!("책 내부 파일 없음: {name}"))?;
    ensure!(
        file.size() <= LIMIT as u64,
        "책의 한 섹션이 읽기 한도를 넘었습니다"
    );
    let mut text = String::new();
    file.take(LIMIT as u64 + 1).read_to_string(&mut text)?;
    ensure!(text.len() <= LIMIT, "책 섹션 크기 초과");
    Ok(text)
}
fn plain(xml: &str) -> Result<String> {
    let doc = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            nodes_limit: 250_000,
            ..Default::default()
        },
    )?;
    let mut out = String::new();
    for n in doc.descendants() {
        if n.ancestors()
            .any(|p| matches!(p.tag_name().name(), "script" | "style" | "head"))
        {
            continue;
        }
        if matches!(
            n.tag_name().name(),
            "p" | "div" | "li" | "h1" | "h2" | "h3" | "br" | "tr"
        ) {
            out.push('\n');
        }
        if n.is_text() {
            out.push_str(n.text().unwrap_or_default());
        }
    }
    Ok(out
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n"))
}
pub(super) fn extract(bytes: &[u8], ext: &str) -> Result<(String, String)> {
    let mut zip = ZipArchive::new(Cursor::new(bytes))?;
    ensure!(zip.len() < 10000, "책 내부 항목이 너무 많습니다");
    if ext == "docx" {
        let text = plain(&entry(&mut zip, "word/document.xml")?)?;
        return Ok((
            text,
            "DOCX 본문 추출 · 각주·도형·수식은 포함하지 않음".into(),
        ));
    }
    let container = entry(&mut zip, "META-INF/container.xml")?;
    let doc = roxmltree::Document::parse(&container)?;
    let opf = doc
        .descendants()
        .find(|n| n.tag_name().name() == "rootfile")
        .and_then(|n| n.attribute("full-path"))
        .context("EPUB 목차 위치 없음")?;
    let package = entry(&mut zip, opf)?;
    let doc = roxmltree::Document::parse(&package)?;
    let manifest: HashMap<_, _> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "item")
        .filter_map(|n| Some((n.attribute("id")?, n.attribute("href")?)))
        .collect();
    let mut text = String::new();
    let mut read = 0;
    let mut partial = false;
    for n in doc
        .descendants()
        .filter(|n| n.tag_name().name() == "itemref")
    {
        let href = manifest
            .get(n.attribute("idref").unwrap_or_default())
            .context("EPUB 본문 순서 참조 오류")?;
        let path = Path::new(opf).parent().unwrap_or(Path::new("")).join(href);
        // Archive lookup only, never extract paths onto the filesystem.
        let mut parts = vec![];
        for c in path.components() {
            match c {
                Component::Normal(p) => parts.push(p.to_string_lossy().into_owned()),
                Component::ParentDir => {
                    ensure!(parts.pop().is_some(), "EPUB 경로 오류");
                }
                Component::CurDir => {}
                _ => anyhow::bail!("EPUB 경로 오류"),
            }
        }
        let xml = entry(&mut zip, &parts.join("/"))?;
        let section = plain(&xml)?;
        if text.len() + section.len() > LIMIT {
            partial = true;
            break;
        }
        read += 1;
        text.push_str(&format!("[EPUB section {read}: {href}]\n{section}\n\n"));
    }
    ensure!(read > 0, "EPUB에서 읽을 수 있는 본문이 없습니다");
    Ok((
        text,
        format!(
            "EPUB 본문 순서대로 {read}개 섹션 추출{}",
            if partial {
                " · 크기 한도로 뒷부분 생략"
            } else {
                ""
            }
        ),
    ))
}
