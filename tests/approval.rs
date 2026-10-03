#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! A page's verification is read from history on a docsys/0.5 tree (R-024,
//! D-126): an approval after its last body change, nothing in the page.

use docsys::approval::{Approvals, State};
use docsys::tree::DocTree;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-approval-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

const PAGE: &str = "---\nid: retry\ntype: reference\nsources: [src/retry.rs]\n---\nThis page states the retry policy; read it before changing it.\n\nThree attempts.\n";

fn project(name: &str) -> (PathBuf, PathBuf) {
    let repo = tmp(name);
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    let root = repo.join("docs");
    fs::create_dir_all(root.join("reference")).unwrap();
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/retry.rs"), "pub fn retry() {}\n").unwrap();
    fs::write(
        root.join(".docmeta.yml"),
        "spec: docsys/0.5\nprofile: project\ndefault_content_language: en\nmaintainers: [ayse <ayse@example.com> @ayse-gh]\n",
    )
    .unwrap();
    fs::write(
        root.join("index.md"),
        "# Docs\n\n- [[reference/|Reference]] -- Facts.\n",
    )
    .unwrap();
    fs::write(root.join("reference/retry.md"), PAGE).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "the retry page"]);
    (repo, root)
}

fn state(root: &Path) -> State {
    Approvals::of(&DocTree::load(root).unwrap()).state("reference/retry.md")
}

fn verified(root: &Path) -> bool {
    matches!(state(root), State::Verified { .. })
}

#[test]
fn an_approval_after_the_last_body_change_verifies_and_a_later_change_undoes_it() {
    let (repo, root) = project("window");
    assert_eq!(state(&root), State::Unverified, "written, not approved");
    // the maintainer's own empty commit names the page
    git(
        &repo,
        &[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "docs: verify retry",
            "-m",
            "Verifies: reference/retry.md\nApproved-by: ayse <ayse@example.com>",
        ],
    );
    assert!(verified(&root), "{:?}", state(&root));
    // a frontmatter change leaves the body as it was: still verified
    let page = root.join("reference/retry.md");
    fs::write(
        &page,
        PAGE.replace(
            "sources: [src/retry.rs]",
            "sources: [src/retry.rs, src/lib.rs]",
        ),
    )
    .unwrap();
    git(&repo, &["commit", "-qam", "a source more"]);
    assert!(verified(&root), "{:?}", state(&root));
    // an edit not yet committed reads as unverified
    fs::write(&page, PAGE.replace("Three attempts.", "Four attempts.")).unwrap();
    assert_eq!(state(&root), State::Unverified);
    // committed without an approval: unverified
    git(&repo, &["commit", "-qam", "four"]);
    assert_eq!(state(&root), State::Unverified);
    // someone who is not a maintainer approves: nothing
    git(
        &repo,
        &[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "lgtm",
            "-m",
            "Verifies: retry\nApproved-by: @mallory",
        ],
    );
    assert_eq!(state(&root), State::Unverified);
    // the maintainer, by login
    git(
        &repo,
        &[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "ok",
            "-m",
            "Verifies: reference/retry\nApproved-by: @ayse-gh",
        ],
    );
    assert!(verified(&root));
    // and takes it back
    git(
        &repo,
        &[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "not yet",
            "-m",
            "Revokes: reference/retry.md",
        ],
    );
    assert_eq!(state(&root), State::Unverified);
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_squash_or_a_merge_that_carries_the_description_carries_the_approval() {
    let (repo, root) = project("squash");
    let page = root.join("reference/retry.md");
    for (mode, body) in [("squash", "Five attempts."), ("merge", "Six attempts.")] {
        git(&repo, &["checkout", "-qb", mode]);
        fs::write(&page, PAGE.replace("Three attempts.", body)).unwrap();
        git(&repo, &["commit", "-qam", "change the policy"]);
        fs::write(
            repo.join("src/retry.rs"),
            format!("pub fn retry() {{ /* {mode} */ }}\n"),
        )
        .unwrap();
        git(&repo, &["commit", "-qam", "the code"]);
        git(&repo, &["checkout", "-q", "main"]);
        // the host's message: the pull request's title and description
        let message =
            "Change the retry policy (#7)\n\nThe policy and its code.\n\nApproved-by: @ayse-gh\n";
        if mode == "squash" {
            git(&repo, &["merge", "-q", "--squash", mode]);
            git(&repo, &["commit", "-qm", message]);
        } else {
            git(&repo, &["merge", "-q", "--no-ff", "-m", message, mode]);
        }
        assert!(verified(&root), "{mode}: {:?}", state(&root));
    }
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_page_without_sources_takes_no_part_and_outside_history_it_is_unknown() {
    let (repo, root) = project("scope");
    fs::write(
        root.join("reference/plain.md"),
        "---\nid: plain\ntype: reference\n---\nThis page states a plain fact; read it any time.\n",
    )
    .unwrap();
    let tree = DocTree::load(&root).unwrap();
    let plain = tree
        .pages
        .iter()
        .find(|p| p.rel == "reference/plain.md")
        .unwrap();
    assert!(!docsys::approval::tracked(&tree, plain));
    let elsewhere = tmp("no-repo");
    let copy = elsewhere.join("docs");
    fs::create_dir_all(copy.join("reference")).unwrap();
    for f in [".docmeta.yml", "index.md", "reference/retry.md"] {
        fs::copy(root.join(f), copy.join(f)).unwrap();
    }
    assert_eq!(state(&copy), State::Unknown);
    let _ = fs::remove_dir_all(&repo);
    let _ = fs::remove_dir_all(&elsewhere);
}

#[test]
fn a_record_from_before_holds_until_the_body_moves() {
    let (repo, root) = project("legacy");
    let blocks: Vec<String> = docsys::blocks::hashes(
        "This page states the retry policy; read it before changing it.\n\nThree attempts.\n",
    );
    let page = root.join("reference/retry.md");
    let with_record = PAGE.replace(
        "sources: [src/retry.rs]\n",
        &format!(
            "sources: [src/retry.rs]\nverification: verified\nverified_by: ayse\nverified_rev: 0000000\nverified_blocks: [{}]\n",
            blocks.join(", ")
        ),
    );
    fs::write(&page, &with_record).unwrap();
    git(&repo, &["commit", "-qam", "a record from before"]);
    assert_eq!(
        state(&root),
        State::Verified {
            by: "ayse".into(),
            commit: "record".into()
        }
    );
    fs::write(
        &page,
        with_record.replace("Three attempts.", "Four attempts."),
    )
    .unwrap();
    git(&repo, &["commit", "-qam", "four"]);
    assert_eq!(state(&root), State::Unverified);
    let _ = fs::remove_dir_all(&repo);
}

fn docsys(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args(args)
        .current_dir(dir)
        .env_remove("DOCSYS_DISPATCHED")
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .output()
        .unwrap()
}

fn head(repo: &Path) -> String {
    String::from_utf8(
        Command::new("git")
            .args(["log", "-1", "--format=%H%n%B"])
            .current_dir(repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
}

#[test]
fn verify_is_the_maintainers_own_commit_and_never_writes_the_page() {
    let (repo, root) = project("verify-cmd");
    git(&repo, &["config", "user.email", "ayse@example.com"]);
    git(&repo, &["config", "user.name", "ayse"]);
    let page = root.join("reference/retry.md");
    let before = fs::read_to_string(&page).unwrap();
    // something staged is refused: the approval is a commit of its own
    fs::write(repo.join("src/retry.rs"), "pub fn retry() { }\n").unwrap();
    git(&repo, &["add", "src/retry.rs"]);
    let out = docsys(&repo, &["verify", "retry"]);
    assert!(!out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("something is staged"),
        "{out:?}"
    );
    git(&repo, &["commit", "-qm", "code"]);
    let out = docsys(&repo, &["verify", "retry"]);
    assert!(out.status.success(), "{out:?}");
    let h = head(&repo);
    assert!(
        h.contains("Verifies: reference/retry.md\nApproved-by: ayse <ayse@example.com>"),
        "{h}"
    );
    assert_eq!(fs::read_to_string(&page).unwrap(), before);
    assert!(verified(&root));
    // again: nothing to do, no commit
    let out = docsys(&repo, &["verify", "retry"]);
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("already verified"),
        "{out:?}"
    );
    assert_eq!(head(&repo), h);
    // taken back by the maintainer's own commit
    let out = docsys(&repo, &["verify", "retry", "--revoke"]);
    assert!(out.status.success(), "{out:?}");
    assert!(head(&repo).contains("Revokes: reference/retry.md"));
    assert_eq!(state(&root), State::Unverified);
    // the approval job's line: for a maintainer only
    let line = docsys(&repo, &["verify", "--approval", "@ayse-gh"]);
    assert_eq!(
        String::from_utf8_lossy(&line.stdout),
        "Approved-by: @ayse-gh\n"
    );
    let none = docsys(&repo, &["verify", "--approval", "@mallory"]);
    assert!(none.status.success() && none.stdout.is_empty(), "{none:?}");
    let _ = fs::remove_dir_all(&repo);
}
