#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// The field report behind these locks: five mechanisms, all silently failed —
// a hook wired to nothing, a gate block dead below `exec`, warn output on a
// channel the model never reads. "Registered" is not "working".

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-doctor-{name}-{}", std::process::id()));
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

fn repo_with_tree(name: &str) -> (PathBuf, PathBuf) {
    let repo = tmp(name);
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    let docs = repo.join("docs");
    docsys::migrate::init_profile(&docs, "en", "project").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "init"]);
    (repo, docs)
}

#[test]
fn doctor_names_every_dead_piece() {
    let (repo, docs) = repo_with_tree("dead");
    let claude = repo.join(".claude");
    // hooks on disk, nothing wired, no git gate — the exact field state
    docsys::agents::install(&claude, false).unwrap();
    let d = docsys::doctor::run(&repo, &docs, &claude);
    assert!(d.failed > 0);
    let all = d.lines.join("\n");
    assert!(all.contains("settings.json"), "{all}");
    assert!(all.contains("no pre-commit hook"), "{all}");
    // a dead gate below exec is called out by name
    fs::create_dir_all(repo.join(".git/hooks")).unwrap();
    fs::write(
        repo.join(".git/hooks/pre-commit"),
        "#!/bin/sh\nexec ./format.sh\ndocsys lint --root docs\n",
    )
    .unwrap();
    let d = docsys::doctor::run(&repo, &docs, &claude);
    let all = d.lines.join("\n");
    assert!(all.contains("dead code"), "{all}");
}

#[test]
fn doctor_passes_a_fully_wired_pipeline() {
    let (repo, docs) = repo_with_tree("alive");
    let claude = repo.join(".claude");
    docsys::agents::install(&claude, false).unwrap();
    fs::write(
        claude.join("settings.json"),
        docsys::agents::SETTINGS_SNIPPET,
    )
    .unwrap();
    // adopt writes the git gate below the shebang — reachable by construction
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let d = docsys::doctor::run(&repo, &docs, &claude);
    assert_eq!(d.failed, 0, "{}", d.lines.join("\n"));
    let all = d.lines.join("\n");
    assert!(all.contains("wired under PreToolUse"), "{all}");
    assert!(all.contains("gate reachable"), "{all}");
}

#[test]
fn the_gate_block_lands_above_an_existing_exec() {
    let (repo, docs) = repo_with_tree("exec");
    // a project hook that ends in exec — the shape that killed the gate twice
    fs::create_dir_all(repo.join(".git/hooks")).unwrap();
    fs::write(
        repo.join(".git/hooks/pre-commit"),
        "#!/usr/bin/env bash\nset -uo pipefail\nexec ./project-format.sh\n",
    )
    .unwrap();
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let hook = fs::read_to_string(repo.join(".git/hooks/pre-commit")).unwrap();
    let gate_at = hook.find("docsys documentation gate").unwrap();
    let exec_at = hook.find("exec ./project-format.sh").unwrap();
    assert!(
        gate_at < exec_at,
        "the gate must run before the exec:\n{hook}"
    );
    let d = docsys::doctor::run(&repo, &docs, &repo.join(".claude"));
    let all = d.lines.join("\n");
    assert!(all.contains("gate reachable"), "{all}");
}

#[test]
fn gate_computes_the_code_without_docs_invariant() {
    let (repo, docs) = repo_with_tree("gate");
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "main.rs"]);
    let (g, _) = docsys::gate::run(&repo, &docs).unwrap();
    assert_eq!(g.code, vec!["main.rs".to_string()]);
    assert_eq!(g.docs, 0);
    assert_eq!(g.scope, "staged");
    // staging a docs change answers the question
    let index = fs::read_to_string(docs.join("index.md")).unwrap();
    fs::write(
        docs.join("index.md"),
        format!("{index}\nThe tree routes its pages.\n"),
    )
    .unwrap();
    git(&repo, &["add", "docs/index.md"]);
    let (g, _) = docsys::gate::run(&repo, &docs).unwrap();
    assert_eq!(g.docs, 1);
    assert!(!g.code.is_empty());
}

#[test]
fn hookspath_is_read_from_git_not_from_config_text() {
    // The field log: doctor pointed at .git/hooks while the gate lived under
    // .githooks — the config carried the key in a spelling the text parser
    // did not match. git itself reads any casing and any scope; the tool must
    // ask git. The lowercase key below is exactly the shape that was missed.
    let (repo, docs) = repo_with_tree("hookspath");
    fs::create_dir_all(repo.join(".githooks")).unwrap();
    let mut cfg = fs::read_to_string(repo.join(".git/config")).unwrap();
    cfg.push_str("[core]\n\thookspath = .githooks\n");
    fs::write(repo.join(".git/config"), cfg).unwrap();
    fs::write(
        repo.join(".githooks/pre-commit"),
        "#!/usr/bin/env bash\ndocsys lint --root docs || true\n",
    )
    .unwrap();
    let d = docsys::doctor::run(&repo, &docs, &repo.join(".claude"));
    let all = d.lines.join("\n");
    assert!(all.contains(".githooks/pre-commit gate reachable"), "{all}");
    assert!(!all.contains("no pre-commit hook"), "{all}");
    // adopt resolves the same way: the gate must land in .githooks, and the
    // marker check must see it (no duplicate under .git/hooks).
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    assert!(
        !repo.join(".git/hooks/pre-commit").exists(),
        "adopt wrote the gate to the wrong hooks dir"
    );
    let hook = fs::read_to_string(repo.join(".githooks/pre-commit")).unwrap();
    assert!(hook.contains("docsys documentation gate"), "{hook}");
}

/// `docsys agents` on a docsys/0.5 tree, from the repository's top, writes
/// no post-edit relay (D-126), and says the layer is wired only when every
/// relay it wrote is.
#[test]
fn agents_on_a_0_5_tree_writes_no_post_edit_relay() {
    let (repo, _) = repo_with_tree("agents-v05");
    let out = Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args(["agents"])
        .current_dir(&repo)
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert!(
        !repo.join(".claude/hooks/post-edit-updated.sh").exists(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(!String::from_utf8_lossy(&out.stdout).contains("PostToolUse"));
    let _ = fs::remove_dir_all(&repo);
}

/// A relay its owner edited is never what doctor tells someone to overwrite:
/// `agents --force` would drop the owner's lines (D-117); doctor names the
/// diff instead. An untouched one from an older release is refreshed as before.
#[test]
fn doctor_never_advises_overwriting_an_owners_relay() {
    let (repo, _) = repo_with_tree("owner-relay");
    let adopt = Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args(["adopt"])
        .current_dir(&repo)
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .output()
        .unwrap();
    assert!(adopt.status.success(), "{adopt:?}");
    // a relay 0.12.0 wrote, untouched (the upgrade case refreshes it)
    let old =
        include_str!("../corpus/upgrades/0.4-to-0.5/before/dot-claude/hooks/session-intent.sh");
    let relay = repo.join(".claude/hooks/session-intent.sh");
    let doctor = || {
        let out = Command::new(env!("CARGO_BIN_EXE_docsys"))
            .args(["doctor"])
            .current_dir(&repo)
            .env("DOCSYS_NO_AUTO_INSTALL", "1")
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    fs::write(&relay, old).unwrap();
    let said = doctor();
    assert!(
        said.lines().any(|l| l.contains("session-intent.sh")
            && l.contains("`docsys agents --force` refreshes it")),
        "an untouched relay from before: {said}"
    );
    fs::write(&relay, format!("{old}export DOCS_QUIET=1 # owner\n")).unwrap();
    let said = doctor();
    let line = said
        .lines()
        .find(|l| l.contains("session-intent.sh") && l.contains("template"))
        .unwrap_or_default();
    assert!(!line.contains("--force"), "{said}");
    assert!(line.contains("edited by its owner"), "{said}");
    let _ = fs::remove_dir_all(&repo);
}
