#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! The manual scripts under `ci/` leave a clone as they found it, an early
//! exit included.
#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-ci-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

/// `ci/pin-replay.sh` that stops early leaves no worktree behind in the
/// clone it was given.
#[test]
fn pin_replay_leaves_no_worktree_when_it_stops_early() {
    let clone = tmp("replay");
    git(&clone, &["init", "-q", "-b", "main"]);
    git(&clone, &["config", "user.email", "t@example.invalid"]);
    git(&clone, &["config", "user.name", "t"]);
    fs::create_dir_all(clone.join("docs/reference")).unwrap();
    fs::write(
        clone.join("docs/.docmeta.yml"),
        "spec: docsys/0.4\nprofile: project\n",
    )
    .unwrap();
    fs::write(
        clone.join("docs/reference/a.md"),
        "---\nid: a\ntype: reference\nverifies:\n  - path: main.rs\n---\nThis page states main.\n",
    )
    .unwrap();
    fs::write(clone.join("main.rs"), "fn main() {}\n").unwrap();
    git(&clone, &["add", "-A"]);
    git(&clone, &["commit", "-qm", "one"]);
    fs::write(clone.join("main.rs"), "fn main() { run() }\n").unwrap();
    git(&clone, &["commit", "-qam", "two"]);
    // binaries that write nothing: the replay's commit has nothing to commit
    // and the script stops there
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("ci/pin-replay.sh");
    let out = Command::new("bash")
        .arg(&script)
        .args([
            clone.to_str().unwrap(),
            "main",
            "1",
            "/usr/bin/true",
            "/usr/bin/true",
        ])
        .env("GIT_CONFIG_PARAMETERS", "'core.hooksPath=/dev/null'")
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "the replay was meant to stop: {out:?}"
    );
    let worktrees = clone.join(".git/worktrees");
    let left: Vec<_> = fs::read_dir(&worktrees)
        .map(|it| it.flatten().map(|e| e.file_name()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "{left:?}");
    let _ = fs::remove_dir_all(&clone);
}
