//! `docsys verify <page>` — a maintainer's "this is true", in one command
//! (D-094). The record R-028 asks for is three frontmatter fields whose
//! values a person has to look up: their own handle, the commit that holds
//! the body they just read, and the rule that the page must be committed
//! first. The command looks them up: the handle from the git identity matched
//! against `maintainers:`, the revision from `HEAD` once the page has no
//! uncommitted change, and it refuses when a source does not resolve. It
//! writes the record; the commit stays the person's act (`--commit` runs it
//! under their own identity, which is what R-208's history half checks).
//! `--revoke` is the reverse: back to `unverified`, the record kept as the last
//! verification — what a page needs after its body moved (R-024, D-101).
//! `--show` reads the record against the body: what a re-verification reads
//! (R-028).

use std::fs;
use std::path::Path;

use crate::fm::Value;
use crate::tree::{DocTree, Kind, Page};

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

/// The record `verify` writes (R-028, D-101): who, at which revision, the
/// blocks of the body that was read, and each consumed source's hash. A
/// docsys/0.4 tree gets the record 0.15 wrote, without blocks (D-118).
pub struct Record<'a> {
    pub by: &'a str,
    pub rev: &'a str,
    pub blocks: Option<&'a [String]>,
    pub sources: &'a [(String, String)],
}

fn record_lines(rec: &Record) -> Vec<String> {
    let mut out = vec![
        "verification: verified".to_string(),
        format!("verified_by: {}", rec.by),
        format!("verified_rev: {}", rec.rev),
    ];
    if let Some(blocks) = rec.blocks {
        out.push(format!("verified_blocks: [{}]", blocks.join(", ")));
    }
    if !rec.sources.is_empty() {
        out.push("verified_sources:".to_string());
        for (s, h) in rec.sources {
            out.push(format!("  - source: \"{s}\""));
            out.push(format!("    hash: \"{h}\""));
        }
    }
    out
}

/// Rewrite the verification record in a page's frontmatter, and on a
/// docsys/0.4 tree bump its `updated:` to `today`. `Some` writes a new record in place of the old one; `None`
/// sets `unverified`, and keeps the record as the last verification when
/// `keep_last` (D-101) — the reading the next verifier compares against — or
/// removes it, as on a docsys/0.4 tree (D-118).
fn write_record(
    text: &str,
    record: Option<&Record>,
    keep_last: bool,
    today: Option<&str>,
) -> Option<String> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    let (fm, tail) = rest.split_at(end);
    let mut out = Vec::new();
    let mut placed = false;
    let mut in_sources_block = false;
    let mut in_reflowed_blocks = false;
    for line in fm.split('\n') {
        if in_sources_block {
            if line.starts_with("  ") {
                continue;
            }
            in_sources_block = false;
        }
        if in_reflowed_blocks {
            in_reflowed_blocks = !line.contains(']');
            continue;
        }
        let replaced = record.is_some() || !keep_last;
        if replaced && (line.starts_with("verified_by:") || line.starts_with("verified_rev:")) {
            continue;
        }
        if replaced && line.starts_with("verified_sources:") {
            in_sources_block = true;
            continue;
        }
        if replaced && line.starts_with("verified_blocks:") {
            // a formatter may have reflowed the list across lines (D-002)
            in_reflowed_blocks = !line.contains(']');
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
    match today {
        Some(today) => Some(crate::hook::bump_updated(&rebuilt, today).unwrap_or(rebuilt)),
        None => Some(rebuilt),
    }
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
    // docsys/0.5: the approval is the merge commit's `Approved-by:` line, and
    // there is nothing to write (D-126)
    if crate::era::Era::of(&tree).verification_from_history() {
        return Ok(Range::Pages(Vec::new()));
    }
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
    if crate::era::Era::of(&tree).verification_from_history() {
        return verify_by_commit(root, &tree, &repo, page, by, revoke);
    }
    crate::fm::refuse_unclosed_in(&page.rel, &page.text)?;
    let path = root.join(&page.rel);
    // a docsys/0.5 page's date is history's (D-122)
    let today = (!crate::era::Era::at(root).derived_dates()).then(crate::migrate::today);
    let today = today.as_deref();
    let mut out = Verified {
        page: page.rel.clone(),
        ..Default::default()
    };
    if revoke {
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let keep = crate::era::Era::at(root).anchored_verification();
        let new = write_record(&text, None, keep, today).ok_or("the page has no frontmatter")?;
        fs::write(&path, new).map_err(|e| e.to_string())?;
        out.by = String::new();
        out.notes.push(if keep {
            "back to unverified; the record stays as the last verification — a maintainer verifies it again (R-025)".into()
        } else {
            "back to unverified; the record removed — another audit sets it again".into()
        });
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
    let era = crate::era::Era::at(root);
    let anchored = era.anchored_verification();
    let body = crate::fresh::body_text(&text);
    let blocks = crate::blocks::hashes(&body);
    if let Some(fm) = &page.fm {
        let get = |k: &str| fm.fields.get(k).and_then(Value::as_str);
        if get("verification") == Some("verified") {
            // already verified, and the body has not moved since: nothing to do.
            // The record's own blocks answer; a record from before them, the
            // body the revision holds — compared as lint compares (R-113).
            let held = match crate::blocks::holds(fm, &text) {
                Some(same) => same,
                None => get("verified_rev")
                    .and_then(|r| git(&repo, &["show", &format!("{r}:{repo_path}")]))
                    .is_some_and(|t| {
                        crate::fresh::content_hash(&crate::fresh::body_text(&t))
                            == crate::fresh::content_hash(&body)
                    }),
            };
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
    let sources: Vec<(String, String)> = if !anchored {
        Vec::new()
    } else {
        page.fm
            .as_ref()
            .map(crate::fresh::consumed_sources)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|s| {
                let h = crate::fresh::source_hash(root, &s)?;
                Some((s, h))
            })
            .collect()
    };
    let record = Record {
        by: &by,
        rev: &rev,
        blocks: anchored.then_some(blocks.as_slice()),
        sources: &sources,
    };
    let new =
        write_record(&text, Some(&record), anchored, today).ok_or("the page has no frontmatter")?;
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

/// How an approval names the maintainer: `handle <email>` when the entry
/// carries one, else `@login`, else the handle (R-208).
pub(crate) fn approver_value(tree: &DocTree, handle: &str) -> String {
    crate::checks::maintainer_handles(tree)
        .into_iter()
        .find(|m| m.handle == crate::checks::record_handle(handle))
        .map_or_else(
            || handle.to_string(),
            |m| match (&m.email, &m.login) {
                (Some(e), _) => format!("{} <{e}>", m.handle),
                (None, Some(l)) => format!("@{l}"),
                (None, None) => m.handle,
            },
        )
}

/// `verify` on a docsys/0.5 tree (R-024, D-126): nothing is written into the
/// page. The maintainer's word is their own commit — an empty one carrying
/// `Verifies: <page> <body hash>` and `Approved-by:`, or `Revokes:` — so a
/// branch that verifies merges with every other branch, and the approval
/// counts wherever the body it read lands.
fn verify_by_commit(
    root: &Path,
    tree: &DocTree,
    repo: &Path,
    page: &Page,
    by: Option<&str>,
    revoke: bool,
) -> Result<Verified, String> {
    let (by, email) = who(tree, repo, by)?;
    let root_rel = root_prefix(repo, root);
    let repo_path = format!("{root_rel}{}", page.rel);
    if git(repo, &["ls-files", "--error-unmatch", "--", &repo_path]).is_none() {
        return Err(format!(
            "`{}` is not committed yet — commit the page first; an approval vouches for the body history holds",
            page.rel
        ));
    }
    if git(repo, &["diff", "--quiet", "HEAD", "--", &repo_path]).is_none() {
        return Err(format!(
            "`{}` has uncommitted changes — commit them first, then verify what you read",
            page.rel
        ));
    }
    if git(repo, &["diff", "--cached", "--quiet"]).is_none() {
        return Err(
            "something is staged — commit or unstage it first; the approval is a commit of its own"
                .into(),
        );
    }
    let state = crate::approval::Approvals::of(tree).state(&page.rel);
    let mut out = Verified {
        page: page.rel.clone(),
        ..Default::default()
    };
    let verified = matches!(state, crate::approval::State::Verified { .. });
    if !revoke {
        if !crate::approval::tracked(tree, page) {
            return Err(format!(
                "`{}` names no source in `sources:` and no pin — there is nothing to check its claims against (§3.2, P/R-025); name what it rests on first",
                page.rel
            ));
        }
        let (report, _) = crate::lint_in(root, Some(repo));
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
        if let crate::approval::State::Verified { by: was, commit } = &state {
            out.by = was.clone();
            out.rev = commit.clone();
            out.notes
                .push("already verified, and the body has not moved since — nothing to do".into());
            return Ok(out);
        }
    } else if !verified {
        out.notes.push("not verified — nothing to take back".into());
        return Ok(out);
    }
    let value = approver_value(tree, &by);
    let name = page.rel.trim_end_matches(".md");
    let (subject, trailers) = if revoke {
        (
            format!("docs: {name} back to unverified"),
            format!(
                "{}: {}\nRevoked-by: {value}",
                crate::approval::REVOKES,
                page.rel
            ),
        )
    } else {
        (
            format!("docs: {name} verified by {by}"),
            format!(
                "{}: {} {}\n{}: {value}",
                crate::approval::VERIFIES,
                page.rel,
                crate::approval::body_hash(&page.text),
                crate::approval::APPROVED_BY
            ),
        )
    };
    // the maintainer's own commit (R-208): their identity, even when the
    // repository's configured one is somebody else's
    let mut cmd = crate::git::cmd(repo);
    if let Some(e) = &email {
        cmd.args([
            "-c",
            &format!("user.email={e}"),
            "-c",
            &format!("user.name={by}"),
        ]);
    }
    let ok = cmd
        .args(["commit", "-q", "--allow-empty", "-m"])
        .arg(&subject)
        .arg("-m")
        .arg(&trailers)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        return Err(refused(root, tree, repo));
    }
    out.by = if revoke { String::new() } else { by };
    out.rev = git(repo, &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
    out.committed = true;
    out.notes.push(if revoke {
        "back to unverified: the commit carries `Revokes:` — a maintainer verifies it again (R-025)".into()
    } else {
        "R-025: this is your reading of the page against its sources — the commit carries `Approved-by:`".into()
    });
    Ok(out)
}

/// Why an approval commit did not land, naming the check of the git gate that
/// refused it: the gate runs `lint` and `refs`, and only the one with an error
/// says why.
fn refused(root: &Path, tree: &DocTree, repo: &Path) -> String {
    let errors = |r: &crate::checks::Report| {
        r.findings
            .iter()
            .any(|f| f.severity == crate::model::Severity::Error)
    };
    let lint = errors(&crate::lint_in(root, Some(repo)).0);
    let refs = errors(&crate::refs::run(repo, tree));
    let named = match (lint, refs) {
        (true, true) => "`docsys lint` and `docsys refs` report errors",
        (true, false) => "`docsys lint` reports errors",
        (false, true) => "`docsys refs` reports errors",
        (false, false) => {
            return "the approval commit did not land — git's output above says why".into()
        }
    };
    format!("the approval commit did not land: the git gate refused it, and {named}")
}

/// The line the approval job adds to a pull request's description when the
/// reviewer is a maintainer (D-126); `None` for anyone else.
pub fn approval_line(root: &Path, login: &str) -> Result<Option<String>, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    let maintainers = crate::checks::maintainer_handles(&tree);
    maintainers.readable()?;
    let login = format!("@{}", login.trim().trim_start_matches('@'));
    if maintainers.anyone() {
        return Ok(None);
    }
    Ok(crate::approval::maintainer_of(&maintainers, &login)
        .map(|_| format!("{}: {login}", crate::approval::APPROVED_BY)))
}

/// A block's first line, cut for one line of output.
fn excerpt(text: &str) -> String {
    let mut lines = text.lines();
    let first = lines.next().unwrap_or("").trim_end();
    let mut out: String = first.chars().take(72).collect();
    if first.chars().count() > 72 || lines.next().is_some() {
        out.push_str(" …");
    }
    out
}

fn indented(text: &str) -> String {
    text.lines().map(|l| format!("    {l}\n")).collect()
}

/// `docsys verify --show <page>` (R-028, D-103), read-only: what a
/// re-verification reads. The current blocks numbered `[1]…[n]` with their
/// lines — the numbers `pin --block` takes — the changed and new ones with
/// their text, the removed ones from `verified_rev` where history holds it,
/// the pins bound to a stale block or to none, and what the page rests on.
pub fn show(root: &Path, target: &str) -> Result<String, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    let era = crate::era::Era::of(&tree);
    if !era.anchored_verification() {
        return Err(
            "a block record is a docsys/0.5 format — `docsys upgrade` moves the tree first (D-118)"
                .into(),
        );
    }
    let page = find_page(&tree, target)
        .ok_or_else(|| format!("no permanent page at `{target}` and none with that id"))?;
    let fm = page
        .fm
        .as_ref()
        .ok_or("the page has no frontmatter (R-050)")?;
    let get = |k: &str| fm.fields.get(k).and_then(Value::as_str).map(str::trim);
    let rel = &page.rel;
    let repo = crate::repo_of(root);
    let body = crate::fresh::body_text(&page.text);
    let blocks = crate::blocks::split(&body);
    let current: Vec<String> = blocks.iter().map(|b| b.hash.clone()).collect();
    let stale = repo
        .as_deref()
        .map(|r| crate::fresh::stale_blocks(root, r, era, fm))
        .unwrap_or_default();
    // what was read last: on docsys/0.5 the body the newest approval vouched
    // for (D-126); else, or with no approval yet, a record in the page
    let from_history = era.verification_from_history();
    let approved = from_history
        .then(|| crate::approval::last_approval(&tree, page))
        .flatten();
    let approvals = from_history.then(|| crate::approval::Approvals::of(&tree));
    let tracked = !from_history || crate::approval::tracked(&tree, page);
    let lost = approved.as_ref().and_then(|a| a.lost.clone());
    let state = if !tracked {
        "takes no part"
    } else if let Some(a) = &approvals {
        match a.state(rel) {
            crate::approval::State::Verified { .. } => "verified",
            crate::approval::State::Unverified => "unverified",
            crate::approval::State::Unknown => "verification unknown",
        }
    } else {
        get("verification").unwrap_or("no verification field")
    };
    // a page that takes no part has no approval to read against
    let (recorded, last, then_text) = match approved {
        _ if !tracked => (None, None, None),
        Some(a) => (
            a.read
                .as_deref()
                .map(|t| crate::blocks::hashes(&crate::fresh::body_text(t))),
            Some((a.by, a.commit)),
            a.read,
        ),
        None => (
            crate::blocks::record_of(fm),
            match (get("verified_by"), get("verified_rev")) {
                (Some(by), Some(rev)) => Some((by.to_string(), rev.to_string())),
                _ => None,
            },
            None,
        ),
    };
    let last = last.as_ref().map(|(b, r)| (b.as_str(), r.as_str()));
    let compared = recorded
        .as_deref()
        .map(|r| crate::blocks::compare(r, &current));
    let mut out = String::new();
    let mut next =
        format!("then: read every block against what it rests on, and `docsys verify {rel}`\n");
    let reading = recorded
        .as_deref()
        .map(|r| crate::blocks::reading_of(r, &page.text, &stale));
    // on docsys/0.5 the step follows the state: a stale pin is refreshed, an
    // approval no longer holding is made again (D-126)
    let unverified = state == "unverified";
    let moved_source = approvals.as_ref().is_some_and(|a| a.source_moved(rel));
    // why the approval does not hold, when something other than the reading says it
    let why = if !tracked {
        Some(" — it names no source in `sources:` and no pin, and such a page takes no part in verification".to_string())
    } else {
        lost.filter(|_| unverified).map(|l| format!(" — {l}"))
    };
    match (&recorded, reading, last) {
        (Some(_), Some(reading), Some((by, rev))) => {
            next = if reading.moved {
                format!("then: read what is marked against what it rests on, and `docsys verify {rel}`\n")
            } else if reading.partial() && from_history {
                format!("then: re-read the marked block against its pinned region, and `docsys pin --refresh {rel}`\n")
            } else if reading.partial() {
                format!("then: read what is marked against what it rests on, and `docsys verify {rel}`\n")
            } else if unverified {
                format!("then: a maintainer reads it again against what it rests on, and `docsys verify {rel}`\n")
            } else {
                "then: nothing — the approval holds the body as it is\n".to_string()
            };
            out.push_str(&format!(
                "{rel} ({state}): {}/{} blocks as verified by {by} at {rev}{}\n",
                reading.found,
                reading.of,
                why.as_deref().unwrap_or(if reading.moved {
                    " — the body moved since"
                } else if reading.partial() {
                    " — a pin a block rests on is stale"
                } else if unverified && moved_source {
                    " — a source it consumes moved since"
                } else if unverified {
                    " — the approval no longer holds"
                } else {
                    " — nothing to re-read"
                })
            ));
        }
        (_, _, Some((by, rev))) if from_history => out.push_str(&format!(
            "{rel} ({state}): approved by {by} at {rev}{}; every block is to be read\n",
            why.as_deref().unwrap_or(" — what it read is not in this history")
        )),
        (_, _, Some((by, rev))) => out.push_str(&format!(
            "{rel} ({state}): no block record — the verification by {by} at {rev} did not record its blocks; every block is to be read\n"
        )),
        _ if !tracked => out.push_str(&format!(
            "{rel} ({state}){}\n",
            why.as_deref().unwrap_or_default()
        )),
        _ => out.push_str(&format!(
            "{rel} ({state}): never verified; every block is to be read\n"
        )),
    }
    let offset = fm.body_start;
    for (k, b) in blocks.iter().enumerate() {
        let (first, last) = (offset + b.first, offset + b.last);
        let at = if first == last {
            format!("[{}] line {first}", k + 1)
        } else {
            format!("[{}] lines {first}-{last}", k + 1)
        };
        let fate = compared.as_ref().and_then(|c| c.current.get(k)).copied();
        let mark = match fate {
            Some(crate::blocks::Fate::Changed) => Some("changed"),
            Some(crate::blocks::Fate::New) => Some("new"),
            Some(crate::blocks::Fate::Same) if stale.contains(&b.hash) => {
                Some("a pin it rests on is stale")
            }
            _ => None,
        };
        match mark {
            Some(m) => out.push_str(&format!("{at}, {m}:\n{}", indented(&b.text))),
            None => out.push_str(&format!("{at}: {}\n", excerpt(&b.text))),
        }
    }
    if let (Some(recorded), Some(c)) = (&recorded, &compared) {
        if !c.removed.is_empty() {
            // the text the record was taken of, where history still holds it
            let then: Vec<crate::blocks::Block> = match (&repo, last) {
                _ if then_text.is_some() => then_text
                    .as_deref()
                    .map(|t| crate::blocks::split(&crate::fresh::body_text(t)))
                    .unwrap_or_default(),
                (Some(repo), Some((_, rev))) => {
                    let spec = format!("{rev}:{}{rel}", root_prefix(repo, root));
                    git(repo, &["show", &spec])
                        .map(|t| crate::blocks::split(&crate::fresh::body_text(&t)))
                        .unwrap_or_default()
                }
                _ => Vec::new(),
            };
            let mut texts = Vec::new();
            let mut lost = 0usize;
            for i in &c.removed {
                let hash = recorded.get(*i).map(String::as_str).unwrap_or("");
                match then.iter().find(|b| b.hash == hash) {
                    Some(b) => texts.push(b.text.clone()),
                    None => lost += 1,
                }
            }
            if !texts.is_empty() {
                out.push_str(&format!("{} removed:\n", texts.len()));
                for t in &texts {
                    out.push_str(&indented(t));
                }
            }
            if lost > 0 {
                out.push_str(&format!("{lost} removed (text not in this history)\n"));
            }
        }
    }
    let pins = crate::fresh::pins_of(fm);
    for p in &pins {
        let Some(b) = &p.block else { continue };
        match current.iter().position(|h| h == b) {
            Some(k) if stale.contains(b) => out.push_str(&format!(
                "pin {} bound to [{}], stale — re-read that block against it, then `docsys pin --refresh {rel}`\n",
                p.label(),
                k + 1
            )),
            Some(_) => {}
            None => out.push_str(&format!(
                "pin {} bound to a block the body no longer holds (R-212) — bind it again with `docsys pin {rel} {}{} --block <n>`\n",
                p.label(),
                p.path,
                p.symbol
                    .as_ref()
                    .map(|s| format!(" --symbol {s}"))
                    .unwrap_or_default()
            )),
        }
    }
    let sources = fm
        .fields
        .get("sources")
        .and_then(Value::as_list)
        .unwrap_or(&[]);
    let mut against = Vec::new();
    if !sources.is_empty() {
        against.push(format!("sources {}", sources.join(", ")));
    }
    if !pins.is_empty() {
        let labels: Vec<String> = pins.iter().map(crate::fresh::Pin::label).collect();
        against.push(format!("pins {}", labels.join(", ")));
    }
    out.push_str(&if against.is_empty() {
        "read against: nothing listed — no `sources:`, no pins\n".to_string()
    } else {
        format!("read against: {}\n", against.join(" · "))
    });
    // a page that takes no part is given its `sources:` first (N7.1, D-126)
    if from_history && !crate::approval::tracked(&tree, page) {
        next = "then: name what it rests on in `sources:` first; a page without them takes no part in verification\n".to_string();
    }
    out.push_str(&next);
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn the_record_carries_the_body_blocks_and_a_revoke_keeps_it() {
        let one = vec!["701b6f9c375e".to_string()];
        let none: Vec<(String, String)> = Vec::new();
        let rec = Record {
            by: "ayse",
            rev: "abc1234",
            blocks: Some(&one),
            sources: &none,
        };
        let text = "---\nid: x\ntype: reference\nverification: unverified\nsources: [a.md]\nupdated: 2026-01-01\n---\nBody.\n";
        let out = write_record(text, Some(&rec), true, Some("2026-09-04")).unwrap();
        assert!(out.contains("verification: verified\nverified_by: ayse\nverified_rev: abc1234\nverified_blocks: [701b6f9c375e]\nsources: [a.md]\nupdated: 2026-09-04\n---\nBody.\n"), "{out}");
        // revoked: unverified, the record kept as the last verification
        let back = write_record(&out, None, true, Some("2026-09-05")).unwrap();
        assert!(back.contains("verification: unverified\nverified_by: ayse\nverified_rev: abc1234\nverified_blocks: [701b6f9c375e]\nsources: [a.md]\nupdated: 2026-09-05\n"), "{back}");
        // verified again: the old record is replaced, never duplicated
        let src = vec![("@up/x".to_string(), "fnv:0123456789abcdef".to_string())];
        let again = Record {
            by: "bora",
            rev: "def5678",
            blocks: Some(&one),
            sources: &src,
        };
        let twice = write_record(&back, Some(&again), true, Some("2026-09-06")).unwrap();
        assert_eq!(twice.matches("verified_by:").count(), 1, "{twice}");
        assert!(
            twice.contains("verified_by: bora\nverified_rev: def5678\n"),
            "{twice}"
        );
        assert!(twice.contains("verified_sources:\n  - source: \"@up/x\"\n    hash: \"fnv:0123456789abcdef\"\nsources: [a.md]\n"), "{twice}");
        let thrice = write_record(&twice, Some(&rec), true, Some("2026-09-07")).unwrap();
        assert!(!thrice.contains("verified_sources"), "{thrice}");
        let never = "---\nid: y\ntype: howto\nupdated: 2026-01-01\n---\nSteps.\n";
        let out = write_record(never, Some(&rec), true, Some("2026-09-04")).unwrap();
        assert!(out.contains("type: howto\nverification: verified\nverified_by: ayse\nverified_rev: abc1234\nverified_blocks: [701b6f9c375e]\nsources: []\nupdated: 2026-09-04\n"), "{out}");
        // a docsys/0.4 tree: the record 0.15 wrote, and a revoke removes it (D-118)
        let old = Record {
            by: "ayse",
            rev: "abc1234",
            blocks: None,
            sources: &none,
        };
        let out = write_record(text, Some(&old), false, Some("2026-09-04")).unwrap();
        assert!(out.contains("verification: verified\nverified_by: ayse\nverified_rev: abc1234\nsources: [a.md]\n"), "{out}");
        let back = write_record(&out, None, false, Some("2026-09-05")).unwrap();
        assert!(
            back.contains("verification: unverified\nsources: [a.md]\n")
                && !back.contains("verified_by"),
            "{back}"
        );
    }

    #[test]
    fn the_block_record_is_one_inline_list_replaced_whole_even_when_reflowed() {
        let two = vec!["941ba81fbfec".to_string(), "28949667d156".to_string()];
        let rec = Record {
            by: "ayse",
            rev: "abc1234",
            blocks: Some(&two),
            sources: &[],
        };
        let text = "---\nid: x\ntype: reference\nverification: unverified\nsources: []\nupdated: 2026-01-01\n---\nBody.\n";
        let out = write_record(text, Some(&rec), true, Some("2026-10-02")).unwrap();
        assert!(out.contains("verified_rev: abc1234\nverified_blocks: [941ba81fbfec, 28949667d156]\nsources: []\n"), "{out}");
        // a formatter reflowed the list (D-002): a new verification replaces it whole
        let reflowed = out.replace(
            "[941ba81fbfec, 28949667d156]",
            "[\n  941ba81fbfec,\n  28949667d156,\n]",
        );
        let one = vec!["701b6f9c375e".to_string()];
        let again = Record {
            blocks: Some(&one),
            ..rec
        };
        let twice = write_record(&reflowed, Some(&again), true, Some("2026-10-03")).unwrap();
        assert_eq!(twice.matches("941ba81fbfec").count(), 0, "{twice}");
        assert!(
            twice.contains(
                "verified_blocks: [701b6f9c375e]\nsources: []\nupdated: 2026-10-03\n---\n"
            ),
            "{twice}"
        );
        // a revoke keeps it as part of the last verification
        let back = write_record(&twice, None, true, Some("2026-10-04")).unwrap();
        assert!(
            back.contains("verification: unverified\nverified_by: ayse\nverified_rev: abc1234\nverified_blocks: [701b6f9c375e]\n"),
            "{back}"
        );
    }
}
