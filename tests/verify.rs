#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// D-092 / R-208: anyone writes, a maintainer vouches — in the project profile.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-verify-{name}-{}", std::process::id()));
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
        "git {args:?}"
    );
}

fn commit_as(dir: &Path, email: &str, msg: &str) {
    git(dir, &["add", "-A"]);
    assert!(Command::new("git")
        .args(["commit", "-q", "--allow-empty", "-m", msg])
        .env("GIT_AUTHOR_EMAIL", email)
        .env("GIT_COMMITTER_EMAIL", email)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_COMMITTER_NAME", "t")
        .current_dir(dir)
        .status()
        .unwrap()
        .success());
}

fn errors(root: &Path, repo: &Path) -> Vec<String> {
    let (r, _) = docsys::lint_in(root, Some(repo));
    r.findings
        .iter()
        .filter(|f| f.severity == docsys::model::Severity::Error)
        .map(|f| format!("{} {} [{}] {}", f.rule.0, f.file, f.subject, f.message))
        .collect()
}

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn project(name: &str) -> (PathBuf, PathBuf) {
    let repo = tmp(name);
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    let root = repo.join("docs");
    write(
        &root,
        ".docmeta.yml",
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\nmaintainers: [ayse <ayse@example.com>, mehmet]\n",
    );
    write(
        &root,
        "index.md",
        "# docs\n\n- [[reference/token-ttl|Token TTL]] -- the lifetime.\n",
    );
    write(&root, "work/journal.md", "# Journal\n");
    write(&root, "work/debt.md", "# Debt\n");
    write(&root, "work/questions.md", "# Questions\n");
    (repo, root)
}

#[test]
fn a_page_from_evidence_is_unverified_and_a_maintainer_verifies_it_in_their_own_commit() {
    let (repo, root) = project("vouch");
    let today = docsys::migrate::today();
    // the session that did the work writes the page: unverified, with sources
    let made = docsys::capture::page_new(&root, "reference", "token-ttl", Some("Token TTL"), true)
        .unwrap();
    assert!(made.contains("created: reference/token-ttl.md"), "{made}");
    let page = root.join("reference/token-ttl.md");
    let text = fs::read_to_string(&page).unwrap();
    assert!(
        text.contains("verification: unverified\nsources: []\n"),
        "{text}"
    );
    fs::write(
        &page,
        text.replace(
            "<!-- opening: one or two sentences that establish this page's own context — what it describes, when to read it (R-032). Then route it from index.md. -->",
            "This page states how long a token lives; read it before caching one.\n\nTwelve hours.",
        ),
    )
    .unwrap();
    commit_as(&repo, "junior@example.com", "docs: token ttl from the code");
    assert!(
        errors(&root, &repo).is_empty(),
        "{:?}",
        errors(&root, &repo)
    );
    let s = docsys::status::status(&root, Some(&repo)).unwrap();
    assert_eq!(s.unverified, vec!["reference/token-ttl.md".to_string()]);
    let rev = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(&repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();

    // a junior "verifies" it: not a declared maintainer → error
    let base = fs::read_to_string(&page).unwrap(); // the unverified page, as committed
    let verified = |who: &str| {
        base.replace(
            "verification: unverified",
            &format!("verification: verified\nverified_by: {who}\nverified_rev: {rev}"),
        )
    };
    fs::write(&page, verified("junior")).unwrap();
    let errs = errors(&root, &repo);
    assert!(
        errs.iter()
            .any(|e| e.starts_with("R-208 reference/token-ttl.md [verified_by]")),
        "{errs:?}"
    );
    // a maintainer's name, but committed by somebody else → error (history half)
    fs::write(&page, verified("ayse")).unwrap();
    commit_as(&repo, "junior@example.com", "docs: verified? no");
    let errs = errors(&root, &repo);
    assert!(
        errs.iter()
            .any(|e| e.starts_with("R-208 reference/token-ttl.md [verified_by]")),
        "the record must be the maintainer's own commit: {errs:?}"
    );
    // the maintainer's own commit → clean
    fs::write(
        &page,
        verified("ayse").replace("Twelve hours.", "Twelve hours.\n"),
    )
    .unwrap();
    commit_as(&repo, "ayse@example.com", "docs: token ttl verified");
    // (the record line is unchanged; git -S finds its first appearance — the junior's commit)
    // so the maintainer re-records it in her own words to own the line:
    let own = fs::read_to_string(&page).unwrap().replace(
        "verified_by: ayse",
        "verified_by: ayse (reviewed against src/token.rs)",
    );
    fs::write(&page, own).unwrap();
    commit_as(
        &repo,
        "ayse@example.com",
        "docs: token ttl verified by ayse",
    );
    let rev2 = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "--short", "HEAD~1"])
            .current_dir(&repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    let fixed = fs::read_to_string(&page).unwrap().replace(
        &format!("verified_rev: {rev}"),
        &format!("verified_rev: {rev2}"),
    );
    fs::write(&page, fixed).unwrap();
    commit_as(&repo, "ayse@example.com", "docs: rev");
    let errs = errors(&root, &repo);
    assert!(
        !errs.iter().any(|e| e.starts_with("R-208")),
        "a maintainer's own record: {errs:?}"
    );
    // the body changes after verification → R-024 (D-077) in the project profile too
    fs::write(
        &page,
        fs::read_to_string(&page)
            .unwrap()
            .replace("Twelve hours.", "Six hours."),
    )
    .unwrap();
    let errs = errors(&root, &repo);
    assert!(
        errs.iter()
            .any(|e| e.starts_with("R-024 reference/token-ttl.md [verification]")),
        "{errs:?}"
    );
    let _ = fs::remove_dir_all(&repo);
    let _ = today;
}

#[test]
fn confirmed_names_a_maintainer_and_no_list_means_anyone() {
    let (repo, root) = project("confirm");
    let today = docsys::migrate::today();
    write(
        &root,
        "reference/token-ttl.md",
        &format!("---\nid: token-ttl\ntype: reference\nupdated: {today}\n---\n# Token TTL\n\nThis page states the lifetime; read it first.\n\nTwelve hours.\n"),
    );
    write(
        &root,
        "work/features/cart-key.md",
        &format!("---\nid: cart-key\nstatus: done\nconfirmed: junior, {today}\nupdated: {today}\n---\n## Context\n\nWhy.\n\n## Decision\n\nPer day.\n\n## Contract surface\n\n## Rejected alternatives\n"),
    );
    commit_as(&repo, "junior@example.com", "work: cart-key done");
    let errs = errors(&root, &repo);
    assert!(
        errs.iter()
            .any(|e| e.starts_with("R-208 work/features/cart-key.md [confirmed]")),
        "{errs:?}"
    );
    // the same tree with no maintainers declared: anyone, as before
    let dm = root.join(".docmeta.yml");
    fs::write(
        &dm,
        fs::read_to_string(&dm).unwrap().replace(
            "maintainers: [ayse <ayse@example.com>, mehmet]\n",
            "maintainers: []\n",
        ),
    )
    .unwrap();
    let errs = errors(&root, &repo);
    assert!(!errs.iter().any(|e| e.starts_with("R-208")), "{errs:?}");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn verify_is_one_command_for_a_maintainer_and_a_refusal_for_anyone_else() {
    let (repo, root) = project("cmd");
    let today = docsys::migrate::today();
    write(
        &root,
        "reference/token-ttl.md",
        &format!("---\nid: token-ttl\ntype: reference\nverification: unverified\nsources: []\nupdated: {today}\n---\n# Token TTL\n\nThis page states the lifetime; read it before caching a token.\n\nTwelve hours.\n"),
    );
    // uncommitted: refused
    git(&repo, &["config", "user.email", "ayse@example.com"]);
    git(&repo, &["config", "user.name", "ayse"]);
    let err = docsys::verify::verify(&root, "token-ttl", None, false, false).unwrap_err();
    assert!(err.contains("not committed yet"), "{err}");
    commit_as(&repo, "junior@example.com", "docs: token ttl from the code");
    // a junior identity: refused, naming the maintainers
    git(&repo, &["config", "user.email", "junior@example.com"]);
    git(&repo, &["config", "user.name", "junior"]);
    let err = docsys::verify::verify(&root, "token-ttl", None, false, false).unwrap_err();
    assert!(err.contains("matches no declared maintainer"), "{err}");
    let err = docsys::verify::verify(&root, "token-ttl", Some("junior"), false, false).unwrap_err();
    assert!(err.contains("not a declared maintainer"), "{err}");
    // the maintainer, by email: the record is written from HEAD
    git(&repo, &["config", "user.email", "ayse@example.com"]);
    git(&repo, &["config", "user.name", "Ayse Y."]);
    let done = docsys::verify::verify(&root, "reference/token-ttl", None, false, false).unwrap();
    assert_eq!(done.by, "ayse");
    let head = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(&repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    assert_eq!(done.rev, head);
    let text = fs::read_to_string(root.join("reference/token-ttl.md")).unwrap();
    assert!(
        text.contains(&format!(
            "verification: verified\nverified_by: ayse\nverified_rev: {head}\n"
        )),
        "{text}"
    );
    // committed as herself: clean, including R-208's history half
    commit_as(&repo, "ayse@example.com", "docs: token ttl verified");
    assert!(
        errors(&root, &repo).is_empty(),
        "{:?}",
        errors(&root, &repo)
    );
    // again at the same revision: nothing to do
    let again = docsys::verify::verify(&root, "token-ttl", None, false, false).unwrap();
    assert!(
        again.notes.iter().any(|n| n.starts_with("already")),
        "{again:?}"
    );
    // the body moves: revoke, then verify with --commit under her identity
    fs::write(
        root.join("reference/token-ttl.md"),
        text.replace("Twelve hours.", "Six hours."),
    )
    .unwrap();
    let errs = errors(&root, &repo);
    assert!(
        errs.iter()
            .any(|e| e.starts_with("R-024 reference/token-ttl.md [verification]")),
        "{errs:?}"
    );
    docsys::verify::verify(&root, "token-ttl", None, false, true).unwrap();
    let text = fs::read_to_string(root.join("reference/token-ttl.md")).unwrap();
    assert!(
        text.contains("verification: unverified\n") && !text.contains("verified_by"),
        "{text}"
    );
    commit_as(&repo, "junior@example.com", "docs: six hours");
    assert!(
        errors(&root, &repo).is_empty(),
        "{:?}",
        errors(&root, &repo)
    );
    let done = docsys::verify::verify(&root, "token-ttl", None, true, false).unwrap();
    assert!(done.committed, "{done:?}");
    let author = String::from_utf8(
        Command::new("git")
            .args(["log", "-1", "--format=%ae"])
            .current_dir(&repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(author.trim(), "ayse@example.com");
    assert!(
        errors(&root, &repo).is_empty(),
        "{:?}",
        errors(&root, &repo)
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_review_approval_verifies_every_page_the_change_touched_under_the_approver() {
    // D-095: the approver's login on the maintainer entry; CI's identity is a bot's
    let (repo, root) = project("review");
    let dm = root.join(".docmeta.yml");
    fs::write(
        &dm,
        fs::read_to_string(&dm).unwrap().replace(
            "maintainers: [ayse <ayse@example.com>, mehmet]",
            "maintainers: [ayse <ayse@example.com> @ayse-gh, mehmet]",
        ),
    )
    .unwrap();
    let today = docsys::migrate::today();
    commit_as(&repo, "bot@ci.invalid", "base");
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
    // the pull request: two pages from evidence, one page without the field
    write(
        &root,
        "reference/token-ttl.md",
        &format!("---\nid: token-ttl\ntype: reference\nverification: unverified\nsources: []\nupdated: {today}\n---\n# Token TTL\n\nThis page states the lifetime; read it before caching a token.\n\nTwelve hours.\n"),
    );
    write(
        &root,
        "howto/rotate.md",
        &format!("---\nid: rotate\ntype: howto\nverification: unverified\nsources: []\nupdated: {today}\n---\n# Rotate\n\nThis page is the rotation, step by step; read it when the calendar says so.\n\n1. Rotate.\n"),
    );
    write(
        &root,
        "explanation/why.md",
        &format!("---\nid: why\ntype: explanation\nupdated: {today}\n---\n# Why\n\nThis page explains why; read it first.\n\nBecause.\n"),
    );
    fs::write(
        root.join("index.md"),
        "# docs\n\n- [[reference/token-ttl|Token TTL]] -- the lifetime.\n- [[howto/rotate|Rotate]] -- the rotation.\n- [[explanation/why|Why]] -- the reason.\n",
    )
    .unwrap();
    commit_as(&repo, "junior@example.com", "docs: three pages");
    // CI: the repository's identity is a bot; the approver is @ayse-gh
    git(&repo, &["config", "user.email", "bot@ci.invalid"]);
    git(&repo, &["config", "user.name", "ci-bot"]);
    let range = format!("{base}...HEAD");
    let err =
        docsys::verify::verify_range(&root, &range, Some("@nobody"), false, true).unwrap_err();
    assert!(err.contains("no declared maintainer's login"), "{err}");
    let done = docsys::verify::verify_range(&root, &range, Some("@ayse-gh"), false, true).unwrap();
    let verified: Vec<&str> = done
        .iter()
        .filter(|v| v.committed)
        .map(|v| v.page.as_str())
        .collect();
    assert_eq!(
        verified,
        vec!["howto/rotate.md", "reference/token-ttl.md"],
        "{done:?}"
    );
    assert!(
        !done.iter().any(|v| v.page == "explanation/why.md"),
        "a page without the field is not the review's to vouch for: {done:?}"
    );
    let author = String::from_utf8(
        Command::new("git")
            .args(["log", "-1", "--format=%ae %s"])
            .current_dir(&repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert!(
        author.starts_with("ayse@example.com docs: 2 page(s) verified by ayse"),
        "{author}"
    );
    assert!(
        errors(&root, &repo).is_empty(),
        "{:?}",
        errors(&root, &repo)
    );
    // running it again: nothing to do
    let again = docsys::verify::verify_range(&root, &range, Some("@ayse-gh"), false, true).unwrap();
    assert!(
        again
            .iter()
            .all(|v| v.notes.iter().any(|n| n.starts_with("already"))),
        "{again:?}"
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_reviewed_by_trailer_is_the_word_on_any_host() {
    let (repo, root) = project("trailer");
    let today = docsys::migrate::today();
    commit_as(&repo, "bot@ci.invalid", "base");
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
    write(
        &root,
        "reference/token-ttl.md",
        &format!("---\nid: token-ttl\ntype: reference\nverification: unverified\nsources: []\nupdated: {today}\n---\n# Token TTL\n\nThis page states the lifetime; read it before caching a token.\n\nTwelve hours.\n"),
    );
    git(&repo, &["add", "-A"]);
    // the change, then a merge-style commit carrying the review's word as a trailer
    assert!(Command::new("git")
        .args([
            "commit",
            "-q",
            "-m",
            "docs: token ttl",
            "-m",
            "Reviewed-by: Ayse Y. <ayse@example.com>"
        ])
        .env("GIT_AUTHOR_EMAIL", "junior@example.com")
        .env("GIT_COMMITTER_EMAIL", "bot@ci.invalid")
        .env("GIT_AUTHOR_NAME", "junior")
        .env("GIT_COMMITTER_NAME", "bot")
        .current_dir(&repo)
        .status()
        .unwrap()
        .success());
    let range = format!("{base}...HEAD");
    // a stranger's trailer is no word
    let (repo2, root2) = project("trailer-stranger");
    commit_as(&repo2, "bot@ci.invalid", "base");
    let base2 = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo2)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    write(&root2, "reference/token-ttl.md", &format!("---\nid: token-ttl\ntype: reference\nverification: unverified\nsources: []\nupdated: {today}\n---\n# Token TTL\n\nThis page states the lifetime; read it first.\n\nTwelve hours.\n"));
    git(&repo2, &["add", "-A"]);
    assert!(Command::new("git")
        .args([
            "commit",
            "-q",
            "-m",
            "docs: ttl",
            "-m",
            "Reviewed-by: Someone <someone@example.com>"
        ])
        .env("GIT_AUTHOR_EMAIL", "junior@example.com")
        .env("GIT_COMMITTER_EMAIL", "junior@example.com")
        .env("GIT_AUTHOR_NAME", "j")
        .env("GIT_COMMITTER_NAME", "j")
        .current_dir(&repo2)
        .status()
        .unwrap()
        .success());
    let err = docsys::verify::verify_range(&root2, &format!("{base2}...HEAD"), None, true, true)
        .unwrap_err();
    assert!(err.contains("name nobody in maintainers"), "{err}");
    // the maintainer's trailer verifies, under her identity, whoever runs it
    git(&repo, &["config", "user.email", "bot@ci.invalid"]);
    git(&repo, &["config", "user.name", "ci-bot"]);
    let done = docsys::verify::verify_range(&root, &range, None, true, true).unwrap();
    assert!(
        done.iter().any(|v| v.committed && v.by == "ayse"),
        "{done:?}"
    );
    let author = String::from_utf8(
        Command::new("git")
            .args(["log", "-1", "--format=%ae"])
            .current_dir(&repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(author.trim(), "ayse@example.com");
    assert!(
        errors(&root, &repo).is_empty(),
        "{:?}",
        errors(&root, &repo)
    );
    let _ = fs::remove_dir_all(&repo);
    let _ = fs::remove_dir_all(&repo2);
}
