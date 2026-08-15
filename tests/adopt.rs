#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// Tests report through panics by design; the production lints stay strict.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-adopt-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn git_init(dir: &Path) {
    assert!(Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir)
        .status()
        .unwrap()
        .success());
}

#[test]
fn greenfield_adopt_scaffolds_everything_and_is_idempotent() {
    let repo = tmp("green");
    git_init(&repo);
    let docs = repo.join("docs");

    let out = docsys::adopt::run(&repo, &docs, "en").unwrap();
    // init skeleton, not a bare config file
    assert!(docs.join(".docmeta.yml").exists());
    assert!(docs.join("index.md").exists());
    // settings.json created when absent, with the hook wires
    let settings = fs::read_to_string(repo.join(".claude/settings.json")).unwrap();
    assert!(settings.contains("UserPromptSubmit"), "{settings}");
    // AGENTS.md managed block + report with the judgment checklist
    let agents = fs::read_to_string(repo.join("AGENTS.md")).unwrap();
    assert!(agents.contains("docsys:rules:begin"));
    let report = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    assert!(report.contains("Judgment checklist"), "{report}");
    assert!(report.contains("doc-extensions.md"), "{report}");
    // no .githooks, no configured hooksPath → gate falls back to .git/hooks
    let gate = fs::read_to_string(repo.join(".git/hooks/pre-commit")).unwrap();
    assert!(gate.contains("docsys documentation gate"), "{gate}");
    assert!(out.summary.iter().any(|s| s.contains("created")), "{:?}", out.summary);

    // Second run: everything already in place, nothing rewritten destructively.
    let again = docsys::adopt::run(&repo, &docs, "en").unwrap();
    assert!(again.summary.iter().any(|s| s.contains(".docmeta.yml: kept")), "{:?}", again.summary);
    assert!(again.summary.iter().any(|s| s.contains("gate: kept")), "{:?}", again.summary);
    // settings now exists → untouched, and the merge snippet moves to the checklist
    assert!(again.summary.iter().any(|s| s.contains("settings.json: untouched")));
    let report = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    assert!(report.contains("Merge the docsys hook wires"), "{report}");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn adopt_refuses_an_unmigrated_tree() {
    let repo = tmp("unmig");
    git_init(&repo);
    fs::create_dir_all(repo.join("docs")).unwrap();
    fs::write(repo.join("docs/old.md"), "# legacy page\n").unwrap();

    let err = docsys::adopt::run(&repo, &repo.join("docs"), "en").unwrap_err();
    assert!(err.contains("migrate inventory"), "{err}");
    // init must not have clobbered anything
    assert!(!repo.join("docs/index.md").exists());
    assert!(!repo.join("docs/.docmeta.yml").exists());
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn gate_prefers_a_tracked_githooks_dir_and_sets_hookspath() {
    let repo = tmp("githooks");
    git_init(&repo);
    fs::create_dir_all(repo.join(".githooks")).unwrap();

    docsys::adopt::run(&repo, &repo.join("docs"), "en").unwrap();
    let gate = fs::read_to_string(repo.join(".githooks/pre-commit")).unwrap();
    assert!(gate.contains("docsys documentation gate"), "{gate}");
    let out = Command::new("git")
        .args(["config", "core.hooksPath"])
        .current_dir(&repo)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), ".githooks");

    // Idempotent: the marker prevents a second append.
    docsys::adopt::run(&repo, &repo.join("docs"), "en").unwrap();
    let gate2 = fs::read_to_string(repo.join(".githooks/pre-commit")).unwrap();
    assert_eq!(gate.matches("docsys documentation gate").count(), 1);
    assert_eq!(gate2.matches("docsys documentation gate").count(), 1);
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn docmeta_upgrade_prepends_missing_keys_and_keeps_owner_lines() {
    let repo = tmp("upgrade");
    git_init(&repo);
    let docs = repo.join("docs");
    fs::create_dir_all(&docs).unwrap();
    fs::write(docs.join(".docmeta.yml"), "custom_key: kept-verbatim\n").unwrap();

    let out = docsys::adopt::run(&repo, &docs, "en").unwrap();
    let meta = fs::read_to_string(docs.join(".docmeta.yml")).unwrap();
    assert!(meta.starts_with("spec: docsys/0.4\n"), "{meta}");
    assert!(meta.contains("profile: project\n"));
    assert!(meta.contains("default_content_language: en\n"));
    assert!(meta.ends_with("custom_key: kept-verbatim\n"), "{meta}");
    assert!(out.summary.iter().any(|s| s.contains("upgraded")), "{:?}", out.summary);
    let _ = fs::remove_dir_all(&repo);
}
