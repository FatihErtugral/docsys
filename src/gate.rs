//! `docsys gate` — the commit-time question, computed by the binary.
//!
//! Field report: `docsys lint` validates the tree's internal consistency and
//! stays green while the actual violation happens — code changes with no
//! documentation change in the same commit. The only thing computing that
//! invariant was a shell script that turned out to be wired to nothing. The
//! binary computes it now; a hook only relays the answer.

use crate::model::Severity;
use std::path::Path;

pub struct GateOutcome {
    pub lint_errors: usize,
    pub lint_warnings: usize,
    /// Changed files outside the docs root ("code moved").
    pub code: Vec<String>,
    /// Changed files inside the docs root.
    pub docs: usize,
    /// Which change set answered: "staged" normally; "working tree" when
    /// nothing is staged yet (`git commit -a` stages at commit time, after
    /// any pre-tool hook has already run).
    pub scope: &'static str,
    /// Plan files in the change set (`SEED.tsv`, `*.seed.tsv`): a plan is a
    /// conversation's draft, never documentation, and never lands (D-091).
    pub plan_files: Vec<String>,
    /// docsys/0.6: the errors the change's base already carries — named,
    /// not counted in `lint_errors`, never blocking (D-136).
    pub inherited: usize,
    /// Why the errors could not be compared with the base, when they could
    /// not: every error then blocks.
    pub unscoped: Option<String>,
}

fn is_plan_file(path: &str) -> bool {
    let base = path.rsplit('/').next().unwrap_or(path);
    base == "SEED.tsv" || base.ends_with(".seed.tsv")
}

pub fn run(repo: &Path, root: &Path) -> Result<(GateOutcome, crate::checks::Report), String> {
    run_scoped(repo, root, None, false)
}

/// The question for a call that runs `git add` before it commits: with
/// nothing staged yet, the working tree's untracked files are changes that
/// `git add` may take into the commit (D-040).
pub fn run_adding(
    repo: &Path,
    root: &Path,
) -> Result<(GateOutcome, crate::checks::Report), String> {
    run_scoped(repo, root, None, true)
}

/// The same question over a commit range (`origin/main...HEAD`): what CI asks
/// of a pull request. No marker, no asking once — the answer is the answer.
pub fn run_range(
    repo: &Path,
    root: &Path,
    range: &str,
) -> Result<(GateOutcome, crate::checks::Report), String> {
    run_scoped(repo, root, Some(range), false)
}

fn run_scoped(
    repo: &Path,
    root: &Path,
    range: Option<&str>,
    untracked: bool,
) -> Result<(GateOutcome, crate::checks::Report), String> {
    let era = crate::era::Era::at(root);
    let base = era
        .scoped_gate()
        .then(|| match range {
            Some(r) => merge_base(repo, r),
            None => commit_of(repo, "HEAD"),
        })
        .flatten();
    let (mut report, _) =
        crate::fresh::with_scope_base(base.as_deref(), || crate::lint_in(root, Some(repo)));
    let mut inherited = 0usize;
    let mut unscoped = None;
    if era.scoped_gate() {
        // refs is part of the verdict, so one comparison covers both (D-136)
        if let Ok(tree) = crate::tree::DocTree::load(root) {
            report
                .findings
                .extend(crate::refs::run(repo, &tree).findings);
        }
        let errors = report
            .findings
            .iter()
            .any(|f| f.severity == Severity::Error);
        if errors {
            match scope(repo, root, range, base.as_deref()) {
                Ok(Some((short, keys))) => {
                    for f in report.findings.iter_mut() {
                        if f.severity == Severity::Error && keys.contains(&key(repo, era, f)) {
                            f.severity = Severity::Warn;
                            f.message = crate::say::gate_inherited(&short, &f.message);
                            inherited += 1;
                        }
                    }
                }
                Ok(None) => {}
                Err(why) => unscoped = Some(why),
            }
        }
    }
    // a `git:` source a pull request adds that only its branch holds: a
    // squash merge drops it (R-059, a warning on every tree)
    if let Some(base) = range.and_then(|r| merge_base(repo, r)) {
        report
            .findings
            .extend(crate::locator::branch_only_citations(repo, root, &base));
    }
    let lint_errors = report
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let lint_warnings = report.findings.len() - lint_errors - inherited;
    let repo_canon = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let root_canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let prefix = root_canon
        .strip_prefix(&repo_canon)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let changed = |args: &[&str]| -> Vec<String> {
        // core.quotePath=false: a non-ASCII page name arrives as itself, not
        // as "docs/g\303\274..." — which no docs-root prefix would match, so a
        // real docs change read as a code change (found with a Turkish name).
        crate::git::cmd(repo)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    let (files, scope) = match range {
        Some(r) => (changed(&["diff", "--name-only", r]), "range"),
        None => {
            let staged = changed(&["diff", "--cached", "--name-only"]);
            if staged.is_empty() {
                let mut files = changed(&["diff", "--name-only"]);
                if untracked {
                    files.extend(changed(&["ls-files", "--others", "--exclude-standard"]));
                }
                (files, "working tree")
            } else {
                (staged, "staged")
            }
        }
    };
    let mut code = Vec::new();
    let mut docs = 0usize;
    let plan_files: Vec<String> = files.iter().filter(|f| is_plan_file(f)).cloned().collect();
    // a knowledge base that is its own repository: its layers are the docs,
    // and what else the repository holds is still code (D-132)
    let base_at_top = prefix.is_empty()
        && crate::tree::docmeta_value(root, "profile").as_deref() == Some("knowledge-base");
    let base_layer = |f: &str| {
        ["wiki/", "raw/", ".federation/"]
            .iter()
            .any(|d| f.starts_with(d))
            || [".docmeta.yml", ".docsys-version", "AGENTS.md"].contains(&f)
    };
    for f in &files {
        let inside = (!prefix.is_empty() && (f == &prefix || f.starts_with(&format!("{prefix}/"))))
            || (base_at_top && base_layer(f));
        if inside {
            docs += 1;
        } else {
            code.push(f.clone());
        }
    }
    Ok((
        GateOutcome {
            lint_errors,
            lint_warnings,
            code,
            docs,
            scope,
            plan_files,
            inherited,
            unscoped,
        },
        report,
    ))
}

/// The full id of a commit `rev` names; `None` when there is none (a
/// repository with no commit yet).
fn commit_of(repo: &Path, rev: &str) -> Option<String> {
    crate::git::cmd(repo)
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// The commit a range's change is measured from: `a...b` from their merge
/// base, `a..b` from `a`.
fn merge_base(repo: &Path, range: &str) -> Option<String> {
    if let Some((a, b)) = range.split_once("...") {
        let b = if b.is_empty() { "HEAD" } else { b };
        let a = if a.is_empty() { "HEAD" } else { a };
        return crate::git::cmd(repo)
            .args(["merge-base", a, b])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty());
    }
    let (a, _) = range.split_once("..")?;
    commit_of(repo, if a.is_empty() { "HEAD" } else { a })
}

/// A finding's identity across two trees (R-158): rule, file and subject,
/// and for a pin the region's text as a hash, so a change that moves a
/// region already stale owns the finding (D-136).
type Key = (String, String, String, Option<String>);

fn key(repo: &Path, era: crate::era::Era, f: &crate::model::Finding) -> Key {
    let region = (f.rule.0 == "R-111" || f.rule.0 == "R-114")
        .then(|| crate::fresh::region_of_label(repo, &f.subject, era))
        .flatten();
    (
        f.rule.0.to_string(),
        f.file.clone(),
        f.subject.clone(),
        region,
    )
}

/// The error keys the change's base carries, with the base's short id; `Ok(None)`
/// when there is no base (a first commit: every error is the change's);
/// `Err` naming why the base could not be read — every error then blocks.
fn scope(
    repo: &Path,
    root: &Path,
    range: Option<&str>,
    base: Option<&str>,
) -> Result<Option<(String, std::collections::BTreeSet<Key>)>, String> {
    let shallow = crate::git::cmd(repo)
        .args(["rev-parse", "--is-shallow-repository"])
        .output()
        .ok()
        .is_some_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "true");
    if shallow {
        return Err(crate::say::GATE_SHALLOW.to_string());
    }
    let Some(base) = base else {
        return match range {
            Some(r) => Err(crate::say::gate_no_base(r)),
            None => Ok(None),
        };
    };
    let short: String = base.chars().take(8).collect();
    base_errors(repo, root, base)
        .map(|keys| Some((short, keys)))
        .ok_or_else(|| crate::say::gate_no_base(base))
}

/// The error keys of lint and refs at commit `rev`, read in a temporary
/// worktree by this same binary in a process of its own: a git hook exports
/// the index of the commit being made, and lint runs `git status`, which must
/// never read or refresh that index from another work tree.
fn base_errors(repo: &Path, root: &Path, rev: &str) -> Option<std::collections::BTreeSet<Key>> {
    let common = crate::git::cmd(repo)
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())?;
    let dir = std::path::PathBuf::from(common).join(format!("docsys-base-{}", std::process::id()));
    let added = crate::git::foreign(Some(repo))
        .args(["worktree", "add", "-q", "--detach"])
        .arg(&dir)
        .arg(rev)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !added {
        return None;
    }
    let rel = crate::fresh::root_rel(repo, root);
    let base_root = if rel.is_empty() {
        dir.clone()
    } else {
        dir.join(&rel)
    };
    let keys = errors_at(&dir, &base_root);
    let removed = crate::git::foreign(Some(repo))
        .args(["worktree", "remove", "--force"])
        .arg(&dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !removed {
        let _ = std::fs::remove_dir_all(&dir);
        let _ = crate::git::foreign(Some(repo))
            .args(["worktree", "prune"])
            .status();
    }
    keys
}

fn errors_at(dir: &Path, root: &Path) -> Option<std::collections::BTreeSet<Key>> {
    let exe = std::env::current_exe().ok()?;
    let era = crate::era::Era::at(root);
    let mut keys = std::collections::BTreeSet::new();
    for command in ["lint", "refs"] {
        let mut c = std::process::Command::new(&exe);
        c.arg(command)
            .arg("--json")
            .arg("--root")
            .arg(root)
            .arg("--repo")
            .arg(dir)
            .current_dir(dir)
            .env(crate::dispatch::GUARD, "1")
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        for var in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_COMMON_DIR",
            "GIT_PREFIX",
        ] {
            c.env_remove(var);
        }
        let out = c.output().ok()?;
        // 0 and 1 are verdicts; anything else could not evaluate the base
        if !matches!(out.status.code(), Some(0 | 1)) {
            return None;
        }
        let json = crate::hook::parse_json(&String::from_utf8_lossy(&out.stdout))?;
        let crate::hook::Json::Obj(fields) = json else {
            return None;
        };
        let Some((_, crate::hook::Json::Arr(findings))) =
            fields.into_iter().find(|(k, _)| k == "findings")
        else {
            return None;
        };
        for f in &findings {
            if f.string_at(&["severity"]) != Some(crate::model::Severity::Error.tag()) {
                continue;
            }
            let field = |k: &str| f.string_at(&[k]).unwrap_or("").to_string();
            let (rule, subject) = (field("rule"), field("subject"));
            let region = (rule == "R-111" || rule == "R-114")
                .then(|| crate::fresh::region_of_label(dir, &subject, era))
                .flatten();
            keys.insert((rule, field("file"), subject, region));
        }
    }
    Some(keys)
}

/// Whether a commit in `range` carries the `Docs:` trailer: on docsys/0.5
/// it records code that needed no page (R-209, D-125).
pub fn range_has_docs(repo: &Path, range: &str) -> bool {
    crate::git::cmd(repo)
        .args(["log", "--format=%B%x1e", range])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split('\u{1e}')
                .any(|m| crate::journal::has_trailer(m.trim_start(), crate::journal::DOCS))
        })
}

/// What the `commit-msg` gate says of a message about to land (D-125).
#[derive(Debug, Default)]
pub struct MessageVerdict {
    /// under `commit_policy: require`, code with no documentation and no
    /// `Docs:` line is refused
    pub refusal: Option<String>,
    /// said, never blocking
    pub reports: Vec<String>,
}

/// A line of a commit message that is a trailer: `Key: value`.
fn is_trailer(line: &str) -> bool {
    line.split_once(": ").is_some_and(|(k, _)| {
        !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    })
}

/// The open items the staged change takes out of a list's topic files, one
/// by one; a line that moves to another topic file, its tag changed with it,
/// is not taken out.
fn items_removed(repo: &Path, dir: &str) -> usize {
    let Some(diff) = crate::git::cmd(repo)
        .args([
            "diff",
            "--cached",
            "-U0",
            "--no-renames",
            "--no-color",
            "--",
            dir,
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
    else {
        return 0;
    };
    let mut added: Vec<String> = diff
        .lines()
        .filter_map(|l| l.strip_prefix('+'))
        .filter(|l| l.starts_with("- [ ] "))
        .map(crate::items::untagged)
        .collect();
    diff.lines()
        .filter_map(|l| l.strip_prefix('-'))
        .filter(|l| l.starts_with("- [ ] "))
        .map(crate::items::untagged)
        .filter(|gone| match added.iter().position(|a| a == gone) {
            Some(i) => {
                added.swap_remove(i);
                false
            }
            None => true,
        })
        .count()
}

/// The `commit-msg` gate on a docsys/0.5 tree: the staged change set read
/// with the message that will carry it.
/// It acts under `commit_policy: require` only: a team's message is its own
/// convention everywhere else.
pub fn message(repo: &Path, root: &Path, text: &str) -> MessageVerdict {
    let mut v = MessageVerdict::default();
    if !crate::era::Era::at(root).journal_from_history()
        || crate::hook::commit_policy(root) != crate::hook::CommitPolicy::Require
    {
        return v;
    }
    let prefix = crate::fresh::root_rel(repo, root);
    let inside = |f: &str| prefix.is_empty() || f == prefix || f.starts_with(&format!("{prefix}/"));
    let staged = |filter: &str| -> Vec<String> {
        crate::git::cmd(repo)
            .args(["diff", "--cached", "--name-only", filter])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    let files = staged("--diff-filter=ACDMRT");
    let docs = files.iter().filter(|f| inside(f)).count();
    let code: Vec<&String> = files.iter().filter(|f| !inside(f)).collect();
    let message: String = text
        .lines()
        .filter(|l| !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let documented = crate::journal::has_trailer(&message, crate::journal::DOCS);
    if !code.is_empty() && docs == 0 && !documented {
        let shown: Vec<&str> = code.iter().take(5).map(|s| s.as_str()).collect();
        v.refusal = Some(crate::say::message_refusal(&shown));
    }
    // a closed item's record is its commit's trailer (R-108)
    let base = if prefix.is_empty() {
        String::new()
    } else {
        format!("{prefix}/")
    };
    for (list, kb) in [
        (crate::items::List::Debt, false),
        (crate::items::List::Questions, false),
        (crate::items::List::Questions, true),
    ] {
        let dir = format!("{base}{}/", list.dir(kb));
        let removed = items_removed(repo, &dir);
        if removed > 0 && !crate::journal::has_trailer(&message, list.trailer()) {
            v.reports.push(crate::say::message_trailer(
                removed,
                list.dir(kb),
                list.trailer(),
            ));
        }
    }
    // the `Docs:` entry keeps its budget (R-101); the rest of a body is the
    // team's own convention
    let entry = docs_entry_lines(&message);
    if entry > 0 {
        let max: usize = crate::tree::DocTree::load(root)
            .ok()
            .and_then(|t| {
                t.docmeta_str("journal_entry_max_lines")
                    .and_then(|s| s.trim().parse().ok())
            })
            .unwrap_or(5);
        if entry > max {
            v.reports.push(crate::say::message_budget(entry, max));
        }
    }
    v
}

/// The lines of a message's `Docs:` entry: its line and the ones that
/// continue it, up to a blank line or the next trailer; 0 without one.
fn docs_entry_lines(message: &str) -> usize {
    let prefix = format!("{}:", crate::journal::DOCS);
    let mut lines = message
        .lines()
        .skip_while(|l| !l.trim_start().starts_with(&prefix));
    if lines.next().is_none() {
        return 0;
    }
    1 + lines
        .take_while(|l| !l.trim().is_empty() && !is_trailer(l))
        .count()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) {
        assert!(Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap()
            .success());
    }

    #[test]
    fn a_non_ascii_docs_page_is_a_docs_change_and_a_non_ascii_source_is_code() {
        let repo = std::env::temp_dir().join(format!("docsys-gate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q"]);
        git(&repo, &["config", "user.email", "t@example.invalid"]);
        git(&repo, &["config", "user.name", "t"]);
        // quotePath ON explicitly — the default the field hit
        git(&repo, &["config", "core.quotePath", "true"]);
        let docs = repo.join("docs");
        crate::migrate::init_profile(&docs, "tr", "project").unwrap();
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "init"]);
        fs::create_dir_all(docs.join("reference")).unwrap();
        fs::write(docs.join("reference/kılavuz.md"), "---\nid: kilavuz\ntype: reference\nupdated: 2026-08-26\n---\nThis page describes the guide; read it when the guide changes.\n").unwrap();
        fs::write(repo.join("çekirdek.rs"), "fn main() {}\n").unwrap();
        git(&repo, &["add", "-A"]);
        let (g, _) = super::run(&repo, &docs).unwrap();
        assert_eq!(g.scope, "staged");
        assert_eq!(g.docs, 1, "the Turkish-named page must count as docs");
        assert_eq!(
            g.code,
            vec!["çekirdek.rs".to_string()],
            "unquoted, as itself"
        );
        // nothing staged: the working tree answers, same rules
        git(&repo, &["commit", "-q", "-m", "both"]);
        fs::write(repo.join("çekirdek.rs"), "fn main() { run() }\n").unwrap();
        let (g, _) = super::run(&repo, &docs).unwrap();
        assert_eq!(g.scope, "working tree");
        assert_eq!((g.docs, g.code.len()), (0, 1));
        let _ = fs::remove_dir_all(&repo);
    }
}
