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

/// The agent's habitual shape: staging happens INSIDE the command, i.e. after
/// this PreToolUse hook has already run against an empty index.
fn add_and_commit_payload() -> &'static str {
    r#"{"tool_name":"Bash","tool_input":{"command":"git add -u src && git commit -q -m x"}}"#
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

/// Run the installed Stop hook; returns its stderr (it never blocks).
fn run_stop(repo: &Path) -> (i32, String) {
    let out = Command::new("bash")
        .arg(repo.join(".claude/hooks/stop-docs-reminder.sh"))
        .current_dir(repo)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Give the repo an upstream so `@{u}..HEAD` resolves, with everything pushed.
fn with_upstream(repo: &Path) {
    let remote = repo.parent().unwrap().join(format!(
        "{}.remote.git",
        repo.file_name().unwrap().to_string_lossy()
    ));
    let _ = fs::remove_dir_all(&remote);
    git(repo, &["init", "-q", "--bare", remote.to_str().unwrap()]);
    git(repo, &["remote", "add", "origin", remote.to_str().unwrap()]);
    git(repo, &["push", "-q", "-u", "origin", "HEAD"]);
}

#[test]
fn stop_reminder_is_silent_on_a_clean_pushed_repo() {
    let repo = build_repo("stop-clean");
    with_upstream(&repo);
    let (code, err) = run_stop(&repo);
    assert_eq!(code, 0);
    assert!(err.is_empty(), "{err}");
}

#[test]
fn stop_reminder_sees_code_committed_but_not_pushed() {
    let repo = build_repo("stop-ahead");
    with_upstream(&repo);
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "main.rs"]);
    git(&repo, &["commit", "-q", "-m", "code only"]);
    // tree is clean — the old reminder saw nothing here
    let (code, err) = run_stop(&repo);
    assert_eq!(code, 0, "warns, never blocks");
    assert!(err.contains("no documentation"), "{err}");
    assert!(err.contains("not yet pushed"), "{err}");
    // a docs commit in the same unpushed range answers it
    fs::write(
        repo.join("docs/work/journal.md"),
        "# Journal\n\n## 2026-08-16 - initialized\n- documentation tree created\n\n\
         ## 2026-08-16 - main added\n- entry point landed\n",
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "docs"]);
    let (_, err) = run_stop(&repo);
    assert!(err.is_empty(), "{err}");
}

#[test]
fn stop_reminder_reads_the_new_path_of_a_rename() {
    let repo = build_repo("stop-rename");
    // a docs page renamed to a code path: the old `awk '{print $2}'` read the
    // OLD side ("docs/…") and counted a code move as a docs change
    git(&repo, &["mv", "docs/index.md", "notes.txt"]);
    let (_, err) = run_stop(&repo);
    assert!(err.contains("no documentation"), "{err}");
    // and the reverse: code renamed INTO docs is a docs change, not a code one
    let repo = build_repo("stop-rename-in");
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "main.rs"]);
    git(&repo, &["commit", "-q", "-m", "code"]);
    git(&repo, &["mv", "main.rs", "docs/main.rs"]);
    let (_, err) = run_stop(&repo);
    assert!(err.is_empty(), "{err}");
}

#[test]
fn ask_once_holds_when_staging_happens_inside_the_command() {
    // Live sequence that broke: ask → a pass that committed nothing (bare
    // `git commit`, index empty) consumed the marker → the real attempt asked
    // again. The marker now lives until HEAD moves.
    let repo = build_repo("askonce-inline");
    // tracked files, modified in place — the live shape (an untracked file
    // is invisible to the working-tree fallback, which reads `git diff`)
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    fs::write(repo.join("lib.rs"), "pub fn f() {}\n").unwrap();
    fs::write(repo.join("more.rs"), "pub fn g() {}\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "code lands"]);
    fs::write(repo.join("main.rs"), "fn main() { run() }\n").unwrap();
    let (code, err) = run_hook(&repo, add_and_commit_payload(), &[]);
    assert_eq!(code, 2, "{err}");
    // a bare retry that dropped the add is stopped once more (D-049) — and
    // that stop must not consume the answer to the original question
    let (code, err) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("did your `git add` run"), "{err}");
    let (code, err) = run_hook(&repo, add_and_commit_payload(), &[]);
    assert_eq!(
        code, 0,
        "asked a second time for the same change set: {err}"
    );
    // a DIFFERENT unstaged change set under the same HEAD is a new question
    fs::write(repo.join("lib.rs"), "pub fn f() -> u8 { 1 }\n").unwrap();
    let (code, _) = run_hook(&repo, add_and_commit_payload(), &[]);
    assert_eq!(code, 2);
    // the commit lands, HEAD moves: the next change is asked afresh, and the
    // old markers are gone
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "code"]);
    fs::write(repo.join("more.rs"), "pub fn g() -> u8 { 2 }\n").unwrap();
    let (code, _) = run_hook(&repo, add_and_commit_payload(), &[]);
    assert_eq!(code, 2);
    assert_eq!(
        fs::read_dir(repo.join(".git/docsys-gate")).unwrap().count(),
        1
    );
}

/// A valid reference page under a non-ASCII name — the only thing the test
/// needs from it is to be a docs change that lint accepts.
fn non_ascii_page(repo: &Path) {
    fs::create_dir_all(repo.join("docs/reference")).unwrap();
    fs::write(
        repo.join("docs/reference/kılavuz.md"),
        "---\nid: kilavuz\ntype: reference\nupdated: 2026-08-26\n---\n\
         This page describes the guide; read it when the guide changes.\n",
    )
    .unwrap();
    let index = repo.join("docs/index.md");
    let mut text = fs::read_to_string(&index).unwrap();
    text.push_str("- [[reference/kılavuz|Guide]] -- The guide.\n");
    fs::write(&index, text).unwrap();
}

#[test]
fn a_non_ascii_docs_page_answers_the_question() {
    // git quotes such a path as "docs/k\304\261lavuz.md" unless told not to,
    // and a quoted path matches no docs-root prefix: the docs change read as
    // a code change and the gate asked anyway.
    let repo = build_repo("nonascii");
    fs::write(repo.join("çekirdek.rs"), "fn main() {}\n").unwrap();
    non_ascii_page(&repo);
    git(&repo, &["add", "-A"]);
    let (code, err) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("GATE "), "{err}");
    // the stop reminder reads the same paths
    let (_, err) = run_stop(&repo);
    assert!(err.is_empty(), "{err}");
    // and a non-ASCII CODE path alone still speaks
    git(&repo, &["commit", "-q", "-m", "both"]);
    fs::write(repo.join("çekirdek.rs"), "fn main() { run() }\n").unwrap();
    let (_, err) = run_stop(&repo);
    assert!(err.contains("no documentation"), "{err}");
}

#[test]
fn an_escaped_quote_before_git_commit_is_still_gated() {
    let repo = build_repo("escaped-quote");
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "main.rs"]);
    let payload =
        r#"{"tool_name":"Bash","tool_input":{"command":"printf \"x\" > y && git commit -m z"}}"#;
    let (code, err) = run_hook(&repo, payload, &[]);
    assert_eq!(
        code, 2,
        "the gate skipped a commit hidden behind an escaped quote: {err}"
    );
}

#[test]
fn a_retry_that_dropped_its_git_add_is_stopped_once() {
    // Live sequence: `git add -A && git commit` blocked whole (the add never
    // ran); the agent retried a bare `git commit`; what landed was the stale
    // index — six deletions under a message describing all the work.
    let repo = build_repo("dropped-add");
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "code lands"]);
    fs::write(repo.join("main.rs"), "fn main() { run() }\n").unwrap();
    let (code, err) = run_hook(&repo, add_and_commit_payload(), &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("whole Bash call was blocked"), "{err}");
    // bare retry, tree still unstaged: stopped once, with the question
    let (code, err) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("did your `git add` run"), "{err}");
    // the same bare retry again: asked once, now proceeds
    let (code, err) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 0, "{err}");
    // the right retry — the original command, add included — passes at once
    let repo = build_repo("dropped-add-right");
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "code lands"]);
    fs::write(repo.join("main.rs"), "fn main() { run() }\n").unwrap();
    let (code, _) = run_hook(&repo, add_and_commit_payload(), &[]);
    assert_eq!(code, 2);
    let (code, err) = run_hook(&repo, add_and_commit_payload(), &[]);
    assert_eq!(code, 0, "{err}");
    // and a bare commit that was bare from the start is never second-guessed
    let repo = build_repo("bare-from-start");
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "main.rs"]);
    fs::write(repo.join("main.rs"), "fn main() { later() }\n").unwrap(); // unstaged on top
    let (code, _) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 2);
    let (code, err) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 0, "{err}");
}

#[test]
fn git_commit_inside_a_heredoc_body_is_not_a_commit() {
    let repo = build_repo("heredoc");
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "main.rs"]);
    // a rule text written through a heredoc quotes the words — no commit here
    let payload = r#"{"tool_name":"Bash","tool_input":{"command":"cat > rules.md <<'EOF'\n# Rules\n\nRun `git commit` only after docs.\nEOF\necho done"}}"#;
    let (code, err) = run_hook(&repo, payload, &[]);
    assert_eq!(code, 0, "gated a call that commits nothing: {err}");
    // the heredoc LINE is a command: a commit fed its message from a heredoc is gated
    let payload = r#"{"tool_name":"Bash","tool_input":{"command":"git commit -q -F - <<'MSG'\nfix: something\n\nbody\nMSG\ngit push"}}"#;
    let (code, err) = run_hook(&repo, payload, &[]);
    assert_eq!(code, 2, "{err}");
    // and a commit AFTER a heredoc on a later line is gated too
    let repo = build_repo("heredoc-after");
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "main.rs"]);
    let payload = r#"{"tool_name":"Bash","tool_input":{"command":"cat > /tmp/m <<'EOF'\nmsg\nEOF\ngit commit -F /tmp/m"}}"#;
    let (code, err) = run_hook(&repo, payload, &[]);
    assert_eq!(code, 2, "{err}");
}
