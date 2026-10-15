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
use theory_core::{Access, BibInfo, Source, doi_in_url, now, same_family, same_title};
const MAX_RESPONSE: u64 = 8_000_000;
/// The server has no such record (HTTP 404), as opposed to a failed connection.
#[derive(Debug)]
struct NotFound;
impl std::fmt::Display for NotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("검색 서버 HTTP 404")
    }
}
impl std::error::Error for NotFound {}
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
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Err(NotFound.into());
            }
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
/// Complete the records of cited sources so that each exported BibTeX entry is a full one
/// with its DOI: PubMed for a bare PMID, then the DOI registry (Crossref, else doi.org for
/// DataCite and the other agencies) for a DOI the source carries — from PubMed, its own
/// address or its printed text — or, failing that, a Crossref title search whose hit must
/// carry the same title (and first author and year, when known). Read-only public metadata;
/// a failed connection leaves the record as it was for the next try. Never changes a
/// frozen citation key.
pub fn enrich_bibliography(
    sources: &mut [Source],
    ids: &[String],
    cancel: &Cancel,
    deadline: Instant,
) -> Vec<String> {
    let mut warnings = vec![];
    let Ok(client) = Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent("TheoryStudio/0.4 (bibliography; read-only)")
        .build()
    else {
        return warnings;
    };
    let wanted: Vec<usize> = sources
        .iter()
        .enumerate()
        .filter(|(_, s)| ids.contains(&s.id) && !s.derived_bib().looked_up)
        .map(|(i, _)| i)
        .collect();
    let pmids: Vec<String> = wanted
        .iter()
        .map(|i| sources[*i].derived_bib())
        .filter(|b| b.authors.is_empty())
        .map(|b| b.pmid)
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
    let mut missing = vec![];
    for (n, i) in wanted.iter().enumerate() {
        let current = sources[*i].derived_bib();
        match registry_record(&client, &current, cancel, deadline) {
            Ok(Lookup::Found(found)) => sources[*i].bib = Some(merge(&current, *found)),
            Ok(Lookup::Missing) => {
                let mut b = current.clone();
                b.looked_up = true;
                sources[*i].bib = Some(b);
                missing.push(format!("“{}”", current.title));
            }
            Ok(Lookup::Unsearchable) => {}
            Err(e) if e.to_string() == "cancelled" => break,
            Err(e) if e.to_string() == "budget_exhausted" => {
                warnings.push(format!(
                    "서지 정보 확인 시간이 다해 {}건은 다음에 다시 확인합니다",
                    wanted.len() - n
                ));
                break;
            }
            // The registry is unreachable: one note, and the rest wait for the next try.
            Err(e) => {
                warnings.push(format!(
                    "서지 정보: {e} — {}건은 다음에 다시 확인합니다",
                    wanted.len() - n
                ));
                break;
            }
        }
    }
    if !missing.is_empty() {
        warnings.push(format!(
            "DOI가 있는 서지 기록을 찾지 못한 인용 {}건 (참고문헌에는 아는 정보만 실음): {}",
            missing.len(),
            missing.join("; ")
        ));
    }
    warnings
}
enum Lookup {
    Found(Box<BibInfo>),
    /// Asked, and no record carries the source's DOI or title.
    Missing,
    /// Nothing to ask with: no DOI and no title long enough to identify a work.
    Unsearchable,
}
fn registry_record(
    client: &Client,
    b: &BibInfo,
    cancel: &Cancel,
    deadline: Instant,
) -> Result<Lookup> {
    let doi = Some(b.doi.trim().to_string())
        .filter(|d| !d.is_empty())
        .or_else(|| doi_in_url(&b.url))
        .filter(|d| d.starts_with("10.") && d.len() < 200 && !d.contains(char::is_whitespace))
        .unwrap_or_default();
    if !doi.is_empty()
        && let Some(found) = by_doi(client, &doi, cancel, deadline)?
    {
        return Ok(Lookup::Found(Box::new(found)));
    }
    // A file name is not a printed title, and too short a title names too many works to
    // trust a search hit.
    if b.origin == "file" || b.title.chars().filter(|c| c.is_alphanumeric()).count() < 20 {
        return Ok(if doi.is_empty() {
            Lookup::Unsearchable
        } else {
            Lookup::Missing
        });
    }
    Ok(match by_title(client, b, cancel, deadline)? {
        Some(found) => Lookup::Found(Box::new(found)),
        None => Lookup::Missing,
    })
}
/// A DOI as a URL path: reserved characters (`?`, `#`, `;`, `<`…) percent-encoded.
fn doi_path(doi: &str) -> String {
    doi.bytes()
        .map(|c| {
            if c.is_ascii_alphanumeric() || b"-._~/:()".contains(&c) {
                (c as char).to_string()
            } else {
                format!("%{c:02X}")
            }
        })
        .collect()
}
fn by_doi(
    client: &Client,
    doi: &str,
    cancel: &Cancel,
    deadline: Instant,
) -> Result<Option<BibInfo>> {
    let path = doi_path(doi);
    match request(
        client.get(format!("https://api.crossref.org/works/{path}")),
        cancel,
        deadline,
    ) {
        Ok(raw) => {
            let v: Value = serde_json::from_str(&raw)?;
            if let Some(b) = parse_crossref(&v["message"]) {
                return Ok(Some(b));
            }
        }
        Err(e) if e.is::<NotFound>() => {}
        Err(e) => return Err(e),
    }
    // Not a Crossref DOI (DataCite: arXiv, Zenodo…; mEDRA; JaLC): ask doi.org for CSL JSON.
    match request(
        client.get(format!("https://doi.org/{path}")).header(
            reqwest::header::ACCEPT,
            "application/vnd.citationstyles.csl+json",
        ),
        cancel,
        deadline,
    ) {
        Ok(raw) => Ok(serde_json::from_str::<Value>(&raw)
            .ok()
            .and_then(|v| parse_csl(&v))
            .map(|mut b| {
                if b.doi.eq_ignore_ascii_case(doi) || b.doi.is_empty() {
                    b.doi = doi.into();
                }
                b
            })),
        Err(e) if e.is::<NotFound>() => Ok(None),
        Err(e) => Err(e),
    }
}
/// The Crossref work with this source's title (and first author and year, when known);
/// a published version is preferred over a preprint of the same title.
fn by_title(
    client: &Client,
    b: &BibInfo,
    cancel: &Cancel,
    deadline: Instant,
) -> Result<Option<BibInfo>> {
    let mut query = vec![
        ("query.bibliographic", b.title.trim().to_string()),
        ("rows", "5".to_string()),
    ];
    if let Some(first) = b.authors.first() {
        query.push(("query.author", BibInfo::family(first)));
    }
    let raw = request(
        client.get("https://api.crossref.org/works").query(&query),
        cancel,
        deadline,
    )?;
    let v: Value = serde_json::from_str(&raw)?;
    Ok(pick_title_match(&v["message"]["items"], b))
}
pub fn pick_title_match(items: &Value, b: &BibInfo) -> Option<BibInfo> {
    let year: Option<i64> = b.year.trim().parse().ok();
    items
        .as_array()?
        .iter()
        .filter(|m| {
            !matches!(
                m["type"].as_str(),
                Some("peer-review" | "component" | "journal-issue" | "journal-volume" | "grant")
            )
        })
        .filter_map(|m| Some((m["type"] == "posted-content", parse_crossref(m)?)))
        .filter(|(_, c)| {
            same_title(&c.title, &b.title)
                && year.is_none_or(|y| c.year.parse::<i64>().is_ok_and(|cy| (cy - y).abs() <= 1))
                && b.authors
                    .first()
                    .is_none_or(|a| c.authors.iter().any(|x| same_family(a, x)))
        })
        .min_by_key(|(preprint, _)| *preprint)
        .map(|(_, c)| c)
}
/// The looked-up record in place of what was known, under the frozen key. A PubMed record
/// keeps its own fields and takes only what it lacks, plus the journal's capitalisation.
fn merge(current: &BibInfo, found: BibInfo) -> BibInfo {
    let mut b = if current.origin == "pubmed" {
        let mut b = current.clone();
        for (mine, theirs) in [
            (&mut b.year, &found.year),
            (&mut b.month, &found.month),
            (&mut b.container, &found.container),
            (&mut b.series, &found.series),
            (&mut b.volume, &found.volume),
            (&mut b.number, &found.number),
            (&mut b.pages, &found.pages),
            (&mut b.publisher, &found.publisher),
            (&mut b.isbn, &found.isbn),
            (&mut b.issn, &found.issn),
            (&mut b.doi, &found.doi),
        ] {
            if mine.trim().is_empty() {
                *mine = theirs.clone();
            }
        }
        if b.authors.is_empty() {
            b.authors = found.authors.clone();
        }
        if b.editors.is_empty() {
            b.editors = found.editors.clone();
        }
        if b.container.to_lowercase() == found.container.to_lowercase() {
            b.container = found.container.clone();
        }
        b
    } else {
        let mut b = found;
        if b.pmid.is_empty() {
            b.pmid = current.pmid.clone();
        }
        b
    };
    b.key = current.key.clone();
    b.looked_up = true;
    b
}
/// Registry text without its markup: tags dropped, entities decoded, spaces collapsed.
fn plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<'
            && chars
                .peek()
                .is_some_and(|n| n.is_ascii_alphabetic() || *n == '/')
        {
            for t in chars.by_ref() {
                if t == '>' {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    let out = out
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn date_parts(parts: &Value) -> Option<(String, String)> {
    let year = parts[0]
        .as_u64()
        .or_else(|| parts[0].as_str()?.parse().ok())?;
    let month = parts[1]
        .as_u64()
        .or_else(|| parts[1].as_str()?.parse().ok())
        .filter(|m| (1..=12).contains(m))
        .map(|m| m.to_string())
        .unwrap_or_default();
    Some((year.to_string(), month))
}
/// "Family, Given" names from a Crossref or CSL JSON name list.
fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| {
            let s = |k: &str| plain(a[k].as_str().unwrap_or_default());
            let (family, given, literal) = (s("family"), s("given"), {
                let n = s("name");
                if n.is_empty() { s("literal") } else { n }
            });
            if !family.is_empty() {
                Some(if given.is_empty() {
                    family
                } else {
                    format!("{family}, {given}")
                })
            } else {
                (!literal.is_empty()).then_some(literal)
            }
        })
        .collect()
}
/// The print value of a typed identifier list (issn-type, isbn-type), else the first.
fn identifier(typed: &Value, plain_list: &Value) -> String {
    let typed: Vec<&Value> = typed.as_array().into_iter().flatten().collect();
    typed
        .iter()
        .find(|t| t["type"] == "print")
        .or_else(|| typed.first())
        .and_then(|t| t["value"].as_str())
        .or_else(|| plain_list[0].as_str())
        .unwrap_or_default()
        .trim()
        .to_string()
}
/// A Crossref REST work as a full bibliographic record.
pub fn parse_crossref(m: &Value) -> Option<BibInfo> {
    let s = |v: &Value| plain(v.as_str().unwrap_or_default());
    let mut title = s(&m["title"][0]);
    if title.is_empty() {
        return None;
    }
    let subtitle = s(&m["subtitle"][0]);
    if !subtitle.is_empty() && !title.to_lowercase().contains(&subtitle.to_lowercase()) {
        title = format!("{}: {subtitle}", title.trim_end_matches(':'));
    }
    let kind = m["type"].as_str().unwrap_or_default();
    let mut b = BibInfo::default();
    b.entry_type = match kind {
        "journal-article" => "article",
        "book" | "monograph" | "edited-book" | "reference-book" => "book",
        "book-chapter" | "book-section" | "book-part" | "reference-entry" => "incollection",
        "proceedings-article" => "inproceedings",
        _ => "misc",
    }
    .into();
    b.title = title;
    b.authors = names(&m["author"]);
    b.editors = names(&m["editor"]);
    // Journals are cited by their print issue; online-only works by first publication.
    if let Some((year, month)) = ["published-print", "published-online", "issued", "created"]
        .iter()
        .find_map(|k| date_parts(&m[k]["date-parts"][0]))
    {
        (b.year, b.month) = (year, month);
    }
    let containers: Vec<String> = m["container-title"]
        .as_array()
        .into_iter()
        .flatten()
        .map(s)
        .filter(|t| !t.is_empty())
        .collect();
    // A chapter in a series lists the series first, then the book.
    if matches!(b.entry_type.as_str(), "incollection" | "inproceedings") && containers.len() > 1 {
        b.series = containers[0].clone();
        b.container = containers.last().cloned().unwrap_or_default();
    } else {
        b.container = containers.first().cloned().unwrap_or_default();
    }
    if kind == "posted-content" {
        b.container = s(&m["institution"][0]["name"]);
        b.note = "Preprint".into();
    }
    b.volume = s(&m["volume"]);
    b.number = s(&m["issue"]);
    b.pages = s(&m["page"]);
    if b.pages.is_empty() {
        b.pages = s(&m["article-number"]);
    }
    b.publisher = s(&m["publisher"]);
    b.issn = identifier(&m["issn-type"], &m["ISSN"]);
    b.isbn = identifier(&m["isbn-type"], &m["ISBN"]);
    b.doi = s(&m["DOI"]);
    b.origin = "crossref".into();
    Some(b)
}
/// A CSL JSON record from doi.org content negotiation (DataCite, mEDRA, JaLC…).
pub fn parse_csl(m: &Value) -> Option<BibInfo> {
    let s = |v: &Value| match v {
        Value::String(x) => plain(x),
        Value::Array(a) => plain(a.first().and_then(Value::as_str).unwrap_or_default()),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    };
    let title = s(&m["title"]);
    if title.is_empty() {
        return None;
    }
    let mut b = BibInfo::default();
    b.entry_type = match m["type"].as_str() {
        Some("article-journal") => "article",
        Some("book") => "book",
        Some("chapter" | "entry-encyclopedia" | "entry-dictionary") => "incollection",
        Some("paper-conference") => "inproceedings",
        _ => "misc",
    }
    .into();
    b.title = title;
    b.authors = names(&m["author"]);
    b.editors = names(&m["editor"]);
    if let Some((year, month)) = date_parts(&m["issued"]["date-parts"][0]) {
        (b.year, b.month) = (year, month);
    }
    b.container = s(&m["container-title"]);
    b.volume = s(&m["volume"]);
    b.number = s(&m["issue"]);
    b.pages = s(&m["page"]);
    b.publisher = s(&m["publisher"]);
    b.issn = s(&m["ISSN"]);
    b.isbn = s(&m["ISBN"]);
    b.doi = s(&m["DOI"]);
    b.origin = "doi.org".into();
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
        // Article numbers, print ISSN, month, subtitles and title markup.
        let m = serde_json::json!({"title":["Metastable dynamics of <i>neural</i> circuits &amp; networks"],"subtitle":["A review"],"type":"journal-article","author":[{"family":"Brinkman","given":"B. A. W."},{"name":"Mood Group"}],"published-online":{"date-parts":[[2022,3,3]]},"published-print":{"date-parts":[[2022,3]]},"container-title":["Applied Physics Reviews"],"volume":"9","issue":"1","article-number":"011313","publisher":"AIP Publishing","ISSN":["1931-9401","0000-0000"],"issn-type":[{"value":"0000-0000","type":"electronic"},{"value":"1931-9401","type":"print"}],"DOI":"10.1063/5.0062603"});
        let b = parse_crossref(&m).unwrap();
        assert_eq!(
            b.title,
            "Metastable dynamics of neural circuits & networks: A review"
        );
        assert_eq!(b.authors, ["Brinkman, B. A. W.", "Mood Group"]);
        assert_eq!(
            (
                b.year.as_str(),
                b.month.as_str(),
                b.pages.as_str(),
                b.issn.as_str(),
                b.publisher.as_str()
            ),
            ("2022", "3", "011313", "1931-9401", "AIP Publishing")
        );
        let t = b.bibtex("brinkman2022metastable");
        assert!(t.contains("  month = mar,\n") && t.contains("  pages = {011313},\n"));
        assert!(t.contains("  url = {https://doi.org/10.1063/5.0062603}\n"));
        // A chapter in a series; a preprint.
        let m = serde_json::json!({"title":["A chapter"],"type":"book-chapter","author":[{"family":"Ruiz","given":"Ana"}],"editor":[{"family":"Kim","given":"Bo"}],"issued":{"date-parts":[[2025]]},"container-title":["Lecture Notes in Computer Science","Neural Networks 2025"],"page":"185-199","publisher":"Springer Nature Switzerland","ISBN":["9783000000002","9783000000001"],"isbn-type":[{"value":"9783000000001","type":"print"}],"DOI":"10.1007/x_14"});
        let b = parse_crossref(&m).unwrap();
        assert_eq!(
            (
                b.entry_type.as_str(),
                b.container.as_str(),
                b.series.as_str(),
                b.isbn.as_str()
            ),
            (
                "incollection",
                "Neural Networks 2025",
                "Lecture Notes in Computer Science",
                "9783000000001"
            )
        );
        assert_eq!(b.editors, ["Kim, Bo"]);
        let m = serde_json::json!({"title":["A preprint"],"type":"posted-content","institution":[{"name":"bioRxiv"}],"posted":{"date-parts":[[2023,1]]},"issued":{"date-parts":[[2023,1,2]]},"publisher":"Cold Spring Harbor Laboratory","DOI":"10.1101/2023.01.02.1"});
        let b = parse_crossref(&m).unwrap();
        assert_eq!(
            (b.container.as_str(), b.note.as_str()),
            ("bioRxiv", "Preprint")
        );
    }
    #[test]
    fn non_crossref_dois_are_read_from_csl_json() {
        let m = serde_json::json!({"type":"article","title":"Attractor landscapes","author":[{"family":"Chen","given":"Ruiqi"}],"issued":{"date-parts":[[2024,11]]},"publisher":"arXiv","DOI":"10.48550/ARXIV.2411.00001"});
        let b = parse_csl(&m).unwrap();
        assert_eq!(
            (b.entry_type.as_str(), b.year.as_str(), b.month.as_str()),
            ("misc", "2024", "11")
        );
        assert_eq!(b.publisher, "arXiv");
        assert!(parse_csl(&serde_json::json!({"type":"article"})).is_none());
    }
    #[test]
    fn title_search_hits_must_be_the_same_work() {
        let title = "Whole-brain network models: from physics to bedside";
        let mut wanted = BibInfo::default();
        wanted.title = format!("{title} | Frontiers");
        let items = serde_json::json!([
            {"type":"journal-article","title":["Whole-brain network models"],"issued":{"date-parts":[[2021]]},"DOI":"10.1/other"},
            {"type":"peer-review","title":[title],"issued":{"date-parts":[[2022]]},"DOI":"10.1/review"},
            {"type":"posted-content","title":[title],"issued":{"date-parts":[[2021]]},"DOI":"10.1/preprint"},
            {"type":"journal-article","title":[title],"author":[{"family":"Müller","given":"A"}],"issued":{"date-parts":[[2022]]},"DOI":"10.3389/fncom.2022.866517"}
        ]);
        assert_eq!(
            pick_title_match(&items, &wanted).unwrap().doi,
            "10.3389/fncom.2022.866517"
        );
        // Known year and first author must agree as well.
        wanted.year = "2015".into();
        assert!(pick_title_match(&items, &wanted).is_none());
        wanted.year = "2022".into();
        wanted.authors = vec!["Muller, A.".into()];
        assert!(pick_title_match(&items, &wanted).is_some());
        wanted.authors = vec!["Deco, Gustavo".into()];
        assert!(pick_title_match(&items, &wanted).is_none());
    }
    #[test]
    fn lookups_complete_pubmed_records_and_replace_web_ones_under_the_frozen_key() {
        let mut pubmed = BibInfo::default();
        pubmed.key = "fasoli2018transitions".into();
        pubmed.origin = "pubmed".into();
        pubmed.title = "Transitions between asynchronous and synchronous states".into();
        pubmed.authors = vec!["Fasoli, Diego".into()];
        pubmed.container = "Journal of computational neuroscience".into();
        pubmed.pages = "25-43".into();
        pubmed.pmid = "29124505".into();
        let mut found = BibInfo::default();
        found.title = "Other capitalisation".into();
        found.authors = vec!["Fasoli, D.".into()];
        found.container = "Journal of Computational Neuroscience".into();
        found.month = "2".into();
        found.pages = "25-43".into();
        found.publisher = "Springer Science and Business Media LLC".into();
        found.doi = "10.1007/s10827-017-0667-3".into();
        found.origin = "crossref".into();
        let b = merge(&pubmed, found.clone());
        assert_eq!(b.key, "fasoli2018transitions");
        assert_eq!(
            (b.title.as_str(), b.authors[0].as_str()),
            (pubmed.title.as_str(), "Fasoli, Diego")
        );
        assert_eq!(b.container, "Journal of Computational Neuroscience");
        assert_eq!(
            (b.month.as_str(), b.doi.as_str()),
            ("2", "10.1007/s10827-017-0667-3")
        );
        assert!(b.looked_up && b.pmid == "29124505");
        let mut web = BibInfo::default();
        web.key = "wholebrain".into();
        web.entry_type = "online".into();
        web.origin = "web".into();
        let b = merge(&web, found);
        assert_eq!(
            (b.key.as_str(), b.origin.as_str()),
            ("wholebrain", "crossref")
        );
        assert!(b.looked_up && b.authors == ["Fasoli, D."]);
        assert_eq!(
            doi_path("10.1002/(SICI)1;2-K<x>#"),
            "10.1002/(SICI)1%3B2-K%3Cx%3E%23"
        );
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
