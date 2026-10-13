//! Read-only literature search. Credentials stay in the engine, never in a model context.
use crate::providers::Cancel;
use anyhow::{Context, Result, ensure};
use reqwest::blocking::{Client, RequestBuilder};
use serde_json::{Value, json};
use std::{
    io::Read,
    sync::{atomic::Ordering, mpsc},
    time::{Duration, Instant},
};
use theory_core::{Access, Source, now};
const MAX_RESPONSE: u64 = 8_000_000;
pub fn tinyfish_configured() -> bool {
    !tinyfish_key().is_empty()
}
fn tinyfish_key() -> String {
    let path = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join("ScholarPen/settings.json");
    let settings: Value = std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    settings["tinyfishApiKey"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_string()
}
fn request(req: RequestBuilder, cancel: &Cancel, deadline: Instant) -> Result<String> {
    ensure!(!cancel.load(Ordering::SeqCst), "cancelled");
    ensure!(Instant::now() < deadline, "budget_exhausted");
    let (tx, rx) = mpsc::sync_channel(1);
    // A cancelled request is discarded; the read-only HTTP worker has a hard timeout.
    std::thread::spawn(move || {
        let result = (|| -> Result<String> {
            let response = req.send().context("검색 서버에 연결하지 못했습니다")?;
            ensure!(
                response.status().is_success(),
                "검색 서버 HTTP {}",
                response.status().as_u16()
            );
            let mut text = String::new();
            response.take(MAX_RESPONSE + 1).read_to_string(&mut text)?;
            ensure!(text.len() as u64 <= MAX_RESPONSE, "검색 응답이 너무 큽니다");
            Ok(text)
        })();
        let _ = tx.send(result);
    });
    loop {
        ensure!(!cancel.load(Ordering::SeqCst), "cancelled");
        ensure!(Instant::now() < deadline, "budget_exhausted");
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(r) => return r,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(e) => return Err(e.into()),
        }
    }
}
pub fn safe_public_url(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    if !["http", "https"].contains(&url.scheme())
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    if !host.contains('.')
        || host == "localhost"
        || host.ends_with(".local")
        || host.ends_with(".localhost")
    {
        return false;
    }
    // Search fetches go through TinyFish; never ask it to visit numeric/internal addresses.
    if host.parse::<std::net::IpAddr>().is_ok() || host.contains(':') {
        return false;
    }
    true
}
fn text(n: roxmltree::Node<'_, '_>) -> String {
    n.descendants()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect::<Vec<_>>()
        .join("")
}
pub fn parse_pubmed(xml: &str) -> Result<Vec<Source>> {
    let doc = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            nodes_limit: 200_000,
            ..Default::default()
        },
    )?;
    let mut sources = vec![];
    for article in doc
        .descendants()
        .filter(|n| n.tag_name().name() == "PubmedArticle")
    {
        let find = |tag| {
            article
                .descendants()
                .find(|n| n.tag_name().name() == tag)
                .map(text)
                .unwrap_or_default()
        };
        let pmid = find("PMID");
        let title = find("ArticleTitle");
        if pmid.is_empty() || title.is_empty() {
            continue;
        }
        let abstract_text = article
            .descendants()
            .filter(|n| n.tag_name().name() == "AbstractText")
            .map(|n| {
                format!(
                    "{}{}",
                    n.attribute("Label")
                        .map(|l| format!("{l}: "))
                        .unwrap_or_default(),
                    text(n)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let access = if abstract_text.is_empty() {
            Access::MetadataOnly
        } else {
            Access::AbstractOnly
        };
        let mut source = Source::new(
            title,
            abstract_text,
            format!("https://pubmed.ncbi.nlm.nih.gov/{pmid}/"),
            access,
        );
        source.import_note = format!(
            "PubMed · PMID {pmid} · {} · {} · 초록만 확보, 전체 논문 아님",
            find("Title"),
            find("Year")
        );
        source.retrieved_at = Some(now());
        sources.push(source);
    }
    Ok(sources)
}
fn pubmed(client: &Client, query: &str, cancel: &Cancel, deadline: Instant) -> Result<Vec<Source>> {
    let raw = request(
        client
            .get("https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi")
            .query(&[
                ("db", "pubmed"),
                ("term", query),
                ("retmode", "json"),
                ("retmax", "5"),
                ("sort", "relevance"),
                ("tool", "TheoryStudio"),
            ]),
        cancel,
        deadline,
    )?;
    let data: Value = serde_json::from_str(&raw)?;
    let ids = data["esearchresult"]["idlist"]
        .as_array()
        .context("PubMed 검색 형식 오류")?
        .iter()
        .filter_map(|v| v.as_str())
        .filter(|s| s.bytes().all(|c| c.is_ascii_digit()))
        .collect::<Vec<_>>()
        .join(",");
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let raw = request(
        client
            .get("https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi")
            .query(&[
                ("db", "pubmed"),
                ("id", ids.as_str()),
                ("retmode", "xml"),
                ("rettype", "abstract"),
                ("tool", "TheoryStudio"),
            ]),
        cancel,
        deadline,
    )?;
    parse_pubmed(&raw)
}
fn tinyfish(
    client: &Client,
    key: &str,
    query: &str,
    cancel: &Cancel,
    deadline: Instant,
    warnings: &mut Vec<String>,
) -> Result<Vec<Source>> {
    let raw = request(
        client
            .get("https://api.search.tinyfish.ai")
            .header("X-API-Key", key)
            .query(&[("query", query), ("domain_type", "research_paper")]),
        cancel,
        deadline,
    )?;
    let data: Value = serde_json::from_str(&raw)?;
    let items = data["results"]
        .as_array()
        .context("TinyFish 검색 형식 오류")?;
    let mut sources: Vec<_> = items
        .iter()
        .filter_map(|r| {
            let url = r["url"].as_str()?;
            if !safe_public_url(url) {
                return None;
            }
            let mut s = Source::new(
                r["title"].as_str()?.into(),
                String::new(),
                url.into(),
                Access::MetadataOnly,
            );
            s.import_note = "TinyFish 검색 결과 · 본문 확보 전".into();
            s.retrieved_at = Some(now());
            Some(s)
        })
        .take(4)
        .collect();
    if sources.is_empty() {
        return Ok(sources);
    }
    let urls: Vec<_> = sources.iter().map(|s| s.locator.clone()).collect();
    let fetched = request(
        client
            .post("https://api.fetch.tinyfish.ai")
            .header("X-API-Key", key)
            .json(&json!({"urls":urls,"format":"markdown","per_url_timeout_ms":20000})),
        cancel,
        deadline,
    )
    .and_then(|s| Ok(serde_json::from_str::<Value>(&s)?));
    match fetched {
        Ok(v) => {
            for s in &mut sources {
                if let Some(page) = v["results"]
                    .as_array()
                    .and_then(|a| a.iter().find(|p| p["url"] == s.locator))
                    && let Some(body) = page["text"].as_str().filter(|s| !s.trim().is_empty())
                {
                    // The fetched page may be an abstract, a landing page, or a full article.
                    // Never label an arbitrary web page as a verified full paper.
                    s.text = body.chars().take(100_000).collect();
                    s.hash = theory_core::hash(s.text.as_bytes());
                    s.access = Access::ProvidedExcerpt;
                    s.import_note =
                        "TinyFish 웹 페이지 발췌 · 초록/본문 범위는 페이지 내용을 확인하세요"
                            .into();
                }
            }
        }
        Err(e) => warnings.push(format!("TinyFish 페이지 읽기 실패: {e}")),
    }
    Ok(sources)
}
pub struct SearchResults {
    pub sources: Vec<Source>,
    pub warnings: Vec<String>,
}
pub fn retrieve(
    queries: &[String],
    cancel: &Cancel,
    deadline: Instant,
    progress: &mut dyn FnMut(String),
) -> Result<SearchResults> {
    let client = Client::builder()
        .timeout(Duration::from_secs(35))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("TheoryStudio/0.2 literature research")
        .build()?;
    let key = tinyfish_key();
    let mut result = SearchResults {
        sources: vec![],
        warnings: vec![],
    };
    for query in queries.iter().take(2) {
        progress(format!("문헌 검색 · PubMed · {query}"));
        match pubmed(&client, query, cancel, deadline) {
            Ok(s) => result.sources.extend(s),
            Err(e) => result.warnings.push(format!("PubMed: {e}")),
        }
        ensure!(!cancel.load(Ordering::SeqCst), "cancelled");
        if !key.is_empty() {
            progress(format!("문헌 검색 · TinyFish · {query}"));
            match tinyfish(&client, &key, query, cancel, deadline, &mut result.warnings) {
                Ok(s) => result.sources.extend(s),
                Err(e) => result.warnings.push(format!("TinyFish: {e}")),
            }
        }
    }
    ensure!(!cancel.load(Ordering::SeqCst), "cancelled");
    ensure!(Instant::now() < deadline, "budget_exhausted");
    let mut seen = std::collections::HashSet::new();
    result.sources.retain(|s| seen.insert(s.locator.clone()));
    for s in &mut result.sources {
        s.search_query = Some(queries.join(" | "));
    }
    if result.sources.is_empty() {
        result
            .warnings
            .push("검색을 마쳤지만 사용할 문헌을 확보하지 못했습니다".into());
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abstracts_and_metadata_are_distinguished() {
        let s=parse_pubmed("<PubmedArticleSet><PubmedArticle><PMID>1</PMID><ArticleTitle>A <i>study</i></ArticleTitle><Abstract><AbstractText Label='RESULTS'>Limited <b>finding</b>.</AbstractText></Abstract></PubmedArticle><PubmedArticle><PMID>2</PMID><ArticleTitle>No abstract</ArticleTitle></PubmedArticle></PubmedArticleSet>").unwrap();
        assert_eq!(s[0].access, Access::AbstractOnly);
        assert_eq!(s[0].text, "RESULTS: Limited finding.");
        assert_eq!(s[1].access, Access::MetadataOnly);
    }
    #[test]
    fn web_targets_exclude_local_and_non_web() {
        for u in [
            "file:///etc/passwd",
            "http://127.0.0.1",
            "http://localhost",
            "http://[::1]",
            "https://user:pass@example.com",
        ] {
            assert!(!safe_public_url(u));
        }
        assert!(safe_public_url("https://pubmed.ncbi.nlm.nih.gov/123/"));
    }
}
