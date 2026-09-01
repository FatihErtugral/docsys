#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// §11 freshness against a real repository: pins recomputed from the code,
// history dating every page, the gate over a commit range, and adoption
// writing the CI workflow and a gate that is hard when the tree is clean.
// A corpus tree cannot carry git state, so these live here.

use docsys::{fresh, gate, lint_in, model::Severity};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-fresh-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn git(dir: &Path, args: &[&str]) {
    git_dated(dir, args, None);
}

/// A commit at a chosen calendar day: history is what the checks read.
fn git_dated(dir: &Path, args: &[&str], date: Option<&str>) {
    let mut cmd = Command::new("git");
    cmd.args(["-c", "commit.gpgsign=false", "-c", "core.quotePath=false"])
        .args(args)
        .current_dir(dir);
    if let Some(d) = date {
        let stamp = format!("{d}T12:00:00+00:00");
        cmd.env("GIT_AUTHOR_DATE", &stamp)
            .env("GIT_COMMITTER_DATE", &stamp);
    }
    assert!(cmd.status().unwrap().success(), "git {args:?}");
}

const AUTH_RS: &str = "use std::time::Duration;\n\npub fn other(x: u32) -> u32 {\n    x + 1\n}\n\n/// doc: refresh\npub fn refresh_token(ttl: Duration) -> Duration {\n    ttl / 2\n}\n";

/// A project repository with one routed reference page and one code file,
/// committed on a chosen day.
fn repo(name: &str, day: &str) -> (PathBuf, PathBuf) {
    let repo = tmp(name);
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    let docs = repo.join("docs");
    docsys::migrate::init_profile(&docs, "en", "project").unwrap();
    fs::create_dir_all(docs.join("reference")).unwrap();
    fs::write(
        docs.join("reference/refresh.md"),
        format!(
            "---\nid: refresh\ntype: reference\nupdated: {day}\n---\n# Refresh\n\n\
             This page states how a token is refreshed; read it before changing the TTL.\n\n\
             A token is refreshed at half its TTL.\n"
        ),
    )
    .unwrap();
    let index = docs.join("index.md");
    let mut text = fs::read_to_string(&index).unwrap();
    text.push_str("\n- [[reference/refresh|Refresh]] -- when a token is refreshed.\n");
    fs::write(&index, text).unwrap();
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/auth.rs"), AUTH_RS).unwrap();
    git(&repo, &["add", "-A"]);
    git_dated(&repo, &["commit", "-q", "-m", "init"], Some(day));
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
fn a_pinned_symbol_detects_drift_in_its_region_only() {
    let today = docsys::migrate::today();
    let (repo, docs) = repo("pins", &today);
    let msg = fresh::pin(
        &docs,
        &repo,
        "reference/refresh",
        "src/auth.rs",
        Some("refresh_token"),
    )
    .unwrap();
    assert!(msg.contains("sha256:"), "{msg}");
    let page = fs::read_to_string(docs.join("reference/refresh.md")).unwrap();
    assert!(
        page.contains(
            "verifies:\n  - path: src/auth.rs\n    symbol: refresh_token\n    hash: \"sha256:"
        ),
        "{page}"
    );
    assert!(page.contains(&format!("updated: {today}")), "{page}");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "pin"]);
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );

    // the other function moves: the pinned region did not
    fs::write(repo.join("src/auth.rs"), AUTH_RS.replace("x + 1", "x + 2")).unwrap();
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );

    // the pinned function moves: stale, an error, naming page and region
    fs::write(
        repo.join("src/auth.rs"),
        AUTH_RS.replace("ttl / 2", "ttl / 3"),
    )
    .unwrap();
    let errs = errors(&docs, &repo);
    assert!(
        errs.contains(&"R-111 reference/refresh.md src/auth.rs#refresh_token".to_string()),
        "{errs:?}"
    );

    // the author re-reads the page and refreshes: current again (by id)
    let msg = fresh::refresh(&docs, &repo, "refresh").unwrap();
    assert!(msg.contains("refreshed"), "{msg}");
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );

    // the region is gone: still an error, saying so
    fs::remove_file(repo.join("src/auth.rs")).unwrap();
    let (r, _) = lint_in(&docs, Some(&repo));
    assert!(
        r.findings
            .iter()
            .any(|f| f.rule.0 == "R-111" && f.message.contains("no longer exists")),
        "{:?}",
        r.findings
    );
}

#[test]
fn an_ambiguous_symbol_or_a_malformed_hash_is_an_error_not_a_guess() {
    let today = docsys::migrate::today();
    let (repo, docs) = repo("ambiguous", &today);
    fs::write(
        repo.join("src/dup.rs"),
        "fn f() {\n    1\n}\nmod inner {\n    pub fn f() {\n        2\n    }\n}\n",
    )
    .unwrap();
    let err = fresh::pin(&docs, &repo, "reference/refresh", "src/dup.rs", Some("f")).unwrap_err();
    assert!(err.contains("ambiguous"), "{err}");
    let err = fresh::pin(
        &docs,
        &repo,
        "reference/refresh",
        "src/dup.rs",
        Some("nope"),
    )
    .unwrap_err();
    assert!(err.contains("not found"), "{err}");
    // the whole file is always resolvable
    fresh::pin(&docs, &repo, "reference/refresh", "src/dup.rs", None).unwrap();

    // a hash written by hand in the wrong form is reported under R-113
    let page = docs.join("reference/refresh.md");
    let text = fs::read_to_string(&page).unwrap();
    let start = text.find("hash: ").unwrap();
    let end = start + text[start..].find('\n').unwrap();
    let broken = format!("{}hash: \"sha256:zz\"{}", &text[..start], &text[end..]);
    fs::write(&page, broken).unwrap();
    let errs = errors(&docs, &repo);
    assert!(
        errs.iter()
            .any(|e| e.starts_with("R-113 reference/refresh.md")),
        "{errs:?}"
    );
}

#[test]
fn updated_behind_history_and_an_untouched_draft_are_errors() {
    let (repo, docs) = repo("history", "2026-08-01");
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );

    // a hand edit that skipped the tooling: body changed, `updated` not
    let page = docs.join("reference/refresh.md");
    let text = fs::read_to_string(&page).unwrap();
    fs::write(&page, text.replace("half its TTL", "a third of its TTL")).unwrap();
    git(&repo, &["add", "-A"]);
    git_dated(
        &repo,
        &["commit", "-q", "-m", "hand edit"],
        Some("2026-08-20"),
    );
    let errs = errors(&docs, &repo);
    assert!(
        errs.contains(&"R-106 reference/refresh.md updated".to_string()),
        "{errs:?}"
    );

    // the field catches up with history: clean
    let text = fs::read_to_string(&page).unwrap();
    fs::write(
        &page,
        text.replace("updated: 2026-08-01", "updated: 2026-08-20"),
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    git_dated(&repo, &["commit", "-q", "-m", "bump"], Some("2026-08-20"));
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );

    // a draft nobody touched since 2025: undeclared abandonment
    fs::create_dir_all(docs.join("work/features")).unwrap();
    fs::write(
        docs.join("work/features/old.md"),
        "---\nid: old\nstatus: draft\nupdated: 2025-01-01\n---\n\n## Context\n\n## Decision\n\n## Contract surface\n\n## Rejected alternatives\n",
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    git_dated(
        &repo,
        &["commit", "-q", "-m", "old draft"],
        Some("2025-01-01"),
    );
    let errs = errors(&docs, &repo);
    assert!(
        errs.contains(&"R-085 work/features/old.md status".to_string()),
        "{errs:?}"
    );

    // the tree declares a longer patience: clean
    let dm = docs.join(".docmeta.yml");
    let mut text = fs::read_to_string(&dm).unwrap();
    text.push_str("stale_active_days: 100000\n");
    fs::write(&dm, text).unwrap();
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );
}

#[test]
fn the_gate_over_a_range_fails_code_without_docs() {
    let today = docsys::migrate::today();
    let (repo, docs) = repo("range", &today);
    let base = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    fs::write(repo.join("src/auth.rs"), AUTH_RS.replace("x + 1", "x + 3")).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "code only"]);
    let (g, _) = gate::run_range(&repo, &docs, &format!("{base}..HEAD")).unwrap();
    assert_eq!(g.scope, "range");
    assert_eq!(g.code, vec!["src/auth.rs".to_string()]);
    assert_eq!(g.docs, 0);

    let journal = docs.join("work/journal.md");
    let mut text = fs::read_to_string(&journal).unwrap();
    text = text.replacen(
        "# Journal\n",
        &format!("# Journal\n\n## {today} - other changed\n- `other` adds three now\n"),
        1,
    );
    fs::write(&journal, text).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "journal"]);
    let (g, _) = gate::run_range(&repo, &docs, &format!("{base}..HEAD")).unwrap();
    assert!(g.docs > 0, "docs={} code={:?}", g.docs, g.code);
}

#[test]
fn adopt_writes_the_ci_workflow_and_hardens_the_gate_once_clean() {
    let repo = tmp("adopt-ci");
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    fs::create_dir_all(repo.join(".github")).unwrap();
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "init"]);

    // a tree with debt: the gate warns, the workflow lands
    let docs = repo.join("docs");
    docsys::migrate::init_profile(&docs, "en", "project").unwrap();
    let index = docs.join("index.md");
    let mut text = fs::read_to_string(&index).unwrap();
    text.push_str("\n- [[reference/ghost|Ghost]] -- dangling on purpose.\n");
    fs::write(&index, text).unwrap();
    let done = docsys::adopt::run(&repo, &docs, "en").unwrap();
    assert!(
        done.summary
            .iter()
            .any(|s| s.contains("git pre-commit gate") && s.contains("warn-mode")),
        "{:?}",
        done.summary
    );
    assert!(
        done.summary
            .iter()
            .any(|s| s.contains("ci workflow") && s.contains("written")),
        "{:?}",
        done.summary
    );
    let wf = fs::read_to_string(repo.join(".github/workflows/docsys.yml")).unwrap();
    assert!(wf.contains("docsys lint --root docs --repo ."), "{wf}");
    assert!(
        wf.contains("--range \"origin/${{ github.base_ref }}...HEAD\""),
        "{wf}"
    );
    let hook = fs::read_to_string(repo.join(".git/hooks/pre-commit")).unwrap();
    assert!(hook.contains("|| true"), "{hook}");

    // the debt is repaid: the next adopt hardens the gate in place
    let text = fs::read_to_string(&index).unwrap();
    fs::write(
        &index,
        text.replace(
            "\n- [[reference/ghost|Ghost]] -- dangling on purpose.\n",
            "",
        ),
    )
    .unwrap();
    let done = docsys::adopt::run(&repo, &docs, "en").unwrap();
    assert!(
        done.summary.iter().any(|s| s.contains("hardened")),
        "{:?}",
        done.summary
    );
    let hook = fs::read_to_string(repo.join(".git/hooks/pre-commit")).unwrap();
    assert!(!hook.contains("|| true"), "{hook}");
    assert!(hook.contains("Hard gate"), "{hook}");
    assert_eq!(
        fs::read_to_string(repo.join(".github/workflows/docsys.yml")).unwrap(),
        wf,
        "the workflow is the project's file after the first write"
    );
}
