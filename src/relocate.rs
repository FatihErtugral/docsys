//! `docsys raw move` — a record leaves the inbox for its domain and every
//! page that rests on it keeps its trail (R-027, D-085).
//!
//! Ingest's sixth step used to be a `git mv` followed by a hand edit of
//! `sources:` in every citing page, and the hand edit is where evidence
//! trails were severed — R-059 caught the break afterwards, R-151 forbids
//! the silence in between. The relocation is one command: the bytes move
//! through git when the record is tracked, the `sources:` entries are
//! rewritten inside the frontmatter only — the body is untouched, so a
//! verification (D-077) survives the move — and `updated:` is bumped as on
//! every tool write (R-052; `pin --refresh` is the precedent), which keeps
//! R-106 quiet after the commit. The domain must be declared (R-026) and an
//! existing destination is refused: a record is never overwritten (R-023).

use std::fs;
use std::path::Path;

use crate::fm::Value;
use crate::tree::DocTree;

#[derive(Debug, Default)]
pub struct Moved {
    pub from: String,
    pub to: String,
    /// every page whose `sources:` named the old path, with the number of
    /// entries rewritten
    pub rewritten: Vec<(String, usize)>,
}

/// `docsys raw move <record> <domain> [--root .]`.
pub fn raw_move(root: &Path, record: &str, domain: &str) -> Result<Moved, String> {
    if !crate::hook::is_knowledge_base(root) {
        return Err(format!(
            "`{}` is not a knowledge base (profile: knowledge-base) — records live under a \
             base's raw/, and `raw move` relocates records",
            root.display()
        ));
    }
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    let from = record.trim().trim_start_matches("./").replace('\\', "/");
    if !from.starts_with("raw/") || !root.join(&from).is_file() {
        return Err(format!(
            "no record at `{from}` — a record is a file under raw/ (raw/inbox/<note>.md)"
        ));
    }
    let domain = domain.trim();
    if domain.is_empty() || domain.contains('/') || domain.starts_with('_') {
        return Err(format!(
            "`{domain}` is not a domain name — one declared word, as in .docmeta.yml `domains:`"
        ));
    }
    if !tree.docmeta_list("domains").iter().any(|d| d == domain) {
        return Err(format!(
            "domain `{domain}` is not declared in .docmeta.yml `domains:` (R-026) — declare it \
             first, or file the note under one that is"
        ));
    }
    let name = from.rsplit('/').next().unwrap_or(from.as_str());
    let to = format!("raw/{domain}/{name}");
    if to == from {
        return Err(format!("`{from}` is already under raw/{domain}/"));
    }
    if root.join(&to).exists() {
        return Err(format!(
            "`{to}` already exists — a record is never overwritten (R-023); the note keeps its \
             place until it has another name"
        ));
    }
    let repo = crate::repo_of(root).unwrap_or_else(|| root.to_path_buf());
    crate::forget::relocate(&repo, root, &from, &to)?;

    let today = crate::migrate::today();
    let mut out = Moved {
        from: from.clone(),
        to: to.clone(),
        rewritten: Vec::new(),
    };
    for page in &tree.pages {
        let cites = page
            .fm
            .as_ref()
            .and_then(|f| f.fields.get("sources"))
            .and_then(Value::as_list)
            .is_some_and(|l| l.iter().any(|s| s.trim() == from));
        if !cites {
            continue;
        }
        let (text, n) = rewrite_sources(&page.text, &from, &to);
        if n == 0 {
            continue;
        }
        let text = crate::hook::bump_updated(&text, &today).unwrap_or(text);
        fs::write(root.join(&page.rel), text).map_err(|e| e.to_string())?;
        out.rewritten.push((page.rel.clone(), n));
    }
    Ok(out)
}

/// Rewrite `from` to `to` in the `sources:` entries of a page's frontmatter
/// — inline list or block list — and nowhere else. The body is the page's
/// content (§2.4) and is not read, let alone written.
fn rewrite_sources(text: &str, from: &str, to: &str) -> (String, usize) {
    let Some(rest) = text.strip_prefix("---\n") else {
        return (text.to_string(), 0);
    };
    let Some(end) = rest.find("\n---\n") else {
        return (text.to_string(), 0);
    };
    let fm = &rest[..end];
    let tail = &rest[end..];
    let mut count = 0;
    let mut in_sources = false;
    let mut lines = Vec::new();
    for line in fm.split('\n') {
        let candidate = if line.starts_with("sources:") {
            in_sources = true;
            true
        } else if line.starts_with(' ') || line.starts_with("- ") {
            in_sources && line.trim_start().starts_with("- ")
        } else {
            in_sources = false;
            false
        };
        if candidate {
            let (l, n) = replace_token(line, from, to);
            count += n;
            lines.push(l);
        } else {
            lines.push(line.to_string());
        }
    }
    (format!("---\n{}{tail}", lines.join("\n")), count)
}

/// Replace `from` where it stands as a whole entry: bounded by the list
/// punctuation YAML allows around a path, never inside a longer path.
fn replace_token(line: &str, from: &str, to: &str) -> (String, usize) {
    const EDGE: &[char] = &['[', ']', ',', ' ', '"', '\''];
    let mut out = String::with_capacity(line.len());
    let mut n = 0;
    let mut rest = line;
    while let Some(pos) = rest.find(from) {
        let before = rest[..pos].chars().next_back();
        let after = rest[pos + from.len()..].chars().next();
        let whole =
            before.is_none_or(|c| EDGE.contains(&c)) && after.is_none_or(|c| EDGE.contains(&c));
        out.push_str(&rest[..pos]);
        if whole {
            out.push_str(to);
            n += 1;
        } else {
            out.push_str(from);
        }
        rest = &rest[pos + from.len()..];
    }
    out.push_str(rest);
    (out, n)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn an_inline_and_a_block_list_are_both_rewritten_and_the_body_is_not() {
        let text =
            "---\nid: x\nsources: [raw/inbox/a.md, raw/coding/b.md]\n---\n\nSee raw/inbox/a.md.\n";
        let (out, n) = rewrite_sources(text, "raw/inbox/a.md", "raw/ops/a.md");
        assert_eq!(n, 1);
        assert_eq!(
            out,
            "---\nid: x\nsources: [raw/ops/a.md, raw/coding/b.md]\n---\n\nSee raw/inbox/a.md.\n"
        );
        let text = "---\nid: x\nsources:\n  - raw/inbox/a.md\n  - \"raw/inbox/a.md.bak\"\ntags: [raw/inbox/a.md]\n---\nBody.\n";
        let (out, n) = rewrite_sources(text, "raw/inbox/a.md", "raw/ops/a.md");
        assert_eq!(n, 1, "{out}");
        assert!(out.contains("  - raw/ops/a.md\n"), "{out}");
        assert!(
            out.contains("raw/inbox/a.md.bak"),
            "a longer path is not the entry"
        );
        assert!(
            out.contains("tags: [raw/inbox/a.md]"),
            "another key is not sources:"
        );
    }

    #[test]
    fn a_page_without_frontmatter_is_left_alone() {
        let (out, n) = rewrite_sources("# no fm\nraw/inbox/a.md\n", "raw/inbox/a.md", "raw/x/a.md");
        assert_eq!(n, 0);
        assert_eq!(out, "# no fm\nraw/inbox/a.md\n");
    }
}
