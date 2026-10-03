//! Pin acknowledgements (R-110, R-113, D-119) — a docsys/0.5 tree keeps a
//! pin's evidence beside the page, never in it.
//!
//! Field report: two real repositories kept every pin's hash in the page's
//! frontmatter. Each refresh rewrote those lines and `updated:`, so pull
//! requests that refreshed one page on different days conflicted on
//! `updated:`, and the ones that moved the same region conflicted on its
//! hash; replayed over ten merged pull requests, 14 and 19 of 45 pairs shared
//! a page both had to refresh. And the hash was the region's canonical text:
//! two pull requests that only inserted `doc:` citations staled 188 pins.
//!
//! So a re-read is recorded as a file, `<root>/.verifies/<page-id>/<hash>`,
//! holding the one line `<page-id> <hash>`, where the hash is the region's
//! token form — what can change the code's meaning, never its layout or its
//! comments. The content is unique to its page and its region version on
//! purpose: git pairs a deleted file with an added one of identical content as
//! a rename, and an empty file made "remove the old acknowledgement, add the
//! new" a rename two branches then fought over.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::checks::Report;
use crate::model::{Finding, RuleId};
use crate::tree::DocTree;

/// The reserved directory under the documentation root (R-044).
pub const DIR: &str = ".verifies";

const R113: RuleId = RuleId("R-113");

// ---------------------------------------------------------------- token form

/// Which comment syntax a file kind has (D-119's table): programming-language
/// data, not natural language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// `//` and `/* */`; quotes are strings.
    Brace,
    /// as Brace, and `'` opens a char literal only when one closes it.
    Rust,
    /// as Brace, plus `<!-- -->`: a component file's script and template.
    ScriptMarkup,
    /// `#` to the end of the line; quotes are strings.
    Hash,
    /// as Hash, and `"""` / `'''` open a string.
    Python,
    /// `<!-- -->` only, and no string literals: prose has apostrophes.
    Markup,
    /// nothing is a comment; whitespace is still layout.
    Plain,
}

fn kind_of(path: &str) -> Kind {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" => Kind::Rust,
        "go" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" | "java" | "kt"
        | "c" | "h" | "cc" | "cpp" | "hpp" | "cs" | "swift" | "scala" | "dart" | "php" | "css"
        | "scss" | "less" => Kind::Brace,
        "vue" | "svelte" => Kind::ScriptMarkup,
        "py" | "pyi" => Kind::Python,
        "sh" | "bash" | "yml" | "yaml" | "toml" | "rb" | "pl" | "r" => Kind::Hash,
        "md" | "html" | "htm" | "xml" => Kind::Markup,
        _ => Kind::Plain,
    }
}

fn starts(chars: &[char], i: usize, pat: &str) -> bool {
    pat.chars()
        .enumerate()
        .all(|(k, c)| chars.get(i + k) == Some(&c))
}

/// Index just past the next `pat` at or after `from`; the end when absent.
fn past(chars: &[char], from: usize, pat: &str) -> usize {
    let len = pat.chars().count();
    (from..chars.len())
        .find(|&j| starts(chars, j, pat))
        .map_or(chars.len(), |j| j + len)
}

fn line_end(chars: &[char], from: usize) -> usize {
    (from..chars.len())
        .find(|&j| chars.get(j) == Some(&'\n'))
        .unwrap_or(chars.len())
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// A string literal from the quote at `i`: its body as written (escapes
/// kept), and the index past its closing delimiter.
fn string_at(chars: &[char], i: usize, close: &str) -> (String, usize) {
    let open = close.chars().count();
    let mut body = String::new();
    let mut j = i + open;
    while j < chars.len() {
        if starts(chars, j, close) {
            return (body, j + open);
        }
        match chars.get(j) {
            Some('\\') => {
                body.push('\\');
                if let Some(next) = chars.get(j + 1) {
                    body.push(*next);
                }
                j += 2;
            }
            Some(c) => {
                body.push(*c);
                j += 1;
            }
            None => break,
        }
    }
    (body, chars.len())
}

/// The token form of a pin's region (R-113, D-119): comments removed, the
/// whitespace between tokens removed, every string literal written with one
/// quote character, and a comma directly before a closing bracket dropped —
/// tokens joined by one space. Layout, comments and `doc:` citations do not
/// change it; what can change the code's meaning does.
pub fn token_form(text: &str, path: &str) -> String {
    let kind = kind_of(path);
    let chars: Vec<char> = text.replace("\r\n", "\n").chars().collect();
    let markup_comments = matches!(kind, Kind::ScriptMarkup | Kind::Markup);
    let brace_comments = matches!(kind, Kind::Brace | Kind::Rust | Kind::ScriptMarkup);
    let hash_comments = matches!(kind, Kind::Hash | Kind::Python);
    let strings = !matches!(kind, Kind::Markup | Kind::Plain);
    let mut toks: Vec<String> = Vec::new();
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        if markup_comments && starts(&chars, i, "<!--") {
            i = past(&chars, i + 4, "-->");
            continue;
        }
        if brace_comments && starts(&chars, i, "/*") {
            i = past(&chars, i + 2, "*/");
            continue;
        }
        // `https://` is not a comment: a `//` right after `:` is a URL's
        let after_colon = i > 0 && chars.get(i - 1) == Some(&':');
        if brace_comments && starts(&chars, i, "//") && !after_colon {
            i = line_end(&chars, i);
            continue;
        }
        // in shell and its kin a `#` starts a comment only where a word starts:
        // `$#`, `${#a[@]}` and `v1#x` are code, and a change to them is one
        let word_start = i == 0
            || chars
                .get(i - 1)
                .is_some_and(|p| p.is_whitespace() || matches!(p, ';' | '|' | '&' | '(' | ')'));
        if hash_comments && c == '#' && (kind == Kind::Python || word_start) {
            i = line_end(&chars, i);
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if strings {
            if kind == Kind::Python && (starts(&chars, i, "\"\"\"") || starts(&chars, i, "'''")) {
                let delim = if c == '"' { "\"\"\"" } else { "'''" };
                let (body, next) = string_at(&chars, i, delim);
                toks.push(format!("\"{body}\""));
                i = next;
                continue;
            }
            if kind == Kind::Rust && c == '\'' {
                // a char literal closes within one character or one escape;
                // anything else is a lifetime or a label
                let closes = match chars.get(i + 1) {
                    Some('\\') => true,
                    Some(_) => chars.get(i + 2) == Some(&'\''),
                    None => false,
                };
                if !closes {
                    toks.push("'".to_string());
                    i += 1;
                    continue;
                }
            }
            if matches!(c, '"' | '\'' | '`') {
                let close = c.to_string();
                let (body, next) = string_at(&chars, i, &close);
                toks.push(if c == '`' {
                    format!("`{body}`")
                } else {
                    format!("\"{body}\"")
                });
                i = next;
                continue;
            }
        }
        if is_word(c) {
            let start = i;
            while chars.get(i).is_some_and(|&w| is_word(w)) {
                i += 1;
            }
            toks.push(chars.get(start..i).unwrap_or(&[]).iter().collect());
            continue;
        }
        toks.push(c.to_string());
        i += 1;
    }
    let mut out: Vec<&str> = Vec::with_capacity(toks.len());
    for (k, t) in toks.iter().enumerate() {
        let next = toks.get(k + 1).map(String::as_str);
        if t == "," && matches!(next, Some(")" | "]" | "}")) {
            continue;
        }
        out.push(t);
    }
    out.join(" ")
}

/// A pin's region hash: 64 lowercase hex over the token form (R-113).
pub fn region_hash(region: &str, path: &str) -> String {
    crate::fresh::sha256_hex(token_form(region, path).as_bytes())
}

// ---------------------------------------------------------------- storage

fn page_dir(root: &Path, page_id: &str) -> PathBuf {
    root.join(DIR).join(page_id)
}

/// The one line an acknowledgement holds.
pub fn content(page_id: &str, hash: &str) -> String {
    format!("{page_id} {hash}\n")
}

/// Is the region `hash` of page `page_id` acknowledged? The name is what is
/// matched; a malformed content is reported separately and never makes a pin
/// fresh by itself.
pub fn holds(root: &Path, page_id: &str, hash: &str) -> bool {
    page_dir(root, page_id).join(hash).is_file()
}

/// Write the acknowledgement; `true` when it was not there before.
pub fn write(root: &Path, page_id: &str, hash: &str) -> std::io::Result<bool> {
    let dir = page_dir(root, page_id);
    let file = dir.join(hash);
    if file.is_file() {
        return Ok(false);
    }
    fs::create_dir_all(&dir)?;
    fs::write(&file, content(page_id, hash))?;
    Ok(true)
}

/// The acknowledgement names one page holds, sorted.
pub fn names(root: &Path, page_id: &str) -> Vec<String> {
    let Ok(entries) = fs::read_dir(page_dir(root, page_id)) else {
        return Vec::new();
    };
    let mut out: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    out.sort();
    out
}

/// The page ids that hold an acknowledgement directory, sorted.
pub fn page_ids(root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root.join(DIR)) else {
        return Vec::new();
    };
    let mut out: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    out.sort();
    out
}

/// Remove every acknowledgement of `page_id` not in `keep`, and the page's
/// directory when nothing is left; the removed names.
pub fn remove_except(
    root: &Path,
    page_id: &str,
    keep: &BTreeSet<String>,
) -> std::io::Result<Vec<String>> {
    let mut removed = Vec::new();
    for name in names(root, page_id) {
        if !keep.contains(&name) {
            fs::remove_file(page_dir(root, page_id).join(&name))?;
            removed.push(name);
        }
    }
    remove_if_empty(root, page_id)?;
    Ok(removed)
}

/// Remove a page's whole acknowledgement directory; the names it held.
pub fn remove_page(root: &Path, page_id: &str) -> std::io::Result<Vec<String>> {
    let gone = names(root, page_id);
    let dir = page_dir(root, page_id);
    if dir.is_dir() {
        fs::remove_dir_all(&dir)?;
    }
    if root.join(DIR).is_dir() && page_ids(root).is_empty() {
        let _ = fs::remove_dir(root.join(DIR));
    }
    Ok(gone)
}

fn remove_if_empty(root: &Path, page_id: &str) -> std::io::Result<()> {
    let dir = page_dir(root, page_id);
    if dir.is_dir() && fs::read_dir(&dir)?.next().is_none() {
        fs::remove_dir(&dir)?;
    }
    if root.join(DIR).is_dir() && page_ids(root).is_empty() {
        let _ = fs::remove_dir(root.join(DIR));
    }
    Ok(())
}

fn is_hash(name: &str) -> bool {
    name.len() == 64
        && name
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// R-113 for acknowledgements: each file is named by a region hash and holds
/// exactly `<page-id> <its own name>`. A hand-made or corrupted file is
/// reported — it is visible, and it can never make a pin fresh, because the
/// name is what lint matches.
pub fn check(tree: &DocTree, r: &mut Report) {
    let mut inspected = 0usize;
    for id in page_ids(&tree.root) {
        for name in names(&tree.root, &id) {
            inspected += 1;
            let file = format!("{DIR}/{id}/{name}");
            let text = fs::read_to_string(tree.root.join(&file)).unwrap_or_default();
            let fine = is_hash(&name) && text == content(&id, &name);
            if !fine {
                r.findings.push(Finding::warn(
                    R113,
                    &file,
                    "acknowledgement",
                    format!(
                        "an acknowledgement is named by a region hash (64 lowercase hex) and holds \
                         the one line `{id} <that hash>` — `docsys pin --refresh <page>` writes it \
                         after a re-read; this file is neither, so it acknowledges nothing"
                    ),
                ));
            }
        }
    }
    r.inspected.insert("pin-acknowledgements", inspected);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn the_token_form_drops_layout_and_comments_per_kind() {
        let cases: [(&str, &str, &str); 12] = [
            ("a.rs", "fn f() {\n    x // note\n}\n", "fn f ( ) { x }"),
            (
                "a.rs",
                "fn f<'a>(s: &'a str) -> char { 'x' }",
                "fn f < ' a > ( s : & ' a str ) - > char { \"x\" }",
            ),
            (
                "a.ts",
                "const u = 'https://x.io/y'; // why\n",
                "const u = \"https://x.io/y\" ;",
            ),
            ("a.ts", "f(a,\n  b,\n)", "f ( a , b )"),
            ("a.ts", "/* one\n two */ g(\"x\")", "g ( \"x\" )"),
            ("a.ts", "const t = `a ${b}`", "const t = `a ${b}`"),
            (
                "a.py",
                "def f():\n    return '# not a comment'  # a comment\n",
                "def f ( ) : return \"# not a comment\"",
            ),
            (
                "a.py",
                "def f():\n    \"\"\"Doc. It's fine.\"\"\"\n",
                "def f ( ) : \"Doc. It's fine.\"",
            ),
            ("a.yml", "key: value # note\n", "key : value"),
            (
                "a.md",
                "Don't <!-- hidden -->stop.\n\n  Next line\n",
                "Don ' t stop . Next line",
            ),
            (
                "a.vue",
                "<!-- c --><p>{{ x }}</p>\n<script>\n// c\nconst y = 1\n</script>",
                "< p > { { x } } < / p > < script > const y = 1 < / script >",
            ),
            ("a.txt", "a   b\n\tc", "a b c"),
        ];
        for (path, text, want) in cases {
            assert_eq!(token_form(text, path), want, "{path}: {text:?}");
        }
    }

    #[test]
    fn reformatting_and_citations_keep_the_hash_and_meaning_moves_it() {
        let a = "export function g(x: number) {\n  return x + 1;\n}\n";
        let reformatted = "// doc: some-page\nexport function g(\n    x: number,\n) {\n\n    return x + 1; // why\n}";
        let changed = "export function g(x: number) {\n  return x + 2;\n}\n";
        assert_eq!(region_hash(a, "a.ts"), region_hash(reformatted, "a.ts"));
        assert_ne!(region_hash(a, "a.ts"), region_hash(changed, "a.ts"));
        assert_eq!(region_hash(a, "a.ts").len(), 64);
    }

    #[test]
    fn writing_is_idempotent_and_removal_keeps_what_is_asked() {
        let root = std::env::temp_dir().join(format!("docsys-ack-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let h1 = "a".repeat(64);
        let h2 = "b".repeat(64);
        assert!(write(&root, "p", &h1).unwrap());
        assert!(!write(&root, "p", &h1).unwrap());
        assert!(write(&root, "p", &h2).unwrap());
        assert_eq!(
            fs::read_to_string(root.join(".verifies/p").join(&h1)).unwrap(),
            format!("p {h1}\n")
        );
        let keep: BTreeSet<String> = [h2.clone()].into_iter().collect();
        assert_eq!(remove_except(&root, "p", &keep).unwrap(), vec![h1.clone()]);
        assert!(holds(&root, "p", &h2) && !holds(&root, "p", &h1));
        assert_eq!(
            remove_except(&root, "p", &BTreeSet::new()).unwrap(),
            vec![h2]
        );
        assert!(!root.join(DIR).exists(), "an empty .verifies/ is removed");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_hash_starts_a_shell_comment_only_where_a_word_starts() {
        let form = |t: &str| token_form(t, "run.sh");
        assert_ne!(
            form("[ $# -eq 2 ] || exit 1\n"),
            form("[ $# -eq 3 ] || exit 0\n")
        );
        assert_ne!(
            form("n=${#arr[@]}; echo 1\n"),
            form("n=${#arr[@]}; echo 2\n")
        );
        assert_ne!(form("v=v1#abc; echo a\n"), form("v=v1#abc; echo b\n"));
        assert_eq!(form("echo x # one note\n"), form("echo x # another\n"));
        assert_eq!(form("# a comment line\necho x\n"), form("echo x\n"));
    }
}
