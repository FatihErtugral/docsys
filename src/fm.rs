//! Frontmatter parser for the registered YAML subset (DECISIONS.md D-002).
//!
//! The subset, deliberately small so two implementations cannot disagree:
//!   - the block starts with a first line exactly `---` and ends at the next
//!     line exactly `---`
//!   - `key: value` — flat string scalar; surrounding single/double quotes on
//!     the value are stripped
//!   - `key: [a, b]` — inline list of scalars
//!   - `key:` followed by `  - item` lines — block list of scalars
//!   - `key:` followed by `  - k: v` items, each continued by `    k: v`
//!     lines — block list of flat maps (the §11 `verifies:` grammar); a list
//!     never mixes scalars and maps
//!   - keys are structural tokens: `[a-z][a-z0-9_]*`
//!   - anything else (nested maps, multi-line scalars, anchors, duplicate
//!     keys) is a parse finding, never a guess

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Str(String),
    List(Vec<String>),
    /// `key:` followed by `  - k: v` items with `    k: v` continuations —
    /// flat maps, one level, no deeper (§11 `verifies:`).
    Maps(Vec<BTreeMap<String, String>>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s.as_str()),
            Value::List(_) | Value::Maps(_) => None,
        }
    }
    pub fn as_list(&self) -> Option<&[String]> {
        match self {
            Value::List(v) => Some(v.as_slice()),
            Value::Str(_) | Value::Maps(_) => None,
        }
    }
    /// A block list of flat maps — the §11 `verifies:` grammar.
    pub fn as_maps(&self) -> Option<&[BTreeMap<String, String>]> {
        match self {
            Value::Maps(v) => Some(v.as_slice()),
            Value::Str(_) | Value::List(_) => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct Frontmatter {
    pub fields: BTreeMap<String, Value>,
    /// Parse problems, as prose; the caller turns them into findings.
    pub problems: Vec<String>,
    /// 0-based line index of the first body line (after the closing `---`).
    pub body_start: usize,
}

fn is_key(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some('a'..='z') => {}
        _ => return false,
    }
    chars.all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_'))
}

fn strip_quotes(v: &str) -> String {
    let v = v.trim();
    let stripped = v
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')));
    stripped.unwrap_or(v).to_string()
}

fn parse_inline_list(v: &str) -> Option<Vec<String>> {
    let inner = v.strip_prefix('[')?.strip_suffix(']')?;
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    // A formatter leaves a trailing comma when it reflows a list; the empty
    // slot after it is punctuation, not an item. Reading it as one turned a
    // path-prefix exclusion into "exclude everything" — a check that inspects
    // nothing (R-011 caught exactly this on a real tree).
    Some(
        inner
            .split(',')
            .map(strip_quotes)
            .filter(|s| !s.is_empty())
            .collect(),
    )
}

/// Parse the frontmatter block of `text`. Returns `None` when the file does
/// not open with `---` — absence is the caller's finding, not a parse error.
pub fn parse(text: &str) -> Option<Frontmatter> {
    let mut lines = text.lines().enumerate();
    match lines.next() {
        Some((_, "---")) => {}
        _ => return None,
    }

    let mut fm = Frontmatter::default();
    let mut pending_list_key: Option<String> = None;
    // A formatter (prettier and friends) reflows a long `key: [a, b, c]` across
    // several lines; the value is the same value, so the parser follows it to
    // the closing bracket instead of reading indented continuations as nesting.
    let mut open_list: Option<(String, String)> = None;

    for (idx, line) in lines {
        if line == "---" {
            fm.body_start = idx + 1;
            return Some(fm);
        }
        if line.trim().is_empty() {
            pending_list_key = None;
            continue;
        }
        // A whole-line comment (D-002): configuration people cannot annotate is
        // configuration people copy without understanding.
        if line.trim_start().starts_with('#') {
            continue;
        }
        if let Some((key, mut acc)) = open_list.take() {
            acc.push(' ');
            acc.push_str(line.trim());
            if acc.contains(']') {
                if let Some(items) = parse_inline_list(acc.trim()) {
                    fm.fields.insert(key, Value::List(items));
                } else {
                    fm.problems
                        .push(format!("line {}: unreadable inline list", idx + 1));
                }
            } else {
                open_list = Some((key, acc));
            }
            continue;
        }
        // Block-list item for the key on the previous line?
        if let Some(key) = pending_list_key.clone() {
            // A formatter may push the opening bracket onto its own line.
            if line.trim_start().starts_with('[') {
                pending_list_key = None;
                let acc = line.trim().to_string();
                if acc.contains(']') {
                    if let Some(items) = parse_inline_list(&acc) {
                        fm.fields.insert(key, Value::List(items));
                    }
                } else {
                    open_list = Some((key, acc));
                }
                continue;
            }
            if let Some(item) = line.strip_prefix("  - ") {
                // `- key: value` opens a map entry (the §11 `verifies:`
                // grammar); any other item is a scalar. A list never mixes
                // the two — that is a finding, not a guess.
                let map_field = item
                    .split_once(':')
                    .filter(|(k, v)| is_key(k.trim()) && (v.is_empty() || v.starts_with(' ')))
                    .map(|(k, v)| (k.trim().to_string(), strip_quotes(v.trim())));
                let entry = fm.fields.entry(key).or_insert_with(|| match map_field {
                    Some(_) => Value::Maps(Vec::new()),
                    None => Value::List(Vec::new()),
                });
                // `key:` with nothing after it was registered as an empty
                // list before its items were seen; the first item decides.
                if map_field.is_some() && matches!(entry, Value::List(items) if items.is_empty()) {
                    *entry = Value::Maps(Vec::new());
                }
                match (entry, map_field) {
                    (Value::Maps(maps), Some((k, v))) => {
                        let mut m = BTreeMap::new();
                        m.insert(k, v);
                        maps.push(m);
                    }
                    (Value::List(items), None) => items.push(strip_quotes(item)),
                    _ => fm
                        .problems
                        .push(format!("line {}: a list mixes scalars and maps", idx + 1)),
                }
                continue;
            }
            // `    key: value` continues the map entry opened just above.
            if line.starts_with("    ") {
                if let Some(Value::Maps(maps)) = fm.fields.get_mut(&key) {
                    let field = line
                        .trim()
                        .split_once(':')
                        .filter(|(k, _)| is_key(k.trim()));
                    if let (Some(last), Some((k, v))) = (maps.last_mut(), field) {
                        last.insert(k.trim().to_string(), strip_quotes(v.trim()));
                        continue;
                    }
                }
            }
            pending_list_key = None;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            fm.problems.push(format!(
                "line {}: nesting beyond the registered subset",
                idx + 1
            ));
            continue;
        }
        let Some((key, rest)) = line.split_once(':') else {
            fm.problems
                .push(format!("line {}: not a `key: value` line", idx + 1));
            continue;
        };
        let key = key.trim();
        if !is_key(key) {
            fm.problems.push(format!(
                "line {}: key `{key}` is not a structural token",
                idx + 1
            ));
            continue;
        }
        if fm.fields.contains_key(key) {
            fm.problems
                .push(format!("line {}: duplicate key `{key}`", idx + 1));
            continue;
        }
        let rest = rest.trim();
        if rest.is_empty() {
            // Either a block list follows, or the field is an empty scalar.
            pending_list_key = Some(key.to_string());
            fm.fields.insert(key.to_string(), Value::List(Vec::new()));
        } else if let Some(items) = parse_inline_list(rest) {
            fm.fields.insert(key.to_string(), Value::List(items));
        } else if rest.starts_with('[') && !rest.contains(']') {
            open_list = Some((key.to_string(), rest.to_string()));
        } else {
            // A comment after the value is not part of it.
            let val = match rest.split_once(" #") {
                Some((v, _)) => v,
                None => rest,
            };
            fm.fields
                .insert(key.to_string(), Value::Str(strip_quotes(val)));
        }
    }

    fm.problems
        .push("frontmatter never closed with `---`".to_string());
    Some(fm)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_block_list_of_flat_maps() {
        let fm = parse(
            "---\nid: x\nverifies:\n  - path: src/a.rs\n    symbol: f\n    hash: \"sha256:ab\"\n  - path: src/b.rs\n    hash: sha256:cd\ntags: [a]\n---\n",
        )
        .unwrap();
        let maps = fm.fields.get("verifies").unwrap().as_maps().unwrap();
        assert_eq!(maps.len(), 2);
        assert_eq!(maps[0].get("symbol").map(String::as_str), Some("f"));
        assert_eq!(maps[0].get("hash").map(String::as_str), Some("sha256:ab"));
        assert_eq!(maps[1].get("path").map(String::as_str), Some("src/b.rs"));
        assert_eq!(fm.fields.get("tags").unwrap().as_list().unwrap(), ["a"]);
        assert!(fm.problems.is_empty(), "{:?}", fm.problems);
        // a scalar list is untouched by the grammar; a URL item is a scalar
        let fm = parse("---\nsources:\n  - raw/x.md\n  - http://a/b\n---\n").unwrap();
        assert_eq!(
            fm.fields.get("sources").unwrap().as_list().unwrap().len(),
            2
        );
        // mixing is a finding
        let fm = parse("---\nk:\n  - a\n  - path: b\n---\n").unwrap();
        assert!(
            fm.problems.iter().any(|p| p.contains("mixes")),
            "{:?}",
            fm.problems
        );
    }

    #[test]
    fn parses_scalars_lists_and_comments() {
        let text = "---\nid: token-ttl\ntype: reference # trailing\ntags: [a, b]\nsources:\n  - raw/x.md\n---\nbody\n";
        let fm = parse(text).unwrap_or_default();
        assert!(fm.problems.is_empty(), "{:?}", fm.problems);
        assert_eq!(
            fm.fields.get("id").and_then(Value::as_str),
            Some("token-ttl")
        );
        assert_eq!(
            fm.fields.get("type").and_then(Value::as_str),
            Some("reference")
        );
        assert_eq!(
            fm.fields
                .get("tags")
                .and_then(Value::as_list)
                .map(<[String]>::len),
            Some(2)
        );
        assert_eq!(fm.body_start, 7);
    }

    #[test]
    fn absent_frontmatter_is_none() {
        assert!(parse("# just a page\n").is_none());
    }

    #[test]
    fn nesting_is_a_problem_not_a_guess() {
        let text = "---\nverifies:\n  path: src/x.rs\n---\n";
        let fm = parse(text).unwrap_or_default();
        assert!(!fm.problems.is_empty());
    }
}
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests_more {
    use super::*;

    fn fm(text: &str) -> Frontmatter {
        parse(text).expect("opens with ---")
    }
    fn s(f: &Frontmatter, k: &str) -> String {
        f.fields[k].as_str().unwrap().to_string()
    }
    fn l(f: &Frontmatter, k: &str) -> Vec<String> {
        f.fields[k].as_list().unwrap().to_vec()
    }

    #[test]
    fn quotes_are_stripped_from_scalars_and_list_items() {
        let f = fm("---\na: \"x y\"\nb: 'z'\nc: [\"p\", 'q', r]\nd: \"unbalanced'\n---\n");
        assert_eq!(s(&f, "a"), "x y");
        assert_eq!(s(&f, "b"), "z");
        assert_eq!(l(&f, "c"), vec!["p", "q", "r"]);
        assert_eq!(s(&f, "d"), "\"unbalanced'");
        assert!(f.problems.is_empty());
    }

    #[test]
    fn a_value_may_contain_a_colon_and_a_trailing_comment_is_dropped() {
        let f = fm("---\ntitle: a: b # note\nurl: http://x/y\n---\n");
        assert_eq!(s(&f, "title"), "a: b");
        assert_eq!(s(&f, "url"), "http://x/y");
    }

    #[test]
    fn lists_inline_block_reflowed_and_empty() {
        let f = fm("---\ni: [a, b,]\nb:\n  - one\n  - \"two\"\nr:\n  [\n    x,\n    y,\n  ]\ne: []\nn:\n---\n");
        assert_eq!(l(&f, "i"), vec!["a", "b"]);
        assert_eq!(l(&f, "b"), vec!["one", "two"]);
        assert_eq!(l(&f, "r"), vec!["x", "y"]);
        assert!(l(&f, "e").is_empty());
        assert!(l(&f, "n").is_empty(), "a bare `key:` is an empty list");
        assert!(f.problems.is_empty(), "{:?}", f.problems);
    }

    #[test]
    fn a_reflowed_inline_list_may_open_on_the_key_line() {
        let f = fm("---\nk: [a,\n  b, c]\n---\n");
        assert_eq!(l(&f, "k"), vec!["a", "b", "c"]);
    }

    #[test]
    fn whole_line_comments_and_blank_lines_are_skipped() {
        let f = fm("---\n# leading comment\na: 1\n\n  # indented comment\nb: 2\n---\nbody");
        assert_eq!(s(&f, "a"), "1");
        assert_eq!(s(&f, "b"), "2");
        assert_eq!(f.body_start, 7);
        assert!(f.problems.is_empty());
    }

    #[test]
    fn problems_are_named_with_their_line_numbers() {
        let f = fm("---\na: 1\na: 2\nBad-Key: x\nno colon here\nnested:\n  child: 1\n---\n");
        let p = f.problems.join("\n");
        assert!(p.contains("line 3: duplicate key `a`"), "{p}");
        assert!(
            p.contains("line 4: key `Bad-Key` is not a structural token"),
            "{p}"
        );
        assert!(p.contains("line 5: not a `key: value` line"), "{p}");
        assert!(
            p.contains("line 7: nesting beyond the registered subset"),
            "{p}"
        );
        assert_eq!(
            s(&f, "a"),
            "1",
            "the first value wins, the duplicate is a finding"
        );
    }

    #[test]
    fn an_unclosed_block_is_a_problem_not_a_guess() {
        let f = fm("---\na: 1\n");
        assert_eq!(f.problems, vec!["frontmatter never closed with `---`"]);
        assert_eq!(s(&f, "a"), "1");
    }

    #[test]
    fn only_an_exact_opening_fence_counts() {
        assert!(parse("--- \na: 1\n---\n").is_none());
        assert!(parse("\n---\na: 1\n---\n").is_none());
        assert!(parse("----\n").is_none());
        assert!(parse("---\n---\n").is_some());
    }

    #[test]
    fn keys_are_structural_tokens() {
        assert!(is_key("a"));
        assert!(is_key("a_b9"));
        assert!(!is_key("A"));
        assert!(!is_key("9a"));
        assert!(!is_key("a-b"));
        assert!(!is_key(""));
        assert!(!is_key("ä"));
    }
}
