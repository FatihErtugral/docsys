#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// Tests report through panics by design; the production lints stay strict.

use docsys::{refs, tree::DocTree};
use std::fs;
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-refs-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

#[test]
fn code_refs_resolve_dangle_and_respect_scan_exclude() {
    let repo = tmp("basic");
    let docs = repo.join("docs");
    fs::create_dir_all(docs.join("reference")).unwrap();
    fs::write(
        docs.join(".docmeta.yml"),
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\nscan_exclude: [vendor/]\n",
    )
    .unwrap();
    fs::write(
        docs.join("reference/token-ttl.md"),
        "---\nid: token-ttl\ntype: reference\nupdated: 2026-08-15\ndefines: adr-*\n---\nBody.\n\nadr-0042 is defined here.\n",
    )
    .unwrap();
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(
        repo.join("src/auth.rs"),
        "// doc: token-ttl\n// doc: adr-0042\n// doc: adr-9999\n// doc: no-such-id.\n// see htmldoc: token-ttl (guarded)\n",
    )
    .unwrap();
    fs::create_dir_all(repo.join("vendor")).unwrap();
    fs::write(repo.join("vendor/x.c"), "// doc: totally-dangling\n").unwrap();

    let tree = DocTree::load(&docs).unwrap();
    let report = refs::run(&repo, &tree);
    let errs: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.severity == docsys::model::Severity::Error)
        .map(|f| f.subject.clone())
        .collect();
    // adr-9999: prefix matches but the member does not occur on the page (R-079).
    // no-such-id: plain dangling, trailing period stripped (R-073).
    // vendor/: excluded by the tree's own scan_exclude (R-077).
    assert_eq!(errs, vec!["adr-9999".to_string(), "no-such-id".to_string()], "{errs:?}");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn agents_install_writes_assets_and_respects_existing() {
    let dir = tmp("agents").join(".claude");
    let done = docsys::agents::install(&dir, false).unwrap();
    assert_eq!(done.written.len(), 6, "{:?}", done.written);
    // Second run without --force skips everything.
    let again = docsys::agents::install(&dir, false).unwrap();
    assert_eq!(again.skipped.len(), 6);
    let skill = fs::read_to_string(dir.join("skills/docsys/SKILL.md")).unwrap();
    assert!(skill.contains("docsys rules --procedures"));
    let _ = fs::remove_dir_all(dir.parent().unwrap_or(&dir));
}

#[test]
fn agents_md_managed_block_is_idempotent_and_preserves_owner_prose() {
    let dir = tmp("managed");
    let path = dir.join("AGENTS.md");
    fs::write(&path, "# My constitution\n\nOwner prose stays.\n").unwrap();
    docsys::rules::write_agents_block(&path).unwrap();
    let once = fs::read_to_string(&path).unwrap();
    assert!(once.starts_with("# My constitution"), "owner prose first");
    assert!(once.contains("docsys:rules:begin"));
    docsys::rules::write_agents_block(&path).unwrap();
    let twice = fs::read_to_string(&path).unwrap();
    assert_eq!(once, twice, "second run must change nothing");
    assert_eq!(twice.matches("docsys:rules:begin").count(), 1, "one block, updated in place");
    let _ = fs::remove_dir_all(&dir);
}
