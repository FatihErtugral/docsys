//! `docsys check <page> --by <label>` — a machine's reading of a page, recorded
//! beside the maintainer's word and never mistaken for it (§21, R-214, D-104).
//! On a team the session that writes cannot vouch (R-025, R-208), and the
//! maintainer's re-read is the bottleneck. An independent session that read
//! every claim against its evidence records what it read; the maintainer then
//! reads the check, not the page from scratch. A check never sets `verified`.

use std::fs;
use std::path::Path;

use crate::fm::Value;
use crate::tree::DocTree;

#[derive(Debug, Default)]
pub struct Checked {
    pub page: String,
    pub by: String,
    pub rev: String,
    pub against: Vec<String>,
    pub committed: bool,
}

fn git(repo: &Path, args: &[&str]) -> Option<String> {
    crate::git::cmd(repo)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// The record in a page's frontmatter: an earlier check replaced, the new
/// lines at the end of the frontmatter, `updated:` bumped (R-052).
fn write_check(
    text: &str,
    by: &str,
    rev: &str,
    hash: &str,
    against: &[String],
    today: &str,
) -> Option<String> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    let (fm, tail) = rest.split_at(end);
    let mut out: Vec<String> = fm
        .split('\n')
        .filter(|l| !l.starts_with("checked_"))
        .map(str::to_string)
        .collect();
    out.push(format!("checked_by: {by}"));
    out.push(format!("checked_rev: {rev}"));
    out.push(format!("checked_hash: \"{hash}\""));
    out.push(format!("checked_against: [{}]", against.join(", ")));
    let rebuilt = format!("---\n{}{tail}", out.join("\n"));
    Some(crate::hook::bump_updated(&rebuilt, today).unwrap_or(rebuilt))
}

/// `docsys check <page> --by <label> [--against <entry>]… [--commit]`.
pub fn check(
    root: &Path,
    target: &str,
    by: &str,
    against: &[String],
    commit: bool,
) -> Result<Checked, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    if !crate::era::Era::of(&tree).machine_checks() {
        return Err(
            "a check record is a docsys/0.5 format — `docsys upgrade` moves the tree first (D-118)"
                .into(),
        );
    }
    let by = by.trim();
    if by.is_empty() {
        return Err("say who checks: `--by <agent or session>`".into());
    }
    let repo = crate::repo_of(root).ok_or_else(|| {
        "a check records a revision, so the tree must be inside a git repository".to_string()
    })?;
    let page = crate::verify::find_page(&tree, target)
        .ok_or_else(|| format!("no permanent page at `{target}` and none with that id"))?;
    let root_rel = {
        let r = crate::fresh::root_rel(&repo, root);
        if r.is_empty() || r == "." {
            String::new()
        } else {
            format!("{r}/")
        }
    };
    let repo_path = format!("{root_rel}{}", page.rel);
    if git(&repo, &["ls-files", "--error-unmatch", "--", &repo_path]).is_none() {
        return Err(format!(
            "`{}` is not committed yet — commit the page first; a check names the revision that holds the body read",
            page.rel
        ));
    }
    if git(&repo, &["diff", "--quiet", "HEAD", "--", &repo_path]).is_none() {
        return Err(format!(
            "`{}` has uncommitted changes — commit them first, then check what is there",
            page.rel
        ));
    }
    // the evidence read: what the caller names, else the page's own sources
    // and the code its pins name
    let fm = page.fm.as_ref();
    let mut evidence: Vec<String> = against.to_vec();
    if evidence.is_empty() {
        if let Some(srcs) = fm
            .and_then(|f| f.fields.get("sources"))
            .and_then(Value::as_list)
        {
            evidence.extend(srcs.iter().cloned());
        }
        if let Some(pins) = fm
            .and_then(|f| f.fields.get("verifies"))
            .and_then(Value::as_maps)
        {
            evidence.extend(pins.iter().filter_map(|p| p.get("path").cloned()));
        }
    }
    evidence.dedup();
    let forms = String::new();
    let mut repo_cache = None;
    let severed: Vec<String> = evidence
        .iter()
        .filter(|e| !e.contains("://"))
        .filter(|e| crate::checks::unresolved_source(&tree, e, &mut repo_cache, &forms).is_some())
        .cloned()
        .collect();
    if !severed.is_empty() {
        return Err(format!(
            "`{}` would be checked against evidence that does not resolve: {} — a check that rests on nothing is a claim",
            page.rel,
            severed.join(", ")
        ));
    }
    let rev = git(&repo, &["rev-parse", "--short", "HEAD"]).ok_or("no HEAD")?;
    let path = root.join(&page.rel);
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let hash = crate::fresh::content_hash(&crate::fresh::body_text(&text));
    let new = write_check(&text, by, &rev, &hash, &evidence, &crate::migrate::today())
        .ok_or("the page has no frontmatter")?;
    fs::write(&path, new).map_err(|e| e.to_string())?;
    let mut out = Checked {
        page: page.rel.clone(),
        by: by.to_string(),
        rev: rev.clone(),
        against: evidence,
        committed: false,
    };
    if commit {
        let ok = crate::git::cmd(&repo)
            .args(["commit", "-q", "-m"])
            .arg(format!(
                "docs: {} checked by {by} at {rev}",
                page.rel.trim_end_matches(".md")
            ))
            .arg("--")
            .arg(&repo_path)
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            return Err(format!(
                "the record is written but the commit did not land — commit `{}` yourself",
                page.rel
            ));
        }
        out.committed = true;
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_new_check_replaces_the_old_one() {
        let text = "---\nid: x\ntype: reference\nupdated: 2026-01-01\nchecked_by: a\nchecked_rev: 1\nchecked_hash: \"h\"\nchecked_against: [old.md]\n---\nBody.\n";
        let out = write_check(
            text,
            "b",
            "2",
            "sha256:x",
            &["src/a.rs".to_string()],
            "2026-10-02",
        )
        .unwrap();
        assert_eq!(out.matches("checked_by:").count(), 1, "{out}");
        assert!(out.contains("updated: 2026-10-02\nchecked_by: b\nchecked_rev: 2\nchecked_hash: \"sha256:x\"\nchecked_against: [src/a.rs]\n---\nBody.\n"), "{out}");
    }
}
