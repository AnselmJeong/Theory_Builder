//! Bibliographic records for the sources a report cites, their pandoc/BibTeX keys and
//! the reference lines readers see. Metadata comes from PubMed, Crossref/DataCite (by a
//! DOI the source carries, or a title search whose hit must carry the same title), the
//! source's own printed header (checked letter by letter) or, at worst, its file name —
//! never a guess.
use crate::{Access, Source};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct BibInfo {
    /// Citation key. Empty until a report cites the source; then frozen so that stored
    /// report text and exported BibTeX keep agreeing.
    pub key: String,
    /// article | book | incollection | inproceedings | misc | online
    pub entry_type: String,
    pub title: String,
    /// "Family, Given" or a collective name.
    pub authors: Vec<String>,
    /// Book editors (chapters), "Family, Given".
    pub editors: Vec<String>,
    pub year: String,
    /// 1–12, empty when unknown.
    pub month: String,
    /// Journal, book or proceedings title.
    pub container: String,
    /// Book series (e.g. Lecture Notes in Computer Science).
    pub series: String,
    pub volume: String,
    pub number: String,
    /// Page range or article number.
    pub pages: String,
    pub publisher: String,
    pub isbn: String,
    pub issn: String,
    pub doi: String,
    pub pmid: String,
    pub url: String,
    /// When a web page was read (YYYY-MM-DD).
    pub urldate: String,
    pub note: String,
    /// pubmed | crossref | doi.org | source-text | file | web
    pub origin: String,
    /// The full record has been looked up (found or not); it is not asked for again.
    pub looked_up: bool,
}
const STOPWORDS: [&str; 23] = [
    "the", "a", "an", "of", "on", "in", "and", "for", "to", "with", "from", "toward", "towards",
    "into", "at", "by", "is", "are", "was", "were", "be", "as", "or",
];
/// Latin letters with diacritics → ASCII; everything else non-ASCII is dropped.
fn ascii_fold(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            if c.is_ascii() {
                return Some(c.to_string());
            }
            let base = match c {
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ą' => "a",
                'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' => "A",
                'ç' | 'ć' | 'č' => "c",
                'Ç' | 'Ć' | 'Č' => "C",
                'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => "e",
                'È' | 'É' | 'Ê' | 'Ë' | 'Ē' => "E",
                'ì' | 'í' | 'î' | 'ï' | 'ī' => "i",
                'Ì' | 'Í' | 'Î' | 'Ï' => "I",
                'ñ' | 'ń' | 'ň' => "n",
                'Ñ' => "N",
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => "o",
                'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => "O",
                'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => "u",
                'Ù' | 'Ú' | 'Û' | 'Ü' => "U",
                'ý' | 'ÿ' => "y",
                'ß' => "ss",
                'ł' => "l",
                'Ł' => "L",
                'š' | 'ś' => "s",
                'Š' | 'Ś' => "S",
                'ž' | 'ź' | 'ż' => "z",
                'Ž' | 'Ź' | 'Ż' => "Z",
                'ř' => "r",
                'æ' => "ae",
                'œ' => "oe",
                '‐' | '‑' | '–' | '—' => "-",
                _ => return None,
            };
            Some(base.to_string())
        })
        .collect()
}
const INITIALS: [&str; 19] = [
    "g", "kk", "n", "d", "tt", "r", "m", "b", "pp", "s", "ss", "", "j", "jj", "ch", "k", "t", "p",
    "h",
];
const MEDIALS: [&str; 21] = [
    "a", "ae", "ya", "yae", "eo", "e", "yeo", "ye", "o", "wa", "wae", "oe", "yo", "u", "wo", "we",
    "wi", "yu", "eu", "ui", "i",
];
const FINALS: [&str; 28] = [
    "", "k", "k", "k", "n", "n", "n", "t", "l", "k", "m", "l", "l", "l", "p", "l", "m", "p", "p",
    "t", "t", "ng", "t", "t", "k", "t", "p", "t",
];
/// Conventional spellings of common Korean family names.
const SURNAMES: [(&str, &str); 40] = [
    ("김", "kim"),
    ("이", "lee"),
    ("박", "park"),
    ("최", "choi"),
    ("정", "jung"),
    ("조", "cho"),
    ("강", "kang"),
    ("윤", "yoon"),
    ("장", "jang"),
    ("임", "lim"),
    ("한", "han"),
    ("오", "oh"),
    ("서", "seo"),
    ("신", "shin"),
    ("권", "kwon"),
    ("황", "hwang"),
    ("안", "ahn"),
    ("송", "song"),
    ("류", "ryu"),
    ("유", "yoo"),
    ("전", "jeon"),
    ("홍", "hong"),
    ("고", "ko"),
    ("문", "moon"),
    ("양", "yang"),
    ("손", "son"),
    ("배", "bae"),
    ("백", "baek"),
    ("허", "heo"),
    ("남", "nam"),
    ("심", "shim"),
    ("노", "noh"),
    ("하", "ha"),
    ("곽", "kwak"),
    ("성", "sung"),
    ("차", "cha"),
    ("주", "joo"),
    ("우", "woo"),
    ("구", "koo"),
    ("민", "min"),
];
const DOUBLE_SURNAMES: [&str; 6] = ["남궁", "제갈", "선우", "황보", "독고", "사공"];
fn is_hangul(c: char) -> bool {
    ('\u{AC00}'..='\u{D7A3}').contains(&c)
}
/// Revised Romanization of Hangul syllables (without sound-change rules), for citation keys.
pub fn romanize(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if is_hangul(c) {
            let i = c as u32 - 0xAC00;
            out.push_str(INITIALS[(i / 588) as usize]);
            out.push_str(MEDIALS[((i % 588) / 28) as usize]);
            out.push_str(FINALS[(i % 28) as usize]);
        } else {
            out.push(c);
        }
    }
    out
}
fn words(s: &str) -> Vec<String> {
    ascii_fold(&romanize(s))
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}
/// A DOI printed in the text, if any.
pub fn find_doi(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let mut from = 0;
    while let Some(rel) = lower[from..].find("10.") {
        let start = from + rel;
        from = start + 3;
        if start > 0
            && lower[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_digit())
        {
            continue;
        }
        let rest = &text[start..];
        let prefix: String = rest[3..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if !(4..=9).contains(&prefix.len()) || !rest[3 + prefix.len()..].starts_with('/') {
            continue;
        }
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '<' | '>' | '\'' | '，' | '。'))
            .unwrap_or(rest.len());
        let doi = rest[..end].trim_end_matches(['.', ',', ';', ':', ')', ']', '}']);
        if doi.len() > prefix.len() + 4 {
            return Some(doi.to_string());
        }
    }
    None
}
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(v) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
/// Page selectors publishers append to a DOI in an article address.
const DOI_URL_SUFFIXES: [&str; 19] = [
    "/full",
    "/abstract",
    "/pdf",
    "/epdf",
    "/pdfdirect",
    "/epub",
    "/fulltext",
    "/full-text",
    "/html",
    "/meta",
    "/summary",
    "/references",
    "/figures",
    "/tables",
    "/metrics",
    "/info",
    ".full",
    ".abstract",
    ".pdf",
];
/// The DOI an article's own address names: a DOI in the path or query (doi.org, Frontiers,
/// PLOS, Springer, Wiley, Science, PNAS, bioRxiv…), nature.com/articles/<id>, or an arXiv
/// abstract. Page selectors (/full, /pdf, .full…) and preprint versions are dropped.
pub fn doi_in_url(url: &str) -> Option<String> {
    let url = percent_decode(url.trim());
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    // The identifier after `prefix`, up to the query; `/` ends it unless `slashes`.
    let segment = |prefix: &str, slashes: bool| {
        path.strip_prefix(prefix).and_then(|p| {
            let id = p
                .split(|c| c == '?' || c == '#' || (c == '/' && !slashes))
                .next()
                .unwrap_or_default();
            let id = id.strip_suffix(".pdf").unwrap_or(id);
            (!id.is_empty()).then(|| id.to_string())
        })
    };
    if (host == "nature.com" || host.ends_with(".nature.com"))
        && let Some(id) = segment("articles/", false)
    {
        return Some(format!("10.1038/{id}"));
    }
    if host == "arxiv.org" || host == "export.arxiv.org" {
        // Old-style identifiers keep their archive: math/0211159.
        let id = segment("abs/", true).or_else(|| segment("pdf/", true))?;
        let id = match id.rsplit_once('v') {
            Some((base, v)) if !v.is_empty() && v.bytes().all(|c| c.is_ascii_digit()) => base,
            _ => id.as_str(),
        };
        return Some(format!("10.48550/arXiv.{id}"));
    }
    let found = find_doi(&url)?;
    let mut doi = found
        .split(['?', '&', '#'])
        .next()
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_string();
    loop {
        let lower = doi.to_lowercase();
        let Some(cut) = DOI_URL_SUFFIXES
            .iter()
            .find(|s| lower.ends_with(**s) && lower.len() > s.len())
            .map(|s| doi.len() - s.len())
        else {
            break;
        };
        doi.truncate(cut);
        doi = doi.trim_end_matches('/').to_string();
    }
    // bioRxiv/medRxiv addresses carry a version the DOI does not.
    if doi.starts_with("10.1101/")
        && let Some((base, v)) = doi.rsplit_once('v')
        && !v.is_empty()
        && v.bytes().all(|c| c.is_ascii_digit())
        && base.ends_with(|c: char| c.is_ascii_digit())
    {
        doi = base.to_string();
    }
    (doi.len() > 8 && doi.contains('/')).then_some(doi)
}
/// Whether a looked-up record's title is the source's title: the same letters, or one
/// inside the other with little to spare (a site name after the title, a subtitle).
pub fn same_title(a: &str, b: &str) -> bool {
    let (a, b) = (normalized(a), normalized(b));
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    if short.chars().count() < 12 {
        return false;
    }
    short == long
        || (short.chars().count() >= 20
            && long.contains(&short)
            && short.len() * 10 >= long.len() * 6)
}
/// Whether two author names share a family name, with or without diacritics.
pub fn same_family(a: &str, b: &str) -> bool {
    let family = |n: &str| words(&BibInfo::family(n)).concat();
    let (a, b) = (family(a), family(b));
    !a.is_empty() && a == b
}
/// "201-9" → ("201", "209"); "S44–S50" → ("S44", "S50"); a single page or article
/// number → None.
fn page_range(pages: &str) -> Option<(String, String)> {
    let p = pages.trim();
    if p.contains(',') {
        return None;
    }
    let (a, b) = p
        .split_once("--")
        .or_else(|| p.split_once(['-', '–', '—', '‐', '‑']))?;
    let (a, b) = (a.trim(), b.trim().trim_start_matches('-'));
    if a.is_empty() || b.is_empty() {
        return None;
    }
    let digits = |s: &str| s.bytes().all(|c| c.is_ascii_digit());
    // MEDLINE abbreviates the last page: 201-9 is 201–209.
    if digits(a) && digits(b) && b.len() < a.len() {
        return Some((a.into(), format!("{}{b}", &a[..a.len() - b.len()])));
    }
    Some((a.into(), b.into()))
}
const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];
impl BibInfo {
    pub fn family(author: &str) -> String {
        if let Some((family, _)) = author.split_once(',') {
            return family.trim().to_string();
        }
        let author = author.trim();
        // Korean names put the family name first, usually without a space.
        if author.chars().next().is_some_and(is_hangul) {
            let first = author.split_whitespace().next().unwrap_or(author);
            if first != author {
                return first.to_string();
            }
            if let Some(double) = DOUBLE_SURNAMES.iter().find(|d| author.starts_with(**d))
                && author.chars().count() >= 3
            {
                return double.to_string();
            }
            return author.chars().next().unwrap().to_string();
        }
        author.split_whitespace().last().unwrap_or("").to_string()
    }
    /// Family name as letters for a key: common Korean surnames by their usual spelling.
    fn family_key(author: &str) -> String {
        let family = Self::family(author);
        match SURNAMES.iter().find(|(k, _)| *k == family) {
            Some((_, latin)) => latin.to_string(),
            None => words(&family).concat(),
        }
    }
    /// How an author is named in an in-text citation: family name, or the whole name for
    /// Korean authors (Korean convention).
    fn cite_name(author: &str) -> String {
        let a = author.trim();
        if !a.contains(',') && a.chars().next().is_some_and(is_hangul) {
            a.to_string()
        } else {
            Self::family(a)
        }
    }
    /// The key this record would get now (before collision suffixes).
    pub fn base_key(&self, fallback: &str) -> String {
        let surname: String = self
            .authors
            .first()
            .map(|a| Self::family_key(a))
            .unwrap_or_default();
        let year: String = self
            .year
            .chars()
            .filter(|c| c.is_ascii_digit())
            .take(4)
            .collect();
        let title_words: Vec<String> = words(&self.title)
            .into_iter()
            .filter(|w| {
                !STOPWORDS.contains(&w.as_str())
                    && w.len() >= 3
                    && w.chars().any(|c| c.is_ascii_alphabetic())
            })
            .collect();
        let key = if !surname.is_empty() {
            format!(
                "{surname}{year}{}",
                title_words.first().map(String::as_str).unwrap_or_default()
            )
        } else if !title_words.is_empty() {
            format!(
                "{}{year}",
                title_words
                    .iter()
                    .take(2)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("")
            )
        } else {
            format!("source{year}{fallback}")
        };
        key.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(40)
            .collect()
    }
    /// "Chang & Chou, 2023" for display next to a claim.
    pub fn short_cite(&self) -> String {
        let year = if self.year.is_empty() {
            "n.d."
        } else {
            &self.year
        };
        let names: Vec<String> = self.authors.iter().map(|a| Self::cite_name(a)).collect();
        let who = match names.len() {
            0 => {
                let t: String = self.title.chars().take(28).collect();
                if self.title.chars().count() > 28 {
                    format!("“{t}…”")
                } else {
                    format!("“{t}”")
                }
            }
            1 => names[0].clone(),
            2 => format!("{} & {}", names[0], names[1]),
            _ => format!("{} et al.", names[0]),
        };
        format!("{who}, {year}")
    }
    /// A plain reference line (APA-like) for readers.
    pub fn reference(&self) -> String {
        let mut s = String::new();
        let year = format!(
            "({}).",
            if self.year.is_empty() {
                "n.d."
            } else {
                &self.year
            }
        );
        if self.authors.is_empty() {
            // No author: the title takes the author position.
            s.push_str(self.title.trim_end_matches('.'));
            s.push_str(&format!(". {year}"));
        } else {
            let mut names: Vec<String> = self.authors.iter().take(6).cloned().collect();
            if self.authors.len() > 6 {
                names.push("et al.".into());
            }
            s.push_str(&names.join("; "));
            s.push_str(&format!(" {year} "));
            s.push_str(self.title.trim_end_matches('.'));
            s.push('.');
        }
        if !self.container.is_empty() {
            s.push_str(&format!(" *{}*", self.container));
            if !self.volume.is_empty() {
                s.push_str(&format!(", {}", self.volume));
                if !self.number.is_empty() {
                    s.push_str(&format!("({})", self.number));
                }
            }
            if !self.pages.is_empty() {
                match page_range(&self.pages) {
                    Some((a, b)) => s.push_str(&format!(", {a}–{b}")),
                    None => s.push_str(&format!(", {}", self.pages)),
                }
            }
            s.push('.');
        } else if !self.publisher.is_empty() {
            s.push_str(&format!(" {}.", self.publisher));
        }
        if !self.note.is_empty() {
            s.push_str(&format!(" {}.", self.note));
        }
        if let Some(link) = self.link() {
            s.push_str(&format!(" {link}"));
        }
        s
    }
    pub fn link(&self) -> Option<String> {
        if !self.doi.is_empty() {
            Some(format!("https://doi.org/{}", self.doi))
        } else if !self.url.is_empty() {
            Some(self.url.clone())
        } else if !self.pmid.is_empty() {
            Some(format!("https://pubmed.ncbi.nlm.nih.gov/{}/", self.pmid))
        } else {
            None
        }
    }
    /// A full BibTeX entry: every field the record has, in a fixed order, page ranges with
    /// `--`, the month as a macro and a DOI link as the url when no other address is known.
    pub fn bibtex(&self, key: &str) -> String {
        let kind = match self.entry_type.as_str() {
            "article" | "book" | "online" | "misc" | "incollection" | "inproceedings" => {
                self.entry_type.as_str()
            }
            _ => "misc",
        };
        let container = match kind {
            "article" => "journal",
            "incollection" | "inproceedings" => "booktitle",
            _ => "howpublished",
        };
        let pages = match page_range(&self.pages) {
            Some((a, b)) => format!("{a}--{b}"),
            None => self.pages.trim().to_string(),
        };
        let url = if self.url.is_empty() && !self.doi.is_empty() {
            format!("https://doi.org/{}", self.doi)
        } else {
            self.url.clone()
        };
        // (field, value, unbraced)
        let mut fields: Vec<(&str, String, bool)> = vec![];
        for (name, names) in [("author", &self.authors), ("editor", &self.editors)] {
            if !names.is_empty() {
                fields.push((name, escape(&names.join(" and ")), false));
            }
        }
        fields.push(("title", escape(&self.title), false));
        for (name, value) in [
            (container, &self.container),
            ("series", &self.series),
            ("year", &self.year),
        ] {
            if !value.is_empty() {
                fields.push((name, escape(value), false));
            }
        }
        match self.month.trim().parse::<usize>() {
            Ok(m @ 1..=12) => fields.push(("month", MONTHS[m - 1].into(), true)),
            _ if !self.month.trim().is_empty() => {
                fields.push(("month", escape(self.month.trim()), false))
            }
            _ => {}
        }
        for (name, value, raw) in [
            ("volume", &self.volume, false),
            ("number", &self.number, false),
            ("pages", &pages, false),
            ("publisher", &self.publisher, false),
            ("isbn", &self.isbn, false),
            ("issn", &self.issn, false),
            ("doi", &self.doi, true),
            ("pmid", &self.pmid, false),
            ("url", &url, true),
            ("urldate", &self.urldate, false),
            ("note", &self.note, false),
        ] {
            if !value.is_empty() {
                fields.push((name, if raw { value.clone() } else { escape(value) }, false));
            }
        }
        let body = fields
            .iter()
            .map(|(k, v, bare)| {
                if *bare {
                    format!("  {k} = {v}")
                } else {
                    format!("  {k} = {{{v}}}")
                }
            })
            .collect::<Vec<_>>()
            .join(",\n");
        format!("@{kind}{{{key},\n{body}\n}}\n")
    }
}
/// BibTeX-safe text: special characters escaped, braces balanced.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0i32;
    for c in s.chars() {
        match c {
            '&' | '%' | '$' | '#' | '_' => {
                out.push('\\');
                out.push(c);
            }
            '{' => {
                depth += 1;
                out.push(c);
            }
            '}' if depth > 0 => {
                depth -= 1;
                out.push(c);
            }
            '}' => {}
            '\\' => out.push_str("\\textbackslash{}"),
            '\n' | '\r' => out.push(' '),
            _ => out.push(c),
        }
    }
    for _ in 0..depth {
        out.push('}');
    }
    out
}
impl Source {
    /// Bibliographic record from what is already known; never contacts a network.
    pub fn derived_bib(&self) -> BibInfo {
        if let Some(b) = &self.bib {
            return b.clone();
        }
        let mut b = BibInfo {
            title: self.title.clone(),
            ..BibInfo::default()
        };
        if let Some(pmid) = self
            .locator
            .strip_prefix("https://pubmed.ncbi.nlm.nih.gov/")
            .map(|r| r.trim_end_matches('/'))
            .filter(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()))
        {
            // Older searches stored "PubMed · PMID n · Journal · Year · …" only.
            let parts: Vec<&str> = self.import_note.split(" · ").collect();
            b.entry_type = "article".into();
            b.pmid = pmid.into();
            b.container = parts
                .get(2)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            b.year = parts
                .get(3)
                .map(|s| s.trim())
                .filter(|y| y.len() == 4 && y.bytes().all(|c| c.is_ascii_digit()))
                .unwrap_or_default()
                .into();
            b.url = self.locator.clone();
            b.origin = "pubmed".into();
            return b;
        }
        if self.locator.starts_with("https://") || self.locator.starts_with("http://") {
            b.entry_type = "online".into();
            b.url = self.locator.clone();
            // An article's own address names its DOI on most publisher sites.
            b.doi = doi_in_url(&self.locator).unwrap_or_default();
            b.urldate = self.retrieved_at.map(crate::civil_date).unwrap_or_default();
            b.origin = "web".into();
            return b;
        }
        // An imported file or a pasted passage: title from the file name; a DOI only if printed.
        b.entry_type = "misc".into();
        b.origin = "file".into();
        if self.asset_hash.is_some() || self.relative_path.is_some() {
            let stem = match self.title.rsplit_once('.') {
                Some((stem, ext)) if ext.len() <= 4 && !stem.is_empty() => stem,
                _ => self.title.as_str(),
            };
            b.title = stem.replace(['_', '+'], " ").trim().to_string();
            b.note = "사용자 제공 자료".into();
        } else if !self.locator.is_empty() {
            b.note = format!("발췌: {}", self.locator);
        }
        if self.access != Access::MetadataOnly {
            let head: String = self.text.lines().take(300).collect::<Vec<_>>().join("\n");
            if let Some(doi) = find_doi(&head) {
                b.doi = doi;
                b.entry_type = "article".into();
            }
        }
        b
    }
    /// The first lines of the text, where a title page or article header normally is.
    pub fn header(&self, max_bytes: usize) -> String {
        let mut out = String::new();
        for line in self.text.lines().take(60) {
            if out.len() + line.len() + 1 > max_bytes {
                break;
            }
            out.push_str(line);
            out.push('\n');
        }
        out
    }
}
fn normalized(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}
/// Keep only the fields of a scribe-supplied candidate that are printed in the source header.
/// Returns the accepted record, or None when even the title is not in the text.
pub fn verify_candidate(
    source: &Source,
    candidate: &crate::BibCandidate,
    notes: &mut Vec<String>,
) -> Option<BibInfo> {
    let header = normalized(&source.header(6000));
    let full_head: String = source.text.lines().take(300).collect::<Vec<_>>().join("\n");
    let title = candidate.title.trim();
    if normalized(title).len() < 6 || !header.contains(&normalized(title)) {
        notes.push(format!(
            "{}: 서지 후보의 제목이 원문 첫머리에 없어 사용하지 않음",
            source.title
        ));
        return None;
    }
    let mut b = source.derived_bib();
    b.title = title.to_string();
    b.origin = "source-text".into();
    let authors: Vec<String> = candidate
        .authors
        .iter()
        .map(|a| a.trim().to_string())
        .filter(|a| {
            let family = normalized(&BibInfo::family(a));
            !family.is_empty() && header.contains(&family)
        })
        .collect();
    if authors.len() < candidate.authors.len() {
        notes.push(format!(
            "{}: 원문에서 찾지 못한 저자 {}명 제외",
            source.title,
            candidate.authors.len() - authors.len()
        ));
    }
    b.authors = authors;
    let year = candidate.year.trim();
    if year.len() == 4 && year.bytes().all(|c| c.is_ascii_digit()) && header.contains(year) {
        b.year = year.into();
    }
    for (value, field) in [
        (&candidate.container, &mut b.container),
        (&candidate.publisher, &mut b.publisher),
    ] {
        if !value.trim().is_empty() && header.contains(&normalized(value)) {
            *field = value.trim().to_string();
        }
    }
    let doi = candidate.doi.trim();
    if !doi.is_empty() && full_head.to_lowercase().contains(&doi.to_lowercase()) {
        b.doi = doi.into();
    }
    if matches!(
        candidate.entry_type.as_str(),
        "article" | "book" | "misc" | "incollection"
    ) {
        b.entry_type = candidate.entry_type.clone();
    }
    b.note = String::new();
    // A confirmed printed title is worth a fresh lookup, even if the file name found nothing.
    b.looked_up = false;
    Some(b)
}
/// Keys for the given sources: frozen keys stay, the rest get a unique key from current
/// metadata. Collisions take a, b, c… suffixes in a deterministic order.
pub fn assign_keys(
    sources: &[&Source],
    frozen_elsewhere: &HashSet<String>,
) -> HashMap<String, String> {
    let mut used: HashSet<String> = frozen_elsewhere.clone();
    let mut out = HashMap::new();
    for s in sources {
        if let Some(key) = s
            .bib
            .as_ref()
            .map(|b| b.key.clone())
            .filter(|k| !k.is_empty())
        {
            used.insert(key.clone());
            out.insert(s.id.clone(), key);
        }
    }
    for s in sources {
        if out.contains_key(&s.id) {
            continue;
        }
        let base = s.derived_bib().base_key(&s.hash[..s.hash.len().min(6)]);
        let mut key = base.clone();
        let mut n = 0u8;
        while used.contains(&key) {
            key = format!("{base}{}", (b'a' + n) as char);
            n = n.saturating_add(1);
            if n > 25 {
                key = format!("{base}{}", &s.id[s.id.len().saturating_sub(6)..]);
                break;
            }
        }
        used.insert(key.clone());
        out.insert(s.id.clone(), key);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record;
    #[test]
    fn keys_are_readable_unique_and_ascii() {
        let mut a = Source::new("x".into(), "".into(), "".into(), Access::ProvidedText);
        a.bib = Some(BibInfo {
            title: "Mixed-mode oscillations in a mood model".into(),
            authors: vec!["Chang, Seung-Ho".into(), "Chou, Tom".into()],
            year: "2023".into(),
            ..BibInfo::default()
        });
        let mut b = a.clone();
        b.id = "S-other".into();
        let mut c = Source::new(
            "기분장애 해설.md".into(),
            "본문".into(),
            "".into(),
            Access::ProvidedText,
        );
        c.asset_hash = Some("ab".repeat(32));
        let mut d = a.clone();
        d.id = "S-frozen".into();
        d.bib.as_mut().unwrap().key = "frozen2020".into();
        let keys = assign_keys(&[&a, &b, &c, &d], &HashSet::new());
        assert_eq!(keys[&a.id], "chang2023mixed");
        assert_eq!(keys[&b.id], "chang2023mixeda");
        assert_eq!(keys[&c.id], "gibunjangaehaeseol");
        assert_eq!(keys[&d.id], "frozen2020");
        assert_eq!(a.derived_bib().short_cite(), "Chang & Chou, 2023");
        let k = BibInfo {
            authors: vec!["김철수".into(), "남궁민".into()],
            year: "2024".into(),
            title: "조울병의 동역학".into(),
            ..BibInfo::default()
        };
        assert_eq!(k.base_key(""), "kim2024joulbyeongui");
        assert_eq!(k.short_cite(), "김철수 & 남궁민, 2024");
        assert_eq!(BibInfo::family("남궁민"), "남궁");
        assert_eq!(romanize("합성 발췌"), "hapseong balchwe");
    }
    #[test]
    fn bibtex_escapes_and_balances() {
        let b = BibInfo {
            entry_type: "article".into(),
            title: "Mood {dynamics & 50% of_cases".into(),
            authors: vec!["Müller, Jörg".into(), "Lee, K".into()],
            year: "2021".into(),
            container: "J. Affect. Disord.".into(),
            doi: "10.1000/x_y".into(),
            ..BibInfo::default()
        };
        let t = b.bibtex("muller2021mood");
        assert!(t.starts_with("@article{muller2021mood,"));
        assert!(t.contains("title = {Mood {dynamics \\& 50\\% of\\_cases}}"));
        assert!(t.contains("author = {Müller, Jörg and Lee, K}"));
        assert!(t.contains("journal = {J. Affect. Disord.}"));
        assert!(t.contains("doi = {10.1000/x_y}"));
        assert_eq!(b.base_key(""), "muller2021mood");
    }
    #[test]
    fn bibtex_entries_are_complete() {
        let b = record::<BibInfo>(|b| {
            b.entry_type = "incollection".into();
            b.title = "Analytical approach".into();
            b.authors = vec!["Ruiz, Ana".into()];
            b.editors = vec!["Kim, Bo".into(), "Lee, Cy".into()];
            b.container = "Neural Information Processing".into();
            b.series = "Lecture Notes in Computer Science".into();
            b.year = "2025".into();
            b.month = "5".into();
            b.pages = "201-9".into();
            b.publisher = "Springer".into();
            b.isbn = "9783032383976".into();
            b.doi = "10.1007/978-3-032-38398-3_14".into();
        });
        assert_eq!(
            b.bibtex("ruiz2025analytical"),
            "@incollection{ruiz2025analytical,\n  author = {Ruiz, Ana},\n  editor = {Kim, Bo and Lee, Cy},\n  title = {Analytical approach},\n  booktitle = {Neural Information Processing},\n  series = {Lecture Notes in Computer Science},\n  year = {2025},\n  month = may,\n  pages = {201--209},\n  publisher = {Springer},\n  isbn = {9783032383976},\n  doi = {10.1007/978-3-032-38398-3_14},\n  url = {https://doi.org/10.1007/978-3-032-38398-3_14}\n}\n"
        );
        assert!(b.reference().contains(", 201–209."));
        assert_eq!(page_range("S44–S50"), Some(("S44".into(), "S50".into())));
        assert_eq!(page_range("119250"), None);
        // A page that names no DOI keeps its own address and the day it was read.
        let mut s = Source::new(
            "A page".into(),
            "".into(),
            "https://example.org/a".into(),
            Access::MetadataOnly,
        );
        s.retrieved_at = Some(1_760_400_000);
        let w = s.derived_bib().bibtex("page");
        assert!(
            w.contains("url = {https://example.org/a}") && w.contains("urldate = {2025-10-14}")
        );
    }
    #[test]
    fn article_addresses_name_their_doi() {
        for (url, doi) in [
            (
                "https://www.nature.com/articles/s42005-022-00950-y",
                "10.1038/s42005-022-00950-y",
            ),
            (
                "https://www.frontiersin.org/journals/computational-neuroscience/articles/10.3389/fncom.2022.866517/full",
                "10.3389/fncom.2022.866517",
            ),
            (
                "https://link.springer.com/chapter/10.1007/978-3-032-38398-3_14",
                "10.1007/978-3-032-38398-3_14",
            ),
            (
                "https://journals.plos.org/plosbiology/article?id=10.1371/journal.pbio.3003916",
                "10.1371/journal.pbio.3003916",
            ),
            (
                "https://journals.plos.org/plosone/article?id=10.1371%2Fjournal.pone.0123456&type=printable",
                "10.1371/journal.pone.0123456",
            ),
            (
                "https://onlinelibrary.wiley.com/doi/epdf/10.1111/bdi.13000",
                "10.1111/bdi.13000",
            ),
            (
                "https://www.biorxiv.org/content/10.1101/2023.01.02.522510v2.full.pdf",
                "10.1101/2023.01.02.522510",
            ),
            (
                "https://arxiv.org/abs/2301.12345v3",
                "10.48550/arXiv.2301.12345",
            ),
            (
                "https://arxiv.org/abs/math/0211159",
                "10.48550/arXiv.math/0211159",
            ),
        ] {
            assert_eq!(doi_in_url(url).as_deref(), Some(doi), "{url}");
        }
        assert_eq!(
            doi_in_url("https://www.sciencedirect.com/science/article/pii/S0166223613000465"),
            None
        );
        let s = Source::new(
            "Emergence of multiple subnetworks".into(),
            "".into(),
            "https://www.nature.com/articles/s41598-024-80510-2".into(),
            Access::MetadataOnly,
        );
        assert_eq!(s.derived_bib().doi, "10.1038/s41598-024-80510-2");
    }
    #[test]
    fn looked_up_titles_must_be_the_same_title() {
        let t = "Metastable oscillatory modes emerge from synchronization in the brain spacetime connectome";
        assert!(same_title(t, &t.to_uppercase()));
        assert!(same_title(&format!("{t} | Communications Physics"), t));
        assert!(same_title(
            "How critical is brain criticality? - ScienceDirect",
            "How critical is brain criticality?"
        ));
        assert!(!same_title(
            "Brain criticality",
            "Brain criticality revisited in sleep"
        ));
        assert!(!same_title(t, "Metastable oscillatory modes"));
        assert!(!same_title("Sleep", "Sleep"));
    }
    #[test]
    fn dois_and_legacy_pubmed_notes_are_read_without_guessing() {
        assert_eq!(
            find_doi("Published: doi:10.1038/s41586-020-2649-2. Next"),
            Some("10.1038/s41586-020-2649-2".into())
        );
        assert_eq!(find_doi("version 210.12/3 and 10.1/x"), None);
        let mut s = Source::new(
            "A title".into(),
            "Abstract".into(),
            "https://pubmed.ncbi.nlm.nih.gov/12345/".into(),
            Access::AbstractOnly,
        );
        s.import_note = "PubMed · PMID 12345 · Bipolar Disord · 2019 · 초록만 확보".into();
        let b = s.derived_bib();
        assert_eq!((b.pmid.as_str(), b.year.as_str()), ("12345", "2019"));
        assert_eq!(b.container, "Bipolar Disord");
    }
    #[test]
    fn scribe_metadata_is_kept_only_where_printed() {
        let s = Source::new(
            "paper.pdf".into(),
            "Journal of Dynamics 12 (2020)\nOscillatory mood states\nAna Ruiz and Bo Kim\ndoi: 10.5555/abc.1".into(),
            "".into(),
            Access::ProvidedText,
        );
        let mut notes = vec![];
        let c = record::<crate::BibCandidate>(|c| {
            c.title = "Oscillatory mood states".into();
            c.authors = vec!["Ruiz, Ana".into(), "Invented, Person".into()];
            c.year = "2020".into();
            c.container = "Journal of Dynamics".into();
            c.doi = "10.5555/abc.1".into();
            c.entry_type = "article".into();
        });
        let b = verify_candidate(&s, &c, &mut notes).unwrap();
        assert_eq!(b.authors, ["Ruiz, Ana"]);
        assert_eq!((b.year.as_str(), b.doi.as_str()), ("2020", "10.5555/abc.1"));
        assert_eq!(b.container, "Journal of Dynamics");
        let mut fake = c.clone();
        fake.title = "A paper that is not there".into();
        assert!(verify_candidate(&s, &fake, &mut notes).is_none());
    }
}
