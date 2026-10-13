//! Resolve formatting-only citation differences against the exact transmitted passage.
//! The raw response stays intact; words and punctuation are never repaired or invented.
use serde_json::Value;
use theory_core::{Envelope, Source};

fn normalized(text: &str) -> (String, Vec<(usize, usize)>) {
    let mut out = String::new();
    let mut map: Vec<(usize, usize)> = vec![];
    for (offset, c) in text.char_indices() {
        let end = offset + c.len_utf8();
        if c.is_whitespace() {
            if out.ends_with(' ') {
                map.last_mut().unwrap().1 = end;
            } else if !out.is_empty() {
                out.push(' ');
                map.push((offset, end));
            }
        } else {
            out.push(c);
            map.extend(std::iter::repeat_n((offset, end), c.len_utf8()));
        }
    }
    (out, map)
}

pub(super) fn reconcile(output: &mut Envelope, sources: &[Source], context: &Value) -> Vec<String> {
    let mut notes = vec![];
    for evidence in &mut output.evidence_refs {
        let Some(source) = sources
            .iter()
            .find(|s| s.id == evidence.source_id && s.hash == evidence.source_hash)
        else {
            continue;
        };
        let Some(sent) = context["sources"]
            .as_array()
            .and_then(|a| a.iter().find(|s| s["id"] == source.id))
        else {
            continue;
        };
        let (Some(first), Some(last)) = (
            sent["included_lines"][0].as_u64(),
            sent["included_lines"][1].as_u64(),
        ) else {
            continue;
        };
        let lines: Vec<_> = source.text.lines().collect();
        if first == 0 || first > last || last > lines.len() as u64 {
            continue;
        }
        let passage = lines[first as usize - 1..last as usize].join("\n");
        let quote = evidence
            .quote
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if quote.is_empty() {
            continue;
        }
        let (flat, map) = normalized(&passage);
        let mut matches = flat.char_indices().map(|(i, _)| i).filter(|i| {
            flat[*i..].starts_with(&quote)
                && theory_core::quote_boundaries(&flat, *i, *i + quote.len())
        });
        let Some(offset) = matches.next() else {
            continue;
        };
        // Ambiguous text is not automatically assigned a source location.
        if matches.next().is_some() {
            continue;
        }
        let start = map[offset].0;
        let end = map[offset + quote.len() - 1].1;
        let actual = &passage[start..end];
        let start_line =
            first as u32 + passage[..start].bytes().filter(|b| *b == b'\n').count() as u32;
        let end_line = start_line + actual.bytes().filter(|b| *b == b'\n').count() as u32;
        if evidence.quote != actual
            || evidence.start_line != start_line
            || evidence.end_line != end_line
        {
            notes.push(format!(
                "{} · {}: 원문 단어·문장부호 일치 확인 후 줄바꿈/위치를 행 {}–{}로 보정",
                evidence.claim_id, source.id, start_line, end_line
            ));
            evidence.quote = actual.into();
            evidence.start_line = start_line;
            evidence.end_line = end_line;
        }
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use theory_core::{Access, Evidence, record};
    #[test]
    fn restores_original_unicode_whitespace_and_location_without_changing_words() {
        let source = Source::new(
            "Paper".into(),
            "Heading\n한글 alpha  \n beta finding.\nOther".into(),
            "Test".into(),
            Access::ProvidedText,
        );
        let mut output = Envelope::default();
        output.evidence_refs.push(record::<Evidence>(|e| {
            e.source_id = source.id.clone();
            e.source_hash = source.hash.clone();
            e.quote = "한글 alpha beta finding.".into();
            e.start_line = 2;
            e.end_line = 2;
        }));
        let context = json!({"sources":[{"id":source.id,"included_lines":[2,3]}]});
        assert_eq!(
            reconcile(&mut output, std::slice::from_ref(&source), &context).len(),
            1
        );
        assert_eq!(
            output.evidence_refs[0].quote,
            "한글 alpha  \n beta finding."
        );
        assert_eq!(output.evidence_refs[0].end_line, 3);
        assert!(reconcile(&mut output, std::slice::from_ref(&source), &context).is_empty());
        output.evidence_refs[0].quote = "한글 alpha invented finding.".into();
        assert!(reconcile(&mut output, &[source], &context).is_empty());
        assert!(output.evidence_refs[0].quote.contains("invented"));
    }
    #[test]
    fn refuses_ambiguous_or_untransmitted_quotes() {
        let source = Source::new(
            "Paper".into(),
            "same finding\nsame finding\nhidden finding".into(),
            "Test".into(),
            Access::ProvidedText,
        );
        let mut output = Envelope::default();
        output.evidence_refs.push(record::<Evidence>(|e| {
            e.source_id = source.id.clone();
            e.source_hash = source.hash.clone();
            e.quote = "same finding".into();
        }));
        let context = json!({"sources":[{"id":source.id,"included_lines":[1,2]}]});
        assert!(reconcile(&mut output, std::slice::from_ref(&source), &context).is_empty());
        output.evidence_refs[0].quote = "hidden finding".into();
        assert!(reconcile(&mut output, &[source], &context).is_empty());
    }
    #[test]
    fn overlapping_matches_are_ambiguous() {
        let source = Source::new(
            "Paper".into(),
            "had had had".into(),
            "Test".into(),
            Access::ProvidedText,
        );
        let mut output = Envelope::default();
        output.evidence_refs.push(record::<Evidence>(|e| {
            e.source_id = source.id.clone();
            e.source_hash = source.hash.clone();
            e.quote = "had had".into();
        }));
        let context = json!({"sources":[{"id":source.id,"included_lines":[1,1]}]});
        assert!(reconcile(&mut output, &[source], &context).is_empty());
        assert_eq!(output.evidence_refs[0].start_line, 0);
    }
    #[test]
    fn cannot_cut_a_word_prefix_to_change_its_meaning() {
        for (text, quote) in [
            ("subcortical activity", "cortical activity"),
            ("nonpositive effect", "positive effect"),
            ("non-positive effect", "positive effect"),
        ] {
            let source = Source::new(
                "Paper".into(),
                text.into(),
                "Test".into(),
                Access::ProvidedText,
            );
            let mut output = Envelope::default();
            output.evidence_refs.push(record::<Evidence>(|e| {
                e.source_id = source.id.clone();
                e.source_hash = source.hash.clone();
                e.quote = quote.into();
            }));
            let context = json!({"sources":[{"id":source.id,"included_lines":[1,1]}]});
            assert!(reconcile(&mut output, &[source], &context).is_empty());
            assert!(!theory_core::quote_boundaries(
                text,
                text.find(quote).unwrap(),
                text.len()
            ));
        }
    }
}
