#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! docsys/0.5 keeps no page verification: no state, no record, no approval,
//! no maintainer (D-130). A page is checked against its sources and its code
//! when a person asks, with `/docsys-crosscheck`; its pins say when the code
//! it describes moved (R-111). A docsys/0.4 tree keeps 0.15.1's verification.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-v05-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
}

fn path() -> std::ffi::OsString {
    let mut dirs = vec![bin().parent().unwrap().to_path_buf()];
    dirs.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    std::env::join_paths(dirs).unwrap()
}

fn git(dir: &Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn docsys(dir: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .current_dir(dir)
        .env("PATH", path())
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .output()
        .unwrap()
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// An adopted docsys/0.5 repository with a page that names its sources and
/// pins a function, committed.
fn adopted(name: &str) -> PathBuf {
    let repo = tmp(name);
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    fs::create_dir_all(repo.join(".github")).unwrap();
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(
        repo.join("src/limits.rs"),
        "pub fn limit() -> u32 {\n    60\n}\n",
    )
    .unwrap();
    let out = docsys(&repo, &["adopt"]);
    assert!(out.status.success(), "{out:?}");
    let page = repo.join("docs/reference/limits.md");
    fs::create_dir_all(page.parent().unwrap()).unwrap();
    fs::write(
        &page,
        "---\nid: limits\ntype: reference\nsources: [src/limits.rs]\n---\n# Limits\n\nThis page states the limit; read it before raising it.\n\nThe limit is sixty.\n",
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "adopt, and a page"]);
    let out = docsys(
        &repo,
        &[
            "pin",
            "reference/limits",
            "src/limits.rs",
            "--symbol",
            "limit",
        ],
    );
    assert!(out.status.success(), "{out:?}");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "the page's pin"]);
    repo
}

/// Nothing a 0.5 tree prints speaks of verification: lint, status and
/// lookup read no state, and an `Approved-by:` in history changes nothing.
#[test]
fn a_0_5_tree_says_nothing_of_verification() {
    let repo = adopted("silent");
    git(
        &repo,
        &[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "docs: limits",
            "-m",
            "Verifies: reference/limits.md\nApproved-by: t",
        ],
    );
    for args in [
        &["lint"][..],
        &["status"],
        &["status", "--json"],
        &["lookup", "limit"],
    ] {
        let said = text(&docsys(&repo, args)).to_lowercase();
        assert!(!said.contains("verif"), "{args:?}: {said}");
        assert!(!said.contains("approv"), "{args:?}: {said}");
    }
    let _ = fs::remove_dir_all(&repo);
}

/// `docsys verify` is a docsys/0.4 tree's: on 0.5 it is refused, naming the
/// cross-check, and writes nothing.
#[test]
fn verify_is_refused_on_a_0_5_tree() {
    let repo = adopted("refused");
    let head = git(&repo, &["rev-parse", "HEAD"]);
    for args in [
        &["verify", "reference/limits"][..],
        &["verify", "reference/limits", "--revoke"],
    ] {
        let out = docsys(&repo, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {out:?}");
        assert!(
            text(&out).contains("/docsys-crosscheck"),
            "{args:?}: {out:?}"
        );
    }
    // what a re-verification would read is no question any more
    let out = docsys(&repo, &["verify", "--show", "reference/limits"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        text(&out).contains("`--show` is no flag of verify"),
        "{out:?}"
    );
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
    let _ = fs::remove_dir_all(&repo);
}

/// A pin says when the code moved; it backs no block of a verification:
/// `--block` is no flag of `pin`, and a 0.5 pin lives in `pins:`, its
/// acknowledgement under `.pins/`.
#[test]
fn a_0_5_pin_is_a_pin_and_nothing_more() {
    let repo = adopted("pins");
    let page = fs::read_to_string(repo.join("docs/reference/limits.md")).unwrap();
    assert!(page.contains("\npins:\n"), "{page}");
    assert!(!page.contains("verifies:"), "{page}");
    assert!(repo.join("docs/.pins").is_dir());
    assert!(!repo.join("docs/.verifies").exists());
    let out = docsys(
        &repo,
        &["pin", "reference/limits", "src/limits.rs", "--block", "1"],
    );
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        text(&out).contains("`--block` is no flag of pin"),
        "{out:?}"
    );
    // the pin still says when the code moved
    fs::write(
        repo.join("src/limits.rs"),
        "pub fn limit() -> u32 {\n    90\n}\n",
    )
    .unwrap();
    let said = text(&docsys(&repo, &["lint"]));
    assert!(said.contains("ERROR R-111 reference/limits.md"), "{said}");
    let _ = fs::remove_dir_all(&repo);
}

/// Adoption writes no maintainer list, no approval job and no step about
/// approvals; the flag that chose an approval job is refused on 0.5.
#[test]
fn adoption_asks_for_no_maintainer_and_no_approval() {
    let repo = adopted("adopt");
    let docmeta = fs::read_to_string(repo.join("docs/.docmeta.yml")).unwrap();
    assert!(!docmeta.contains("maintainers"), "{docmeta}");
    let workflow = fs::read_to_string(repo.join(".github/workflows/docsys.yml")).unwrap();
    for word in ["approv", "pull_request_review", "verif"] {
        assert!(
            !workflow.to_lowercase().contains(word),
            "{word}: {workflow}"
        );
    }
    let adoption = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    for word in ["maintainer", "approved-by", "verif"] {
        assert!(
            !adoption.to_lowercase().contains(word),
            "{word}: {adoption}"
        );
    }
    let out = docsys(&repo, &["adopt", "--verify-on-approval", "description"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let _ = fs::remove_dir_all(&repo);
}

/// Graduation's confirmation is the word of whoever runs it: no maintainer
/// list stands in front of it on 0.5.
#[test]
fn a_confirmation_needs_no_maintainer() {
    let repo = adopted("confirm");
    let meta = repo.join("docs/.docmeta.yml");
    fs::write(
        &meta,
        fs::read_to_string(&meta).unwrap() + "maintainers: [ayse]\n",
    )
    .unwrap();
    fs::create_dir_all(repo.join("docs/work/features")).unwrap();
    fs::write(
        repo.join("docs/work/features/x.md"),
        "---\nid: x\nstatus: done\n---\n# X\n\n## Decisions\n\n- a decision\n",
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "a finished feature"]);
    let plan = docsys(&repo, &["graduate", "plan", "work/features/x.md"]);
    let plan_file = repo.with_extension("plan.tsv");
    fs::write(&plan_file, &plan.stdout).unwrap();
    let out = docsys(
        &repo,
        &[
            "graduate",
            "apply",
            "--plan",
            plan_file.to_str().unwrap(),
            "--confirmed",
            "bora",
        ],
    );
    // past the confirmation: whatever stops it now is graduation's own rule
    let said = text(&out);
    assert!(!said.contains("maintainer"), "{said}");
    assert!(said.contains("graduate apply: block"), "{said}");
    let _ = fs::remove_dir_all(&repo);
}
