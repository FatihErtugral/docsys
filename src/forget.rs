//! `docsys forget` — "take this out of your memory", honestly (D-084).
//!
//! Forgetting is not deleting: the base's record layer is content-immutable
//! (R-023) and its history is git. What the command does is make a topic
//! unknown to every organ and every check while leaving the bytes where an
//! audit can still find them:
//!
//! - a wiki page moves to `_archive/` (R-045) with a tombstone for its
//!   identifier (R-066, R-047), its router line goes, and a skill compiled
//!   from it goes with it — the tree walk never enters `_archive/`, so
//!   `lookup`, the agent layer and lint stop seeing it;
//! - a record moves to `raw/_forgotten/` — still under `raw/`, so it stays a
//!   record (immutable, never edited) but no organ reads it; and because the
//!   connector's deduplication reads all of `raw/`, the same item never
//!   lands again;
//! - every forgetting is one line in `.forgotten.yml`: date, path, reason.
//!
//! A record a page still cites is refused: forget the page first.
//! Erasing from history is a person's `git filter-repo`, never this tool.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::fm::Value;
use crate::tree::{DocTree, Kind};

#[derive(Debug, Default)]
pub struct Forgotten {
    pub steps: Vec<String>,
}

/// Move a path inside the repository, through git when it is tracked.
fn relocate(repo: &Path, root: &Path, from: &str, to: &str) -> Result<(), String> {
    let src = root.join(from);
    let dst = root.join(to);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tracked = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["ls-files", "--error-unmatch", "--"])
        .arg(&src)
        .output()
        .is_ok_and(|o| o.status.success());
    if tracked {
        let ok = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["mv", "-k", "--"])
            .arg(&src)
            .arg(&dst)
            .status()
            .is_ok_and(|s| s.success());
        if ok && dst.exists() {
            return Ok(());
        }
    }
    fs::rename(&src, &dst).map_err(|e| format!("cannot move `{from}` to `{to}`: {e}"))
}

fn append(root: &Path, file: &str, text: &str) -> Result<(), String> {
    let path = root.join(file);
    let mut current = fs::read_to_string(&path).unwrap_or_default();
    if !current.is_empty() && !current.ends_with('\n') {
        current.push('\n');
    }
    current.push_str(text);
    fs::write(&path, current).map_err(|e| e.to_string())
}

fn yaml_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The pages whose `sources:` name this record.
fn citing_pages(tree: &DocTree, rel: &str) -> Vec<String> {
    tree.pages
        .iter()
        .filter(|p| {
            p.fm.as_ref()
                .and_then(|f| f.fields.get("sources"))
                .and_then(Value::as_list)
                .is_some_and(|l| l.iter().any(|s| s.trim() == rel))
        })
        .map(|p| p.rel.clone())
        .collect()
}

/// Drop the router lines that link to `rel` (without `.md`) in the tree's
/// index pages; a forgotten page is not routed anywhere.
fn unroute(
    tree: &DocTree,
    root: &Path,
    rel_no_md: &str,
    out: &mut Forgotten,
) -> Result<(), String> {
    // a knowledge base links from the wiki root (`ops/howto/x`), a project
    // from the docs root (`howto/x`): both spellings are looked for
    let mut forms = vec![rel_no_md.to_string()];
    if let Some(short) = rel_no_md.strip_prefix("wiki/") {
        forms.push(short.to_string());
    }
    let needles: Vec<String> = forms
        .iter()
        .flat_map(|f| [format!("[[{f}|"), format!("[[{f}]]")])
        .collect();
    for page in tree.pages.iter().filter(|p| p.kind == Kind::Router) {
        let kept: Vec<&str> = page
            .text
            .lines()
            .filter(|l| !needles.iter().any(|n| l.contains(n.as_str())))
            .collect();
        if kept.len() != page.text.lines().count() {
            let mut text = kept.join("\n");
            text.push('\n');
            fs::write(root.join(&page.rel), text).map_err(|e| e.to_string())?;
            out.steps
                .push(format!("router: {} no longer lists it", page.rel));
        }
    }
    Ok(())
}

/// `docsys forget <page-id|page-path|record-path> --reason <text>`.
pub fn forget(root: &Path, target: &str, reason: &str) -> Result<Forgotten, String> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(
            "forgetting needs `--reason <text>` — the ledger records why, in the person's words"
                .into(),
        );
    }
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    let repo = crate::repo_of(root).unwrap_or_else(|| root.to_path_buf());
    let today = crate::migrate::today();
    let mut out = Forgotten::default();
    let wanted = target.trim_start_matches("./").trim_end_matches(".md");

    // a record?
    let record_rel = format!("{wanted}.md");
    if record_rel.starts_with("raw/") && root.join(&record_rel).is_file() {
        if record_rel.starts_with("raw/_forgotten/") {
            return Err(format!("`{record_rel}` is already forgotten"));
        }
        let citing = citing_pages(&tree, &record_rel);
        if !citing.is_empty() {
            return Err(format!(
                "`{record_rel}` is cited by {} — forget the page first, or it keeps resting on a \
                 record you asked to forget",
                citing.join(", ")
            ));
        }
        let to = format!("raw/_forgotten/{}", record_rel.trim_start_matches("raw/"));
        relocate(&repo, root, &record_rel, &to)?;
        out.steps.push(format!(
            "record: {record_rel} → {to} (still a record, never read)"
        ));
        append(
            root,
            ".forgotten.yml",
            &format!(
                "- date: {today}\n  kind: record\n  path: {record_rel}\n  reason: {}\n",
                yaml_quote(reason)
            ),
        )?;
        out.steps
            .push(".forgotten.yml: one line, date and reason".into());
        return Ok(out);
    }

    // a page, by path or by id
    let page = tree
        .pages
        .iter()
        .filter(|p| p.kind == Kind::Permanent)
        .find(|p| {
            p.rel.trim_end_matches(".md") == wanted
                || p.fm
                    .as_ref()
                    .and_then(|f| f.fields.get("id"))
                    .and_then(Value::as_str)
                    == Some(target.trim())
        })
        .ok_or_else(|| {
            format!("no page at `{target}`, no page with that id, and no record at that path")
        })?;
    let rel = page.rel.clone();
    let id = page
        .fm
        .as_ref()
        .and_then(|f| f.fields.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let to = format!("_archive/{rel}");
    relocate(&repo, root, &rel, &to)?;
    out.steps
        .push(format!("page: {rel} → {to} (outside every walk)"));
    if let Some(id) = &id {
        append(
            root,
            ".tombstones.yml",
            &format!(
                "- id: {id}\n  date: {today}\n  forgotten: {}\n",
                yaml_quote(reason)
            ),
        )?;
        out.steps.push(format!(
            ".tombstones.yml: `{id}` reserved, never reused (R-066)"
        ));
        // a skill compiled from it is derived; it goes with the page
        let skill = repo.join(".claude").join("skills").join(id);
        if fs::read_to_string(skill.join("SKILL.md"))
            .is_ok_and(|t| t.contains(crate::compile::SOURCE_KEY))
        {
            let _ = Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(["rm", "-rq", "--"])
                .arg(&skill)
                .status();
            let _ = fs::remove_dir_all(&skill);
            out.steps.push(format!(
                ".claude/skills/{id}: compiled from the page, removed with it"
            ));
        }
    }
    unroute(&tree, root, rel.trim_end_matches(".md"), &mut out)?;
    append(
        root,
        ".forgotten.yml",
        &format!(
            "- date: {today}\n  kind: page\n  path: {rel}\n  reason: {}\n",
            yaml_quote(reason)
        ),
    )?;
    out.steps
        .push(".forgotten.yml: one line, date and reason".into());
    Ok(out)
}

/// How many forgettings the ledger holds.
pub fn count(root: &Path) -> usize {
    fs::read_to_string(root.join(".forgotten.yml"))
        .map(|t| t.lines().filter(|l| l.starts_with("- date:")).count())
        .unwrap_or(0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_reason_is_quoted_for_the_ledger() {
        assert_eq!(yaml_quote(r#"he said "no""#), r#""he said \"no\"""#);
    }

    #[test]
    fn a_ledger_counts_its_entries() {
        let dir = std::env::temp_dir().join(format!("docsys-forget-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(count(&dir), 0);
        fs::write(
            dir.join(".forgotten.yml"),
            "- date: 2026-09-02\n  kind: page\n  path: wiki/x.md\n  reason: \"done\"\n- date: 2026-09-03\n  kind: record\n  path: raw/y.md\n  reason: \"done\"\n",
        )
        .unwrap();
        assert_eq!(count(&dir), 2);
    }
}
