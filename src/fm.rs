//! Frontmatter parser for the registered YAML subset (DECISIONS.md D-002).
//!
//! The subset, deliberately small so two implementations cannot disagree:
//!   - the block starts with a first line exactly `---` and ends at the next
//!     line exactly `---`
//!   - `key: value` — flat string scalar; surrounding single/double quotes on
//!     the value are stripped
//!   - `key: [a, b]` — inline list of scalars
//!   - `key:` followed by `  - item` lines — block list of scalars
//!   - keys are structural tokens: `[a-z][a-z0-9_]*`
//!   - anything else (nested maps, multi-line scalars, anchors, duplicate
//!     keys) is a parse finding, never a guess

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Str(String),
    List(Vec<String>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s.as_str()),
            Value::List(_) => None,
        }
    }
    pub fn as_list(&self) -> Option<&[String]> {
        match self {
            Value::Str(_) => None,
            Value::List(v) => Some(v.as_slice()),
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
                let entry = fm
                    .fields
                    .entry(key)
                    .or_insert_with(|| Value::List(Vec::new()));
                if let Value::List(items) = entry {
                    items.push(strip_quotes(item));
                }
                continue;
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
mod tests {
    use super::*;

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
