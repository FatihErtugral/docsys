#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// A verification that carries its own evidence (D-101) and a maintainer's act
// read from the record's own history (D-102): a squash merge keeps both, a
// maintainer re-verifying is seen, a record re-used over another body is not,
// an edit demotes a verified page by itself, and a bookkeeping commit never
// makes `updated:` read as behind (R-106, §2.4).

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

/// Verify inside a pull request, squash it onto main, delete the branch: the
/// revision is gone from every clone, the hash still vouches — and the
/// maintainer's act survives through the trailer the host writes.
fn squashed(name: &str, trailer: bool) -> (PathBuf, PathBuf) {
    let (repo, root) = project(name);
    git(&repo, &["checkout", "-q", "-b", "pr"]);
    // the pull request changes the page; the maintainer verifies it there
    let p = root.join("reference/token-ttl.md");
    fs::write(
        &p,
        fs::read_to_string(&p)
            .unwrap()
            .replace("Twelve hours.", "Ten hours."),
    )
    .unwrap();
    commit_as(&repo, "junior@example.com", "docs: ten hours", None);
    let done = docsys::verify::verify(&root, "token-ttl", None, true, false).unwrap();
    assert!(done.committed, "{done:?}");
    git(&repo, &["checkout", "-q", "main"]);
    git(&repo, &["merge", "--squash", "-q", "pr"]);
    let msg = if trailer {
        "docs: token ttl verified (#1)\n\nCo-authored-by: ayse <ayse@example.com>"
    } else {
        "docs: token ttl verified (#1)"
    };
    commit_as(&repo, "junior@example.com", msg, None);
    git(&repo, &["branch", "-q", "-D", "pr"]);
    prune(&repo);
    (repo, root)
}

#[test]
fn a_squash_keeps_the_verification_by_its_hash_and_the_act_by_its_trailer() {
    let (repo, root) = squashed("squash", true);
    assert_eq!(findings(&root, &repo), Vec::<String>::new());
    let s = docsys::status::status(&root, Some(&repo)).unwrap();
    assert_eq!(s.rev_gone, 1, "{s:?}");
    let _ = fs::remove_dir_all(&repo);

    // the check seen failing: the same squash, the host wrote no trailer
    let (repo, root) = squashed("squash-bare", false);
    assert_eq!(
        findings(&root, &repo),
        vec!["R-208 reference/token-ttl.md [verified_by]".to_string()]
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_record_from_before_the_hash_still_needs_its_revision() {
    let (repo, root) = project("legacy");
    git(&repo, &["checkout", "-q", "-b", "pr"]);
    // a branch-only commit is the revision the record names: unreachable once
    // the branch is squashed and deleted
    git(
        &repo,
        &["commit", "-q", "--allow-empty", "-m", "branch-only"],
    );
    let rev = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(&repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let legacy = page().replace(
        "verification: unverified\n",
        &format!(
            "verification: verified\nverified_by: ayse\nverified_rev: {}\n",
            rev.trim()
        ),
    );
    write(&root, "reference/token-ttl.md", &legacy);
    commit_as(&repo, "ayse@example.com", "docs: verified by hand", None);
    git(&repo, &["checkout", "-q", "main"]);
    git(&repo, &["merge", "--squash", "-q", "pr"]);
    commit_as(&repo, "ayse@example.com", "docs: squash", None);
    git(&repo, &["branch", "-q", "-D", "pr"]);
    prune(&repo);
    assert!(
        findings(&root, &repo).contains(&"R-028 reference/token-ttl.md [verified_rev]".to_string()),
        "{:?}",
        findings(&root, &repo)
    );
    let _ = fs::remove_dir_all(&repo);
}

/// The gap the earlier check documented: a junior typed the record, the
/// maintainer then verified the page — with a byte-identical `verified_by:`
/// line. The maintainer's commit is the act, and it is seen.
#[test]
fn a_maintainer_verifying_a_line_a_junior_typed_is_the_maintainers_act() {
    let (repo, root) = project("retyped");
    let typed = page().replace(
        "verification: unverified\n",
        "verification: unverified\nverified_by: ayse\n",
    );
    write(&root, "reference/token-ttl.md", &typed);
    commit_as(
        &repo,
        "junior@example.com",
        "docs: the junior types the name",
        None,
    );
    let done = docsys::verify::verify(&root, "token-ttl", None, true, false).unwrap();
    assert!(done.committed, "{done:?}");
    assert_eq!(findings(&root, &repo), Vec::<String>::new());
    let _ = fs::remove_dir_all(&repo);
}

/// A record re-used over a body nobody verified: the junior changes the body
/// and keeps `verified`, rewriting the hash so R-024 stays quiet. No commit
/// since the body changed is the maintainer's.
#[test]
fn a_record_carried_over_a_new_body_is_not_the_maintainers_act() {
    let (repo, root) = project("forged");
    docsys::verify::verify(&root, "token-ttl", None, true, false).unwrap();
    assert_eq!(findings(&root, &repo), Vec::<String>::new());
    let text = fs::read_to_string(root.join("reference/token-ttl.md")).unwrap();
    let old_hash = text
        .lines()
        .find_map(|l| l.strip_prefix("verified_hash: "))
        .unwrap()
        .trim_matches('"')
        .to_string();
    let new_body = text.replace("Twelve hours.", "Six hours.");
    let body_at = new_body.find("\n---\n").unwrap() + 5;
    let new_hash = docsys::fresh::content_hash(&new_body[body_at..]);
    write(
        &root,
        "reference/token-ttl.md",
        &new_body.replace(&old_hash, &new_hash),
    );
    commit_as(&repo, "junior@example.com", "docs: six hours", None);
    assert_eq!(
        findings(&root, &repo),
        vec!["R-208 reference/token-ttl.md [verified_by]".to_string()]
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn an_edit_that_changes_a_verified_body_demotes_the_page_and_keeps_the_record() {
    let (repo, root) = project("demote");
    docsys::verify::verify(&root, "token-ttl", None, true, false).unwrap();
    let page = root.join("reference/token-ttl.md");
    let payload = format!(
        r#"{{"tool_name":"Edit","tool_input":{{"file_path":"{}"}}}}"#,
        page.display()
    );
    // layout only: canonically the same body, nothing happens
    let verified = fs::read_to_string(&page).unwrap();
    fs::write(&page, verified.replace("Twelve hours.", "Twelve hours.   ")).unwrap();
    let r = docsys::hook::post_tool_use(&repo, &root, &payload, "2026-10-02");
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(fs::read_to_string(&page)
        .unwrap()
        .contains("verification: verified\n"));
    // a real change: unverified, the record kept, the agent told
    fs::write(&page, verified.replace("Twelve hours.", "Six hours.")).unwrap();
    let r = docsys::hook::post_tool_use(&repo, &root, &payload, "2026-10-02");
    assert_eq!(r.code, 2);
    assert!(
        r.stderr
            .contains("is `verification: unverified` now (R-024)"),
        "{}",
        r.stderr
    );
    let after = fs::read_to_string(&page).unwrap();
    assert!(
        after.contains("verification: unverified\nverified_by: ayse\n"),
        "{after}"
    );
    assert!(after.contains("verified_hash: \"sha256:"), "{after}");
    // the lookup and lint agree: unverified, no error to clear by hand
    let hits = docsys::lookup::lookup(&root, &["token".to_string()]).unwrap();
    assert_eq!(
        hits.first().and_then(|h| h.caveat.clone()).as_deref(),
        Some("unverified")
    );
    assert_eq!(findings(&root, &repo), Vec::<String>::new());
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn lookup_says_a_verified_body_changed_without_any_history() {
    let (repo, root) = project("lookup");
    docsys::verify::verify(&root, "token-ttl", None, false, false).unwrap();
    let page = root.join("reference/token-ttl.md");
    let text = fs::read_to_string(&page).unwrap();
    fs::write(&page, text.replace("Twelve hours.", "Six hours.")).unwrap();
    let hits = docsys::lookup::lookup(&root, &["token".to_string()]).unwrap();
    assert_eq!(
        hits.first().and_then(|h| h.caveat.clone()).as_deref(),
        Some("verified, but the body changed since")
    );
    let _ = fs::remove_dir_all(&repo);
}

/// §2.4: a commit that changes only bookkeeping — here the verification
/// record — is not a content change, so `updated:` is not behind it. A body
/// change is, and R-106 still sees it.
#[test]
fn a_bookkeeping_commit_never_puts_updated_behind_history() {
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
    let dated = "---\nid: token-ttl\ntype: reference\nupdated: 2026-09-01\nverification: unverified\nverified_by: ayse\nverified_rev: 1111111\nsources: []\n---\n\n# Token TTL\n\nTwelve hours.\n";
    write(&root, "reference/token-ttl.md", dated);
    commit_as(&repo, "ayse@example.com", "docs: dated", Some("2026-09-01"));
    // the record moves, the content does not: `updated:` is not behind it
    let record_only = dated.replace("verified_rev: 1111111", "verified_rev: 2222222");
    write(&root, "reference/token-ttl.md", &record_only);
    commit_as(
        &repo,
        "ayse@example.com",
        "docs: record only",
        Some("2026-09-10"),
    );
    assert_eq!(findings(&root, &repo), Vec::<String>::new());
    // the check seen failing: a body change on a later day, `updated:` left behind
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
    assert_eq!(
        findings(&root, &repo),
        vec!["R-106 reference/token-ttl.md [updated]".to_string()]
    );
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
        !text.contains("verified_hash"),
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
