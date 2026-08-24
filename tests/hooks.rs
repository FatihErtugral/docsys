#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// The hook SCRIPTS executed for real — Rust unit tests cannot catch a payload
// grammar mistake or a wrong exit code in bash, and those are exactly the
// failures the field report found. Unix-only: the scripts are bash.
#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-hooks-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn git(dir: &Path, args: &[&str]) {
    assert!(Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap()
        .success());
}

/// Run the installed PreToolUse hook with a payload, `docsys` on PATH via the
/// test binary Cargo built, and an isolated TMPDIR so ask-once markers cannot
/// leak between tests.
fn run_hook(repo: &Path, payload: &str, extra_env: &[(&str, &str)]) -> (i32, String) {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_docsys"));
    let bin_dir = bin.parent().unwrap();
    let path = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let markers = repo.join(".markers");
    let _ = fs::create_dir_all(&markers);
    let mut cmd = Command::new("bash");
    cmd.arg(repo.join(".claude/hooks/pre-commit-docs.sh"))
        .current_dir(repo)
        .env("PATH", path)
        .env("TMPDIR", &markers);
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    use std::io::Write as _;
    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn commit_payload() -> &'static str {
    r#"{"tool_name":"Bash","tool_input":{"command":"git commit -m x"}}"#
}

fn build_repo(name: &str) -> PathBuf {
    let repo = tmp(name);
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    docsys::migrate::init_profile(&repo.join("docs"), "en", "project").unwrap();
    docsys::agents::install(&repo.join(".claude"), false).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "init"]);
    repo
}

#[test]
fn non_commit_commands_pass_untouched() {
    let repo = build_repo("noncommit");
    let (code, _) = run_hook(
        &repo,
        r#"{"tool_name":"Bash","tool_input":{"command":"cargo test"}}"#,
        &[],
    );
    assert_eq!(code, 0);
}

#[test]
fn lint_errors_block_the_commit_and_reach_stderr() {
    let repo = build_repo("linterr");
    // a dangling wiki-link — the silently-wrong class that blocks
    fs::write(
        repo.join("docs/index.md"),
        "# docs\n\nSee [[reference/ghost|ghost]].\n",
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    let (code, err) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("R-071"), "{err}");
    assert!(err.contains("lint errors block"), "{err}");
}

#[test]
fn code_without_docs_asks_once_then_proceeds() {
    let repo = build_repo("askonce");
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "main.rs"]);
    // first attempt: the question, on stderr, exit 2
    let (code, err) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("GATE "), "{err}");
    assert!(err.contains("asks once"), "{err}");
    // the same commit again: proceeds
    let (code, err) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 0, "{err}");
    // a NEW change set asks again
    fs::write(repo.join("lib.rs"), "pub fn f() {}\n").unwrap();
    git(&repo, &["add", "lib.rs"]);
    let (code, _) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 2);
}

#[test]
fn staged_docs_answer_the_question_silently() {
    let repo = build_repo("answered");
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    fs::write(
        repo.join("docs/work/journal.md"),
        "# Journal\n\n## 2026-08-16 - initialized\n- documentation tree created\n\n\
         ## 2026-08-16 - main added\n- entry point landed\n",
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    let (code, err) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("GATE "), "{err}");
}

#[test]
fn docsys_skip_bypasses_once() {
    let repo = build_repo("skip");
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "main.rs"]);
    let (code, _) = run_hook(&repo, commit_payload(), &[("DOCSYS_SKIP", "1")]);
    assert_eq!(code, 0);
}
