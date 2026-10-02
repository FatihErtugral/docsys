#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// The relays and the git gate as adopt writes them (D-099, D-100): one command
// form that runs from any directory, the tree's own root in every script, a
// wire recognised in every spelling, a gate git places where it runs hooks —
// a linked worktree included — and a tree that needs a newer docsys named in
// one line. Unix-only: the relays and the gate are bash.
#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-relays-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
}

fn path_with(dir: &Path) -> String {
    format!(
        "{}:{}",
        dir.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

fn git(dir: &Path, path: &str, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("PATH", path)
        .env("DOCSYS_TODAY", "2026-10-02")
        .output()
        .unwrap()
}

fn ok(dir: &Path, args: &[&str]) {
    let real = path_with(bin().parent().unwrap());
    let out = git(dir, &real, args);
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
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

fn repo(name: &str) -> PathBuf {
    let r = tmp(name);
    ok(&r, &["init", "-q"]);
    ok(&r, &["config", "user.email", "t@example.invalid"]);
    ok(&r, &["config", "user.name", "t"]);
    r
}

#[test]
fn adopt_wires_the_project_dir_form_and_bakes_the_trees_root() {
    let r = repo("form");
    let (code, out) = docsys(&r, &["adopt", "--root", "documentation"]);
    assert_eq!(code, 0, "{out}");
    let settings = fs::read_to_string(r.join(".claude/settings.json")).unwrap();
    for name in [
        "session-intent",
        "pre-commit-docs",
        "post-edit-updated",
        "stop-docs-reminder",
    ] {
        assert!(
            settings.contains(&format!(
                r#""command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/{name}.sh""#
            )),
            "{name}: {settings}"
        );
        let script = fs::read_to_string(r.join(format!(".claude/hooks/{name}.sh"))).unwrap();
        assert!(
            script.contains("${DOCS_ROOT:-documentation}"),
            "{name} defaults to another root: {script}"
        );
        assert!(
            !script.contains(r.to_str().unwrap()),
            "an absolute path: {script}"
        );
        assert!(
            script.contains("cd \"${CLAUDE_PROJECT_DIR:-.}\""),
            "{script}"
        );
    }
    let gate = fs::read_to_string(r.join(".git/hooks/pre-commit")).unwrap();
    assert!(
        gate.contains("docsys lint --repo . --root documentation"),
        "{gate}"
    );
    assert!(
        gate.contains(&format!("# docsys-template: {}", env!("CARGO_PKG_VERSION"))),
        "{gate}"
    );
    let _ = fs::remove_dir_all(&r);
}

#[test]
fn a_relay_already_wired_in_any_spelling_gets_no_second_wire() {
    for spelling in [
        ".claude/hooks/pre-commit-docs.sh",
        "./.claude/hooks/pre-commit-docs.sh",
        "\\\"$CLAUDE_PROJECT_DIR\\\"/.claude/hooks/pre-commit-docs.sh",
        "\\\"$CLAUDE_PROJECT_DIR/.claude/hooks/pre-commit-docs.sh\\\"",
        "bash ${CLAUDE_PROJECT_DIR}/.claude/hooks/pre-commit-docs.sh",
        "cd \\\"$CLAUDE_PROJECT_DIR\\\" && .claude/hooks/pre-commit-docs.sh",
    ] {
        let r = repo("spelling");
        fs::create_dir_all(r.join(".claude")).unwrap();
        fs::write(
            r.join(".claude/settings.json"),
            format!(
                r#"{{"hooks":{{"PreToolUse":[{{"matcher":"Bash","hooks":[{{"type":"command","command":"{spelling}"}}]}}]}}}}"#
            ),
        )
        .unwrap();
        let (code, out) = docsys(&r, &["adopt"]);
        assert_eq!(code, 0, "{out}");
        let settings = fs::read_to_string(r.join(".claude/settings.json")).unwrap();
        assert_eq!(
            settings.matches("pre-commit-docs.sh").count(),
            1,
            "`{spelling}` was wired twice: {settings}"
        );
        assert!(settings.contains("session-intent.sh"), "{settings}");
        let _ = fs::remove_dir_all(&r);
    }
}

#[test]
fn duplicates_fold_into_the_one_form_and_an_owner_wrapper_stays() {
    // the shape a real base had: the same relay wired twice under one event,
    // once wrapped in `cd … &&`, once relative — and an owner's own wrapper
    let mut doc = docsys::hook::parse_json(
        r#"{"hooks":{
            "PreToolUse":[
              {"matcher":"Bash","hooks":[{"type":"command","command":"cd \"$CLAUDE_PROJECT_DIR\" && .claude/hooks/pre-commit-docs.sh"}]},
              {"matcher":"Bash","hooks":[{"type":"command","command":".claude/hooks/pre-commit-docs.sh"}]},
              {"matcher":"Write|Edit","hooks":[{"type":"command","command":".claude/hooks/pre-commit-docs.sh"}]}
            ],
            "Stop":[{"hooks":[{"type":"command","command":"bash -c 'lint; .claude/hooks/stop-docs-reminder.sh'"}]}]
        }}"#,
    )
    .unwrap();
    let changes = docsys::agents::canonicalize_wires(&mut doc).unwrap();
    assert!(changes > 0);
    let out = doc.render();
    assert_eq!(
        out.matches(r#""\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/pre-commit-docs.sh""#)
            .count(),
        2,
        "one wire per matcher: {out}"
    );
    assert!(
        !out.contains(r#""command": ".claude/hooks/pre-commit-docs.sh""#),
        "{out}"
    );
    assert!(
        out.contains("bash -c 'lint; .claude/hooks/stop-docs-reminder.sh'"),
        "{out}"
    );
    assert_eq!(
        docsys::agents::canonicalize_wires(&mut doc),
        Some(0),
        "a second pass changes nothing"
    );
}

/// `git worktree add`, then adopt inside the worktree: `.git` is a file there,
/// and the gate lands where git runs the worktree's hooks.
#[test]
fn adopt_in_a_worktree_writes_a_gate_that_runs() {
    let r = repo("wt-main");
    fs::write(r.join("README.md"), "x\n").unwrap();
    ok(&r, &["add", "README.md"]);
    ok(&r, &["commit", "-qm", "base"]);
    let wt = r.with_file_name(format!("{}-wt", r.file_name().unwrap().to_string_lossy()));
    let _ = fs::remove_dir_all(&wt);
    ok(&r, &["worktree", "add", "-q", wt.to_str().unwrap()]);
    let (code, out) = docsys(&wt, &["adopt"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("git pre-commit gate: written (hard)"), "{out}");
    ok(&wt, &["add", "-A"]);
    ok(&wt, &["commit", "-qm", "adopt"]);
    let (_, dr) = docsys(&wt, &["doctor"]);
    assert!(dr.contains("pre-commit gate reachable"), "{dr}");

    // the gate runs: a dangling link stops the commit, its repair passes
    fs::write(
        wt.join("docs/index.md"),
        fs::read_to_string(wt.join("docs/index.md")).unwrap() + "\nSee [[reference/ghost]].\n",
    )
    .unwrap();
    ok(&wt, &["add", "docs/index.md"]);
    let real = path_with(bin().parent().unwrap());
    let stopped = git(&wt, &real, &["commit", "-qm", "dangling"]);
    assert!(!stopped.status.success(), "the worktree gate did not run");
    fs::write(
        wt.join("docs/index.md"),
        fs::read_to_string(wt.join("docs/index.md"))
            .unwrap()
            .replace("\nSee [[reference/ghost]].\n", "\n"),
    )
    .unwrap();
    ok(&wt, &["add", "docs/index.md"]);
    let passed = git(&wt, &real, &["commit", "-qm", "repaired", "--allow-empty"]);
    assert!(
        passed.status.success(),
        "{}",
        String::from_utf8_lossy(&passed.stderr)
    );
    let _ = fs::remove_dir_all(&wt);
    let _ = fs::remove_dir_all(&r);
}

#[test]
fn a_configured_hooks_path_receives_the_gate() {
    let r = repo("hookspath");
    ok(&r, &["config", "core.hooksPath", "custom-hooks"]);
    let (code, out) = docsys(&r, &["adopt"]);
    assert_eq!(code, 0, "{out}");
    let gate = fs::read_to_string(r.join("custom-hooks/pre-commit")).unwrap();
    assert!(gate.contains("docsys documentation gate"), "{gate}");
    assert!(!r.join(".git/hooks/pre-commit").exists());
    let _ = fs::remove_dir_all(&r);
}

#[test]
fn doctor_names_a_relative_wire_and_an_owner_wrapper() {
    let r = repo("doctor");
    let (code, out) = docsys(&r, &["adopt"]);
    assert_eq!(code, 0, "{out}");
    let s = r.join(".claude/settings.json");
    let text = fs::read_to_string(&s)
        .unwrap()
        .replace(
            r#""\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/session-intent.sh""#,
            r#"".claude/hooks/session-intent.sh""#,
        )
        .replace(
            r#""\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/stop-docs-reminder.sh""#,
            r#""bash -c 'x; .claude/hooks/stop-docs-reminder.sh'""#,
        );
    fs::write(&s, text).unwrap();
    let (code, dr) = docsys(&r, &["doctor"]);
    assert_eq!(code, 0, "relative wires still run from the top: {dr}");
    assert!(
        dr.contains("info `.claude/hooks/session-intent.sh` is wired by a relative path"),
        "{dr}"
    );
    assert!(
        dr.contains("info `bash -c 'x; .claude/hooks/stop-docs-reminder.sh'` wraps a docsys relay"),
        "{dr}"
    );
    let _ = fs::remove_dir_all(&r);
}

#[test]
fn a_base_installed_from_its_own_directory_names_itself_dot() {
    let b = repo("kb");
    let (code, out) = docsys(&b, &["init", "--profile", "knowledge-base", "--root", "."]);
    assert_eq!(code, 0, "{out}");
    let (code, out) = docsys(&b, &["agents", "--kb", "--root", "."]);
    assert_eq!(code, 0, "{out}");
    let script = fs::read_to_string(b.join(".claude/hooks/session-intent.sh")).unwrap();
    assert!(script.contains("${DOCS_ROOT:-.}"), "{script}");
    assert!(!script.contains(b.to_str().unwrap()), "{script}");
    let _ = fs::remove_dir_all(&b);
}

/// A tree that declares a newer spec than the installed docsys implements:
/// the relays and the gate say so in one line and stop, instead of letting an
/// older binary flood the session with findings it cannot read.
#[test]
fn an_older_docsys_under_upgraded_relays_is_named_in_one_line() {
    let r = repo("skew");
    let (code, out) = docsys(&r, &["adopt"]);
    assert_eq!(code, 0, "{out}");
    ok(&r, &["add", "-A"]);
    ok(&r, &["commit", "-qm", "adopt"]);
    // a docsys from before `--version`: usage on stderr, exit 2
    let stub_dir = tmp("skew-stub");
    let stub = stub_dir.join("docsys");
    fs::write(
        &stub,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'Usage: docsys …' >&2; exit 2; fi\nexit 0\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
    let stub_path = path_with(&stub_dir);

    let run = |script: &str| -> (i32, String) {
        let mut child = Command::new("bash")
            .arg(r.join(".claude/hooks").join(script))
            .current_dir(&r)
            .env("PATH", &stub_path)
            .env("CLAUDE_PROJECT_DIR", &r)
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write as _;
        child.stdin.take().unwrap().write_all(b"{}").unwrap();
        let o = child.wait_with_output().unwrap();
        (
            o.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    };
    let docmeta = r.join("docs/.docmeta.yml");
    let declared = fs::read_to_string(&docmeta).unwrap().replace(
        &format!("spec: docsys/{}", docsys::rules::spec_version()),
        "spec: docsys/0.4",
    );
    fs::write(&docmeta, &declared).unwrap();

    // control: a tree the stub implements (docsys/0.4) — no line, the relay runs
    for script in ["pre-commit-docs.sh", "session-intent.sh"] {
        let (code, err) = run(script);
        assert_eq!(code, 0, "{script}: {err}");
        assert!(!err.contains("needs docsys"), "{script}: {err}");
    }

    // the tree moved on: every relay names the minimum and stops
    fs::write(
        &docmeta,
        declared.replace("spec: docsys/0.4", "spec: docsys/0.9"),
    )
    .unwrap();
    for script in [
        "pre-commit-docs.sh",
        "stop-docs-reminder.sh",
        "post-edit-updated.sh",
        "session-intent.sh",
    ] {
        let (code, err) = run(script);
        assert_eq!(code, 1, "{script}: {err}");
        assert_eq!(err.lines().count(), 1, "{script}: {err}");
        assert!(
            err.contains(&format!(
                "this tree needs docsys >= {} (it declares docsys/0.9); install: cargo install docsys --version {} --locked",
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_VERSION")
            )),
            "{script}: {err}"
        );
    }
    // the git gate does the same and stops the commit
    ok(&r, &["add", "docs/.docmeta.yml"]);
    let out = git(&r, &stub_path, &["commit", "-qm", "spec bump"]);
    assert!(!out.status.success(), "the gate let an older docsys commit");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("this tree needs docsys >= "),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&stub_dir);
    let _ = fs::remove_dir_all(&r);
}
