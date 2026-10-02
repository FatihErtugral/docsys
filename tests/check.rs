#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// A machine's check, beside the maintainer's word (§21, R-214, D-104): it
// records what was read, refuses to rest on nothing, never sets `verified`,
// and `lookup` and `status` say when it stopped holding the body.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-check-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
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

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn project(name: &str, spec: &str) -> (PathBuf, PathBuf) {
    let repo = tmp(name);
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    let root = repo.join("docs");
    write(
        &root,
        ".docmeta.yml",
        &format!("spec: docsys/{spec}\nprofile: project\ndefault_content_language: en\n"),
    );
    write(
        &root,
        "index.md",
        "# Docs\n\n- [[reference/ttl|TTL]] -- the lifetime.\n",
    );
    write(&root, "work/journal.md", "# Journal\n");
    write(&root, "notes/evidence.md", "Twelve hours, measured.\n");
    write(
        &root,
        "reference/ttl.md",
        &format!(
            "---\nid: ttl\ntype: reference\nupdated: {}\nverification: unverified\nsources: [notes/evidence.md]\n---\n\n# TTL\n\nTwelve hours.\n",
            docsys::migrate::today()
        ),
    );
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "docs"]);
    (repo, root)
}

fn warnings(root: &Path) -> Vec<String> {
    let (r, _) = docsys::lint(root);
    r.findings
        .iter()
        .map(|f| {
            format!(
                "{} {} {} [{}]",
                f.severity.tag(),
                f.rule.0,
                f.file,
                f.subject
            )
        })
        .collect()
}

#[test]
fn a_check_records_what_was_read_and_never_verifies() {
    let (repo, root) = project("record", "0.5");
    let done = docsys::check::check(&root, "ttl", "an audit session", &[], true).unwrap();
    assert!(done.committed);
    assert_eq!(done.against, vec!["notes/evidence.md".to_string()]);
    let text = fs::read_to_string(root.join("reference/ttl.md")).unwrap();
    assert!(text.contains("verification: unverified\n"), "{text}");
    assert!(
        text.contains("checked_by: an audit session\nchecked_rev: "),
        "{text}"
    );
    assert!(
        text.contains("checked_against: [notes/evidence.md]\n"),
        "{text}"
    );
    assert_eq!(warnings(&root), Vec::<String>::new());
    let hits = docsys::lookup::lookup(&root, &["ttl".to_string()]).unwrap();
    assert_eq!(
        hits.first().and_then(|h| h.caveat.clone()).as_deref(),
        Some("unverified · checked by an audit session")
    );
    let s = docsys::status::status(&root, Some(&repo)).unwrap();
    assert_eq!((s.checked, s.checked_stale), (1, 0));
    assert!(docsys::status::render_json(&s).contains("\"checked\":1,\"checked_stale\":0"));

    // the body moves: the check stops holding, and everyone can see it
    let p = root.join("reference/ttl.md");
    fs::write(
        &p,
        fs::read_to_string(&p)
            .unwrap()
            .replace("Twelve hours.", "Six hours."),
    )
    .unwrap();
    assert_eq!(
        warnings(&root),
        vec!["WARN R-214 reference/ttl.md [checked_hash]".to_string()]
    );
    let hits = docsys::lookup::lookup(&root, &["ttl".to_string()]).unwrap();
    assert_eq!(
        hits.first().and_then(|h| h.caveat.clone()).as_deref(),
        Some("unverified · check stale")
    );
    let s = docsys::status::status(&root, Some(&repo)).unwrap();
    assert_eq!((s.checked, s.checked_stale), (0, 1));
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_check_refuses_an_uncommitted_page_and_evidence_that_is_gone() {
    let (repo, root) = project("refuse", "0.5");
    let p = root.join("reference/ttl.md");
    fs::write(&p, fs::read_to_string(&p).unwrap() + "\nMore.\n").unwrap();
    let e = docsys::check::check(&root, "ttl", "s", &[], false).unwrap_err();
    assert!(e.contains("uncommitted changes"), "{e}");
    git(&repo, &["checkout", "--", "."]);
    let e =
        docsys::check::check(&root, "ttl", "s", &["notes/gone.md".to_string()], false).unwrap_err();
    assert!(e.contains("does not resolve: notes/gone.md"), "{e}");
    let e = docsys::check::check(&root, "ttl", " ", &[], false).unwrap_err();
    assert!(e.contains("--by"), "{e}");
    let _ = fs::remove_dir_all(&repo);
}

/// A check record is a docsys/0.5 format (D-118): a 0.4 tree is refused and
/// its page is left as it was.
#[test]
fn a_check_on_a_0_4_tree_is_refused() {
    let (repo, root) = project("v04", "0.4");
    let before = fs::read_to_string(root.join("reference/ttl.md")).unwrap();
    let e = docsys::check::check(&root, "ttl", "s", &[], false).unwrap_err();
    assert!(e.contains("docsys/0.5 format"), "{e}");
    assert_eq!(
        fs::read_to_string(root.join("reference/ttl.md")).unwrap(),
        before
    );
    let _ = fs::remove_dir_all(&repo);
}
