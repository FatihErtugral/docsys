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

/// A docsys/0.4 tree as 0.15.1 kept it: its journal in `work/journal.md`.
fn build_repo04(name: &str) -> PathBuf {
    let repo = build_repo(name);
    let dm = repo.join("docs/.docmeta.yml");
    fs::write(
        &dm,
        fs::read_to_string(&dm)
            .unwrap()
            .replace("spec: docsys/0.5", "spec: docsys/0.4"),
    )
    .unwrap();
    fs::create_dir_all(repo.join("docs/work")).unwrap();
    fs::write(
        repo.join("docs/work/journal.md"),
        "# Journal\n\n## 2026-08-16 - initialized\n- documentation tree created\n",
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "a 0.4 tree"]);
    repo
}

/// A documentation change: a page about the entry point.
fn write_page(repo: &Path) {
    fs::create_dir_all(repo.join("docs/reference")).unwrap();
    fs::write(
        repo.join("docs/reference/entry.md"),
        "---\nid: entry\ntype: reference\n---\nThis page states what the entry point does; read it before changing it.\n",
    )
    .unwrap();
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
    write_page(&repo);
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
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_docsys"));
    let path = format!(
        "{}:{}",
        bin.parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = Command::new("bash")
        .arg(repo.join(".claude/hooks/stop-docs-reminder.sh"))
        .current_dir(repo)
        .env("PATH", path)
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
    write_page(&repo);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "docs"]);
    let (_, err) = run_stop(&repo);
    assert!(err.is_empty(), "{err}");
}

#[test]
fn stop_reminder_asks_for_the_journal_line_when_only_a_draft_moved() {
    let repo = build_repo04("stop-journal");
    with_upstream(&repo);
    // code moved and a research draft moved with it — the old reminder read
    // "documentation changed" and stayed silent; the session's record, the
    // journal line, was never written
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    fs::create_dir_all(repo.join("docs/work/research")).unwrap();
    fs::write(
        repo.join("docs/work/research/x.md"),
        "---\nid: x\nstatus: draft\nupdated: 2026-08-16\n---\n\n## Question\n\n## Tried\n\n## Learned\n\n## Why no decision\n",
    )
    .unwrap();
    let (code, err) = run_stop(&repo);
    assert_eq!(code, 0, "warns, never blocks");
    assert!(err.contains("work/journal.md"), "{err}");
    assert!(err.contains("journal entry"), "{err}");
    // the journal line answers it
    fs::write(
        repo.join("docs/work/journal.md"),
        "# Journal\n\n## 2026-08-16 - main added\n- entry point landed\n\n\
         ## 2026-08-16 - initialized\n- documentation tree created\n",
    )
    .unwrap();
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
    // the stop reminder reads the same paths: the docs page counts as
    // documentation (the journal-line reminder is a different question)
    let (_, err) = run_stop(&repo);
    assert!(!err.contains("no documentation"), "{err}");
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

/// Table: payload → does the gate treat it as a commit? Measured by the ask
/// (exit 2) against a fresh marker dir each time; non-commits exit 0 before
/// the gate runs at all.
#[test]
fn command_matcher_table() {
    let repo = build_repo("matcher-table");
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "main.rs"]);
    let cases: &[(&str, bool)] = &[
        (r#"{"tool_input":{"command":"git commit -m x"}}"#, true),
        (r#"{"tool_input":{"command":"git   commit -m x"}}"#, false), // not our spelling — git itself rejects it
        (
            r#"{"tool_input":{"command":"git add -A && git commit -m x"}}"#,
            true,
        ),
        (
            r#"{"tool_input":{"command":"printf \"x\" > y && git commit -m z"}}"#,
            true,
        ),
        (
            r#"{"tool_input":{"command":"echo 'it''s' && git commit -m \"q \\\"x\\\"\""}}"#,
            true,
        ),
        (
            r#"{"tool_input":{"command":"git commit -q -F - <<'MSG'\nbody\nMSG"}}"#,
            true,
        ),
        (
            r#"{"tool_input":{"command":"cat <<'EOF'\nrun git commit later\nEOF"}}"#,
            false,
        ),
        (
            r#"{"tool_input":{"command":"cat <<EOF\ngit commit\nEOF\ngit commit -m real"}}"#,
            true,
        ),
        (
            r#"{"tool_input":{"command":"cat <<-\tEOF\n\tgit commit\n\tEOF"}}"#,
            false,
        ),
        (
            r#"{"tool_input":{"command":"git status && git log --oneline -3"}}"#,
            false,
        ),
        (
            r#"{"tool_input":{"command":"grep -rn 'git commit' docs/"}}"#,
            true,
        ), // quoted on a command line: cheap false positive, by design
        (
            r#"{"tool_input":{"command":"gitk && git-commit-graph"}}"#,
            false,
        ),
        (
            r#"{"tool_input":{"command":"echo çekirdek && git commit -m \"günlük\""}}"#,
            true,
        ),
        (
            r#"{"tool_input":{"command":"ls"},"other":"git commit"}"#,
            false,
        ),
        (r#"not json at all"#, false),
        (r#""#, false),
    ];
    for (payload, is_commit) in cases {
        let _ = fs::remove_dir_all(repo.join(".git/docsys-gate"));
        let (code, err) = run_hook(&repo, payload, &[]);
        let expected = if *is_commit { 2 } else { 0 };
        assert_eq!(code, expected, "payload {payload:?}: {err}");
    }
}

#[test]
fn a_staged_seed_plan_blocks_the_commit_and_names_the_way_out() {
    // D-091: the plan is a draft; landing it is `docsys seed apply`, never a commit
    let repo = build_repo("seedplan");
    fs::write(
        repo.join("SEED.tsv"),
        "# head: abc1234\nresearch\tsync\t-\n",
    )
    .unwrap();
    write_page(&repo);
    git(&repo, &["add", "-A"]);
    let (code, err) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("SEED.tsv"), "{err}");
    assert!(err.contains("docsys seed apply"), "{err}");
    // unstaged, the same docs change commits
    git(&repo, &["reset", "-q", "--", "SEED.tsv"]);
    let (code, err) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 0, "{err}");
}

// ── commit_policy: require (D-093, R-209) ───────────────────────────────────

fn require(repo: &Path) {
    let dm = repo.join("docs/.docmeta.yml");
    let mut text = fs::read_to_string(&dm).unwrap();
    if text.contains("commit_policy:") {
        text = text.replace("commit_policy: ask", "commit_policy: require");
    } else {
        text.push_str("commit_policy: require\n");
    }
    fs::write(&dm, text).unwrap();
}

fn run_stop_with(repo: &Path, payload: &str) -> (i32, String) {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_docsys"));
    let path = format!(
        "{}:{}",
        bin.parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut child = Command::new("bash")
        .arg(repo.join(".claude/hooks/stop-docs-reminder.sh"))
        .current_dir(repo)
        .env("PATH", path)
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write as _;
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

#[test]
fn under_require_the_gate_refuses_every_time_and_a_bypass_leaves_debt() {
    let repo = build_repo("require");
    require(&repo);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "policy"]);
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "main.rs"]);
    let (code, err) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("commit_policy: require"), "{err}");
    // the work types are the routing's; the refusal names the work
    assert!(err.contains("name the work"), "{err}");
    // the same commit again: still refused — a refusal, not a question
    let (code, err) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 2, "{err}");
    // a `Docs:` line in the message answers it: the commit is the journal
    // entry (D-125)
    let documented = r#"{"tool_name":"Bash","tool_input":{"command":"git commit -m 'main added' -m 'Docs: the entry point; no page changes'"}}"#;
    let (code, err) = run_hook(&repo, documented, &[]);
    assert_eq!(code, 0, "{err}");
    git(&repo, &["commit", "-q", "-m", "main with its journal line"]);
    // a bypass under require leaves a debt item
    fs::write(repo.join("lib.rs"), "pub fn f() {}\n").unwrap();
    git(&repo, &["add", "lib.rs"]);
    let (code, _) = run_hook(&repo, commit_payload(), &[("DOCSYS_SKIP", "1")]);
    assert_eq!(code, 0);
    // its own file on a docsys/0.5 tree (D-124)
    let items: Vec<_> = fs::read_dir(repo.join("docs/work/debt"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    assert_eq!(items.len(), 1, "{items:?}");
    let debt = fs::read_to_string(items.first().unwrap()).unwrap();
    assert!(
        debt.starts_with("- [ ] ")
            && debt.contains("committed without documentation (DOCSYS_SKIP): lib.rs"),
        "{debt}"
    );
    assert!(
        debt.contains("-- deferred:") && debt.contains("-- repay when:"),
        "{debt}"
    );
}

#[test]
fn under_require_the_end_of_a_turn_holds_once_until_the_work_is_recorded() {
    let repo = build_repo("hold");
    require(&repo);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "policy"]);
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    // code changed, nothing recorded: the turn is held
    let (code, err) = run_stop_with(&repo, r#"{"session_id":"s1","stop_hook_active":false}"#);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("this turn holds until"), "{err}");
    // the retry (Claude Code sets stop_hook_active) is not held again
    let (code, err) = run_stop_with(&repo, r#"{"session_id":"s1","stop_hook_active":true}"#);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("changed code but no documentation"), "{err}");
    // the work recorded: no hold
    write_page(&repo);
    let (code, err) = run_stop_with(&repo, r#"{"session_id":"s1","stop_hook_active":false}"#);
    assert_eq!(code, 0, "{err}");
    // under the default policy the same state only reminds
    let dm = repo.join("docs/.docmeta.yml");
    fs::write(
        &dm,
        fs::read_to_string(&dm)
            .unwrap()
            .replace("commit_policy: require", "commit_policy: ask"),
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    git(
        &repo,
        &["commit", "-q", "-m", "recorded; policy back to ask"],
    );
    fs::write(repo.join("lib.rs"), "pub fn f() {}\n").unwrap();
    let (code, err) = run_stop_with(&repo, r#"{"session_id":"s1","stop_hook_active":false}"#);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("changed code but no documentation"), "{err}");
}

#[test]
fn the_first_turn_names_what_the_tree_holds() {
    let repo = build_repo("digest");
    fs::create_dir_all(repo.join("docs/work/features")).unwrap();
    fs::write(
        repo.join("docs/work/features/dark-mode.md"),
        "---\nid: dark-mode\nstatus: active\nupdated: 2026-09-03\n---\n## Context\n\nA toggle.\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args(["hook", "user-prompt-submit", "--root", "docs"])
        .current_dir(&repo)
        .env("TMPDIR", repo.join(".markers"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            use std::io::Write as _;
            c.stdin
                .take()
                .unwrap()
                .write_all(br#"{"session_id":"digest-1","prompt":"add a toggle"}"#)?;
            c.wait_with_output()
        })
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("<docs-in-hand>"), "{text}");
    assert!(
        text.contains("Work in flight: work/features/dark-mode.md (active)"),
        "{text}"
    );
    assert!(
        text.contains("improvement (refactor, performance, cleanup)"),
        "{text}"
    );
}

/// D-125: on docsys/0.5 the commit message is the journal entry, so an
/// unpushed code-only commit that says why with `Docs:` is recorded.
#[test]
fn stop_reminder_reads_a_docs_line_in_an_unpushed_commit() {
    let repo = build_repo("stop-docs-line");
    with_upstream(&repo);
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "main.rs"]);
    git(&repo, &["commit", "-q", "-m", "code only"]);
    let (_, err) = run_stop(&repo);
    assert!(err.contains("no documentation"), "{err}");
    assert!(err.contains("`Docs: <why>`"), "{err}");
    fs::write(repo.join("main.rs"), "fn main() { run() }\n").unwrap();
    git(&repo, &["add", "main.rs"]);
    git(
        &repo,
        &[
            "commit",
            "-q",
            "-m",
            "run on start",
            "-m",
            "Docs: the entry point only calls run",
        ],
    );
    let (code, err) = run_stop(&repo);
    assert_eq!(code, 0);
    assert!(err.is_empty(), "{err}");
}

/// A relay message says each thing once (D-114): the GATE line names what
/// moved and the question follows once; a `git add` is named only when the
/// command ran one. Under `require` the way out is said once and the work
/// types are the routing's one set; the stop relay says what it does.
#[test]
fn a_relay_message_says_each_thing_once() {
    let repo = build_repo("say-once");
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/x.rs"), "fn x() {}\n").unwrap();
    git(&repo, &["add", "src/x.rs"]);
    let (code, msg) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 2, "{msg}");
    assert_eq!(msg.matches("no docs change").count(), 1, "{msg}");
    assert!(!msg.contains("no documentation change"), "{msg}");
    assert!(!msg.contains("git add"), "{msg}");
    // the command that staged inside itself hears why it runs again whole
    let other = build_repo("say-once-add");
    fs::create_dir_all(other.join("src")).unwrap();
    fs::write(other.join("src/x.rs"), "fn x() {}\n").unwrap();
    git(&other, &["add", "src/x.rs"]);
    git(&other, &["commit", "-q", "-m", "x"]);
    fs::write(other.join("src/x.rs"), "fn x() { }\n").unwrap();
    let (code, msg) = run_hook(&other, add_and_commit_payload(), &[]);
    assert_eq!(code, 2, "{msg}");
    assert_eq!(msg.matches("its `git add` with it").count(), 1, "{msg}");
    let _ = fs::remove_dir_all(&other);
    // under require
    let dm = repo.join("docs/.docmeta.yml");
    let text = fs::read_to_string(&dm).unwrap();
    fs::write(
        &dm,
        text.replace("commit_policy: ask", "commit_policy: require"),
    )
    .unwrap();
    git(
        &repo,
        &["commit", "-q", "-m", "require", "docs/.docmeta.yml"],
    );
    let (code, msg) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 2, "{msg}");
    assert_eq!(msg.matches("run the SAME commit again").count(), 1, "{msg}");
    assert!(!msg.contains("git add"), "{msg}");
    // the work types are D-093's four, said once, in the routing; the relay
    // names the work and leaves the list there
    assert!(msg.contains("name the work"), "{msg}");
    assert!(!msg.contains("feature | bug"), "{msg}");
    let routing = docsys::hook::ROUTING;
    assert!(
        routing.contains(
            "one of feature, bug,\nimprovement (refactor, performance, cleanup), research. "
        ),
        "{routing}"
    );
    assert!(!routing.contains("idea-note"), "{routing}");
    let (code, msg) = run_stop(&repo);
    assert_eq!(code, 2, "{msg}");
    assert_eq!(msg.matches("Docs: <why>").count(), 1, "{msg}");
    let relay = fs::read_to_string(repo.join(".claude/hooks/stop-docs-reminder.sh")).unwrap();
    assert!(!relay.contains("never blocks"), "{relay}");
    let _ = fs::remove_dir_all(&repo);
}

/// Six words in a row that a text says twice.
fn said_twice(text: &str) -> Vec<String> {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    words
        .windows(6)
        .map(|w| w.join(" "))
        .filter(|p| !seen.insert(p.clone()))
        .collect()
}

/// A refusal says how to run again once, and names a `git add` only when the
/// blocked call ran one — under either policy, for a lint error and for code
/// without documentation alike (D-129).
#[test]
fn a_refusal_says_the_re_run_once_and_a_git_add_only_when_it_ran_one() {
    for policy in ["ask", "require"] {
        let repo = build_repo(&format!("rerun-{policy}"));
        let dm = repo.join("docs/.docmeta.yml");
        let text = fs::read_to_string(&dm).unwrap();
        fs::write(
            &dm,
            text.replace("commit_policy: ask", &format!("commit_policy: {policy}")),
        )
        .unwrap();
        if policy != "ask" {
            git(
                &repo,
                &["commit", "-q", "-m", "policy", "docs/.docmeta.yml"],
            );
        }
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/x.rs"), "fn x() {}\n").unwrap();
        git(&repo, &["add", "src/x.rs"]);
        git(&repo, &["commit", "-q", "-m", "x", "--no-verify"]);
        fs::write(repo.join("src/x.rs"), "fn x() { }\n").unwrap();
        // the call stages its own files: the re-run is the whole call
        let (code, msg) = run_hook(&repo, add_and_commit_payload(), &[]);
        assert_eq!(code, 2, "{policy}: {msg}");
        assert_eq!(
            msg.to_lowercase().matches("same").count(),
            1,
            "{policy}: {msg}"
        );
        assert_eq!(msg.matches("git add").count(), 1, "{policy}: {msg}");
        assert!(said_twice(&msg).is_empty(), "{policy}: {msg}");
        // a staged change and a plain commit: no `git add` to speak of
        fs::write(repo.join("src/y.rs"), "fn y() {}\n").unwrap();
        git(&repo, &["add", "src/y.rs"]);
        let (code, msg) = run_hook(&repo, commit_payload(), &[]);
        assert_eq!(code, 2, "{policy}: {msg}");
        assert!(!msg.contains("git add"), "{policy}: {msg}");
        assert!(said_twice(&msg).is_empty(), "{policy}: {msg}");
        // a lint error under a plain commit
        git(&repo, &["reset", "-q"]);
        fs::write(
            repo.join("docs/index.md"),
            fs::read_to_string(repo.join("docs/index.md")).unwrap() + "\nSee [[reference/nope]].\n",
        )
        .unwrap();
        git(&repo, &["add", "docs/index.md"]);
        let (code, msg) = run_hook(&repo, commit_payload(), &[]);
        assert_eq!(code, 2, "{policy}: {msg}");
        assert!(msg.contains("lint errors block"), "{policy}: {msg}");
        assert!(!msg.contains("git add"), "{policy}: {msg}");
        let _ = fs::remove_dir_all(&repo);
    }
}

/// The commit relay's header says what it does under either policy: a
/// question asked once under `ask`, a refusal every time under `require`.
#[test]
fn the_commit_relay_says_what_it_does_under_either_policy() {
    let repo = build_repo("relay-header");
    let relay = fs::read_to_string(repo.join(".claude/hooks/pre-commit-docs.sh")).unwrap();
    let header: String = relay
        .lines()
        .take_while(|l| l.starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(header.contains("commit_policy: require"), "{header}");
    assert!(header.contains("every time"), "{header}");
    let _ = fs::remove_dir_all(&repo);
}

// ── what a skip and a closed reader leave of the gate's verdict ─────────────

/// PATH with the test binary first: the gate and the relays run `docsys`.
fn path_with_bin() -> String {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_docsys"));
    format!(
        "{}:{}",
        bin.parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

/// A repository `docsys adopt` set up, its git gate included, committed.
fn adopted(name: &str) -> PathBuf {
    let repo = tmp(name);
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    fs::write(repo.join("README.md"), "# r\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_docsys"))
        .arg("adopt")
        .current_dir(&repo)
        .env("PATH", path_with_bin())
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    commit_through_gate(&repo, &["-q", "-m", "adopt"], &[]);
    repo
}

/// `git add -A && git commit <args>` through the git gate: its status and
/// what it said on stderr.
fn commit_through_gate(repo: &Path, args: &[&str], env: &[(&str, &str)]) -> (i32, String) {
    git(repo, &["add", "-A"]);
    let mut cmd = Command::new("git");
    cmd.arg("commit")
        .args(args)
        .current_dir(repo)
        .env("PATH", path_with_bin())
        .env("DOCSYS_NO_AUTO_INSTALL", "1");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// A skipped commit is the person's word (R-209): the gate's only job is the
/// record `require` asks for, and it says something only when that record
/// could not be written — never a pointer to a reason nobody sees.
#[test]
fn a_skipped_commit_says_only_what_its_record_needs() {
    let repo = adopted("skip-lint");
    require(&repo);
    commit_through_gate(
        &repo,
        &["-q", "-m", "policy", "-m", "Docs: the policy"],
        &[],
    );
    // code and a documentation change with a lint error: documented, so no
    // record is due, and nothing is said
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/x.rs"), "fn x() {}\n").unwrap();
    let index = repo.join("docs/index.md");
    fs::write(
        &index,
        fs::read_to_string(&index).unwrap() + "\nSee [[reference/nope]].\n",
    )
    .unwrap();
    let (code, err) = commit_through_gate(&repo, &["-q", "-m", "both"], &[("DOCSYS_SKIP", "1")]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("not recorded"), "{err}");
    assert!(!repo.join("docs/work/debt").exists());
    // code alone while that lint error stands: the record is written, and
    // nothing claims it was not
    fs::write(repo.join("src/y.rs"), "fn y() {}\n").unwrap();
    let (code, err) = commit_through_gate(&repo, &["-q", "-m", "code"], &[("DOCSYS_SKIP", "1")]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("not recorded"), "{err}");
    let debt = fs::read_to_string(repo.join("docs/work/debt/general.md")).unwrap();
    assert!(debt.contains("(DOCSYS_SKIP): src/y.rs"), "{debt}");
    // a record that cannot be written says why, above the line that points at it
    // the record lands beside the commit, untracked: an empty read-only
    // directory is no change to commit, and takes no file
    use std::os::unix::fs::PermissionsExt as _;
    let debt_dir = repo.join("docs/work/debt");
    fs::remove_dir_all(&debt_dir).unwrap();
    fs::create_dir_all(&debt_dir).unwrap();
    fs::set_permissions(&debt_dir, fs::Permissions::from_mode(0o555)).unwrap();
    fs::write(repo.join("src/z.rs"), "fn z() {}\n").unwrap();
    let (code, err) = commit_through_gate(&repo, &["-q", "-m", "more"], &[("DOCSYS_SKIP", "1")]);
    fs::set_permissions(&debt_dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(code, 0, "{err}");
    let lines: Vec<&str> = err.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.contains("not recorded — the line above says why"))
        .unwrap_or_else(|| panic!("{err}"));
    assert!(
        at.checked_sub(1)
            .and_then(|above| lines.get(above))
            .is_some_and(|l| l.contains("could not record the bypass")),
        "{err}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// A process whose stream at `which` has no reader: its status.
fn status_with_closed(mut cmd: Command, which: &str, stdin: &str) -> i32 {
    use std::io::Write as _;
    use std::process::Stdio;
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    // the reader leaves before the process says a word
    match which {
        "stdout" => drop(child.stdout.take()),
        _ => drop(child.stderr.take()),
    }
    let _ = child.stdin.take().unwrap().write_all(stdin.as_bytes());
    let out = child.wait_with_output().unwrap();
    out.status.code().unwrap_or(-1)
}

/// A verdict is a status: a relay or a gate whose reader closed its end
/// exits with what it decided, whoever runs it — never 141 (R-209).
#[test]
fn a_closed_reader_never_changes_a_verdict() {
    let repo = adopted("closed-reader");
    require(&repo);
    commit_through_gate(
        &repo,
        &["-q", "-m", "policy", "-m", "Docs: the policy"],
        &[],
    );
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/x.rs"), "fn x() {}\n").unwrap();
    git(&repo, &["add", "src/x.rs"]);
    // the relay refuses, its stderr's reader gone or not
    for which in ["stderr", "stdout"] {
        let mut relay = Command::new("bash");
        relay
            .arg(repo.join(".claude/hooks/pre-commit-docs.sh"))
            .current_dir(&repo)
            .env("PATH", path_with_bin())
            .env("TMPDIR", repo.join(".markers"));
        assert_eq!(
            status_with_closed(relay, which, commit_payload()),
            2,
            "relay, {which}"
        );
        let mut direct = Command::new(env!("CARGO_BIN_EXE_docsys"));
        direct
            .args(["hook", "pre-tool-use", "--root", "docs"])
            .current_dir(&repo)
            .env("TMPDIR", repo.join(".markers"));
        assert_eq!(
            status_with_closed(direct, which, commit_payload()),
            2,
            "hook, {which}"
        );
    }
    // the git hook run by hand, outside git: the gate lets docs through
    git(&repo, &["reset", "-q"]);
    let index = repo.join("docs/index.md");
    fs::write(&index, fs::read_to_string(&index).unwrap() + "\nMore.\n").unwrap();
    git(&repo, &["add", "docs/index.md"]);
    for which in ["stdout", "stderr"] {
        let mut hook = Command::new("bash");
        hook.arg(".git/hooks/pre-commit")
            .current_dir(&repo)
            .env("PATH", path_with_bin())
            .env_remove("GIT_EXEC_PATH")
            .env_remove("GIT_INDEX_FILE");
        assert_eq!(
            status_with_closed(hook, which, ""),
            0,
            "pre-commit, {which}"
        );
    }
    let _ = fs::remove_dir_all(&repo);
}

/// A call that stages a new file and commits it is asked about that file as
/// about a changed one: `git add` in the call takes untracked files too
/// (D-040).
#[test]
fn a_new_file_the_call_stages_is_part_of_its_commit() {
    for policy in ["ask", "require"] {
        let repo = build_repo(&format!("new-file-{policy}"));
        if policy == "require" {
            require(&repo);
            git(&repo, &["add", "-A"]);
            git(&repo, &["commit", "-q", "-m", "policy"]);
        }
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/new.rs"), "fn n() {}\n").unwrap();
        let payload = r#"{"tool_name":"Bash","tool_input":{"command":"git add src/new.rs && git commit -m wip"}}"#;
        let (code, msg) = run_hook(&repo, payload, &[]);
        assert_eq!(code, 2, "{policy}: {msg}");
        assert!(msg.contains("src/new.rs"), "{policy}: {msg}");
        // a call that stages nothing commits nothing new: an untracked file
        // stays out of it
        let (code, msg) = run_hook(&repo, commit_payload(), &[]);
        assert_eq!(code, 0, "{policy}: {msg}");
        let _ = fs::remove_dir_all(&repo);
    }
    // a docsys/0.4 tree is asked what 0.15.1 asked: tracked changes alone (D-118)
    let repo = build_repo04("new-file-04");
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/new.rs"), "fn n() {}\n").unwrap();
    let payload = r#"{"tool_name":"Bash","tool_input":{"command":"git add src/new.rs && git commit -m wip"}}"#;
    let (code, msg) = run_hook(&repo, payload, &[]);
    assert_eq!(code, 0, "{msg}");
    let _ = fs::remove_dir_all(&repo);
}

/// A relay's text is read by the agent it refuses: the bypass it names is the
/// person's, since the same relay refuses `DOCSYS_SKIP=1` in the agent's own
/// command (R-209).
#[test]
fn the_bypass_a_relay_names_is_the_persons() {
    let skip_commit =
        r#"{"tool_name":"Bash","tool_input":{"command":"DOCSYS_SKIP=1 git commit -m x"}}"#;
    // a lint error, under ask
    let repo = build_repo("bypass-lint");
    let index = repo.join("docs/index.md");
    fs::write(
        &index,
        fs::read_to_string(&index).unwrap() + "\nSee [[reference/nope]].\n",
    )
    .unwrap();
    git(&repo, &["add", "docs/index.md"]);
    let (code, msg) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 2, "{msg}");
    assert!(
        msg.contains("DOCSYS_SKIP=1") && msg.contains("person"),
        "{msg}"
    );
    let (code, again) = run_hook(&repo, skip_commit, &[]);
    assert_eq!(code, 2, "{again}");
    let _ = fs::remove_dir_all(&repo);
    // code with no documentation, under require
    let repo = build_repo("bypass-require");
    require(&repo);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "policy"]);
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "main.rs"]);
    let (code, msg) = run_hook(&repo, commit_payload(), &[]);
    assert_eq!(code, 2, "{msg}");
    assert!(
        msg.contains("DOCSYS_SKIP=1") && msg.contains("person"),
        "{msg}"
    );
    let (code, again) = run_hook(&repo, skip_commit, &[]);
    assert_eq!(code, 2, "{again}");
    let _ = fs::remove_dir_all(&repo);
}
