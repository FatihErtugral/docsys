#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// R-059 against a real repository: a `git:<sha>` that a clone cannot hold.
// A shallow clone lacks every commit below its depth, and a squash merge
// leaves a branch's commits on no branch; a corpus tree carries no git state,
// so these live here.

use docsys::model::Severity;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-sources-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?} in {}", dir.display());
}

fn head(dir: &Path) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

/// A project repository on `develop` with one code file, committed.
fn project(name: &str) -> PathBuf {
    let repo = tmp(name);
    git(&repo, &["init", "-q", "-b", "develop"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    docsys::migrate::init_profile(&repo.join("docs"), "en", "project").unwrap();
    write(&repo, "src/a.txt", "1\n");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "one"]);
    repo
}

fn cite(sources: &str) -> String {
    format!(
        "---\nid: a\ntype: reference\nsources: [{sources}]\n---\nThis page describes a; read it when a changes.\n"
    )
}

/// The R-059 findings of the tree at `<repo>/<root>`: severity, file, subject.
fn r059(repo: &Path, root: &str) -> Vec<(Severity, String, String, String)> {
    let (report, _) = docsys::lint_in(&repo.join(root), Some(repo));
    report
        .findings
        .into_iter()
        .filter(|f| f.rule.0 == "R-059")
        .map(|f| (f.severity, f.file, f.subject, f.message))
        .collect()
}

const SHALLOW: &str =
    "history is shallow here; this entry cannot be checked — check out with fetch-depth: 0";

#[test]
fn a_commit_a_shallow_clone_lacks_is_reported_never_a_severed_trail() {
    let repo = project("shallow");
    let first = head(&repo);
    write(&repo, "docs/reference/a.md", &cite(&format!("git:{first}")));
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "two"]);
    let full = r059(&repo, "docs");
    assert!(full.is_empty(), "the full clone holds the commit: {full:?}");

    let clone = tmp("shallow-clone");
    let _ = fs::remove_dir_all(&clone);
    let url = format!("file://{}", repo.display());
    git(
        repo.parent().unwrap(),
        &["clone", "-q", "--depth", "1", &url, clone.to_str().unwrap()],
    );
    let found = r059(&clone, "docs");
    assert_eq!(
        found
            .iter()
            .map(|(s, f, subj, _)| (*s, f.as_str(), subj.as_str()))
            .collect::<Vec<_>>(),
        vec![(
            Severity::Warn,
            "reference/a.md",
            format!("git:{first}").as_str()
        )],
        "{found:?}"
    );
    assert!(
        found.first().unwrap().3.ends_with(SHALLOW),
        "{}",
        found.first().unwrap().3
    );
    assert!(
        !found.first().unwrap().3.contains("severed"),
        "{}",
        found.first().unwrap().3
    );

    // a knowledge base blocks on a severed trail; a shallow clone is no
    // severed trail, so it reports there too
    let kb = tmp("shallow-kb");
    git(&kb, &["init", "-q", "-b", "develop"]);
    git(&kb, &["config", "user.email", "t@example.invalid"]);
    git(&kb, &["config", "user.name", "t"]);
    write(&kb, "notes.txt", "1\n");
    git(&kb, &["add", "-A"]);
    git(&kb, &["commit", "-q", "-m", "one"]);
    let kb_first = head(&kb);
    write(
        &kb,
        "kb/.docmeta.yml",
        "spec: docsys/0.4\nprofile: knowledge-base\ndefault_content_language: en\ndomains: [coding]\n",
    );
    write(
        &kb,
        "kb/wiki/index.md",
        "# wiki\n\n- [[coding/howto/sample|Sample]] -- A sample page.\n",
    );
    write(
        &kb,
        "kb/wiki/coding/howto/sample.md",
        &format!(
            "---\nid: sample\ntype: howto\ndomain: coding\nverification: unverified\n\
             updated: 2026-08-16\nsources: [git:{kb_first}]\n---\n\n# Sample\n\n\
             This page is a sample; it anchors the test tree.\n"
        ),
    );
    git(&kb, &["add", "-A"]);
    git(&kb, &["commit", "-q", "-m", "two"]);
    assert!(r059(&kb, "kb").is_empty());
    let kb_clone = tmp("shallow-kb-clone");
    let _ = fs::remove_dir_all(&kb_clone);
    let kb_url = format!("file://{}", kb.display());
    git(
        kb.parent().unwrap(),
        &[
            "clone",
            "-q",
            "--depth",
            "1",
            &kb_url,
            kb_clone.to_str().unwrap(),
        ],
    );
    let found = r059(&kb_clone, "kb");
    assert_eq!(
        found
            .iter()
            .map(|(s, f, subj, _)| (*s, f.as_str(), subj.as_str()))
            .collect::<Vec<_>>(),
        vec![(
            Severity::Warn,
            "wiki/coding/howto/sample.md",
            format!("git:{kb_first}").as_str()
        )],
        "{found:?}"
    );
    assert!(
        found.first().unwrap().3.ends_with(SHALLOW),
        "{}",
        found.first().unwrap().3
    );
    for d in [repo, clone, kb, kb_clone] {
        let _ = fs::remove_dir_all(d);
    }
}

#[test]
fn a_branch_commit_a_squash_merge_dropped_names_the_squash() {
    let repo = project("squashed");
    git(&repo, &["switch", "-q", "-c", "feature"]);
    write(&repo, "src/a.txt", "2\n");
    git(&repo, &["commit", "-q", "-am", "feature changes a"]);
    let branch_commit = head(&repo);
    write(
        &repo,
        "docs/reference/a.md",
        &cite(&format!("git:{branch_commit}")),
    );
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "feature documents a"]);
    git(&repo, &["switch", "-q", "develop"]);
    git(&repo, &["merge", "-q", "--squash", "feature"]);
    git(&repo, &["commit", "-q", "-m", "feature (#1)"]);

    // what a CI checkout of develop holds: develop's whole history, no branch
    let clone = tmp("squashed-clone");
    let _ = fs::remove_dir_all(&clone);
    let url = format!("file://{}", repo.display());
    git(
        repo.parent().unwrap(),
        &[
            "clone",
            "-q",
            "--single-branch",
            "--branch",
            "develop",
            &url,
            clone.to_str().unwrap(),
        ],
    );
    let found = r059(&clone, "docs");
    assert_eq!(
        found
            .iter()
            .map(|(s, f, subj, _)| (*s, f.as_str(), subj.as_str()))
            .collect::<Vec<_>>(),
        vec![(
            Severity::Warn,
            "reference/a.md",
            format!("git:{branch_commit}").as_str()
        )],
        "{found:?}"
    );
    assert!(
        found.first().unwrap().3.contains(&format!(
            "commit {branch_commit} is not in this repository (a squash merge or a rebase \
             drops a branch commit — cite the commit that landed, the pull request URL, or a path)"
        )),
        "{}",
        found.first().unwrap().3
    );
    assert!(
        !found.first().unwrap().3.contains("shallow"),
        "{}",
        found.first().unwrap().3
    );
    let _ = fs::remove_dir_all(&repo);
    let _ = fs::remove_dir_all(&clone);
}

#[test]
fn a_range_that_cites_a_commit_only_its_branch_holds_is_warned() {
    let repo = project("branch-only");
    let landed = head(&repo);
    write(
        &repo,
        "docs/reference/b.md",
        &cite(&format!("git:{landed}")).replace("id: a", "id: b"),
    );
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "b cites a landed commit"]);
    git(&repo, &["switch", "-q", "-c", "feature"]);
    write(&repo, "src/a.txt", "2\n");
    git(&repo, &["commit", "-q", "-am", "feature changes a"]);
    let branch_commit = head(&repo);
    write(
        &repo,
        "docs/reference/a.md",
        &cite(&format!(
            "git:{branch_commit}, git:{landed}, git:{branch_commit}:src/a.txt"
        )),
    );
    write(
        &repo,
        "docs/reference/b.md",
        &cite(&format!("git:{landed}, git:{branch_commit}")).replace("id: a", "id: b"),
    );
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "feature documents a and b"]);

    let found: Vec<(Severity, String, String, String)> =
        docsys::locator::branch_only_citations(&repo, &repo.join("docs"), "develop")
            .into_iter()
            .map(|f| (f.severity, f.rule.0.to_string(), f.file, f.subject))
            .collect();
    let warn = |file: &str, subject: String| {
        (
            Severity::Warn,
            "R-059".to_string(),
            file.to_string(),
            subject,
        )
    };
    assert_eq!(
        found,
        vec![
            warn("reference/a.md", format!("git:{branch_commit}")),
            warn("reference/a.md", format!("git:{branch_commit}:src/a.txt")),
            warn("reference/b.md", format!("git:{branch_commit}")),
        ],
        "a landed commit is not the branch's, wherever the range adds it"
    );
    let messages: Vec<String> =
        docsys::locator::branch_only_citations(&repo, &repo.join("docs"), "develop")
            .into_iter()
            .map(|f| f.message)
            .collect();
    for m in &messages {
        assert!(
            m.ends_with(
                "this commit exists only on this branch; a squash merge drops it — cite the \
                 commit that lands, the pull request URL, or a path"
            ),
            "{m}"
        );
    }
    let _ = fs::remove_dir_all(&repo);
}
