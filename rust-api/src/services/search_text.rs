//! Episode search: the plain text of a description, and the terms a search box is split into.
//!
//! Descriptions are stored as feed HTML, so matching them directly also matches link targets,
//! markup and URLs repeated in every episode. Search matches `DescriptionText` instead: the
//! words a listener actually sees, without tags or web addresses.

use regex::Regex;
use std::sync::LazyLock;

/// Most terms one search uses; the rest are ignored.
const MAX_TERMS: usize = 8;
/// Longest term kept, in characters.
const MAX_TERM_CHARS: usize = 100;

/// The visible words of an HTML (or plain) description: tags removed, the contents of
/// `<script>`/`<style>` dropped, entities decoded, web addresses removed, whitespace collapsed.
pub fn plain_text(html: &str) -> String {
    let stripped = strip_tags(html);
    let decoded = decode_entities(&stripped);
    let mut out = String::with_capacity(decoded.len());
    for word in decoded.split_whitespace() {
        if is_web_address(word) {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        // A '<' that doesn't open a tag ("a < b") is text.
        let opens_tag = after
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '/' || c == '!' || c == '?');
        if !opens_tag {
            out.push('<');
            rest = after;
            continue;
        }
        let Some(end) = after.find('>') else {
            // Unclosed tag: drop the remainder.
            rest = "";
            break;
        };
        let tag = &after[..end];
        rest = &after[end + 1..];
        out.push(' ');
        let name: String = tag
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        if name == "script" || name == "style" {
            let close = format!("</{}", name);
            match rest.to_ascii_lowercase().find(&close) {
                Some(i) => {
                    rest = &rest[i..];
                    // The closing tag itself is consumed by the next loop pass.
                }
                None => rest = "",
            }
        }
    }
    out.push_str(rest);
    out
}

fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let decoded = after
            .find(';')
            .filter(|&semi| semi > 0 && semi <= 10)
            .and_then(|semi| entity(&after[..semi]).map(|c| (c, semi)));
        match decoded {
            Some((c, semi)) => {
                out.push(c);
                rest = &after[semi + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    if let Some(num) = name.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse().ok()?,
        };
        return char::from_u32(code).map(|c| if c == '\u{a0}' { ' ' } else { c });
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" | "rsquo" | "lsquo" => '\'',
        "rdquo" | "ldquo" => '"',
        "nbsp" => ' ',
        "mdash" => '—',
        "ndash" => '–',
        "hellip" => '…',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        _ => return None,
    })
}

/// `https://…`, `www.…`, or a bare `domain.tld/path`.
fn is_web_address(word: &str) -> bool {
    static BARE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^[(\[<]?[a-z0-9-]+(\.[a-z0-9-]+)*\.[a-z]{2,}/\S*$").unwrap());
    let w = word.trim_start_matches(['(', '[', '<', '"', '\'']).to_ascii_lowercase();
    w.starts_with("http://") || w.starts_with("https://") || w.starts_with("www.") || BARE.is_match(&w)
}

/// What a search box holds, split into terms: words, or "quoted phrases" kept together.
/// Lowercased, deduplicated, at most [`MAX_TERMS`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchTerms(Vec<String>);

impl SearchTerms {
    pub fn parse(input: &str) -> Self {
        let mut terms: Vec<String> = Vec::new();
        let mut push = |t: &str| {
            let t: String = t.trim().chars().take(MAX_TERM_CHARS).collect::<String>().to_lowercase();
            if !t.is_empty() && !terms.contains(&t) && terms.len() < MAX_TERMS {
                terms.push(t);
            }
        };
        for (i, part) in input.split('"').enumerate() {
            // Odd parts sit between quotes (an unclosed quote runs to the end).
            if i % 2 == 1 {
                push(&part.split_whitespace().collect::<Vec<_>>().join(" "));
            } else {
                part.split_whitespace().for_each(&mut push);
            }
        }
        SearchTerms(terms)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn terms(&self) -> &[String] {
        &self.0
    }

    /// One `LIKE ... ESCAPE '!'` pattern per term: `%term%` with `!`, `%` and `_` escaped.
    pub fn like_patterns(&self) -> Vec<String> {
        self.0
            .iter()
            .map(|t| {
                let mut p = String::with_capacity(t.len() + 2);
                p.push('%');
                for c in t.chars() {
                    if matches!(c, '!' | '%' | '_') {
                        p.push('!');
                    }
                    p.push(c);
                }
                p.push('%');
                p
            })
            .collect()
    }

    /// 0 when every term is in the title, 1 when some are, 2 when none are (the episode
    /// matched on its description alone). Mirrors the SQL ordering.
    pub fn title_rank(&self, title: &str) -> i32 {
        if self.0.is_empty() {
            return 0;
        }
        let title = title.to_lowercase();
        let hits = self.0.iter().filter(|t| title.contains(t.as_str())).count();
        match hits {
            n if n == self.0.len() => 0,
            0 => 2,
            _ => 1,
        }
    }
}

/// SQL fragments for a search over `title_col` and `text_col`, with each term's pattern bound
/// as a parameter. `param(i)` names the placeholder for term `i` (`$3`, `?`, ...).
pub struct SearchSql {
    /// ` AND (title LIKE p1 OR text LIKE p1) AND (...)`: every term somewhere.
    pub filter: String,
    /// `CASE ... END`: 0 all terms in the title, 1 some, 2 none.
    pub rank: String,
}

impl SearchSql {
    pub fn new(terms: &SearchTerms, like: &str, title_col: &str, text_col: &str, param: impl Fn(usize) -> String) -> Self {
        if terms.is_empty() {
            return SearchSql { filter: String::new(), rank: "0".into() };
        }
        let n = terms.len();
        let filter: String = (0..n)
            .map(|i| {
                let p = param(i);
                format!(" AND ({title_col} {like} {p} ESCAPE '!' OR {text_col} {like} {p} ESCAPE '!')")
            })
            .collect();
        let in_title = |i: usize| format!("{title_col} {like} {} ESCAPE '!'", param(i));
        let all: Vec<String> = (0..n).map(in_title).collect();
        let rank = format!("CASE WHEN {} THEN 0 WHEN {} THEN 1 ELSE 2 END", all.join(" AND "), all.join(" OR "));
        SearchSql { filter, rank }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_markup_links_and_entities() {
        let html = r#"<p>Building the <b>100 Ornithopters</b> cube &amp; more.</p>
            <p>Our cubes: <a href="https://cubecobra.com/cube/overview/100-ornithopters">here</a>
            https://cubecobra.com/cube/list/vintage www.patreon.com/show cubecobra.com/cube/abc</p>
            <script>var x = "hidden";</script><style>.a{}</style>Caf&eacute; &#8212; &#x41;"#;
        assert_eq!(
            plain_text(html),
            "Building the 100 Ornithopters cube & more. Our cubes: here Caf&eacute; — A"
        );
    }

    #[test]
    fn keeps_text_that_only_looks_like_markup() {
        assert_eq!(plain_text("a < b and 3<4"), "a < b and 3<4");
        assert_eq!(plain_text("Q&A with R&D"), "Q&A with R&D");
        assert_eq!(plain_text("version 1.2 and node.js"), "version 1.2 and node.js");
    }

    #[test]
    fn parses_terms_and_phrases() {
        let t = SearchTerms::parse(r#"  Ornithopters  "100  cube" ornithopters "unclosed phrase"#);
        assert_eq!(t.terms(), ["ornithopters", "100 cube", "unclosed phrase"]);
        assert!(SearchTerms::parse("  \"\" ").is_empty());
        assert_eq!(SearchTerms::parse("a b c d e f g h i j").len(), MAX_TERMS);
    }

    #[test]
    fn escapes_like_wildcards() {
        assert_eq!(SearchTerms::parse("50% a_b !x").like_patterns(), ["%50!%%", "%a!_b%", "%!!x%"]);
    }

    #[test]
    fn ranks_title_hits() {
        let t = SearchTerms::parse("100 ornithopters");
        assert_eq!(t.title_rank("The 100 Ornithopters Cube"), 0);
        assert_eq!(t.title_rank("Ornithopters, again"), 1);
        assert_eq!(t.title_rank("Episode 12"), 2);
        assert_eq!(SearchTerms::default().title_rank("x"), 0);
    }

    #[test]
    fn builds_sql() {
        let t = SearchTerms::parse("a b");
        let s = SearchSql::new(&t, "ILIKE", "title", "text", |i| format!("${}", i + 3));
        assert_eq!(
            s.filter,
            " AND (title ILIKE $3 ESCAPE '!' OR text ILIKE $3 ESCAPE '!') AND (title ILIKE $4 ESCAPE '!' OR text ILIKE $4 ESCAPE '!')"
        );
        assert_eq!(
            s.rank,
            "CASE WHEN title ILIKE $3 ESCAPE '!' AND title ILIKE $4 ESCAPE '!' THEN 0 WHEN title ILIKE $3 ESCAPE '!' OR title ILIKE $4 ESCAPE '!' THEN 1 ELSE 2 END"
        );
    }
}
