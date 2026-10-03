#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! The journal is history on a docsys/0.5 tree (§10, D-125): `docsys journal`
//! reads it, the commit-msg gate holds what `commit_policy: require` asks of a
//! message, and a range in CI is answered by a `Docs:` line.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-journal-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
}

/// git with this build first on PATH, so the gate the hooks run is this one.
fn git(dir: &Path, args: &[&str]) -> Output {
    let path = format!(
        "{}:{}",
        bin().parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("PATH", path)
        .env_remove("DOCSYS_DISPATCHED")
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .output()
        .unwrap()
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let out = git(dir, args);
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn docsys(dir: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .current_dir(dir)
        .env_remove("DOCSYS_DISPATCHED")
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .output()
        .unwrap()
}

/// An adopted repository, its gate hard, committed.
fn adopted(name: &str, policy: &str) -> PathBuf {
    let repo = tmp(name);
    ok(&repo, &["init", "-q", "-b", "main"]);
    ok(&repo, &["config", "user.email", "t@example.invalid"]);
    ok(&repo, &["config", "user.name", "t"]);
    ok(&repo, &["config", "commit.gpgsign", "false"]);
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    let out = docsys(&repo, &["adopt"]);
    assert!(out.status.success(), "{out:?}");
    let meta = repo.join("docs/.docmeta.yml");
    let text = fs::read_to_string(&meta).unwrap();
    fs::write(
        &meta,
        text.replace("commit_policy: ask", &format!("commit_policy: {policy}")),
    )
    .unwrap();
    ok(&repo, &["add", "-A"]);
    ok(
        &repo,
        &[
            "commit",
            "-qm",
            "adopt docsys",
            "-m",
            "Docs: the tree itself",
        ],
    );
    repo
}

#[test]
fn the_journal_is_read_from_history_newest_first_then_the_frozen_files() {
    let repo = adopted("render", "ask");
    // a code change that needed no page, said in its message
    fs::write(repo.join("main.rs"), "fn main() { run() }\n").unwrap();
    ok(
        &repo,
        &[
            "commit",
            "-qam",
            "run on start",
            "-m",
            "Docs: the entry point only calls run",
        ],
    );
    // a code change that says nothing: no entry
    fs::write(repo.join("main.rs"), "fn main() { run(); }\n").unwrap();
    ok(&repo, &["commit", "-qam", "format"]);
    // a docs change: an entry
    fs::create_dir_all(repo.join("docs/reference")).unwrap();
    fs::write(
        repo.join("docs/reference/run.md"),
        "---\nid: run\ntype: reference\n---\nThis page states what run does; read it before changing it.\n",
    )
    .unwrap();
    ok(&repo, &["add", "-A"]);
    ok(
        &repo,
        &[
            "commit",
            "-qm",
            "the run page",
            "-m",
            "what run does, for the next reader",
        ],
    );
    // a 0.4 tree's journal, frozen by the move
    fs::create_dir_all(repo.join("docs/_archive/journal")).unwrap();
    fs::write(
        repo.join("docs/_archive/journal/journal.md"),
        "# Journal\n\n## 2026-08-01 - before the move\n- written by hand\n",
    )
    .unwrap();
    let out = docsys(&repo, &["journal"]);
    assert!(out.status.success(), "{out:?}");
    let text = String::from_utf8_lossy(&out.stdout);
    let heads: Vec<&str> = text.lines().filter(|l| l.starts_with("## ")).collect();
    let titles: Vec<&str> = heads
        .iter()
        .map(|h| h.split_once(" - ").map_or("", |(_, t)| t))
        .collect();
    assert_eq!(
        titles,
        [
            "the run page",
            "run on start",
            "adopt docsys",
            "before the move"
        ],
        "{text}"
    );
    assert!(text.contains("## 2026-08-01 - before the move"), "{text}");
    assert!(
        !text.contains(" - format"),
        "a silent code change is no entry: {text}"
    );
    assert!(
        text.contains("Docs: the entry point only calls run"),
        "{text}"
    );
    assert!(
        text.contains("<!-- frozen: _archive/journal/journal.md -->"),
        "{text}"
    );
    // since: history's entries from that day, the frozen files left out
    let today = docsys::migrate::today();
    let out = docsys(&repo, &["journal", "--since", &today]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!text.contains("before the move"), "{text}");
    // `journal add` writes nothing: it hands back the message
    let out = docsys(
        &repo,
        &["journal", "add", "Run settled", "--link", "reference/run"],
    );
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "Run settled\n\nDocs: reference/run\n"
    );
    assert_eq!(ok(&repo, &["status", "--porcelain"]), "?? docs/_archive/\n");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn outside_history_the_journal_is_the_frozen_files_and_says_so() {
    let root = tmp("no-repo").join("docs");
    docsys::migrate::init_profile(&root, "en", "project").unwrap();
    let text = docsys::journal::render(None, &root, None);
    assert!(text.contains("history: unknown"), "{text}");
}

#[test]
fn under_require_the_message_gate_wants_why_and_a_closed_item_wants_its_trailer() {
    let repo = adopted("require", "require");
    assert!(
        fs::read_to_string(repo.join(".git/hooks/commit-msg"))
            .unwrap()
            .contains("docsys gate --repo . --root docs --message"),
        "adopt writes the commit-msg half"
    );
    // code alone, and a message that says nothing: refused
    fs::write(repo.join("main.rs"), "fn main() { run() }\n").unwrap();
    ok(&repo, &["add", "main.rs"]);
    let out = git(&repo, &["commit", "-qm", "run on start"]);
    assert!(!out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("add a `Docs: <why>` line"),
        "{out:?}"
    );
    // the same commit saying why lands
    ok(
        &repo,
        &[
            "commit",
            "-qm",
            "run on start",
            "-m",
            "Docs: the entry point only calls run",
        ],
    );
    // a debt closed without its trailer lands, and is said
    let added = docsys(
        &repo,
        &[
            "debt",
            "add",
            "run has no timeout",
            "--deferred",
            "no load",
            "--repay-when",
            "first slow start",
        ],
    );
    assert!(added.status.success(), "{added:?}");
    ok(&repo, &["add", "-A"]);
    ok(&repo, &["commit", "-qm", "a debt"]);
    let closed = docsys(&repo, &["debt", "close", "1", "--note", "bounded"]);
    assert!(closed.status.success(), "{closed:?}");
    ok(&repo, &["add", "-A"]);
    let out = git(&repo, &["commit", "-qm", "repaid"]);
    assert!(out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("carries no `Resolved:` line"),
        "{out:?}"
    );
    // a long message is said too: an entry keeps five lines
    fs::write(repo.join("main.rs"), "fn main() { run(); }\n").unwrap();
    ok(&repo, &["add", "main.rs"]);
    let out = git(
        &repo,
        &[
            "commit",
            "-qm",
            "long",
            "-m",
            "1\n2\n3\n4\n5\n6",
            "-m",
            "Docs: why",
        ],
    );
    assert!(out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("a journal entry keeps 5"),
        "{out:?}"
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_range_is_answered_by_a_docs_line_in_any_of_its_commits() {
    let repo = adopted("range", "ask");
    let base = ok(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    fs::write(repo.join("main.rs"), "fn main() { run() }\n").unwrap();
    ok(&repo, &["commit", "-qam", "run on start"]);
    let range = format!("{base}..HEAD");
    let out = docsys(&repo, &["gate", "--range", &range]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    ok(
        &repo,
        &[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "why run",
            "-m",
            "Docs: the entry point only calls run",
        ],
    );
    let out = docsys(&repo, &["gate", "--range", &range]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let _ = fs::remove_dir_all(&repo);
}
