#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use docsys::{model::Severity, refs, tree::DocTree};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn write(root: &Path, rel: &str, text: &str) {
    let file = root.join(rel);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, text).unwrap();
}

fn fixture(name: &str, consumer: bool) -> PathBuf {
    let repo = std::env::temp_dir().join(format!("docsys-foreign-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(&repo).unwrap();
    let metadata = format!(
        "spec: docsys/0.5\nprofile: project\ndefault_content_language: en\nnamespace: consumer\nscan_exclude: [vendor/]\n{}",
        if consumer { "consume: [upstream=../provider#docs]\n" } else { "" }
    );
    write(&repo, "docs/.docmeta.yml", &metadata);
    write(&repo, "docs/index.md", "# Documentation\n");
    repo
}

fn foreign_findings(repo: &Path) -> Vec<(String, String, Severity)> {
    let tree = DocTree::load(&repo.join("docs")).unwrap();
    refs::run(repo, &tree)
        .findings
        .into_iter()
        .filter(|f| f.rule.0 == "R-076")
        .map(|f| (f.file, f.subject, f.severity))
        .collect()
}

#[test]
fn missing_foreign_citations_block_both_commands_without_fetching() {
    let repo = fixture("cli", true);
    assert!(Command::new("git")
        .args(["init", "-q"])
        .current_dir(&repo)
        .status()
        .unwrap()
        .success());
    write(
        &repo,
        "docs/index.md",
        "# Documentation\n\nSee doc: @upstream/missing.\n",
    );
    write(&repo, "src/main.rs", "// doc: @upstream/missing\n");
    for command in ["lint", "refs"] {
        let out = Command::new(env!("CARGO_BIN_EXE_docsys"))
            .args([command, "--root", "docs", "--json"])
            .current_dir(&repo)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        let stdout = String::from_utf8(out.stdout).unwrap();
        assert!(stdout.contains("\"severity\":\"ERROR\""), "{stdout}");
        assert!(stdout.contains("@upstream/missing"), "{stdout}");
    }
    assert!(!repo.join("docs/.federation").exists());
    fs::remove_dir_all(repo).unwrap();
}

#[test]
fn repository_markdown_checks_foreign_citations_and_respects_scan_boundaries() {
    let repo = fixture("markdown", true);
    write(&repo, "knowledge/relations.md", "# Relations\n\nSee doc: @upstream/known.\nAnd doc: @upstream/missing.\n\n```md\ndoc: @upstream/example\n```\n\nLocal prose: doc: unrelated-local-id\n");
    write(&repo, "vendor/notes.md", "doc: @upstream/excluded\n");
    write(
        &repo,
        "docs/index.md",
        "# Documentation\n\ndoc: @upstream/lint-owned\n",
    );
    write(
        &repo,
        "docs/.federation/upstream/known.md",
        "---\nid: known\ntype: reference\n---\n# Known\n\ndoc: @other/provider-owned\n",
    );
    let findings = foreign_findings(&repo);
    assert_eq!(
        findings,
        vec![(
            "knowledge/relations.md".into(),
            "@upstream/missing".into(),
            Severity::Error
        )]
    );
    fs::remove_dir_all(repo).unwrap();
}

#[test]
fn trees_without_federation_state_still_warn_and_snapshots_make_missing_ids_errors() {
    let repo = fixture("states", false);
    write(
        &repo,
        "docs/index.md",
        "# Documentation\n\ndoc: @upstream/missing\n",
    );
    write(&repo, "src/main.rs", "// doc: @upstream/missing\n");
    let (lint, _) = docsys::lint(&repo.join("docs"));
    assert!(lint
        .findings
        .iter()
        .any(|f| f.rule.0 == "R-076" && f.severity == Severity::Warn));
    assert_eq!(
        foreign_findings(&repo)
            .into_iter()
            .map(|f| f.2)
            .collect::<Vec<_>>(),
        vec![Severity::Warn]
    );
    fs::create_dir_all(repo.join("docs/.federation")).unwrap();
    let (lint, _) = docsys::lint(&repo.join("docs"));
    assert!(lint
        .findings
        .iter()
        .any(|f| f.rule.0 == "R-076" && f.severity == Severity::Error));
    assert_eq!(
        foreign_findings(&repo)
            .into_iter()
            .map(|f| f.2)
            .collect::<Vec<_>>(),
        vec![Severity::Error]
    );
    fs::remove_dir_all(repo).unwrap();
}

#[test]
fn restoring_a_materialized_page_clears_the_error_without_contacting_a_provider() {
    let repo = fixture("restore", true);
    write(&repo, "knowledge/map.md", "# Map\n\ndoc: @upstream/known\n");
    let page = "docs/.federation/upstream/known.md";
    write(&repo, page, "# A local snapshot\n");
    assert!(foreign_findings(&repo).is_empty());
    fs::remove_file(repo.join(page)).unwrap();
    assert_eq!(
        foreign_findings(&repo)
            .into_iter()
            .map(|f| f.2)
            .collect::<Vec<_>>(),
        vec![Severity::Error]
    );
    write(&repo, page, "# A local snapshot\n");
    assert!(foreign_findings(&repo).is_empty());
    assert!(!repo.join("docs/.federation/.checkouts").exists());
    fs::remove_dir_all(repo).unwrap();
}

#[test]
fn version_control_ignored_markdown_does_not_introduce_citations() {
    let repo = fixture("ignored", true);
    assert!(Command::new("git")
        .args(["init", "-q"])
        .current_dir(&repo)
        .status()
        .unwrap()
        .success());
    write(&repo, ".gitignore", "ignored/\n");
    write(&repo, "ignored/map.md", "doc: @upstream/ignored\n");
    write(&repo, "knowledge/map.md", "doc: @upstream/missing\n");
    assert_eq!(
        foreign_findings(&repo),
        vec![(
            "knowledge/map.md".into(),
            "@upstream/missing".into(),
            Severity::Error
        )]
    );
    fs::remove_dir_all(repo).unwrap();
}

#[test]
fn legacy_local_markdown_citations_keep_the_existing_code_reference_policy() {
    let repo = fixture("legacy-markdown", false);
    let meta = repo.join("docs/.docmeta.yml");
    let text = fs::read_to_string(&meta)
        .unwrap()
        .replace("docsys/0.5", "docsys/0.4");
    fs::write(meta, text).unwrap();
    write(&repo, "knowledge/map.md", "See doc: missing-local-id.\n");
    assert_eq!(
        foreign_findings(&repo),
        vec![(
            "knowledge/map.md".into(),
            "missing-local-id".into(),
            Severity::Error
        )]
    );
    fs::remove_dir_all(repo).unwrap();
}
