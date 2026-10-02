#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// A project's raw/ is its optional record layer (D-112): records are
// content-immutable at the gate (R-023), counted by `status` without an
// inbox, and `inbox add` names how to start one. R-023 needs a git working
// tree (D-031), so these tests build one, as tests/kb.rs does for a base.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-rawproj-{name}-{}", std::process::id()));
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

/// A committed project tree at `<repo>/docs` holding two records, one of
/// them cited by a page.
fn build_project(name: &str) -> (PathBuf, PathBuf) {
    let repo = tmp(name);
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    let root = repo.join("docs");
    let w = |rel: &str, text: &str| {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    };
    w(
        ".docmeta.yml",
        "spec: docsys/0.5\nprofile: project\ndefault_content_language: en\n",
    );
    w(
        "index.md",
        "# Docs\n\n- [[reference/cache|Cache]] -- What the cache interview settled.\n",
    );
    w(
        "reference/cache.md",
        &format!(
            "---\nid: cache\ntype: reference\nupdated: {}\nsources: [raw/inbox/answers.md]\n---\n\
             This page holds what the cache interview settled; read it before changing the cache.\n",
            docsys::migrate::today()
        ),
    );
    w(
        "raw/inbox/answers.md",
        "# Answers\n\nThe cache lives a day.\n",
    );
    w("raw/inbox/aside.md", "# Aside\n\nNobody cites this yet.\n");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "project tree"]);
    (repo, root)
}

fn r023(root: &Path, repo: &Path) -> Vec<(String, String)> {
    let (report, _) = docsys::lint_in(root, Some(repo));
    report
        .findings
        .iter()
        .filter(|f| f.rule.0 == "R-023")
        .map(|f| (f.file.clone(), f.subject.clone()))
        .collect()
}

#[test]
fn an_edited_tracked_record_is_an_error_at_lint() {
    let (repo, root) = build_project("edit");
    let f = root.join("raw/inbox/answers.md");
    let mut text = fs::read_to_string(&f).unwrap();
    text.push_str("A line added after the fact.\n");
    fs::write(&f, text).unwrap();
    assert_eq!(
        r023(&root, &repo),
        vec![("raw/inbox/answers.md".to_string(), "content".to_string())]
    );
    let (_, outcome) = docsys::lint_in(&root, Some(&repo));
    assert!(matches!(outcome, docsys::Outcome::Errors));
}

#[test]
fn a_new_untracked_record_is_not_a_finding() {
    let (repo, root) = build_project("new");
    fs::write(root.join("raw/inbox/later.md"), "# Later\n\nJust landed.\n").unwrap();
    assert!(r023(&root, &repo).is_empty());
}

#[test]
fn status_counts_records_and_never_calls_them_waiting() {
    let (repo, root) = build_project("status");
    let s = docsys::status::status(&root, Some(&repo)).unwrap();
    let text = docsys::status::render(&s, &root);
    assert!(text.contains("records: 2 (1 cited by no page)\n"), "{text}");
    assert!(!text.contains("inbox"), "{text}");
    assert!(!text.contains("waiting"), "{text}");
    let json = docsys::status::render_json(&s);
    assert!(
        json.contains("\"records\":2,\"records_uncited\":1"),
        "{json}"
    );
}

#[test]
fn inbox_add_in_a_project_without_records_names_both_ways() {
    let (_repo, root) = build_project("inbox");
    fs::remove_dir_all(root.join("raw")).unwrap();
    let p = docsys::inbox::Provenance {
        source: "interview".into(),
        source_id: "q-2".into(),
        title: "Second answers".into(),
        url: None,
        date: "2026-09-21".into(),
    };
    let err = docsys::inbox::add(&root, &p, "").unwrap_err();
    assert!(
        err.contains(&format!("mkdir -p {}/raw/inbox", root.display())),
        "{err}"
    );
    assert!(err.contains("--profile knowledge-base"), "{err}");
    fs::create_dir_all(root.join("raw/inbox")).unwrap();
    assert_eq!(
        docsys::inbox::add(&root, &p, "").unwrap(),
        "captured: raw/inbox/2026-09-21-interview-second-answers.md"
    );
}
