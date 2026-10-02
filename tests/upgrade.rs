#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! `docsys upgrade` (D-117). The conformance case R-179 asks for: a tree as
//! docsys 0.15 left it, moved to docsys/0.5, compared file by file against the
//! expected tree; a second run changes nothing. Beside it, what the move
//! refuses and what it only shows.
//!
//! `DOCSYS_BLESS=1 cargo test --test upgrade` writes the expected files from
//! the current build — for a maintainer to read before they are committed.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const CASE: &str = "corpus/upgrades/0.4-to-0.5";

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-upgrade-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
}

/// PATH with this build first: the git gate a commit runs is this docsys.
fn path() -> String {
    format!(
        "{}:{}",
        bin().parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

fn git_at(dir: &Path, day: Option<&str>, args: &[&str]) -> String {
    let mut c = Command::new("git");
    c.args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("PATH", path())
        .current_dir(dir);
    if let Some(day) = day {
        let at = format!("{day}T12:00:00+00:00");
        c.env("GIT_AUTHOR_DATE", &at).env("GIT_COMMITTER_DATE", &at);
    }
    let out = c.output().unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn git(dir: &Path, args: &[&str]) -> String {
    git_at(dir, None, args)
}

fn docsys(dir: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .env("PATH", path())
        .current_dir(dir)
        .output()
        .unwrap()
}

/// A stored top-level `dot-claude` is the repository's `.claude`: the case
/// keeps its agent layer under another name, so an agent working in this
/// repository never loads the fixture's skills or hooks.
fn real_name(rel: &str) -> String {
    match rel.strip_prefix("dot-") {
        Some(rest) => format!(".{rest}"),
        None => rel.to_string(),
    }
}

fn stored_name(rel: &str) -> String {
    match rel.strip_prefix('.') {
        Some(rest) if !rel.starts_with(".git/") => format!("dot-{rest}"),
        _ => rel.to_string(),
    }
}

fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let rel = p
            .strip_prefix(base)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if rel == ".git" {
            continue;
        }
        if p.is_dir() {
            walk(&p, base, out);
        } else {
            out.push(rel);
        }
    }
}

/// Every file under `dir` by its repository path, `rev` written back as the
/// placeholder.
fn snapshot(dir: &Path, rev: &str, stored: bool) -> BTreeMap<String, String> {
    let mut files = Vec::new();
    walk(dir, dir, &mut files);
    files
        .into_iter()
        .map(|rel| {
            let text = fs::read_to_string(dir.join(&rel)).unwrap();
            let key = if stored { real_name(&rel) } else { rel };
            (key, text.replace(rev, "@REV@"))
        })
        .collect()
}

fn copy_in(from: &Path, to: &Path, rev: Option<&str>) {
    let mut files = Vec::new();
    walk(from, from, &mut files);
    for rel in files {
        let target = to.join(real_name(&rel));
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let text = fs::read_to_string(from.join(&rel)).unwrap();
        fs::write(
            &target,
            rev.map_or(text.clone(), |r| text.replace("@REV@", r)),
        )
        .unwrap();
    }
}

/// The case's history: the tree committed as 0.15 left it, its pages verified
/// at that revision; then a second commit moves one page's body and the code
/// under one pin; the 0.15 gate is this clone's pre-commit hook.
fn build(name: &str) -> (PathBuf, String) {
    let case = Path::new(env!("CARGO_MANIFEST_DIR")).join(CASE);
    let repo = tmp(name);
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    copy_in(&case.join("before"), &repo, None);
    git(&repo, &["add", "-A"]);
    git_at(
        &repo,
        Some("2026-09-01"),
        &["commit", "-qm", "the tree as 0.15 left it"],
    );
    let rev = git(&repo, &["rev-parse", "--short=7", "HEAD"]);
    let mut files = Vec::new();
    walk(&repo, &repo, &mut files);
    for rel in files {
        let p = repo.join(&rel);
        let text = fs::read_to_string(&p).unwrap();
        if text.contains("@REV@") {
            fs::write(&p, text.replace("@REV@", &rev)).unwrap();
        }
    }
    copy_in(&case.join("edits"), &repo, Some(&rev));
    git(&repo, &["add", "-A"]);
    git_at(
        &repo,
        Some("2026-09-02"),
        &["commit", "-qm", "expiry: revocation"],
    );
    let hook = repo.join(".git/hooks/pre-commit");
    fs::copy(case.join("hooks/pre-commit"), &hook).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    (repo, rev)
}

fn bless() -> bool {
    std::env::var_os("DOCSYS_BLESS").is_some()
}

fn expect_text(rel: &str, got: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(CASE).join(rel);
    if bless() {
        fs::write(&path, got).unwrap();
        return;
    }
    let want = fs::read_to_string(&path).unwrap_or_default();
    assert!(
        want == got,
        "{rel} differs\n{}",
        docsys::diff::unified(&want, got, rel, "this build", 3)
    );
}

fn findings(root: &Path, repo: &Path) -> String {
    let (r, outcome) = docsys::lint_in(root, Some(repo));
    let mut lines: Vec<String> = r
        .findings
        .iter()
        .map(|f| {
            format!(
                "{}\t{}\t{}\t{}",
                f.severity.tag(),
                f.rule,
                f.file,
                f.subject
            )
        })
        .collect();
    lines.sort();
    lines.dedup();
    let exit = match outcome {
        docsys::Outcome::Clean => 0,
        docsys::Outcome::Errors => 1,
        docsys::Outcome::Config => 2,
    };
    format!("EXIT\t{exit}\n{}", lines.join("\n") + "\n")
}

/// R-179: the 0.4 tree, moved, is the expected 0.5 tree — file for file.
#[test]
fn a_0_4_tree_moves_to_the_expected_0_5_tree_and_a_second_run_changes_nothing() {
    let (repo, rev) = build("conformance");
    let before = snapshot(&repo, &rev, false);

    // the plan writes nothing
    let out = docsys(&repo, &["upgrade"]);
    assert!(out.status.success(), "{out:?}");
    expect_text(
        "plan.txt",
        &String::from_utf8_lossy(&out.stdout).replace(&rev, "@REV@"),
    );
    assert_eq!(snapshot(&repo, &rev, false), before, "the plan wrote");

    // the move is one commit, and leaves nothing behind
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stdout)
            .contains("committed: docsys: upgrade the tree to docsys/0.5"),
        "{out:?}"
    );
    assert_eq!(
        git(&repo, &["rev-list", "--count", &format!("{head}..HEAD")]),
        "1"
    );
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");

    let after_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CASE)
        .join("after");
    let got = snapshot(&repo, &rev, false);
    if bless() {
        let _ = fs::remove_dir_all(&after_dir);
        for (rel, text) in &got {
            let p = after_dir.join(stored_name(rel));
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        }
    } else {
        let want = snapshot(&after_dir, "@REV@", true);
        let names = |m: &BTreeMap<String, String>| m.keys().cloned().collect::<Vec<_>>();
        assert_eq!(names(&got), names(&want), "the files differ");
        for (rel, text) in &want {
            let g = got.get(rel).unwrap();
            assert!(
                g == text,
                "{rel} differs\n{}",
                docsys::diff::unified(text, g, rel, "this build", 3)
            );
        }
    }
    expect_text(
        "hooks/pre-commit.after",
        &fs::read_to_string(repo.join(".git/hooks/pre-commit")).unwrap(),
    );
    expect_text("expected.tsv", &findings(&repo.join("docs"), &repo));

    // a second run: nothing automatic, nothing written, no commit
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.lines().any(|l| l.starts_with("auto ")), "{stdout}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(snapshot(&repo, &rev, false), got, "the second run wrote");
    let _ = fs::remove_dir_all(&repo);
}

/// R-097: the move is a commit of its own, so it does not start on top of
/// someone's unfinished work.
#[test]
fn an_upgrade_refuses_a_dirty_working_tree() {
    let (repo, rev) = build("dirty");
    let p = repo.join("docs/index.md");
    fs::write(
        &p,
        fs::read_to_string(&p).unwrap() + "\nA line in progress.\n",
    )
    .unwrap();
    let before = snapshot(&repo, &rev, false);
    let out = docsys(&repo, &["upgrade", "--apply"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("uncommitted changes"),
        "{out:?}"
    );
    assert_eq!(snapshot(&repo, &rev, false), before);
    let _ = fs::remove_dir_all(&repo);
}

/// A workflow its owner edited — a self-hosted runner, their own install —
/// is never rewritten: the upgrade shows the diff and leaves the file alone.
#[test]
fn an_owners_workflow_is_shown_as_a_diff_and_never_rewritten() {
    let (repo, rev) = build("owned-ci");
    let wf = repo.join(".github/workflows/docsys.yml");
    let owned = fs::read_to_string(&wf)
        .unwrap()
        .replace("runs-on: ubuntu-latest", "runs-on: [self-hosted, linux]")
        .replace(
            "      - run: cargo install docsys\n",
            "      - run: ./tools/install-docsys.sh\n",
        );
    fs::write(&wf, &owned).unwrap();
    git(&repo, &["commit", "-qam", "ci: our runner"]);
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("manual  ci-workflow        .github/workflows/docsys.yml"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\n# .github/workflows/docsys.yml\n"),
        "{stdout}"
    );
    assert_eq!(fs::read_to_string(&wf).unwrap(), owned);
    assert!(!git(&repo, &["show", "--stat", "HEAD"]).contains("docsys.yml"));
    let _ = rev;
    let _ = fs::remove_dir_all(&repo);
}

/// R-171: a tree that has not moved hears it in one line, naming the command;
/// a tree that has moved, and the upgrade itself, hear nothing.
#[test]
fn a_0_4_tree_is_told_once_how_it_moves() {
    let (repo, _) = build("notice");
    let notice = |args: &[&str]| -> Vec<String> {
        let out = docsys(&repo, args);
        String::from_utf8_lossy(&out.stderr)
            .lines()
            .filter(|l| l.starts_with("docsys: this tree declares"))
            .map(str::to_string)
            .collect()
    };
    assert_eq!(
        notice(&["lint"]),
        vec![format!(
            "docsys: this tree declares docsys/0.4 and is served by its rules; `docsys upgrade` moves it to docsys/{} when the repository is ready",
            docsys::rules::spec_version()
        )]
    );
    assert_eq!(notice(&["upgrade"]), Vec::<String>::new());
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(notice(&["lint"]), Vec::<String>::new());
    let _ = fs::remove_dir_all(&repo);
}
