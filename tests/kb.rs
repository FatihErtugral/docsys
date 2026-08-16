#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// R-023 has no corpus case by design: content-immutability is observable only
// against a git working tree (D-031), and corpus trees carry no repository.
// These tests build one.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-kb-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn git(dir: &Path, args: &[&str]) {
    assert!(
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap()
            .success(),
        "git {args:?} failed in {}",
        dir.display()
    );
}

/// A committed knowledge-base tree at `<repo>/kb` — the docs root sits below
/// the repository root so the porcelain-prefix stripping is exercised too.
fn build_kb(name: &str) -> (PathBuf, PathBuf) {
    let repo = tmp(name);
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    let root = repo.join("kb");
    let w = |rel: &str, text: &str| {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    };
    w(
        ".docmeta.yml",
        "spec: docsys/0.4\nprofile: knowledge-base\ndefault_content_language: en\ndomains: [coding]\n",
    );
    w(
        "wiki/index.md",
        "# wiki\n\n- [[coding/howto/sample|Sample]] -- A sample page.\n",
    );
    w(
        "wiki/coding/howto/sample.md",
        "---\nid: sample\ntype: howto\ndomain: coding\nverification: unverified\n\
         updated: 2026-08-16\nsources: [raw/coding/source-note.md]\n---\n\n# Sample\n\n\
         This page is a sample; it anchors the test tree.\n",
    );
    w(
        "raw/coding/source-note.md",
        "# Raw note\n\nSource content.\n",
    );
    w(
        "raw/inbox/to-relocate.md",
        "# Unsorted note\n\nWill move.\n",
    );
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "kb tree"]);
    (repo, root)
}

fn r023_findings(root: &Path) -> Vec<(String, String)> {
    let (report, _) = docsys::lint(root);
    report
        .findings
        .iter()
        .filter(|f| f.rule.0 == "R-023")
        .map(|f| (f.file.clone(), f.subject.clone()))
        .collect()
}

#[test]
fn a_committed_kb_tree_lints_clean() {
    let (_repo, root) = build_kb("clean");
    let (report, outcome) = docsys::lint(&root);
    assert!(
        report.findings.is_empty(),
        "expected a clean tree, got: {:?}",
        report.findings
    );
    assert!(matches!(outcome, docsys::Outcome::Clean));
}

#[test]
fn modifying_a_tracked_raw_record_is_an_error() {
    let (_repo, root) = build_kb("modify");
    let f = root.join("raw/coding/source-note.md");
    let mut text = fs::read_to_string(&f).unwrap();
    text.push_str("a line added after the fact\n");
    fs::write(&f, text).unwrap();
    let hits = r023_findings(&root);
    assert_eq!(
        hits,
        vec![(
            "raw/coding/source-note.md".to_string(),
            "content".to_string()
        )],
        "modification must be reported as R-023"
    );
}

#[test]
fn relocating_a_raw_record_is_permitted() {
    let (_repo, root) = build_kb("relocate");
    // inbox → domain is R-023's expected flow: the bytes survive, the path moves.
    fs::rename(
        root.join("raw/inbox/to-relocate.md"),
        root.join("raw/coding/to-relocate.md"),
    )
    .unwrap();
    assert!(
        r023_findings(&root).is_empty(),
        "a relocation must not be read as a deletion"
    );
}

#[test]
fn deleting_a_raw_record_is_an_error() {
    let (_repo, root) = build_kb("delete");
    fs::remove_file(root.join("raw/inbox/to-relocate.md")).unwrap();
    let hits = r023_findings(&root);
    assert_eq!(
        hits,
        vec![(
            "raw/inbox/to-relocate.md".to_string(),
            "deleted".to_string()
        )],
        "deletion without relocation must be reported as R-023"
    );
}

#[test]
fn adopt_and_graduate_refuse_the_profile_honestly() {
    let (repo, root) = build_kb("refuse");
    let adopt = docsys::adopt::run(&repo, &root, "en");
    assert!(
        adopt.is_err() && adopt.unwrap_err().contains("knowledge-base"),
        "adopt must refuse a knowledge-base tree by name"
    );
    let plan = docsys::graduate::plan(&root, "wiki/coding/howto/sample.md");
    assert!(
        plan.is_err() && plan.unwrap_err().contains("distillation"),
        "graduate must name distillation (R-092), not half-run"
    );
}

#[test]
fn a_personal_base_stands_up_in_two_commands() {
    let base = tmp("greenfield");
    // 1. the tree
    docsys::migrate::init_profile(&base, "en", "knowledge-base").unwrap();
    assert!(base.join(".docmeta.yml").is_file());
    assert!(base.join("wiki/index.md").is_file());
    assert!(base.join("raw/inbox").is_dir());
    // an empty base is a clean base — nothing to report, nothing pretended
    let (report, outcome) = docsys::lint(&base);
    assert!(report.findings.is_empty(), "{:?}", report.findings);
    assert!(matches!(outcome, docsys::Outcome::Clean));
    // 2. the agent layer
    let claude = base.join(".claude");
    let done = docsys::agents::install_kb(&claude, &base, false).unwrap();
    assert_eq!(done.written.len(), 5, "{:?}", done.written);
    for organ in ["kb-capture", "kb-ingest", "kb-audit", "kb-lookup"] {
        assert!(claude.join(format!("skills/{organ}/SKILL.md")).is_file());
    }
    let constitution = fs::read_to_string(base.join("AGENTS.md")).unwrap();
    assert!(constitution.contains("capture"), "{constitution}");
    // the owner's AGENTS.md is never overwritten without --force
    fs::write(base.join("AGENTS.md"), "my own words\n").unwrap();
    let again = docsys::agents::install_kb(&claude, &base, false).unwrap();
    assert!(again.written.is_empty(), "{:?}", again.written);
    assert_eq!(
        fs::read_to_string(base.join("AGENTS.md")).unwrap(),
        "my own words\n"
    );
    // the project profile is unchanged by all this
    let proj = tmp("greenfield-proj");
    docsys::migrate::init_profile(&proj, "en", "project").unwrap();
    assert!(proj.join("work/journal.md").is_file());
    assert!(docsys::migrate::init_profile(&proj, "en", "nonsense").is_err());
}
