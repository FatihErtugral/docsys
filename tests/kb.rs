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
    // four organs, AGENTS.md, four hook relays, settings.json (D-076)
    assert_eq!(done.written.len(), 10, "{:?}", done.written);
    assert!(claude.join("hooks/pre-commit-docs.sh").is_file());
    assert!(claude.join("settings.json").is_file());
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

// ── `docsys raw move` (R-027, D-085): the relocation is one command ─────────

fn error_lines(root: &Path) -> Vec<String> {
    let (report, _) = docsys::lint(root);
    report
        .findings
        .iter()
        .filter(|f| f.severity == docsys::model::Severity::Error)
        .map(|f| format!("{} {} [{}]", f.rule.0, f.file, f.subject))
        .collect()
}

fn head_short(repo: &Path) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(repo)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn write_in(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn route(root: &Path, line: &str) {
    let idx = root.join("wiki/index.md");
    let mut text = fs::read_to_string(&idx).unwrap();
    text.push_str(line);
    fs::write(idx, text).unwrap();
}

#[test]
fn raw_move_rewrites_sources_and_keeps_the_verification() {
    let (repo, root) = build_kb("rawmove");
    let today = docsys::migrate::today();
    let body =
        "# Moved\n\nThis page states what the unsorted note said; read it first.\n\nWill move.\n";
    write_in(
        &root,
        "wiki/coding/reference/moved.md",
        &format!(
            "---\nid: moved\ntype: reference\ndomain: coding\nverification: unverified\n\
             updated: {today}\nsources: [raw/inbox/to-relocate.md]\n---\n{body}"
        ),
    );
    route(
        &root,
        "- [[coding/reference/moved|Moved]] -- What the note said.\n",
    );
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "page"]);
    // an audit in another session: the record names the revision that holds the body
    let rev = head_short(&repo);
    write_in(
        &root,
        "wiki/coding/reference/moved.md",
        &format!(
            "---\nid: moved\ntype: reference\ndomain: coding\nverification: verified\n\
             verified_by: auditor\nverified_rev: {rev}\nupdated: {today}\n\
             sources: [raw/inbox/to-relocate.md]\n---\n{body}"
        ),
    );
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "audit"]);
    assert!(error_lines(&root).is_empty(), "{:?}", error_lines(&root));
    let before = fs::read(root.join("raw/inbox/to-relocate.md")).unwrap();

    let done = docsys::relocate::raw_move(&root, "raw/inbox/to-relocate.md", "coding").unwrap();
    assert_eq!(done.from, "raw/inbox/to-relocate.md");
    assert_eq!(done.to, "raw/coding/to-relocate.md");
    assert_eq!(
        done.rewritten,
        vec![("wiki/coding/reference/moved.md".to_string(), 1)]
    );
    assert!(!root.join("raw/inbox/to-relocate.md").exists());
    assert_eq!(
        fs::read(root.join("raw/coding/to-relocate.md")).unwrap(),
        before,
        "the bytes are untouched (R-023)"
    );
    let page = fs::read_to_string(root.join("wiki/coding/reference/moved.md")).unwrap();
    assert!(
        page.contains("sources: [raw/coding/to-relocate.md]"),
        "{page}"
    );
    assert!(
        page.contains("verification: verified"),
        "the body did not change, so the verification stands (D-077):\n{page}"
    );
    assert!(page.ends_with(body), "the body is byte-identical:\n{page}");
    assert!(page.contains(&format!("updated: {today}")), "{page}");
    assert!(
        error_lines(&root).is_empty(),
        "the trail is intact (R-059) and the move is a relocation, not a deletion (R-023): {:?}",
        error_lines(&root)
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn raw_move_refuses_what_would_lie() {
    let (repo, root) = build_kb("rawmove-refuse");
    let err = docsys::relocate::raw_move(&root, "raw/inbox/to-relocate.md", "ops").unwrap_err();
    assert!(err.contains("R-026"), "an undeclared domain: {err}");
    fs::write(root.join("raw/coding/to-relocate.md"), "an older note\n").unwrap();
    let err = docsys::relocate::raw_move(&root, "raw/inbox/to-relocate.md", "coding").unwrap_err();
    assert!(err.contains("R-023"), "an existing destination: {err}");
    assert!(
        root.join("raw/inbox/to-relocate.md").is_file(),
        "nothing moved"
    );
    assert_eq!(
        fs::read_to_string(root.join("raw/coding/to-relocate.md")).unwrap(),
        "an older note\n",
        "nothing overwritten"
    );
    let err = docsys::relocate::raw_move(&root, "raw/inbox/missing.md", "coding").unwrap_err();
    assert!(err.contains("no record"), "{err}");
    let err = docsys::relocate::raw_move(&root, "wiki/index.md", "coding").unwrap_err();
    assert!(err.contains("no record"), "a page is not a record: {err}");
    // a project tree has no records
    let project = tmp("rawmove-project");
    fs::write(
        project.join(".docmeta.yml"),
        "spec: docsys/0.4\nprofile: project\n",
    )
    .unwrap();
    let err = docsys::relocate::raw_move(&project, "raw/inbox/x.md", "coding").unwrap_err();
    assert!(err.contains("knowledge base"), "{err}");
    let _ = fs::remove_dir_all(&repo);
    let _ = fs::remove_dir_all(&project);
}

#[test]
fn a_page_may_cite_the_destination_before_the_move_and_lint_says_so() {
    let (repo, root) = build_kb("rawmove-order");
    let today = docsys::migrate::today();
    write_in(
        &root,
        "wiki/coding/reference/early.md",
        &format!(
            "---\nid: early\ntype: reference\ndomain: coding\nverification: unverified\n\
             updated: {today}\nsources: [raw/coding/to-relocate.md]\n---\n# Early\n\n\
             This page cites where the note will be; read it after the move.\n"
        ),
    );
    route(
        &root,
        "- [[coding/reference/early|Early]] -- Written before the move.\n",
    );
    let errs = error_lines(&root);
    assert!(
        errs.iter()
            .any(|e| e.starts_with("R-059 wiki/coding/reference/early.md")
                && e.contains("raw/coding/to-relocate.md")),
        "the trail is severed until the note arrives: {errs:?}"
    );
    let done = docsys::relocate::raw_move(&root, "raw/inbox/to-relocate.md", "coding").unwrap();
    assert!(
        done.rewritten.is_empty(),
        "nobody cited the old path: {:?}",
        done.rewritten
    );
    assert!(error_lines(&root).is_empty(), "{:?}", error_lines(&root));
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn the_binary_dispatches_raw_move_and_reports_what_it_did() {
    // the library tests above never reach main.rs; the two-word command
    // list there is what a person's shell hits
    let (repo, root) = build_kb("rawmove-bin");
    let out = Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args([
            "raw",
            "move",
            "raw/inbox/to-relocate.md",
            "coding",
            "--root",
        ])
        .arg(&root)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("moved: raw/inbox/to-relocate.md -> raw/coding/to-relocate.md"),
        "{stdout}"
    );
    assert!(stdout.contains("no page cited it"), "{stdout}");
    let out = Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args([
            "raw",
            "move",
            "raw/coding/to-relocate.md",
            "nowhere",
            "--root",
        ])
        .arg(&root)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("R-026"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn open_questions_is_the_bases_questions_ledger_under_r108() {
    // D-090: a free-form rewrite (items without checkboxes) is an error;
    // dated `- [ ]` lines are clean and counted by status
    let (repo, root) = build_kb("open-questions");
    let oq = root.join("wiki/open-questions.md");
    fs::write(
        &oq,
        "# Open Questions\n\nA header of the session's own.\n\n- 2026-09-03 wiki/coding/howto/sample.md — the page says four, the source says six\n",
    )
    .unwrap();
    let errs = error_lines(&root);
    assert!(
        errs.iter()
            .any(|e| e.starts_with("R-108 wiki/open-questions.md")),
        "{errs:?}"
    );
    fs::write(
        &oq,
        "# Open questions\n\nWhat the base cannot settle by itself; one dated line each.\n\n- [ ] 2026-09-03 sample: the page says four attempts, `raw/coding/source-note.md` says six\n",
    )
    .unwrap();
    assert!(error_lines(&root).is_empty(), "{:?}", error_lines(&root));
    let s = docsys::status::status(&root, None).unwrap();
    assert_eq!(s.questions_open, 1);
    let _ = fs::remove_dir_all(&repo);
}
