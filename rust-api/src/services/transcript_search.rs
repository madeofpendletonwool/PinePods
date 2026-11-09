//! Searching inside transcripts: reading the search box into terms, writing those terms for each
//! backend's full-text search, and finding the moments in a transcript where they were said.

use serde::{Deserialize, Serialize};

const MAX_TERMS: usize = 8;
const MAX_TERM_CHARS: usize = 100;
/// Characters of context kept around a match.
const SNIPPET_CHARS: usize = 220;
/// A match this soon after the previous moment belongs to it.
const MERGE_SECONDS: f64 = 20.0;

/// One word or quoted phrase from the search box, lowercased.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    pub text: String,
    pub phrase: bool,
    /// `-word`: the transcript must not contain it.
    pub negate: bool,
}

/// Read a search box query: words, `"quoted phrases"` and `-excluded` words or phrases. Every
/// term that isn't excluded must appear.
pub fn parse_query(input: &str) -> Vec<Term> {
    let mut terms = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        let mut negate = false;
        if c == '-' {
            negate = true;
            chars.next();
        }
        let raw: String = if chars.peek() == Some(&'"') {
            chars.next();
            let s: String = chars.by_ref().take_while(|&c| c != '"').collect();
            s
        } else {
            let mut s = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                s.push(c);
                chars.next();
            }
            s
        };
        let words = clean_words(&raw);
        if words.is_empty() {
            continue;
        }
        let text: String = words.join(" ").chars().take(MAX_TERM_CHARS).collect();
        terms.push(Term { phrase: words.len() > 1, text, negate });
        if terms.len() == MAX_TERMS {
            break;
        }
    }
    terms
}

/// Lowercased words of a term, split where the full-text parsers would split it.
fn clean_words(raw: &str) -> Vec<String> {
    raw.split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .map(|w| w.trim_matches('\'').to_lowercase())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Terms that must appear (what snippets look for).
pub fn wanted(terms: &[Term]) -> Vec<&Term> {
    terms.iter().filter(|t| !t.negate).collect()
}

/// The query for PostgreSQL's `websearch_to_tsquery`.
pub fn websearch_query(terms: &[Term]) -> String {
    terms
        .iter()
        .map(|t| {
            let body = if t.phrase { format!("\"{}\"", t.text) } else { t.text.clone() };
            if t.negate { format!("-{body}") } else { body }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The query for MariaDB/MySQL `MATCH ... AGAINST (... IN BOOLEAN MODE)`: every term required,
/// excluded terms prefixed with `-`. Terms are already stripped of boolean-mode operators.
pub fn boolean_mode_query(terms: &[Term]) -> String {
    terms
        .iter()
        .map(|t| {
            let body = if t.phrase { format!("\"{}\"", t.text) } else { t.text.clone() };
            format!("{}{body}", if t.negate { '-' } else { '+' })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A place in a transcript where the search terms were said.
#[derive(Debug, Clone, Serialize, PartialEq, utoipa::ToSchema)]
pub struct Moment {
    /// Seconds into the episode; null for transcripts without timings.
    pub start: Option<f64>,
    /// The words around the match.
    pub text: String,
}

#[derive(Deserialize)]
struct Segment {
    #[serde(default)]
    start: f64,
    #[serde(default)]
    text: String,
}

/// Up to `max` moments where the wanted terms appear, and how many times they appear in all.
/// Uses the timed segments when there are any, otherwise the plain text. Whole-word matches
/// first; if there are none, any substring (how SQLite matched).
pub fn find_moments(segments_json: Option<&str>, full_text: Option<&str>, terms: &[Term], max: usize) -> (Vec<Moment>, usize) {
    let wanted = wanted(terms);
    if wanted.is_empty() {
        return (Vec::new(), 0);
    }
    let needles: Vec<Vec<char>> = wanted.iter().map(|t| fold(&t.text)).collect();

    let segments: Vec<Segment> = segments_json
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    if !segments.is_empty() {
        for whole_words in [true, false] {
            let found = moments_in_segments(&segments, &needles, whole_words, max);
            if found.1 > 0 {
                return found;
            }
        }
        return (Vec::new(), 0);
    }

    let Some(text) = full_text.filter(|t| !t.trim().is_empty()) else { return (Vec::new(), 0) };
    let original: Vec<char> = text.chars().collect();
    let folded = fold(text);
    for whole_words in [true, false] {
        let mut hits: Vec<(usize, usize)> = needles
            .iter()
            .flat_map(|n| find_all(&folded, n, whole_words).into_iter().map(move |i| (i, n.len())))
            .collect();
        if hits.is_empty() {
            continue;
        }
        hits.sort_unstable();
        let total = hits.len();
        let mut moments = Vec::new();
        let mut covered_to = 0;
        for (at, len) in hits {
            if at < covered_to {
                continue;
            }
            let (text, end) = window(&original, at, len);
            covered_to = end;
            moments.push(Moment { start: None, text });
            if moments.len() == max {
                break;
            }
        }
        return (moments, total);
    }
    (Vec::new(), 0)
}

fn moments_in_segments(segments: &[Segment], needles: &[Vec<char>], whole_words: bool, max: usize) -> (Vec<Moment>, usize) {
    let mut total = 0;
    let mut moments: Vec<Moment> = Vec::new();
    let mut last_start: Option<f64> = None;
    for (i, seg) in segments.iter().enumerate() {
        let folded = fold(&seg.text);
        let mut first: Option<(usize, usize)> = None;
        for n in needles {
            let found = find_all(&folded, n, whole_words);
            total += found.len();
            if let Some(&at) = found.first() {
                if first.is_none_or(|(f, _)| at < f) {
                    first = Some((at, n.len()));
                }
            }
        }
        let Some((at, len)) = first else { continue };
        if last_start.is_some_and(|s| seg.start - s < MERGE_SECONDS) || moments.len() == max {
            continue;
        }
        last_start = Some(seg.start);

        // Short segments read better with their neighbours around them.
        let mut before = String::new();
        let mut after = String::new();
        if seg.text.chars().count() < SNIPPET_CHARS / 2 {
            if let Some(prev) = i.checked_sub(1).and_then(|p| segments.get(p)) {
                before = format!("{} ", prev.text.trim());
            }
            if let Some(next) = segments.get(i + 1) {
                after = format!(" {}", next.text.trim());
            }
        }
        let trimmed = seg.text.trim_start();
        let lead = seg.text.chars().count() - trimmed.chars().count();
        let joined: Vec<char> = format!("{before}{}{after}", seg.text.trim()).chars().collect();
        let offset = before.chars().count() + at.saturating_sub(lead);
        let (text, _) = window(&joined, offset, len);
        moments.push(Moment { start: Some(seg.start), text });
    }
    (moments, total)
}

/// Lowercase one char for one char, so positions line up with the original text.
fn fold(s: &str) -> Vec<char> {
    s.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect()
}

fn find_all(hay: &[char], needle: &[char], whole_words: bool) -> Vec<usize> {
    let mut out = Vec::new();
    if needle.is_empty() || needle.len() > hay.len() {
        return out;
    }
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        if hay[i..i + needle.len()] == *needle {
            let starts = i == 0 || !hay[i - 1].is_alphanumeric();
            let ends = hay.get(i + needle.len()).is_none_or(|c| !c.is_alphanumeric());
            if !whole_words || (starts && ends) {
                out.push(i);
                i += needle.len();
                continue;
            }
        }
        i += 1;
    }
    out
}

/// About [`SNIPPET_CHARS`] of text around `chars[at..at + len]`, cut at word boundaries, with
/// an ellipsis where it was cut. Also returns where the window ends.
fn window(chars: &[char], at: usize, len: usize) -> (String, usize) {
    let half = SNIPPET_CHARS.saturating_sub(len) / 2;
    let mut start = at.saturating_sub(half);
    let mut end = (at + len + half).min(chars.len());
    if start > 0 {
        while start < at && !chars[start - 1].is_whitespace() {
            start += 1;
        }
    }
    if end < chars.len() {
        while end > at + len && !chars[end].is_whitespace() {
            end -= 1;
        }
    }
    let body: String = chars[start..end].iter().collect();
    let body = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut text = String::new();
    if start > 0 {
        text.push('…');
    }
    text.push_str(&body);
    if end < chars.len() {
        text.push('…');
    }
    (text, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_words_phrases_and_exclusions() {
        let t = parse_query(r#"Rust  "memory safety" -garbage -"go lang""#);
        assert_eq!(
            t,
            vec![
                Term { text: "rust".into(), phrase: false, negate: false },
                Term { text: "memory safety".into(), phrase: true, negate: false },
                Term { text: "garbage".into(), phrase: false, negate: true },
                Term { text: "go lang".into(), phrase: true, negate: true },
            ]
        );
        assert_eq!(websearch_query(&t), r#"rust "memory safety" -garbage -"go lang""#);
        assert_eq!(boolean_mode_query(&t), r#"+rust +"memory safety" -garbage -"go lang""#);
    }

    #[test]
    fn strips_operators_and_splits_hyphenated_words() {
        let t = parse_query(r#"covid-19 +(test)* "" - @x"#);
        assert_eq!(t[0], Term { text: "covid 19".into(), phrase: true, negate: false });
        assert_eq!(t[1], Term { text: "test".into(), phrase: false, negate: false });
        assert_eq!(t[2], Term { text: "x".into(), phrase: false, negate: false });
        assert_eq!(t.len(), 3);
        assert!(parse_query("  \"\"  -  ").is_empty());
    }

    #[test]
    fn finds_timed_moments_and_merges_close_ones() {
        let segs = r#"[
            {"start": 0.0, "end": 4.0, "text": "Welcome back to the show."},
            {"start": 4.0, "end": 9.0, "text": "Today we talk about Kubernetes."},
            {"start": 9.0, "end": 14.0, "text": "Kubernetes is everywhere."},
            {"start": 300.0, "end": 305.0, "text": "Back to kubernetes networking."}
        ]"#;
        let (moments, total) = find_moments(Some(segs), None, &parse_query("kubernetes"), 10);
        assert_eq!(total, 3);
        assert_eq!(moments.len(), 2);
        assert_eq!(moments[0].start, Some(4.0));
        assert!(moments[0].text.contains("Today we talk about Kubernetes."));
        assert!(moments[0].text.starts_with("Welcome back"));
        assert_eq!(moments[1].start, Some(300.0));
    }

    #[test]
    fn whole_words_first_then_substrings() {
        let segs = r#"[{"start": 1.0, "text": "running late"}, {"start": 60.0, "text": "a run"}]"#;
        let (m, total) = find_moments(Some(segs), None, &parse_query("run"), 5);
        assert_eq!((m.len(), total), (1, 1));
        assert_eq!(m[0].start, Some(60.0));
        let (m, total) = find_moments(Some(segs), None, &parse_query("runn"), 5);
        assert_eq!((m.len(), total), (1, 1));
        assert_eq!(m[0].start, Some(1.0));
    }

    #[test]
    fn untimed_text_gets_windows_with_ellipses() {
        let filler = "word ".repeat(100);
        let text = format!("{filler}the secret ingredient is love {filler}");
        let (m, total) = find_moments(None, Some(&text), &parse_query("\"secret ingredient\""), 5);
        assert_eq!(total, 1);
        assert_eq!(m[0].start, None);
        assert!(m[0].text.starts_with('…') && m[0].text.ends_with('…'));
        assert!(m[0].text.contains("the secret ingredient is love"));
        assert!(m[0].text.chars().count() <= SNIPPET_CHARS + 2);
    }

    #[test]
    fn non_ascii_text_keeps_positions() {
        let segs = r#"[{"start": 2.0, "text": "Ärger über die Straße"}]"#;
        let (m, _) = find_moments(Some(segs), None, &parse_query("straße"), 5);
        assert_eq!(m[0].text, "Ärger über die Straße");
    }
}
