//! Bibliographic records for the sources a report cites, their pandoc/BibTeX keys and
//! the reference lines readers see. Metadata comes from PubMed, Crossref, the source's own
//! printed header (checked letter by letter) or, at worst, its file name — never a guess.
use crate::{Access, Source};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct BibInfo {
    /// Citation key. Empty until a report cites the source; then frozen so that stored
    /// report text and exported BibTeX keep agreeing.
    #[serde(default)]
    pub key: String,
    /// article | book | misc | online
    pub entry_type: String,
    pub title: String,
    /// "Family, Given" or a collective name.
    pub authors: Vec<String>,
    pub year: String,
    /// Journal or book title.
    pub container: String,
    pub volume: String,
    pub number: String,
    pub pages: String,
    pub publisher: String,
    pub doi: String,
    pub pmid: String,
    pub url: String,
    pub note: String,
    /// pubmed | crossref | source-text | file | web
    pub origin: String,
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
                s.push_str(&format!(", {}", self.pages));
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
    pub fn bibtex(&self, key: &str) -> String {
        let kind = match self.entry_type.as_str() {
            "article" | "book" | "online" | "misc" | "incollection" | "inproceedings" => {
                self.entry_type.as_str()
            }
            _ => "misc",
        };
        let mut fields: Vec<(&str, String)> = vec![];
        if !self.authors.is_empty() {
            fields.push(("author", self.authors.join(" and ")));
        }
        fields.push(("title", self.title.clone()));
        let container = match kind {
            "article" => "journal",
            "incollection" | "inproceedings" => "booktitle",
            _ => "howpublished",
        };
        for (name, value) in [
            ("year", &self.year),
            (container, &self.container),
            ("volume", &self.volume),
            ("number", &self.number),
            ("pages", &self.pages),
            ("publisher", &self.publisher),
            ("doi", &self.doi),
            ("pmid", &self.pmid),
            ("url", &self.url),
            ("note", &self.note),
        ] {
            if !value.is_empty() {
                fields.push((name, value.clone()));
            }
        }
        let body = fields
            .iter()
            .map(|(k, v)| {
                let raw = matches!(*k, "url" | "doi");
                format!("  {k} = {{{}}}", if raw { v.clone() } else { escape(v) })
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
