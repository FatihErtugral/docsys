#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// A howto compiled into a skill (R-094), pinned to its source (R-095), and
// the lint that fails when the page moves. No git needed: the hash is over
// the page body, and the skills live beside the docs root.

use docsys::{compile, lint_in, model::Severity};
use std::fs;
use std::path::{Path, PathBuf};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-compile-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

const RELEASE_MD: &str = "---\nid: release\ntype: howto\nupdated: 2026-09-02\n---\n# Release a version\n\n\
This page lists the steps that publish a version; read it before tagging.\n\n\
1. Bump `version` in the manifest.\n2. Move the changelog's unreleased section under the version and date.\n\
3. Commit `release <version>`, tag `v<version>`, push both.\n4. Publish to the registry.\n";

fn project(name: &str) -> (PathBuf, PathBuf) {
    let repo = tmp(name);
    let docs = repo.join("docs");
    docsys::migrate::init_profile(&docs, "en", "project").unwrap();
    fs::create_dir_all(docs.join("howto")).unwrap();
    fs::create_dir_all(docs.join("reference")).unwrap();
    fs::write(docs.join("howto/release.md"), RELEASE_MD).unwrap();
    fs::write(
        docs.join("reference/registry.md"),
        "---\nid: registry\ntype: reference\nupdated: 2026-09-02\n---\n# Registry\n\nThis page names the registry; read it before publishing.\n\nThe registry is crates.io.\n",
    )
    .unwrap();
    let index = docs.join("index.md");
    let mut text = fs::read_to_string(&index).unwrap();
    text.push_str(
        "\n- [[howto/release|Release a version]] -- the publishing steps.\n\
         - [[reference/registry|Registry]] -- where versions go.\n",
    );
    fs::write(&index, text).unwrap();
    (repo, docs)
}

fn errors(docs: &Path, repo: &Path) -> Vec<String> {
    let (r, _) = lint_in(docs, Some(repo));
    r.findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| format!("{} {} {}", f.rule, f.file, f.subject))
        .collect()
}

#[test]
fn a_howto_compiles_verbatim_and_goes_stale_with_its_page() {
    let (repo, docs) = project("basic");
    let claude = repo.join(".claude");
    let msg = compile::compile(&docs, &claude, "release", false).unwrap();
    assert!(msg.contains("sha256:"), "{msg}");
    let skill = fs::read_to_string(claude.join("skills/release/SKILL.md")).unwrap();
    assert!(skill.starts_with("---\nname: release\n"), "{skill}");
    assert!(skill.contains("docsys_source: release\n"), "{skill}");
    assert!(skill.contains("docsys_source_hash: sha256:"), "{skill}");
    // the body is the page, byte for byte, after the tool's own comment
    let body = RELEASE_MD.split("---\n").nth(2).unwrap();
    assert!(skill.ends_with(body), "{skill}");
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );

    // the page moves: the skill is stale, an error naming the page
    fs::write(
        docs.join("howto/release.md"),
        RELEASE_MD.replace(
            "4. Publish to the registry.",
            "4. Publish to the registry.\n5. Check the release workflow.",
        ),
    )
    .unwrap();
    let errs = errors(&docs, &repo);
    assert!(
        errs.contains(&"R-095 .claude/skills/release/SKILL.md release".to_string()),
        "{errs:?}"
    );

    // compiled again from the page as it now reads: current
    compile::compile(&docs, &claude, "howto/release", false).unwrap();
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );
    assert!(fs::read_to_string(claude.join("skills/release/SKILL.md"))
        .unwrap()
        .contains("5. Check the release workflow."));

    // the page is gone: still an error, saying so
    fs::remove_file(docs.join("howto/release.md")).unwrap();
    let (r, _) = lint_in(&docs, Some(&repo));
    assert!(
        r.findings
            .iter()
            .any(|f| f.rule.0 == "R-095" && f.message.contains("no longer exists")),
        "{:?}",
        r.findings
    );
}

#[test]
fn only_a_howto_compiles_and_an_authored_skill_is_kept() {
    let (repo, docs) = project("refusals");
    let claude = repo.join(".claude");
    let err = compile::compile(&docs, &claude, "registry", false).unwrap_err();
    assert!(err.contains("only a `howto`"), "{err}");
    let err = compile::compile(&docs, &claude, "ghost", false).unwrap_err();
    assert!(err.contains("no page"), "{err}");

    // a person's own skill under the same name is never clobbered silently
    fs::create_dir_all(claude.join("skills/release")).unwrap();
    fs::write(
        claude.join("skills/release/SKILL.md"),
        "---\nname: release\ndescription: mine\n---\nMy own steps.\n",
    )
    .unwrap();
    let err = compile::compile(&docs, &claude, "release", false).unwrap_err();
    assert!(err.contains("--force"), "{err}");
    assert!(
        errors(&docs, &repo).is_empty(),
        "an authored skill is not checked"
    );
    compile::compile(&docs, &claude, "release", true).unwrap();
    assert!(fs::read_to_string(claude.join("skills/release/SKILL.md"))
        .unwrap()
        .contains("docsys_source: release"));
}

#[test]
fn a_knowledge_base_howto_compiles_only_when_verified() {
    let base = tmp("kb");
    docsys::migrate::init_profile(&base, "en", "knowledge-base").unwrap();
    let dm = base.join(".docmeta.yml");
    let text = fs::read_to_string(&dm)
        .unwrap()
        .replace("domains: []", "domains: [ops]");
    fs::write(&dm, text).unwrap();
    fs::create_dir_all(base.join("wiki/ops/howto")).unwrap();
    fs::create_dir_all(base.join("raw/ops")).unwrap();
    fs::write(
        base.join("raw/ops/2026-09-02-note.md"),
        "The steps, as noted.\n",
    )
    .unwrap();
    let page = base.join("wiki/ops/howto/rotate-keys.md");
    fs::write(
        &page,
        "---\nid: rotate-keys\ntype: howto\ndomain: ops\nverification: unverified\nupdated: 2026-09-02\nsources: [raw/ops/2026-09-02-note.md]\n---\n# Rotate keys\n\nThis page lists the rotation steps; read it before rotating.\n\n1. Generate the new key.\n2. Swap it in.\n",
    )
    .unwrap();
    fs::write(
        base.join("wiki/ops/index.md"),
        "# ops\n\n- [[ops/howto/rotate-keys|Rotate keys]] -- the rotation steps.\n",
    )
    .unwrap();
    fs::write(
        base.join("wiki/index.md"),
        "# Knowledge base\n\n- [[ops/index|Ops]] -- operations.\n",
    )
    .unwrap();
    let claude = base.join(".claude");
    let err = compile::compile(&base, &claude, "rotate-keys", false).unwrap_err();
    assert!(err.contains("verified"), "{err}");
    let text = fs::read_to_string(&page).unwrap().replace(
        "verification: unverified",
        "verification: verified\nverified_by: auditor\nverified_rev: abc123",
    );
    fs::write(&page, text).unwrap();
    compile::compile(&base, &claude, "rotate-keys", false).unwrap();
    assert!(claude.join("skills/rotate-keys/SKILL.md").is_file());
}
