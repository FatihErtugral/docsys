#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! A docsys/0.5 tree reaches a page under a type directory by its layout
//! (R-034, D-123): no route line is written for a type directory, so none is
//! a route to nothing and no two branches meet on one. The lines an earlier
//! 0.16 build appended leave, exactly those; a 0.4 tree reads as before.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-routes-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
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

fn docsys(dir: &Path, args: &[&str]) -> Output {
    // this build first on PATH: the git gate a command commits through runs it
    let mut dirs = vec![bin().parent().unwrap().to_path_buf()];
    dirs.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    Command::new(bin())
        .args(args)
        .current_dir(dir)
        .env("PATH", std::env::join_paths(dirs).unwrap())
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .output()
        .unwrap()
}

/// The route lines an earlier 0.16 build wrote, exactly.
const WRITTEN: [&str; 4] = [
    "- [[reference/|Reference]] -- Facts the code cannot state: values, limits, formats.",
    "- [[howto/|How-to]] -- Procedures that reach a goal, step by step.",
    "- [[explanation/|Explanation]] -- Why things are the way they are: decisions and their reasons.",
    "- [[tutorial/|Tutorial]] -- Guided learning from zero.",
];

fn orphans(root: &Path) -> Vec<String> {
    let (report, _) = docsys::lint(root);
    report
        .findings
        .iter()
        .filter(|f| f.rule.0 == "R-034")
        .map(|f| f.file.clone())
        .collect()
}

#[test]
fn a_new_tree_routes_no_type_directory_and_reaches_its_pages() {
    let root = tmp("init").join("docs");
    docsys::migrate::init_profile(&root, "en", "project").unwrap();
    let index = fs::read_to_string(root.join("index.md")).unwrap();
    for line in WRITTEN {
        assert!(!index.contains(line), "{index}");
    }
    fs::create_dir_all(root.join("howto")).unwrap();
    fs::write(
        root.join("howto/release.md"),
        "---\nid: release\ntype: howto\n---\n# Release\n\nThis page lists the release steps; read it before tagging.\n",
    )
    .unwrap();
    assert!(orphans(&root).is_empty(), "{:?}", orphans(&root));
}

#[test]
fn a_0_4_tree_still_routes_its_pages_by_its_lines() {
    let root = tmp("v04").join("docs");
    fs::create_dir_all(root.join("howto")).unwrap();
    fs::write(
        root.join(".docmeta.yml"),
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n",
    )
    .unwrap();
    fs::write(root.join("index.md"), "# Documentation\n").unwrap();
    fs::write(
        root.join("howto/release.md"),
        "---\nid: release\ntype: howto\n---\n# Release\n\nThis page lists the release steps; read it before tagging.\n",
    )
    .unwrap();
    assert_eq!(orphans(&root), ["howto/release.md"]);
}

#[test]
fn an_upgrade_takes_out_the_lines_docsys_wrote_and_no_other() {
    let repo = tmp("upgrade");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    let root = repo.join("docs");
    docsys::migrate::init_profile(&root, "en", "project").unwrap();
    let own = "- [[reference/|Reference]] -- Our limits and formats.";
    let index = format!(
        "# Documentation\n\n## History\n\n- [[explanation/why|Why]] -- How it came to be.\n{}\n\n## Ours\n\n{own}\n",
        WRITTEN.join("\n")
    );
    fs::write(root.join("index.md"), &index).unwrap();
    fs::create_dir_all(root.join("explanation")).unwrap();
    fs::write(
        root.join("explanation/why.md"),
        "---\nid: why\ntype: explanation\n---\n# Why\n\nThis page explains how it came to be.\n",
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    git(
        &repo,
        &["commit", "-qm", "a tree an earlier 0.16 build routed"],
    );
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    let now = fs::read_to_string(root.join("index.md")).unwrap();
    assert_eq!(
        now,
        format!("# Documentation\n\n## History\n\n- [[explanation/why|Why]] -- How it came to be.\n\n## Ours\n\n{own}\n"),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(orphans(&root).is_empty(), "{:?}", orphans(&root));
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_new_page_under_a_type_directory_is_not_told_to_route_itself() {
    let root = tmp("page-new").join("docs");
    docsys::migrate::init_profile(&root, "en", "project").unwrap();
    fs::write(root.join("index.md"), "# Documentation\n").unwrap();
    docsys::capture::page_new(&root, "howto", "release", None, None, false).unwrap();
    let page = fs::read_to_string(root.join("howto/release.md")).unwrap();
    assert!(!page.contains("route it"), "{page}");
}

/// A type's folder appears with its first page: `page new` makes it, and the
/// layout reaches it with no index line (R-043, D-123).
#[test]
fn a_type_folder_appears_with_its_first_page() {
    let root = tmp("first-page").join("docs");
    docsys::migrate::init_profile(&root, "en", "project").unwrap();
    assert!(!root.join("howto").exists());
    docsys::capture::page_new(&root, "howto", "rotate-keys", None, None, false).unwrap();
    assert!(root.join("howto/rotate-keys.md").is_file());
    let page = root.join("howto/rotate-keys.md");
    let text = fs::read_to_string(&page).unwrap();
    fs::write(
        &page,
        text.replace(
            &text[text.find("<!--").unwrap()..text.find("-->").unwrap() + 3],
            "This page lists the key rotation steps; read it before rotating.",
        ),
    )
    .unwrap();
    assert!(orphans(&root).is_empty(), "{:?}", orphans(&root));
    let index = fs::read_to_string(root.join("index.md")).unwrap();
    assert_eq!(index, "# Documentation\n");
}

/// A docsys/0.5 page carries no verification: no command writes a state
/// field into a page, and none asks for one (D-130).
#[test]
fn no_0_5_command_writes_or_asks_for_a_page_state_field() {
    let repo = tmp("no-state");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/a.rs"), "pub fn a() {}\n").unwrap();
    let out = docsys(&repo, &["adopt"]);
    assert!(out.status.success(), "{out:?}");
    let root = repo.join("docs");
    for args in [
        &["page", "new", "reference", "plain"][..],
        &["page", "new", "reference", "drafted"],
    ] {
        let out = docsys(&repo, args);
        assert!(out.status.success(), "{args:?}: {out:?}");
    }
    let out = docsys(
        &repo,
        &["page", "new", "reference", "marked", "--unverified"],
    );
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(!root.join("reference/marked.md").exists());
    for id in ["plain", "drafted"] {
        let p = root.join(format!("reference/{id}.md"));
        let text = fs::read_to_string(&p).unwrap();
        let text = text.replace("sources: []", "sources: [src/a.rs]").replace(
            &text[text.find("<!--").unwrap()..text.find("-->").unwrap() + 3],
            "This page states what a does; read it before changing it.",
        );
        let text = if text.contains("sources:") {
            text
        } else {
            text.replacen(
                "type: reference\n",
                "type: reference\nsources: [src/a.rs]\n",
                1,
            )
        };
        fs::write(&p, text).unwrap();
    }
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "two pages"]);
    let head = || {
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo)
            .output()
            .unwrap()
            .stdout
    };
    let before = head();
    let out = docsys(&repo, &["verify", "reference/plain", "--commit"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert_eq!(head(), before);
    // a migration into the tree
    fs::create_dir_all(repo.join("notes")).unwrap();
    fs::write(
        repo.join("notes/setup.md"),
        "# Setup\n\nThis page says how to set it up.\n",
    )
    .unwrap();
    fs::write(repo.join("plan.tsv"), "setup.md\thowto\n").unwrap();
    let out = docsys(
        &repo,
        &["migrate", "apply", "--plan", "plan.tsv", "--root", "notes"],
    );
    assert!(repo.join("notes/howto/setup.md").is_file(), "{out:?}");
    for page in [
        root.join("reference/plain.md"),
        root.join("reference/drafted.md"),
        repo.join("notes/howto/setup.md"),
    ] {
        let text = fs::read_to_string(&page).unwrap();
        for field in [
            "verification:",
            "verified_by:",
            "verified_rev:",
            "verified_blocks:",
            "verified_sources:",
        ] {
            assert!(!text.contains(field), "{}: {field}\n{text}", page.display());
        }
    }
    // and lint asks for none
    let (report, _) = docsys::lint(&root);
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.message.contains("verification")),
        "{:?}",
        report.findings
    );
    let _ = fs::remove_dir_all(&repo);
}
