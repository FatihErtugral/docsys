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
    fs::write(
        docs.join("work/debt.md"),
        "# Debt\n\n- [ ] 2026-08-16 x -- deferred: y -- repay when: z\n",
    )
    .unwrap();
    git(&repo, &["add", "docs/work/debt.md"]);
    let (g, _) = docsys::gate::run(&repo, &docs).unwrap();
    assert_eq!(g.docs, 1);
    assert!(!g.code.is_empty());
}
