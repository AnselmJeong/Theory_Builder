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
use theory_core::{Access, BibInfo, Source, now};
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
        source.bib = Some(pubmed_bib(article, &pmid, &source.title));
        sources.push(source);
    }
    Ok(sources)
}
/// Bibliographic fields exactly as PubMed records them.
fn pubmed_bib(article: roxmltree::Node<'_, '_>, pmid: &str, title: &str) -> BibInfo {
    let first = |tag: &str| {
        article
            .descendants()
            .find(|n| n.tag_name().name() == tag)
            .map(text)
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let authors = article
        .descendants()
        .filter(|n| n.tag_name().name() == "Author")
        .filter_map(|a| {
            let child = |tag: &str| {
                a.children()
                    .find(|n| n.tag_name().name() == tag)
                    .map(text)
                    .unwrap_or_default()
            };
            let (last, fore, initials, collective) = (
                child("LastName"),
                child("ForeName"),
                child("Initials"),
                child("CollectiveName"),
            );
            if !last.is_empty() {
                let given = if fore.is_empty() { initials } else { fore };
                Some(if given.is_empty() {
                    last
                } else {
                    format!("{last}, {given}")
                })
            } else {
                (!collective.is_empty()).then_some(collective)
            }
        })
        .collect();
    let pub_date = article
        .descendants()
        .find(|n| n.tag_name().name() == "PubDate");
    let year = pub_date
        .and_then(|d| {
            d.children()
                .find(|n| n.tag_name().name() == "Year")
                .map(text)
                .or_else(|| {
                    d.children()
                        .find(|n| n.tag_name().name() == "MedlineDate")
                        .map(|n| text(n).chars().take(4).collect())
                })
        })
        .unwrap_or_default();
    let doi = article
        .descendants()
        .find(|n| {
            (n.tag_name().name() == "ArticleId" && n.attribute("IdType") == Some("doi"))
                || (n.tag_name().name() == "ELocationID" && n.attribute("EIdType") == Some("doi"))
        })
        .map(text)
        .unwrap_or_default();
    let mut b = BibInfo::default();
    b.entry_type = "article".into();
    b.title = title.trim().trim_end_matches('.').to_string();
    b.authors = authors;
    b.year = year.trim().into();
    b.container = {
        let iso = first("ISOAbbreviation");
        let full = first("Title");
        if full.is_empty() { iso } else { full }
    };
    b.volume = first("Volume");
    b.number = first("Issue");
    b.pages = first("MedlinePgn");
    b.doi = doi.trim().into();
    b.pmid = pmid.into();
    b.url = format!("https://pubmed.ncbi.nlm.nih.gov/{pmid}/");
    b.origin = "pubmed".into();
    b
}
/// Fill in authors and journal details for cited sources from their PMID (PubMed) or a DOI
/// printed in the source (Crossref). Read-only public metadata; failures leave the
/// record as it was. Never changes a frozen citation key.
pub fn enrich_bibliography(
    sources: &mut [Source],
    ids: &[String],
    cancel: &Cancel,
    deadline: Instant,
) -> Vec<String> {
    let mut warnings = vec![];
    let Ok(client) = Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent("TheoryStudio/0.4 (bibliography; read-only)")
        .build()
    else {
        return warnings;
    };
    let wanted: Vec<usize> = sources
        .iter()
        .enumerate()
        .filter(|(_, s)| ids.contains(&s.id))
        .filter(|(_, s)| {
            let b = s.derived_bib();
            b.authors.is_empty() && (!b.pmid.is_empty() || !b.doi.is_empty())
        })
        .map(|(i, _)| i)
        .collect();
    let pmids: Vec<String> = wanted
        .iter()
        .map(|i| sources[*i].derived_bib().pmid)
        .filter(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()))
        .collect();
    if !pmids.is_empty() {
        let joined = pmids.join(",");
        match request(
            client
                .get("https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi")
                .query(&[
                    ("db", "pubmed"),
                    ("id", joined.as_str()),
                    ("retmode", "xml"),
                    ("rettype", "abstract"),
                    ("tool", "TheoryStudio"),
                ]),
            cancel,
            deadline,
        )
        .and_then(|xml| parse_pubmed(&xml))
        {
            Ok(found) => {
                for f in found {
                    let Some(fresh) = f.bib else { continue };
                    for i in &wanted {
                        let s = &mut sources[*i];
                        if s.derived_bib().pmid == fresh.pmid {
                            let key = s.bib.as_ref().map(|b| b.key.clone()).unwrap_or_default();
                            let mut b = fresh.clone();
                            b.key = key;
                            s.bib = Some(b);
                        }
                    }
                }
            }
            Err(e) => warnings.push(format!("PubMed 서지 정보: {e}")),
        }
    }
    for i in wanted {
        let current = sources[i].derived_bib();
        if !current.authors.is_empty() || current.doi.is_empty() {
            continue;
        }
        match crossref(&client, &current.doi, cancel, deadline) {
            Ok(Some(mut b)) => {
                b.key = current.key.clone();
                sources[i].bib = Some(b);
            }
            Ok(None) => {}
            Err(e) => warnings.push(format!("Crossref 서지 정보: {e}")),
        }
    }
    warnings
}
fn crossref(
    client: &Client,
    doi: &str,
    cancel: &Cancel,
    deadline: Instant,
) -> Result<Option<BibInfo>> {
    ensure!(
        doi.starts_with("10.") && doi.len() < 200 && !doi.contains(char::is_whitespace),
        "DOI 형식 오류"
    );
    let raw = request(
        client.get(format!("https://api.crossref.org/works/{doi}")),
        cancel,
        deadline,
    )?;
    let v: Value = serde_json::from_str(&raw)?;
    Ok(parse_crossref(&v["message"]))
}
pub fn parse_crossref(m: &Value) -> Option<BibInfo> {
    let title = m["title"][0].as_str()?.trim().to_string();
    let s = |v: &Value| v.as_str().unwrap_or_default().trim().to_string();
    let mut b = BibInfo::default();
    b.entry_type = match m["type"].as_str() {
        Some("journal-article") => "article",
        Some("book") | Some("monograph") => "book",
        Some("book-chapter") => "incollection",
        Some("proceedings-article") => "inproceedings",
        _ => "misc",
    }
    .into();
    b.title = title;
    b.authors = m["author"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| {
            let family = s(&a["family"]);
            let given = s(&a["given"]);
            let name = s(&a["name"]);
            if !family.is_empty() {
                Some(if given.is_empty() {
                    family
                } else {
                    format!("{family}, {given}")
                })
            } else {
                (!name.is_empty()).then_some(name)
            }
        })
        .collect();
    b.year = ["published-print", "published-online", "issued", "created"]
        .iter()
        .find_map(|k| m[k]["date-parts"][0][0].as_u64())
        .map(|y| y.to_string())
        .unwrap_or_default();
    b.container = s(&m["container-title"][0]);
    b.volume = s(&m["volume"]);
    b.number = s(&m["issue"]);
    b.pages = s(&m["page"]);
    b.publisher = if b.entry_type == "article" {
        String::new()
    } else {
        s(&m["publisher"])
    };
    b.doi = s(&m["DOI"]);
    b.origin = "crossref".into();
    Some(b)
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
        .user_agent("TheoryStudio/0.4 literature research")
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
        let full = parse_pubmed("<PubmedArticleSet><PubmedArticle><MedlineCitation><PMID>9</PMID><Article><Journal><JournalIssue><Volume>21</Volume><Issue>3</Issue><PubDate><Year>2019</Year></PubDate></JournalIssue><Title>Bipolar Disorders</Title><ISOAbbreviation>Bipolar Disord</ISOAbbreviation></Journal><ArticleTitle>Mood cycles.</ArticleTitle><Pagination><MedlinePgn>201-9</MedlinePgn></Pagination><AuthorList><Author><LastName>Bonsall</LastName><ForeName>Michael B</ForeName></Author><Author><CollectiveName>Mood Group</CollectiveName></Author></AuthorList></Article></MedlineCitation><PubmedData><ArticleIdList><ArticleId IdType=\"doi\">10.1111/bdi.1</ArticleId></ArticleIdList></PubmedData></PubmedArticle></PubmedArticleSet>").unwrap();
        let b = full[0].bib.clone().unwrap();
        assert_eq!(b.authors, ["Bonsall, Michael B", "Mood Group"]);
        assert_eq!(
            (
                b.year.as_str(),
                b.container.as_str(),
                b.pages.as_str(),
                b.doi.as_str()
            ),
            ("2019", "Bipolar Disorders", "201-9", "10.1111/bdi.1")
        );
        assert_eq!(b.title, "Mood cycles");
    }
    #[test]
    fn crossref_records_become_bib_entries() {
        let m = serde_json::json!({"title":["A model"],"type":"journal-article","author":[{"family":"Goldbeter","given":"Albert"}],"issued":{"date-parts":[[2011]]},"container-title":["Pharmacopsychiatry"],"volume":"44","page":"S44-S50","DOI":"10.1055/s-1"});
        let b = parse_crossref(&m).unwrap();
        assert_eq!(b.base_key(""), "goldbeter2011model");
        assert_eq!(b.entry_type, "article");
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
