#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// The feedback channel (D-116): one issue format, the tracker's templates held
// equal to it, a draft that fills what the tool knows and leaves out what names
// people and places, and the pointer under a finding that may be wrong.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-feedback-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn docsys(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// A project tree with a dangling link, maintainers with addresses, a
/// consumed provider at a local path, and a namespace.
fn tree(name: &str, spec: &str) -> PathBuf {
    let repo = tmp(name);
    assert!(Command::new("git")
        .args(["init", "-q"])
        .current_dir(&repo)
        .status()
        .unwrap()
        .success());
    let docs = repo.join("docs");
    fs::create_dir_all(docs.join("reference")).unwrap();
    fs::write(
        docs.join(".docmeta.yml"),
        format!("spec: docsys/{spec}\nprofile: project\ndefault_content_language: en\nnamespace: shopfront\nmaintainers: [ayse <ayse@example.com> @ayse-gh]\nconsume: [auth=/srv/checkouts/auth#docs]\n"),
    )
    .unwrap();
    fs::write(
        docs.join("index.md"),
        "# Docs\n\n- [[reference/a|A]] -- A page with a dangling link.\n",
    )
    .unwrap();
    fs::write(
        docs.join("reference/a.md"),
        "---\nid: a\ntype: reference\nupdated: 2026-10-02\n---\n\n# A\n\nSee [[reference/ghost]].\n",
    )
    .unwrap();
    repo
}

#[test]
fn the_tracker_templates_are_the_binarys_format() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/ISSUE_TEMPLATE");
    for kind in docsys::feedback::TYPES {
        let file = fs::read_to_string(dir.join(format!("{kind}.md"))).unwrap();
        assert_eq!(
            file,
            docsys::feedback::template(kind).unwrap(),
            ".github/ISSUE_TEMPLATE/{kind}.md drifted from src/feedback.rs"
        );
    }
    let (code, guide, _) = docsys(&tmp("guide"), &["feedback"]);
    assert_eq!(code, 0);
    assert!(guide.contains(docsys::feedback::FORMAT), "{guide}");
    assert!(
        guide.contains("/issues/new?template=false-positive.md"),
        "{guide}"
    );
}

#[test]
fn a_draft_fills_the_facts_and_leaves_out_people_and_places() {
    let repo = tree("draft", "0.5");
    let (code, body, err) = docsys(
        &repo,
        &[
            "feedback",
            "--draft",
            "--rule",
            "R-071",
            "--command",
            "docsys lint",
        ],
    );
    assert_eq!(code, 0, "{err}");
    for want in [
        "[false positive] TODO",
        "### Problem or need\nTODO",
        "R-071 (lint · MUST): Every link target MUST resolve.",
        "`docsys lint` exited 1:",
        "ERROR R-071 reference/a.md [reference/ghost]",
        "- reference/a.md",
        "- tree: profile project, docsys/0.5",
        std::env::consts::OS,
        concat!("- docsys ", env!("CARGO_PKG_VERSION")),
    ] {
        assert!(body.contains(want), "missing `{want}` in:\n{body}");
    }
    for secret in ["ayse", "example.com", "/srv/checkouts", "shopfront"] {
        assert!(
            !body.contains(secret),
            "`{secret}` leaked into the draft:\n{body}"
        );
    }
    assert!(
        err.contains("/issues/new?template=false-positive.md"),
        "{err}"
    );
    // written to a file when asked
    let out = repo.join("issue.md");
    let (code, _, err) = docsys(
        &repo,
        &[
            "feedback",
            "--draft",
            "--type",
            "need",
            "--out",
            out.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "{err}");
    assert!(fs::read_to_string(&out).unwrap().starts_with("[need] TODO"));
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_command_that_is_not_docsys_is_refused() {
    let repo = tree("refuse", "0.5");
    let (code, _, err) = docsys(&repo, &["feedback", "--draft", "--command", "rm -rf docs"]);
    assert_eq!(code, 2);
    assert!(err.contains("not a docsys command"), "{err}");
    assert!(repo.join("docs").is_dir());
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_disputed_finding_points_to_feedback_once_on_a_0_5_tree() {
    let repo = tree("pointer", "0.5");
    let (_, out, _) = docsys(&repo, &["lint"]);
    assert_eq!(
        out.matches("Finding wrong? `docsys feedback --rule R-071`")
            .count(),
        1,
        "{out}"
    );
    let (_, json, _) = docsys(&repo, &["lint", "--json"]);
    assert!(!json.contains("Finding wrong?"), "{json}");
    // no finding, no pointer
    fs::write(
        repo.join("docs/reference/a.md"),
        "---\nid: a\ntype: reference\nupdated: 2026-10-02\n---\n\n# A\n\nNo links.\n",
    )
    .unwrap();
    let (_, clean, _) = docsys(&repo, &["lint"]);
    assert!(!clean.contains("Finding wrong?"), "{clean}");
    let _ = fs::remove_dir_all(&repo);
    // a docsys/0.4 tree prints what 0.15.1 printed (D-118)
    let old = tree("pointer-04", "0.4");
    let (_, out, _) = docsys(&old, &["lint"]);
    assert!(out.contains("ERROR R-071"), "{out}");
    assert!(!out.contains("Finding wrong?"), "{out}");
    let _ = fs::remove_dir_all(&old);
}
