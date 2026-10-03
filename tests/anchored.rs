#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// A docsys/0.4 tree keeps the verification record 0.15.1 wrote (D-118), and
// a bookkeeping commit never moves a page's date (§2.4). A docsys/0.5 tree
// keeps no verification: tests/no_verification.rs (D-130).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-anchored-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Commit everything as `email`, on `date` when given (author and committer).
fn commit_as(dir: &Path, email: &str, msg: &str, date: Option<&str>) {
    git(dir, &["add", "-A"]);
    let mut c = Command::new("git");
    c.args(["commit", "-q", "--allow-empty", "-m", msg])
        .env("GIT_AUTHOR_EMAIL", email)
        .env("GIT_COMMITTER_EMAIL", email)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_COMMITTER_NAME", "t")
        .current_dir(dir);
    if let Some(d) = date {
        c.env("GIT_AUTHOR_DATE", format!("{d}T12:00:00"))
            .env("GIT_COMMITTER_DATE", format!("{d}T12:00:00"));
    }
    assert!(c.status().unwrap().success());
}

fn findings(root: &Path, repo: &Path) -> Vec<String> {
    let (r, _) = docsys::lint_in(root, Some(repo));
    r.findings
        .iter()
        .filter(|f| f.severity == docsys::model::Severity::Error)
        .map(|f| format!("{} {} [{}]", f.rule.0, f.file, f.subject))
        .collect()
}

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

/// The page, dated today: a fixed day would put `updated:` behind every commit
/// made on a later day (R-106).
fn page() -> String {
    format!(
        "---\nid: token-ttl\ntype: reference\nupdated: {}\nverification: unverified\nsources: []\n---\n\n# Token TTL\n\nTwelve hours.\n",
        docsys::migrate::today()
    )
}

fn project(name: &str) -> (PathBuf, PathBuf) {
    let repo = tmp(name);
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    git(&repo, &["config", "user.email", "ayse@example.com"]);
    git(&repo, &["config", "user.name", "ayse"]);
    let root = repo.join("docs");
    write(
        &root,
        ".docmeta.yml",
        "spec: docsys/0.5\nprofile: project\ndefault_content_language: en\nmaintainers: [ayse <ayse@example.com>, mehmet <mehmet@example.com>]\n",
    );
    write(
        &root,
        "index.md",
        "# docs\n\n- [[reference/token-ttl|Token TTL]] -- the lifetime.\n",
    );
    write(&root, "work/journal.md", "# Journal\n");
    write(&root, "work/debt.md", "# Debt\n");
    write(&root, "work/questions.md", "# Questions\n");
    write(&root, "reference/token-ttl.md", &page());
    commit_as(&repo, "junior@example.com", "docs: token ttl", None);
    (repo, root)
}

fn prune(repo: &Path) {
    git(repo, &["reflog", "expire", "--expire=now", "--all"]);
    git(repo, &["gc", "-q", "--prune=now"]);
}

/// §2.4: a commit that changes only bookkeeping — here the verification
/// record — is not a content change, so it does not move a page's date. A
/// body change does (R-050: the date is history's, D-122).
#[test]
fn a_bookkeeping_commit_never_moves_a_page_date() {
    // a history of its own, every commit dated, in order
    let repo = tmp("bookkeeping");
    git(&repo, &["init", "-q", "-b", "main"]);
    let root = repo.join("docs");
    write(
        &root,
        ".docmeta.yml",
        "spec: docsys/0.5\nprofile: project\ndefault_content_language: en\n",
    );
    write(
        &root,
        "index.md",
        "# docs\n\n- [[reference/token-ttl|Token TTL]] -- the lifetime.\n",
    );
    let page = "---\nid: token-ttl\ntype: reference\nverification: unverified\nverified_by: ayse\nverified_rev: 1111111\nsources: []\n---\n\n# Token TTL\n\nTwelve hours.\n";
    write(&root, "reference/token-ttl.md", page);
    let date = || {
        let tree = docsys::tree::DocTree::load(&root).unwrap();
        docsys::fresh::Dates::of(&tree).of_page("reference/token-ttl.md", None)
    };
    assert_eq!(date(), "unknown", "nothing committed yet");
    commit_as(&repo, "ayse@example.com", "docs: dated", Some("2026-09-01"));
    assert_eq!(date(), "2026-09-01");
    // the record moves, the content does not: the date stays
    let record_only = page.replace("verified_rev: 1111111", "verified_rev: 2222222");
    write(&root, "reference/token-ttl.md", &record_only);
    commit_as(
        &repo,
        "ayse@example.com",
        "docs: record only",
        Some("2026-09-10"),
    );
    assert_eq!(date(), "2026-09-01");
    // a body change on a later day moves it
    write(
        &root,
        "reference/token-ttl.md",
        &record_only.replace("Twelve hours.", "Six hours."),
    );
    commit_as(
        &repo,
        "ayse@example.com",
        "docs: six hours",
        Some("2026-09-20"),
    );
    assert_eq!(date(), "2026-09-20");
    assert_eq!(findings(&root, &repo), Vec::<String>::new());
    let _ = fs::remove_dir_all(&repo);
}

/// A docsys/0.4 tree is judged and written as 0.15 did (D-118): one binary
/// serves trees that have not moved yet.
fn as_04(repo: &Path, root: &Path) {
    let dm = root.join(".docmeta.yml");
    fs::write(
        &dm,
        fs::read_to_string(&dm)
            .unwrap()
            .replace("spec: docsys/0.5", "spec: docsys/0.4"),
    )
    .unwrap();
    commit_as(
        repo,
        "ayse@example.com",
        "docs: a tree that has not moved",
        None,
    );
}

#[test]
fn on_a_0_4_tree_verify_writes_the_old_record_and_an_edit_demotes_nothing() {
    let (repo, root) = project("v04-verify");
    as_04(&repo, &root);
    docsys::verify::verify(&root, "token-ttl", None, true, false).unwrap();
    let page = root.join("reference/token-ttl.md");
    let text = fs::read_to_string(&page).unwrap();
    assert!(
        text.contains("verification: verified\nverified_by: ayse\nverified_rev: "),
        "{text}"
    );
    assert!(
        !text.contains("verified_blocks"),
        "a 0.5 field on a 0.4 tree: {text}"
    );
    let payload = format!(
        r#"{{"tool_name":"Edit","tool_input":{{"file_path":"{}"}}}}"#,
        page.display()
    );
    fs::write(&page, text.replace("Twelve hours.", "Six hours.")).unwrap();
    let r = docsys::hook::post_tool_use(&repo, &root, &payload, &docsys::migrate::today());
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(fs::read_to_string(&page)
        .unwrap()
        .contains("verification: verified\n"));
    docsys::verify::verify(&root, "token-ttl", None, false, true).unwrap();
    let back = fs::read_to_string(&page).unwrap();
    assert!(
        back.contains("verification: unverified\n") && !back.contains("verified_by"),
        "{back}"
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn on_a_0_4_tree_a_squash_still_strands_the_revision_as_it_did() {
    let (repo, root) = project("v04-squash");
    as_04(&repo, &root);
    git(&repo, &["checkout", "-q", "-b", "pr"]);
    let p = root.join("reference/token-ttl.md");
    fs::write(
        &p,
        fs::read_to_string(&p)
            .unwrap()
            .replace("Twelve hours.", "Ten hours."),
    )
    .unwrap();
    commit_as(&repo, "junior@example.com", "docs: ten hours", None);
    docsys::verify::verify(&root, "token-ttl", None, true, false).unwrap();
    git(&repo, &["checkout", "-q", "main"]);
    git(&repo, &["merge", "--squash", "-q", "pr"]);
    commit_as(
        &repo,
        "junior@example.com",
        "docs: verified (#1)\n\nCo-authored-by: ayse <ayse@example.com>",
        None,
    );
    git(&repo, &["branch", "-q", "-D", "pr"]);
    prune(&repo);
    let f = findings(&root, &repo);
    assert!(
        f.contains(&"R-028 reference/token-ttl.md [verified_rev]".to_string()),
        "{f:?}"
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn on_a_0_4_tree_a_bookkeeping_commit_still_counts_for_r_106() {
    let repo = tmp("v04-bookkeeping");
    git(&repo, &["init", "-q", "-b", "main"]);
    let root = repo.join("docs");
    write(
        &root,
        ".docmeta.yml",
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n",
    );
    write(
        &root,
        "index.md",
        "# docs\n\n- [[reference/token-ttl|Token TTL]] -- the lifetime.\n",
    );
    let dated = "---\nid: token-ttl\ntype: reference\nupdated: 2026-09-01\nverification: unverified\nverified_by: ayse\nverified_rev: 1111111\nsources: []\n---\n\n# Token TTL\n\nTwelve hours.\n";
    write(&root, "reference/token-ttl.md", dated);
    commit_as(&repo, "ayse@example.com", "docs: dated", Some("2026-09-01"));
    write(
        &root,
        "reference/token-ttl.md",
        &dated.replace("verified_rev: 1111111", "verified_rev: 2222222"),
    );
    commit_as(
        &repo,
        "ayse@example.com",
        "docs: record only",
        Some("2026-09-10"),
    );
    assert_eq!(
        findings(&root, &repo),
        vec!["R-106 reference/token-ttl.md [updated]".to_string()]
    );
    let _ = fs::remove_dir_all(&repo);
}
