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
    assert_eq!(
        errs,
        vec!["adr-9999".to_string(), "no-such-id".to_string()],
        "{errs:?}"
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_code_citation_opens_a_comment_and_doc_mid_sentence_is_prose() {
    let repo = tmp("positional");
    let docs = repo.join("docs");
    fs::create_dir_all(docs.join("reference")).unwrap();
    fs::write(
        docs.join(".docmeta.yml"),
        "spec: docsys/0.5\nprofile: project\ndefault_content_language: en\n",
    )
    .unwrap();
    fs::write(
        docs.join("reference/real-id.md"),
        "---\nid: real-id\ntype: reference\nupdated: 2026-10-02\n---\nBody.\n",
    )
    .unwrap();
    fs::create_dir_all(repo.join("src")).unwrap();
    for (file, text) in [
        ("src/line.rs", "// doc: real-id\nfn a() {}\n"),
        ("src/trailing.rs", "fn b() {\n    x(); // doc: real-id\n}\n"),
        (
            "src/block.js",
            "/**\n * doc: real-id\n */\nfunction c() {}\n",
        ),
        (
            "src/doc.py",
            "def d():\n    \"\"\"Read the page.\n\n    doc: real-id\n    \"\"\"\n",
        ),
        ("src/prose.sh", "# see the provider doc: it caps\nexit 0\n"),
        ("src/config.yml", "api_doc: foo\n"),
        ("src/string.rs", "let s = \"doc: x\";\n"),
        ("src/ghost.rs", "// doc: ghost\n"),
    ] {
        fs::write(repo.join(file), text).unwrap();
    }
    let tree = DocTree::load(&docs).unwrap();
    let report = refs::run(&repo, &tree);
    let found: Vec<(String, String, String)> = report
        .findings
        .iter()
        .map(|f| {
            (
                f.severity.tag().to_string(),
                f.file.clone(),
                f.subject.clone(),
            )
        })
        .collect();
    assert_eq!(
        found,
        vec![("ERROR".into(), "src/ghost.rs".into(), "ghost".into())],
        "{:?}",
        report.findings
    );
    // four citations of real-id and the ghost: nothing else read as one
    assert_eq!(report.inspected.get("code-doc-refs"), Some(&5));
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn agents_install_writes_assets_and_respects_existing() {
    // a docsys/0.5 tree: its three relays, the commands and the skills
    let base = tmp("agents");
    fs::create_dir_all(base.join("docs")).unwrap();
    fs::write(
        base.join("docs/.docmeta.yml"),
        "spec: docsys/0.5\nprofile: project\n",
    )
    .unwrap();
    let dir = base.join(".claude");
    let done = docsys::agents::install(&dir, false).unwrap();
    assert_eq!(done.written.len(), 10, "{:?}", done.written);
    assert!(!done.written.iter().any(|f| f.contains("post-edit")));
    // Second run without --force skips everything.
    let again = docsys::agents::install(&dir, false).unwrap();
    assert_eq!(again.skipped.len(), 10);
    // what people know is the interview's; what code and history say is the
    // seed's — each command's description says which, and the interview ends
    // by naming the next step
    let interview = fs::read_to_string(dir.join("commands/docsys-interview.md")).unwrap();
    assert!(
        interview.contains("description: Collect what people know"),
        "{interview}"
    );
    assert!(
        interview.contains("`docsys graduate plan <work-file>`"),
        "{interview}"
    );
    let seed = fs::read_to_string(dir.join("commands/docsys-seed.md")).unwrap();
    assert!(
        seed.contains("from what its code and history say"),
        "{seed}"
    );
    // the procedure an agent follows to finish an upgrade with the person
    // (D-104); the rules it rests on are the block's, said there once (D-129)
    let upgrade = fs::read_to_string(dir.join("commands/docsys-upgrade.md")).unwrap();
    for must in [
        "docsys upgrade --json",
        "re-read it as the rules block says",
        "A sha256 value is never written",
        "`SHA256SUMS`",
        "git log -1 --format=%B",
    ] {
        assert!(upgrade.contains(must), "{must}");
    }
    // a docsys/0.5 page carries no verification (D-130)
    assert!(!upgrade.contains("verif"), "{upgrade}");
    assert!(
        upgrade.lines().count() <= 40,
        "{} lines",
        upgrade.lines().count()
    );
    let skill = fs::read_to_string(dir.join("skills/docsys/SKILL.md")).unwrap();
    // the skill holds its procedures; the rules block holds the rules (one home)
    assert!(skill.contains("docsys migrate inventory"));
    assert!(!skill.contains("docsys rules --procedures"));
    // a repository is set up by `adopt`, whose help says what it writes; a
    // block appended by hand has no markers and doubles on a re-run
    assert!(skill.contains("`docsys adopt`"), "{skill}");
    assert!(!skill.contains("rules --agents-md >>"), "{skill}");
    let export = fs::read_to_string(dir.join("skills/docsys-export/SKILL.md")).unwrap();
    assert!(export.contains("--audience"));
    assert!(export.contains("P/R-123"));
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
    assert_eq!(
        twice.matches("docsys:rules:begin").count(),
        1,
        "one block, updated in place"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn stray_docs_page_outside_tree_is_reported() {
    let repo = tmp("stray");
    let docs = repo.join("docs");
    fs::create_dir_all(&docs).unwrap();
    fs::write(
        docs.join(".docmeta.yml"),
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n",
    )
    .unwrap();
    fs::create_dir_all(repo.join("components/ui")).unwrap();
    fs::write(
        repo.join("components/ui/README.md"),
        "---\nid: ui-guide\ntype: howto\nupdated: 2026-08-15\n---\nA page pretending outside the tree.\n",
    )
    .unwrap();
    let tree = DocTree::load(&docs).unwrap();
    let report = refs::run(&repo, &tree);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.subject == "stray" && f.file == "components/ui/README.md"),
        "{:?}",
        report.findings
    );
    let _ = fs::remove_dir_all(&repo);
}

#[cfg(unix)] // the alias is built with a symlink; the invariant is platform-neutral
#[test]
fn repo_walk_excludes_docs_root_under_any_spelling() {
    let repo = tmp("rootspell");
    let docs = repo.join("docs");
    fs::create_dir_all(&docs).unwrap();
    fs::write(docs.join("page.md"), "x\n").unwrap();
    fs::write(repo.join("code.c"), "y\n").unwrap();
    // The same tree under a different spelling (the `--repo . --root docs`
    // class of mismatch): exclusion must hold by identity, not by string.
    let alias = repo.join("docs-alias");
    std::os::unix::fs::symlink(&docs, &alias).unwrap();
    let files = docsys::migrate::repo_text_files(&repo, &alias);
    assert!(
        files.iter().all(|f| !f.starts_with(&docs)),
        "docs tree leaked into the code walk: {files:?}"
    );
    assert!(files.iter().any(|f| f.ends_with("code.c")));
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn the_repo_walk_asks_git_and_honors_gitignore() {
    let repo = tmp("gitignore");
    let docs = repo.join("docs");
    fs::create_dir_all(&docs).unwrap();
    fs::write(docs.join("page.md"), "x\n").unwrap();
    fs::create_dir_all(repo.join("build/nested")).unwrap();
    fs::write(repo.join("build/nested/generated.c"), "// doc: ghost\n").unwrap();
    fs::create_dir_all(repo.join("vendored")).unwrap();
    fs::write(repo.join("vendored/copy.c"), "// doc: ghost\n").unwrap();
    fs::write(repo.join("code.c"), "// doc: real\n").unwrap();
    // `vendored/` is ignored by name, not by any built-in skip list: only the
    // project's own .gitignore knows it does not belong to the project.
    fs::write(repo.join(".gitignore"), "vendored/\nbuild/\n").unwrap();
    assert!(std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&repo)
        .status()
        .unwrap()
        .success());

    let files = docsys::migrate::repo_text_files(&repo, &docs);
    let names: Vec<String> = files
        .iter()
        .map(|f| {
            f.strip_prefix(&repo)
                .unwrap_or(f)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    assert!(names.contains(&"code.c".to_string()), "{names:?}");
    assert!(
        !names.iter().any(|n| n.starts_with("vendored/")),
        "{names:?}"
    );
    assert!(!names.iter().any(|n| n.starts_with("build/")), "{names:?}");
    assert!(!names.iter().any(|n| n.starts_with("docs/")), "{names:?}");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn scan_exclude_spellings_reduce_to_a_prefix_and_globs_are_named() {
    use docsys::tree::{scan_prefix, under_prefix};
    for e in ["spec", "spec/", "./spec", "./spec/", "spec/**", "./spec/**"] {
        assert_eq!(scan_prefix(e).as_deref(), Ok("spec"), "{e}");
    }
    assert_eq!(scan_prefix("tools/vendor/").as_deref(), Ok("tools/vendor"));
    for e in ["**/spec/**", "../spec", "spec/*.md", "", "./"] {
        assert!(scan_prefix(e).is_err(), "{e}");
    }
    assert!(under_prefix("spec/a.md", "spec"));
    assert!(under_prefix("spec", "spec"));
    assert!(!under_prefix("specification.md", "spec"));
}

/// `docsys refs` takes the repository from the tree, as its help and
/// ADOPTION.md say: the bare command CI is told to run works as written.
#[test]
fn refs_without_repo_reads_the_trees_repository() {
    let repo = tmp("refs-bare");
    let git = |args: &[&str]| {
        assert!(std::process::Command::new("git")
            .args(args)
            .current_dir(&repo)
            .output()
            .unwrap()
            .status
            .success());
    };
    git(&["init", "-q"]);
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_docsys"))
            .args(args)
            .current_dir(&repo)
            .env("DOCSYS_NO_AUTO_INSTALL", "1")
            .output()
            .unwrap()
    };
    assert!(run(&["adopt"]).status.success());
    let adoption = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    assert!(adoption.contains("`docsys refs`"), "{adoption}");
    let out = run(&["refs"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let _ = fs::remove_dir_all(&repo);
}

/// D-098: a given `--repo` is its repository's top level, so `refs --repo .`
/// from a subdirectory — the command the agent texts name — inspects what
/// `refs` at the top inspects.
#[test]
fn refs_from_a_subdirectory_sees_what_refs_at_the_top_sees() {
    let repo = tmp("refs-sub");
    let git = |args: &[&str]| {
        assert!(std::process::Command::new("git")
            .args(args)
            .current_dir(&repo)
            .output()
            .unwrap()
            .status
            .success());
    };
    git(&["init", "-q"]);
    let run = |dir: &std::path::Path, args: &[&str]| {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_docsys"))
            .args(args)
            .current_dir(dir)
            .env("DOCSYS_NO_AUTO_INSTALL", "1")
            .output()
            .unwrap();
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
        )
    };
    assert_eq!(run(&repo, &["adopt"]).0, Some(0));
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/f.rs"), "// doc: nonexistent-page\n").unwrap();
    let sub = repo.join("pkg/sub");
    fs::create_dir_all(&sub).unwrap();
    let top = run(&repo, &["refs"]);
    assert_eq!(top.0, Some(1), "{}", top.1);
    assert!(top.1.contains("src/f.rs [nonexistent-page]"), "{}", top.1);
    for args in [&["refs", "--repo", "."][..], &["refs"][..]] {
        assert_eq!(run(&sub, args), top, "{args:?}");
    }
    let _ = fs::remove_dir_all(&repo);
}
