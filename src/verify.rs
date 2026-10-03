//! `docsys verify <page>` — a docsys/0.4 maintainer's "this is true", in one
//! command, as 0.15.1 kept it (D-094, D-118); a docsys/0.5 page carries no
//! verification and the command is refused there (D-130). The record R-028
//! asks for is three frontmatter fields whose values a person has to look
//! up: their own handle, the commit that holds the body they just read, and
//! the rule that the page must be committed first. The command looks them
//! up: the handle from the git identity matched against `maintainers:`, the
//! revision from `HEAD` once the page has no uncommitted change, and it
//! refuses when a source does not resolve. It writes the record; the commit
//! stays the person's act (`--commit` runs it under their own identity, which
//! is what R-208's history half checks). `--revoke` is the reverse: back to
//! `unverified`, the record removed.

use std::fs;
use std::path::Path;

use crate::fm::Value;
use crate::tree::{DocTree, Kind};

#[derive(Debug, Default)]
pub struct Verified {
    pub page: String,
    pub by: String,
    pub rev: String,
    pub committed: bool,
    pub notes: Vec<String>,
}

fn git(repo: &Path, args: &[&str]) -> Option<String> {
    crate::git::cmd(repo)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// The page named by a path (root-relative, with or without `.md`) or an id.
pub(crate) fn find_page<'a>(tree: &'a DocTree, target: &str) -> Option<&'a crate::tree::Page> {
    let wanted = target
        .trim()
        .trim_start_matches("./")
        .trim_end_matches(".md")
        .to_string();
    tree.pages
        .iter()
        .filter(|p| p.kind == Kind::Permanent)
        .find(|p| {
            p.rel.trim_end_matches(".md") == wanted
                || p.rel.trim_end_matches(".md").trim_start_matches("wiki/") == wanted
                || p.fm
                    .as_ref()
                    .and_then(|f| f.fields.get("id"))
                    .and_then(Value::as_str)
                    == Some(target.trim())
        })
}

/// Who is verifying: `--by <handle|@login>`, else the git identity matched
/// against `maintainers:` (by email, then by name), else the git user name.
/// Returns the handle and, when known, the email the record's commit must
/// be authored with (D-095: CI records a review approval under the
/// approver's identity).
fn who(tree: &DocTree, repo: &Path, by: Option<&str>) -> Result<(String, Option<String>), String> {
    let maintainers = crate::checks::maintainer_handles(tree);
    maintainers.readable()?;
    let list = || {
        maintainers
            .iter()
            .map(|m| match (&m.email, &m.login) {
                (Some(e), Some(l)) => format!("{} <{e}> @{l}", m.handle),
                (Some(e), None) => format!("{} <{e}>", m.handle),
                (None, Some(l)) => format!("{} @{l}", m.handle),
                (None, None) => m.handle.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let email = git(repo, &["config", "user.email"])
        .unwrap_or_default()
        .to_lowercase();
    let name = git(repo, &["config", "user.name"]).unwrap_or_default();
    if let Some(b) = by {
        let b = b.trim();
        if let Some(login) = b.strip_prefix('@') {
            let l = login.to_lowercase();
            return maintainers
                .iter()
                .find(|m| m.login_or_handle() == l.as_str())
                .map(|m| (m.handle.clone(), m.email.clone()))
                .ok_or_else(|| {
                    format!(
                        "`@{login}` is no declared maintainer's login ({}) — add `@{login}` to their entry in .docmeta.yml maintainers: (R-208)",
                        list()
                    )
                });
        }
        let h = crate::checks::record_handle(b);
        if maintainers.anyone() {
            return Ok((b.to_string(), None));
        }
        return maintainers
            .iter()
            .find(|m| m.handle == h)
            .map(|m| (b.to_string(), m.email.clone()))
            .ok_or_else(|| {
                format!(
                    "`{b}` is not a declared maintainer ({}) — a maintainer vouches (R-208)",
                    list()
                )
            });
    }
    if maintainers.anyone() {
        return if name.is_empty() {
            Err("no git identity and no `--by <handle>` — say who verifies".into())
        } else {
            Ok((name, None))
        };
    }
    if let Some(m) = maintainers
        .iter()
        .find(|m| !email.is_empty() && m.email.as_deref() == Some(email.as_str()))
    {
        return Ok((m.handle.clone(), m.email.clone()));
    }
    let lname = name.to_lowercase();
    if let Some(m) = maintainers.iter().find(|m| m.handle == lname) {
        return Ok((m.handle.clone(), m.email.clone()));
    }
    Err(format!(
        "your git identity ({name} <{email}>) matches no declared maintainer ({}) — only a maintainer verifies (R-208); pass `--by <handle>` if the list names you differently",
        list()
    ))
}

/// The record `verify` writes on a docsys/0.4 tree, as 0.15 wrote it (R-028,
/// D-118): who, and at which revision.
pub struct Record<'a> {
    pub by: &'a str,
    pub rev: &'a str,
}

fn record_lines(rec: &Record) -> Vec<String> {
    vec![
        "verification: verified".to_string(),
        format!("verified_by: {}", rec.by),
        format!("verified_rev: {}", rec.rev),
    ]
}

/// Rewrite the verification record in a page's frontmatter and bump its
/// `updated:` to `today`: `Some` writes a new record in place of the old one;
/// `None` sets `unverified` and removes the record (D-118).
fn write_record(text: &str, record: Option<&Record>, today: &str) -> Option<String> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    let (fm, tail) = rest.split_at(end);
    let mut out = Vec::new();
    let mut placed = false;
    for line in fm.split('\n') {
        if line.starts_with("verified_by:") || line.starts_with("verified_rev:") {
            continue;
        }
        if line.starts_with("verification:") {
            match record {
                Some(rec) => out.extend(record_lines(rec)),
                None => out.push("verification: unverified".to_string()),
            }
            placed = true;
            continue;
        }
        out.push(line.to_string());
    }
    if !placed {
        // a page that never carried the field: it goes after `type:`
        let mut with = Vec::new();
        for line in out {
            let is_type = line.starts_with("type:");
            with.push(line);
            if is_type {
                match record {
                    Some(rec) => with.extend(record_lines(rec)),
                    None => with.push("verification: unverified".to_string()),
                }
                if !fm.contains("\nsources:") && !fm.starts_with("sources:") {
                    with.push("sources: []".to_string());
                }
            }
        }
        out = with;
    }
    let rebuilt = format!("---\n{}{tail}", out.join("\n"));
    Some(crate::hook::bump_updated(&rebuilt, today).unwrap_or(rebuilt))
}

/// What `verify --range` came to.
#[derive(Debug)]
pub enum Range {
    /// every page the range touched that carries `verification:`
    Pages(Vec<Verified>),
    /// `--by @login` is on no maintainer entry: a host adapter runs once per
    /// approver, so an approver outside the list is skipped, not a failure
    /// (D-105)
    NotAMaintainer(String),
}

/// `docsys verify --range <a>...<b> --by @login [--commit]` (D-095): every
/// permanent page the range touched that carries `verification:` and whose
/// body is not what a verification already holds — the review approved
/// them all. One commit for the lot, under the approver's identity.
pub fn verify_range(
    root: &Path,
    range: &str,
    by: Option<&str>,
    from_trailers: bool,
    commit: bool,
) -> Result<Range, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    let repo = crate::repo_of(root).ok_or("the tree must be inside a git repository")?;
    // a range git cannot read fails the job on every tree (D-105)
    let changed = git(&repo, &["diff", "--name-only", range])
        .ok_or_else(|| format!("`{range}` is not a range git can read"))?;
    // the approver first: an unknown one is not a page-by-page skip, it is no run at all
    crate::checks::maintainer_handles(&tree).readable()?;
    let trailer_by;
    let by = if from_trailers {
        trailer_by = approver_from_trailers(&tree, &repo, range)?;
        Some(trailer_by.as_str())
    } else {
        by
    };
    if let Some(login) = by.and_then(|b| b.trim().strip_prefix('@')) {
        let l = login.to_lowercase();
        if !crate::checks::maintainer_handles(&tree)
            .iter()
            .any(|m| m.login_or_handle() == l.as_str())
        {
            return Ok(Range::NotAMaintainer(login.to_string()));
        }
    }
    who(&tree, &repo, by)?;
    let root_rel = root_prefix(&repo, root);
    let mut done = Vec::new();
    let mut paths = Vec::new();
    for f in changed.lines() {
        let Some(rel) = f.strip_prefix(root_rel.as_str()) else {
            continue;
        };
        let Some(page) = tree
            .pages
            .iter()
            .find(|p| p.rel == rel && p.kind == Kind::Permanent)
        else {
            continue;
        };
        if page
            .fm
            .as_ref()
            .and_then(|f| f.fields.get("verification"))
            .is_none()
        {
            continue; // a page that never opted into verification is not the review's to vouch for
        }
        match verify(root, &page.rel, by, false, false) {
            Ok(v) => {
                if !v.notes.iter().any(|n| n.starts_with("already")) {
                    paths.push(format!("{root_rel}{}", page.rel));
                }
                done.push(v);
            }
            Err(e) => {
                let mut v = Verified {
                    page: page.rel.clone(),
                    ..Default::default()
                };
                v.notes.push(format!("skipped: {e}"));
                done.push(v);
            }
        }
    }
    if commit && !paths.is_empty() {
        let (handle, email) = who(&tree, &repo, by)?;
        let mut cmd = crate::git::cmd(&repo);
        if let Some(e) = &email {
            cmd.args([
                "-c",
                &format!("user.email={e}"),
                "-c",
                &format!("user.name={handle}"),
            ]);
        }
        let ok = cmd
            .args(["commit", "-q", "-m"])
            .arg(format!(
                "docs: {} page(s) verified by {handle} — review approval on {range}",
                paths.len()
            ))
            .arg("--")
            .args(&paths)
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            return Err("the records are written but the commit did not land — the gate may have refused; see `docsys lint`".into());
        }
        for v in &mut done {
            if !v
                .notes
                .iter()
                .any(|n| n.starts_with("already") || n.starts_with("skipped"))
            {
                v.committed = true;
            }
        }
    }
    Ok(Range::Pages(done))
}

/// The review's word without a host (D-095): `Reviewed-by:` / `Approved-by:`
/// trailers on the commits of the range — the convention every forge, and
/// an e-mailed patch, can carry. The first trailer whose e-mail is a declared
/// maintainer's names the verifier; none is an error, not a silent pass.
fn approver_from_trailers(tree: &DocTree, repo: &Path, range: &str) -> Result<String, String> {
    let maintainers = crate::checks::maintainer_handles(tree);
    let out = git(
        repo,
        &[
            "log",
            "--format=%(trailers:key=Reviewed-by,valueonly)%n%(trailers:key=Approved-by,valueonly)",
            range,
        ],
    )
    .ok_or_else(|| format!("`{range}` is not a range git can read"))?;
    let mut seen = Vec::new();
    for line in out.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let email = line
            .rsplit_once('<')
            .map(|(_, e)| e.trim_end_matches('>').trim().to_lowercase())
            .unwrap_or_else(|| line.to_lowercase());
        seen.push(line.to_string());
        if let Some(m) = maintainers
            .iter()
            .find(|m| m.email.as_deref() == Some(email.as_str()))
        {
            return Ok(m.handle.clone());
        }
    }
    Err(if seen.is_empty() {
        format!("no Reviewed-by: or Approved-by: trailer on the commits of {range} — the review left no word in git; add the trailer to the merge commit, or pass --by")
    } else {
        format!(
            "the trailers on {range} name nobody in maintainers: ({}) — {}",
            seen.join("; "),
            maintainers
                .iter()
                .map(|m| match &m.email {
                    Some(e) => format!("{} <{e}>", m.handle),
                    None => m.handle.clone(),
                })
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

fn root_prefix(repo: &Path, root: &Path) -> String {
    let mut root_rel = crate::fresh::root_rel(repo, root);
    if root_rel == "." {
        root_rel.clear();
    }
    if !root_rel.is_empty() && !root_rel.ends_with('/') {
        root_rel.push('/');
    }
    root_rel
}

/// `docsys verify <page> [--by <handle|@login>] [--commit] [--revoke]`.
pub fn verify(
    root: &Path,
    target: &str,
    by: Option<&str>,
    commit: bool,
    revoke: bool,
) -> Result<Verified, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    let repo = crate::repo_of(root).ok_or_else(|| {
        "verification records a revision, so the tree must be inside a git repository".to_string()
    })?;
    let page = find_page(&tree, target)
        .ok_or_else(|| format!("no permanent page at `{target}` and none with that id"))?;
    crate::fm::refuse_unclosed_in(&page.rel, &page.text)?;
    let path = root.join(&page.rel);
    let today = crate::migrate::today();
    let today = today.as_str();
    let mut out = Verified {
        page: page.rel.clone(),
        ..Default::default()
    };
    if revoke {
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let new = write_record(&text, None, today).ok_or("the page has no frontmatter")?;
        fs::write(&path, new).map_err(|e| e.to_string())?;
        out.by = String::new();
        out.notes
            .push("back to unverified; the record removed — another audit sets it again".into());
        return Ok(out);
    }
    // the person
    let (by, email) = who(&tree, &repo, by)?;
    // the page must be committed as it is: the revision has to hold this body
    let root_rel = root_prefix(&repo, root);
    let repo_path = format!("{root_rel}{}", page.rel);
    let tracked = git(&repo, &["ls-files", "--error-unmatch", "--", &repo_path]).is_some();
    if !tracked {
        return Err(format!(
            "`{}` is not committed yet — commit the page first; a verification names the revision that holds the body you read",
            page.rel
        ));
    }
    let dirty = git(&repo, &["diff", "--quiet", "HEAD", "--", &repo_path]).is_none();
    if dirty {
        return Err(format!(
            "`{}` has uncommitted changes — commit them first, then verify what you read (or `docsys verify --revoke` if the body moved)",
            page.rel
        ));
    }
    // the sources must resolve — a verification that rests on nothing is a claim
    let (report, _) = crate::lint_in(root, Some(&repo));
    let severed: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.file == page.rel && f.rule.0 == "R-059")
        .map(|f| f.subject.clone())
        .collect();
    if !severed.is_empty() {
        return Err(format!(
            "`{}` cites sources that do not resolve: {} — fix `sources:` first (R-059)",
            page.rel,
            severed.join(", ")
        ));
    }
    let rev = git(&repo, &["rev-parse", "--short", "HEAD"]).ok_or("no HEAD")?;
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let body = crate::fresh::body_text(&text);
    if let Some(fm) = &page.fm {
        let get = |k: &str| fm.fields.get(k).and_then(Value::as_str);
        if get("verification") == Some("verified") {
            // already verified, and the body has not moved since: nothing to
            // do — the body the revision holds, compared as lint compares
            // (R-113)
            let held = get("verified_rev")
                .and_then(|r| git(&repo, &["show", &format!("{r}:{repo_path}")]))
                .is_some_and(|t| {
                    crate::fresh::content_hash(&crate::fresh::body_text(&t))
                        == crate::fresh::content_hash(&body)
                });
            if held {
                out.by = get("verified_by").unwrap_or("").to_string();
                out.rev = get("verified_rev").unwrap_or("").to_string();
                out.notes.push(
                    "already verified, and the body has not moved since — nothing to do".into(),
                );
                return Ok(out);
            }
        }
    }
    let record = Record { by: &by, rev: &rev };
    let new = write_record(&text, Some(&record), today).ok_or("the page has no frontmatter")?;
    fs::write(&path, new).map_err(|e| e.to_string())?;
    out.by = by.clone();
    out.rev = rev.clone();
    out.notes.push(
        "R-025: this is your reading of the page against its sources — not the session that wrote it"
            .into(),
    );
    if commit {
        // the record is the maintainer's own commit (R-208): their identity, even
        // when the repository's configured identity is somebody else's (CI)
        let mut cmd = crate::git::cmd(&repo);
        if let Some(e) = &email {
            cmd.args([
                "-c",
                &format!("user.email={e}"),
                "-c",
                &format!("user.name={by}"),
            ]);
        }
        let ok = cmd
            .args(["commit", "-q", "-m"])
            .arg(format!(
                "docs: {} verified by {by} at {rev}",
                page.rel.trim_end_matches(".md")
            ))
            .arg("--")
            .arg(&repo_path)
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            return Err(format!(
                "the record is written but the commit did not land — commit `{}` yourself (the gate may have refused; `docsys lint --root {}` says why)",
                page.rel,
                root.display()
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
    fn the_record_is_0_15_s_and_a_revoke_removes_it() {
        let rec = Record {
            by: "ayse",
            rev: "abc1234",
        };
        let text = "---\nid: x\ntype: reference\nverification: unverified\nsources: [a.md]\nupdated: 2026-01-01\n---\nBody.\n";
        let out = write_record(text, Some(&rec), "2026-09-04").unwrap();
        assert!(out.contains("verification: verified\nverified_by: ayse\nverified_rev: abc1234\nsources: [a.md]\nupdated: 2026-09-04\n---\nBody.\n"), "{out}");
        let again = Record {
            by: "bora",
            rev: "def5678",
        };
        let twice = write_record(&out, Some(&again), "2026-09-06").unwrap();
        assert_eq!(twice.matches("verified_by:").count(), 1, "{twice}");
        let back = write_record(&twice, None, "2026-09-07").unwrap();
        assert!(
            back.contains("verification: unverified\nsources: [a.md]\n")
                && !back.contains("verified_by"),
            "{back}"
        );
        let never = "---\nid: y\ntype: howto\nupdated: 2026-01-01\n---\nSteps.\n";
        let out = write_record(never, Some(&rec), "2026-09-04").unwrap();
        assert!(out.contains("type: howto\nverification: verified\nverified_by: ayse\nverified_rev: abc1234\nsources: []\nupdated: 2026-09-04\n"), "{out}");
    }
}
