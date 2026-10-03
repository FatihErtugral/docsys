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
    // an empty list names nothing to check against: no part, and verify says so
    fs::write(
        root.join("reference/empty.md"),
        "---\nid: empty\ntype: reference\nsources: []\n---\nThis page states a fact; read it any time.\n",
    )
    .unwrap();
    let tree = DocTree::load(&root).unwrap();
    let empty = tree
        .pages
        .iter()
        .find(|p| p.rel == "reference/empty.md")
        .unwrap();
    assert!(!docsys::approval::tracked(&tree, empty));
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "two pages"]);
    git(&repo, &["config", "user.email", "ayse@example.com"]);
    let out = docsys(&repo, &["verify", "reference/empty.md"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("names no source in `sources:` and no pin"),
        "{out:?}"
    );
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
    // the body it read, by its hash: what a rebase is held to
    let read = docsys::approval::body_hash(&before);
    assert!(
        h.contains(&format!(
            "Verifies: reference/retry.md {read}\nApproved-by: ayse <ayse@example.com>"
        )),
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

/// R-208 on docsys/0.5: a work file's `confirmed:` is still the maintainer's
/// own act — authored by them, or named in a trailer of the commit that
/// recorded it (D-126 moves pages, not work files).
#[test]
fn a_confirmed_line_is_the_maintainers_act() {
    let (repo, root) = project("confirmed");
    let file = root.join("work/features/cart.md");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let r208 = |root: &Path| -> Vec<String> {
        let (r, _) = docsys::lint_in(root, Some(&repo));
        r.findings
            .iter()
            .filter(|f| f.rule.0 == "R-208")
            .map(|f| format!("{} [{}]", f.file, f.subject))
            .collect()
    };
    let text = "---\nid: cart\nstatus: done\nconfirmed: ayse\n---\n\n## Context\n\n## Decision\n\n## Contract surface\n\n## Rejected alternatives\n";
    fs::write(&file, text).unwrap();
    let commit_as = |email: &str, msg: &[&str]| {
        let mut args = vec!["-c", "core.hooksPath=/dev/null", "commit", "-q"];
        for m in msg {
            args.push("-m");
            args.push(m);
        }
        git(&repo, &["add", "-A"]);
        let out = Command::new("git")
            .args(&args)
            .current_dir(&repo)
            .env("GIT_AUTHOR_EMAIL", email)
            .env("GIT_COMMITTER_EMAIL", email)
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
    };
    commit_as("junior@example.com", &["a junior types the name"]);
    assert_eq!(r208(&root), ["work/features/cart.md [confirmed]"]);
    // the line recorded again, in a commit that names the maintainer
    fs::write(
        &file,
        text.replace("confirmed: ayse", "confirmed: ayse, 2026-10-03"),
    )
    .unwrap();
    commit_as(
        "junior@example.com",
        &["squashed (#3)", "Co-authored-by: ayse <ayse@example.com>"],
    );
    assert!(r208(&root).is_empty(), "{:?}", r208(&root));
    let _ = fs::remove_dir_all(&repo);
}

/// How a branch reaches main.
#[derive(Clone, Copy, Debug)]
enum Mode {
    FastForward,
    NoFf,
    Squash,
    Rebase,
}

const APPROVE: &str = "Verifies: reference/retry.md\nApproved-by: ayse <ayse@example.com>";

/// A branch `b` from main does `steps` — `verify` is `docsys verify`, which
/// names the body it read; `approve` a trailer typed by hand, which names
/// none; `concurrent` has main edit the page's other block meanwhile. Main
/// moves on (but for a fast-forward); the branch reaches main in `mode`, a
/// rebase making its commits again an hour later, as a host's does.
fn merged(name: &str, mode: Mode, steps: &[&str]) -> PathBuf {
    let (repo, root) = project(name);
    let page = root.join("reference/retry.md");
    git(&repo, &["checkout", "-qb", "b"]);
    for step in steps {
        match *step {
            "approve" => git(
                &repo,
                &[
                    "commit",
                    "-q",
                    "--allow-empty",
                    "-m",
                    "docs: verify retry",
                    "-m",
                    APPROVE,
                ],
            ),
            "verify" => {
                git(&repo, &["config", "user.email", "ayse@example.com"]);
                let out = docsys(&repo, &["verify", "reference/retry.md"]);
                assert!(out.status.success(), "{out:?}");
                git(&repo, &["config", "user.email", "t@example.invalid"]);
            }
            "edit" => {
                let text = fs::read_to_string(&page).unwrap();
                fs::write(&page, text.replace("Three attempts.", "Four attempts.")).unwrap();
                git(&repo, &["commit", "-qam", "retry: four attempts"]);
            }
            "concurrent" => {}
            other => panic!("{other}"),
        }
    }
    git(&repo, &["checkout", "-q", "main"]);
    if !matches!(mode, Mode::FastForward) {
        fs::write(repo.join("src/other.rs"), "pub fn other() {}\n").unwrap();
        if steps.contains(&"concurrent") {
            let text = fs::read_to_string(&page).unwrap();
            fs::write(
                &page,
                text.replace("read it before", "read it in full before"),
            )
            .unwrap();
        }
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-qm", "main moves on"]);
    }
    match mode {
        Mode::FastForward => git(&repo, &["merge", "-q", "--ff-only", "b"]),
        Mode::NoFf => git(&repo, &["merge", "-q", "--no-ff", "--no-edit", "b"]),
        Mode::Squash => {
            git(&repo, &["merge", "-q", "--squash", "b"]);
            // git's own squash message: every commit's message, indented
            git(&repo, &["commit", "-q", "--allow-empty", "--no-edit"]);
        }
        Mode::Rebase => {
            git(&repo, &["checkout", "-q", "b"]);
            let later = Command::new("git")
                .args(["log", "-1", "--format=%ct"])
                .current_dir(&repo)
                .output()
                .unwrap();
            let later: u64 = String::from_utf8_lossy(&later.stdout)
                .trim()
                .parse()
                .unwrap();
            let out = Command::new("git")
                .args(["-c", "core.hooksPath=/dev/null", "rebase", "-q", "main"])
                .env("GIT_COMMITTER_DATE", format!("{} +0000", later + 3600))
                .current_dir(&repo)
                .output()
                .unwrap();
            assert!(out.status.success(), "{out:?}");
            git(&repo, &["checkout", "-q", "main"]);
            git(&repo, &["merge", "-q", "--ff-only", "b"]);
        }
    }
    root
}

/// N7 holds whatever way a branch reaches main: an approval counts where the
/// body it read landed. `docsys verify` names that body, so its approval
/// holds through a rebase; a later edit on the branch, or a concurrent one on
/// main, outruns it in every mode. A trailer typed by hand names none: it
/// counts on the commit its author made, and a rebase makes that commit again.
#[test]
fn an_approval_made_on_a_branch_holds_under_every_merge_mode() {
    for mode in [Mode::FastForward, Mode::NoFf, Mode::Squash, Mode::Rebase] {
        let label = format!("{mode:?}").to_lowercase();
        let rebase = matches!(mode, Mode::Rebase);
        let concurrent = !matches!(mode, Mode::FastForward);
        for (steps, holds) in [
            (&["verify"][..], true),
            (&["edit", "verify"][..], true),
            (&["verify", "edit"][..], false),
            (&["approve"][..], !rebase),
            (&["edit", "approve"][..], !rebase),
            (&["approve", "edit"][..], false),
            (&["concurrent", "edit", "verify"][..], !concurrent),
            (&["concurrent", "edit", "approve"][..], !concurrent),
        ] {
            let root = merged(&format!("{label}-{}", steps.join("-")), mode, steps);
            assert_eq!(verified(&root), holds, "{mode:?}: {steps:?}");
            let _ = fs::remove_dir_all(root.parent().unwrap());
        }
    }
}

/// A range git cannot read fails, on every tree (D-105): an approval job
/// that read nothing must not pass as one that recorded nothing.
#[test]
fn an_unreadable_range_is_an_error() {
    let (repo, _) = project("bad-range");
    let out = docsys(
        &repo,
        &["verify", "--range", "nope...main", "--by", "@ayse-gh"],
    );
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("is not a range git can read"),
        "{out:?}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// `verify --show`'s next step is the one that changes the page's state:
/// after a revoke the page reads unverified and a maintainer verifies it
/// again; a stale bound pin is refreshed, not verified; a page that carries no
/// `sources:` names them first (D-103, D-126).
#[test]
fn show_names_the_step_the_state_asks_for() {
    let (repo, root) = project("show-next");
    let show = || {
        let out = docsys(&repo, &["verify", "--show", "reference/retry.md"]);
        assert!(out.status.success(), "{out:?}");
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let head = |s: &str| s.lines().next().unwrap_or("").to_string();
    let then = |s: &str| {
        s.lines()
            .find(|l| l.starts_with("then:"))
            .unwrap_or("")
            .to_string()
    };
    // bound to the code, then approved
    let out = docsys(
        &repo,
        &["pin", "reference/retry", "src/retry.rs", "--block", "1"],
    );
    assert!(out.status.success(), "{out:?}");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "retry: pinned"]);
    git(
        &repo,
        &[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "docs: verify retry",
            "-m",
            APPROVE,
        ],
    );
    let s = show();
    assert!(
        head(&s).contains("(verified)") && then(&s).contains("nothing"),
        "{s}"
    );
    // taken back: unverified, and the next step is a new approval
    git(
        &repo,
        &[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "docs: revoke retry",
            "-m",
            "Revokes: reference/retry.md",
        ],
    );
    let s = show();
    assert!(head(&s).contains("(unverified)"), "{s}");
    assert!(!head(&s).contains("nothing to re-read"), "{s}");
    assert!(then(&s).contains("docsys verify reference/retry.md"), "{s}");
    // approved again, then the pinned code moves: the pin is refreshed
    git(
        &repo,
        &[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "docs: verify retry",
            "-m",
            APPROVE,
        ],
    );
    fs::write(repo.join("src/retry.rs"), "pub fn retry() { loop {} }\n").unwrap();
    git(&repo, &["commit", "-qam", "retry: loops"]);
    let s = show();
    assert!(
        then(&s).contains("docsys pin --refresh reference/retry.md"),
        "{s}"
    );
    assert!(!then(&s).contains("docsys verify"), "{s}");
    // a page that takes no part names its sources first
    fs::write(
        root.join("reference/plain.md"),
        "---\nid: plain\ntype: reference\n---\nThis page states a plain fact; read it when you need it.\n",
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "a plain page"]);
    let out = docsys(&repo, &["verify", "--show", "reference/plain.md"]);
    let s = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        then(&s).contains("`sources:`") && !then(&s).contains("docsys verify"),
        "{s}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// A maintainer entry with no `@login` — R-208's `handle` form — is matched
/// by a host's approval through its handle: the handle doubles as the login.
#[test]
fn a_handle_without_a_login_doubles_as_the_login() {
    let (repo, root) = project("handle-login");
    let meta = root.join(".docmeta.yml");
    let text = fs::read_to_string(&meta).unwrap();
    fs::write(
        &meta,
        text.replace(
            "maintainers: [ayse <ayse@example.com> @ayse-gh]",
            "maintainers: [tester]",
        ),
    )
    .unwrap();
    git(&repo, &["commit", "-qam", "one maintainer, by handle"]);
    let out = docsys(&repo, &["verify", "--approval", "@tester"]);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "Approved-by: @tester",
        "{out:?}"
    );
    git(
        &repo,
        &[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "squash of an approved pull request",
            "-m",
            "Verifies: reference/retry.md\nApproved-by: @tester",
        ],
    );
    assert!(verified(&root));
    let _ = fs::remove_dir_all(&repo);
}

/// `verify --show` says why an approval no longer holds — its own reason, not
/// a list of possible ones — names the commit the approval was made on, and
/// marks what moved since its approver read the page (D-103, D-126).
#[test]
fn show_says_why_an_approval_no_longer_holds() {
    let show = |root: &Path| -> String {
        let out = docsys(
            root.parent().unwrap(),
            &["verify", "--show", "reference/retry.md"],
        );
        assert!(out.status.success(), "{out:?}");
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let head = |s: &str| s.lines().next().unwrap_or("").to_string();
    let short = |repo: &Path, rev: &str| {
        let out = Command::new("git")
            .args(["rev-parse", rev])
            .current_dir(repo)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)[..7].to_string()
    };
    // a later edit on the branch: the branch's approval, the edited block marked
    let root = merged("show-later", Mode::NoFf, &["approve", "edit"]);
    let repo = root.parent().unwrap().to_path_buf();
    let approval = short(&repo, "b~1");
    let s = show(&root);
    assert!(
        head(&s).contains(&format!(
            "(unverified): 1/2 blocks as verified by ayse at {approval}"
        )) && head(&s).contains("the body moved since"),
        "{s}"
    );
    assert!(
        s.contains("[2] line 8, changed:\n    Four attempts."),
        "{s}"
    );
    // taken back: by which commit
    git(
        &repo,
        &[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "docs: revoke retry",
            "-m",
            "Revokes: reference/retry.md",
        ],
    );
    let revoke = short(&repo, "HEAD");
    let s = show(&root);
    assert!(head(&s).contains(&format!("taken back by {revoke}")), "{s}");
    // the page no longer names what it rests on: it takes no part
    let page = root.join("reference/retry.md");
    let text = fs::read_to_string(&page).unwrap();
    fs::write(&page, text.replace("sources: [src/retry.rs]\n", "")).unwrap();
    git(&repo, &["commit", "-qam", "retry: no sources"]);
    let s = show(&root);
    assert!(
        head(&s).contains("names no source in `sources:` and no pin"),
        "{s}"
    );
    assert!(!head(&s).contains("taken back"), "{s}");
    let _ = fs::remove_dir_all(&repo);
    // a concurrent edit a rebase put under the approval
    let root = merged(
        "show-concurrent",
        Mode::Rebase,
        &["concurrent", "edit", "verify"],
    );
    let s = show(&root);
    assert!(
        head(&s).contains("the body that landed is not the body it read"),
        "{s}"
    );
    let _ = fs::remove_dir_all(root.parent().unwrap());
    // a trailer typed by hand, its commit made again by a rebase
    let root = merged("show-remade", Mode::Rebase, &["approve"]);
    let s = show(&root);
    assert!(head(&s).contains("made its commit again"), "{s}");
    let _ = fs::remove_dir_all(root.parent().unwrap());
}
