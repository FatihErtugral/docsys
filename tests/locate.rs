#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// Where docsys runs (D-098): a command or a relay started in a subdirectory,
// a git hook in a linked worktree (git exports GIT_DIR there), a monorepo
// with a tree per package, and a stray tree above a repository. Unix-only:
// the relays and the git hook are bash.
#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-locate-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
}

fn path_with_bin() -> String {
    format!(
        "{}:{}",
        bin().parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

fn git(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("PATH", path_with_bin())
        .env("DOCSYS_TODAY", "2026-10-02")
        // history on the day docsys is told it is, so `updated:` and the last
        // change agree whatever day the suite runs (R-106)
        .env("GIT_AUTHOR_DATE", "2026-10-02T12:00:00+00:00")
        .env("GIT_COMMITTER_DATE", "2026-10-02T12:00:00+00:00")
        .output()
        .unwrap()
}

fn ok(dir: &Path, args: &[&str]) {
    let out = git(dir, args);
    assert!(
        out.status.success(),
        "git {args:?}: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn docsys(dir: &Path, args: &[&str]) -> (i32, String) {
    let out = Command::new(bin())
        .args(args)
        .current_dir(dir)
        .env("DOCSYS_TODAY", "2026-10-02")
        .env_remove("CLAUDE_PROJECT_DIR")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

const AUTH_RS: &str =
    "pub fn other() -> u32 {\n    1\n}\n\npub fn refresh_token(ttl: u32) -> u32 {\n    ttl / 2\n}\n";

/// An adopted repository with one routed page pinned to `src/auth.rs`, and a
/// package directory to stand in.
fn project(name: &str) -> PathBuf {
    let repo = tmp(name);
    ok(&repo, &["init", "-q"]);
    ok(&repo, &["config", "user.email", "t@example.invalid"]);
    ok(&repo, &["config", "user.name", "t"]);
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/auth.rs"), AUTH_RS).unwrap();
    fs::create_dir_all(repo.join("apps/x/src")).unwrap();
    fs::write(repo.join("apps/x/src/main.rs"), "fn main() {}\n").unwrap();
    let (code, out) = docsys(&repo, &["adopt"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("git pre-commit gate: written (hard)"), "{out}");
    fs::create_dir_all(repo.join("docs/reference")).unwrap();
    fs::write(
        repo.join("docs/reference/token-ttl.md"),
        "---\nid: token-ttl\ntype: reference\n---\n\n# Token TTL\n\nThis page says how long a token lives; read it before changing the refresh.\n",
    )
    .unwrap();
    let index = fs::read_to_string(repo.join("docs/index.md")).unwrap();
    fs::write(
        repo.join("docs/index.md"),
        format!("{index}- [[reference/token-ttl|Token TTL]] -- How long a token lives.\n"),
    )
    .unwrap();
    let (code, out) = docsys(
        &repo,
        &[
            "pin",
            "reference/token-ttl",
            "src/auth.rs",
            "--symbol",
            "refresh_token",
        ],
    );
    assert_eq!(code, 0, "{out}");
    ok(&repo, &["add", "-A"]);
    ok(
        &repo,
        &["commit", "-qm", "adopt docsys, pin the token page"],
    );
    repo
}

#[test]
fn a_command_in_a_subdirectory_answers_as_it_does_at_the_top() {
    let repo = project("sub");
    let deep = repo.join("apps/x/src");
    for args in [
        vec!["lint"],
        vec!["status"],
        vec!["lookup", "token"],
        vec!["gate"],
        vec!["backlinks", "token-ttl"],
    ] {
        let top = docsys(&repo, &args);
        let below = docsys(&deep, &args);
        assert_eq!(
            top, below,
            "{args:?} differs between the top and a subdirectory"
        );
    }
    // the pin resolves from below too, and a refresh lands on the same page
    let (code, out) = docsys(&deep, &["pin", "--refresh", "reference/token-ttl"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        git(&repo, &["status", "--porcelain"]).stdout.is_empty() || {
            // `pin --refresh` bumps `updated:` only when the day moved; nothing else may change
            let diff =
                String::from_utf8_lossy(&git(&repo, &["diff", "--stat"]).stdout).into_owned();
            diff.contains("docs/reference/token-ttl.md") && !diff.contains("apps/")
        }
    );
    let _ = fs::remove_dir_all(&repo);
}

fn relay(repo: &Path, script: &str, cwd: &Path, payload: &str) -> (i32, String, String) {
    let markers = repo.join(".git/test-tmp");
    let _ = fs::create_dir_all(&markers);
    let mut child = Command::new("bash")
        .arg(repo.join(".claude/hooks").join(script))
        .current_dir(cwd)
        .env("PATH", path_with_bin())
        .env("TMPDIR", &markers)
        .env("DOCSYS_TODAY", "2026-10-02")
        .env("CLAUDE_PROJECT_DIR", repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// The measurement from a real repository, as a test: every relay run from a
/// package directory behaves as it does from the top.
#[test]
fn every_relay_works_from_a_subdirectory() {
    let repo = project("relays");
    let deep = repo.join("apps/x/src");
    let page = repo.join("docs/reference/token-ttl.md");
    let cwd = |d: &Path| d.display().to_string();

    // the commit gate: nothing staged but a page, so it passes — from below too
    fs::write(
        &page,
        fs::read_to_string(&page).unwrap() + "\nOne more sentence.\n",
    )
    .unwrap();
    ok(&repo, &["add", "docs"]);
    let commit = |d: &Path| {
        format!(
            r#"{{"cwd":"{}","tool_name":"Bash","tool_input":{{"command":"git commit -m x"}}}}"#,
            cwd(d)
        )
    };
    let top = relay(&repo, "pre-commit-docs.sh", &repo, &commit(&repo));
    let below = relay(&repo, "pre-commit-docs.sh", &deep, &commit(&deep));
    assert_eq!(top.0, 0, "{}", top.2);
    assert_eq!(
        below.0, 0,
        "the gate blocked from a subdirectory: {}",
        below.2
    );
    ok(&repo, &["commit", "-qm", "page"]);

    // the post-edit bookkeeping: a verified page whose body no longer reads
    // as its record, edited by a session standing below, is demoted
    fs::write(
        &page,
        fs::read_to_string(&page).unwrap().replace(
            "type: reference\n",
            "type: reference\nverification: verified\nverified_by: t\nverified_rev: 0000000\nverified_blocks: [000000000000]\n",
        ),
    )
    .unwrap();
    let edit = format!(
        r#"{{"cwd":"{}","tool_name":"Edit","tool_input":{{"file_path":"{}"}}}}"#,
        cwd(&deep),
        page.display()
    );
    let (code, _, err) = relay(&repo, "post-edit-updated.sh", &deep, &edit);
    assert_eq!(code, 2, "{err}");
    assert!(
        fs::read_to_string(&page)
            .unwrap()
            .contains("verification: unverified"),
        "the demotion was a silent no-op from a subdirectory"
    );

    // the first-turn digest counts the tree's pages
    let prompt = format!(
        r#"{{"cwd":"{}","session_id":"s-{}","prompt":"hi"}}"#,
        cwd(&deep),
        std::process::id()
    );
    let (code, out, err) = relay(&repo, "session-intent.sh", &deep, &prompt);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("1 permanent page(s)"),
        "digest from below: {out}"
    );

    // the end-of-turn reminder sees the page change as documentation
    fs::write(repo.join("src/auth.rs"), AUTH_RS.replace("1\n", "2\n")).unwrap();
    fs::write(
        &page,
        fs::read_to_string(&page).unwrap() + "\nA second sentence.\n",
    )
    .unwrap();
    let stop = format!(r#"{{"cwd":"{}","stop_hook_active":false}}"#, cwd(&deep));
    let (_, _, below_msg) = relay(&repo, "stop-docs-reminder.sh", &deep, &stop);
    let (_, _, top_msg) = relay(&repo, "stop-docs-reminder.sh", &repo, &stop);
    assert_eq!(below_msg, top_msg);
    assert!(
        !below_msg.contains("no documentation"),
        "a page change read as none: {below_msg}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// git exports GIT_DIR to the hooks of a linked worktree. The pre-commit gate
/// must still find the repository's top, resolve the pins there, pass a clean
/// commit — and stop the commit whose pinned code moved.
#[test]
fn the_git_gate_in_a_worktree_resolves_pins_at_the_top() {
    let repo = project("worktree");
    let wt = repo.with_file_name(format!(
        "{}-wt",
        repo.file_name().unwrap().to_string_lossy()
    ));
    let _ = fs::remove_dir_all(&wt);
    ok(&repo, &["worktree", "add", "-q", wt.to_str().unwrap()]);

    // a clean commit from a package directory of the worktree: the gate runs
    // with GIT_DIR exported and passes
    fs::write(wt.join("apps/x/src/main.rs"), "fn main() { let _ = 1; }\n").unwrap();
    fs::write(
        wt.join("docs/reference/main.md"),
        "---\nid: main\ntype: reference\n---\nThis page states what main does; read it before changing it.\n",
    )
    .unwrap();
    ok(&wt.join("apps/x"), &["add", "-A", "."]);
    ok(&wt.join("apps/x"), &["add", "../../docs"]);
    let out = git(
        &wt.join("apps/x"),
        &["commit", "-m", "touch main with a journal line"],
    );
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "a clean commit was stopped: {all}");
    assert!(
        !all.contains("R-111"),
        "pins read as moved under GIT_DIR: {all}"
    );

    // the check seen failing: the pinned region moves, the gate stops it
    fs::write(
        wt.join("src/auth.rs"),
        AUTH_RS.replace("ttl / 2", "ttl / 3"),
    )
    .unwrap();
    ok(&wt, &["add", "src/auth.rs"]);
    let out = git(&wt, &["commit", "-m", "move the pinned region"]);
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!out.status.success(), "a moved pin passed the gate: {all}");
    assert!(
        all.contains("ERROR R-111 reference/token-ttl.md [src/auth.rs#refresh_token]"),
        "{all}"
    );
    let _ = fs::remove_dir_all(&wt);
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn in_a_monorepo_the_nearest_tree_answers() {
    let repo = project("mono");
    fs::create_dir_all(repo.join("apps/y/src")).unwrap();
    let (code, out) = docsys(&repo.join("apps/y"), &["init", "--root", "docs"]);
    assert_eq!(code, 0, "{out}");
    fs::create_dir_all(repo.join("apps/y/docs/reference")).unwrap();
    fs::write(
        repo.join("apps/y/docs/reference/only-here.md"),
        "---\nid: only-here\ntype: reference\nupdated: 2026-10-02\n---\n\n# Only here\n\nSee [[reference/ghost]].\n",
    )
    .unwrap();
    let (_, below) = docsys(&repo.join("apps/y/src"), &["lint"]);
    assert!(below.contains("reference/only-here.md"), "{below}");
    let (_, top) = docsys(&repo, &["lint"]);
    assert!(!top.contains("only-here"), "{top}");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_tree_above_the_repository_is_never_adopted() {
    let outer = tmp("stray");
    fs::create_dir_all(outer.join("docs")).unwrap();
    fs::write(
        outer.join("docs/.docmeta.yml"),
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n",
    )
    .unwrap();
    let inner = outer.join("inner");
    fs::create_dir_all(inner.join("src")).unwrap();
    ok(&inner, &["init", "-q"]);
    let (code, out) = docsys(&inner.join("src"), &["lint"]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("R-160"), "{out}");
    let _ = fs::remove_dir_all(&outer);
}
