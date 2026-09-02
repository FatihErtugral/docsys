#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// "Take this out of your memory": a page to _archive/ with a tombstone, its
// router line and compiled skill gone, a record to raw/_forgotten/ — refused
// while a page still cites it — and the ledger that says when and why. Lint
// stays clean throughout; lookup and the connector never bring it back.

use docsys::{compile, forget, inbox, lint_in, lookup, model::Severity, status};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-forget-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn git(dir: &Path, args: &[&str]) {
    assert!(Command::new("git")
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap()
        .success());
}

fn write(base: &Path, rel: &str, text: &str) {
    let p = base.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn errors(base: &Path) -> Vec<String> {
    let (r, _) = lint_in(base, Some(base));
    r.findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| format!("{} {} {}", f.rule, f.file, f.subject))
        .collect()
}

/// A base with one record, one verified howto resting on it, its compiled
/// skill, all committed.
fn build(name: &str) -> PathBuf {
    let base = tmp(name);
    git(&base, &["init", "-q"]);
    git(&base, &["config", "user.email", "t@example.invalid"]);
    git(&base, &["config", "user.name", "t"]);
    docsys::migrate::init_profile(&base, "en", "knowledge-base").unwrap();
    let dm = base.join(".docmeta.yml");
    fs::write(
        &dm,
        fs::read_to_string(&dm)
            .unwrap()
            .replace("domains: []", "domains: [ops]"),
    )
    .unwrap();
    let today = docsys::migrate::today();
    inbox::add(
        &base,
        &inbox::Provenance {
            source: "calendar".into(),
            source_id: "evt-7".into(),
            title: "Rotate keys".into(),
            url: None,
            date: today.clone(),
        },
        "Rotate the keys monthly.\n",
    )
    .unwrap();
    let record = format!("raw/inbox/{today}-calendar-rotate-keys.md");
    assert!(base.join(&record).is_file());
    write(
        &base,
        "wiki/ops/howto/rotate-keys.md",
        &format!("---\nid: rotate-keys\ntype: howto\ndomain: ops\nverification: unverified\nupdated: {today}\nsources: [{record}]\n---\n# Rotate keys\n\nThis page lists the rotation steps; read it before rotating.\n\n1. Generate the new key.\n2. Swap it in.\n"),
    );
    write(
        &base,
        "wiki/ops/index.md",
        "# ops\n\n- [[ops/howto/rotate-keys|Rotate keys]] -- the steps.\n",
    );
    write(
        &base,
        "wiki/index.md",
        "# Knowledge base\n\n- [[ops/index|Ops]] -- operations.\n",
    );
    git(&base, &["add", "-A"]);
    git(&base, &["commit", "-q", "-m", "learned"]);
    let rev = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(&base)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    let page = base.join("wiki/ops/howto/rotate-keys.md");
    fs::write(
        &page,
        fs::read_to_string(&page).unwrap().replace(
            "verification: unverified",
            &format!("verification: verified\nverified_by: other\nverified_rev: {rev}"),
        ),
    )
    .unwrap();
    compile::compile(&base, &base.join(".claude"), "rotate-keys", false).unwrap();
    git(&base, &["add", "-A"]);
    git(&base, &["commit", "-q", "-m", "verified, compiled"]);
    assert!(errors(&base).is_empty(), "{:?}", errors(&base));
    base
}

#[test]
fn a_page_is_forgotten_with_its_tombstone_route_and_skill() {
    let base = build("page");
    let today = docsys::migrate::today();
    let record = format!("raw/inbox/{today}-calendar-rotate-keys.md");
    // the record is still cited: refused, naming the page
    let err = forget::forget(&base, &record, "no longer mine").unwrap_err();
    assert!(err.contains("wiki/ops/howto/rotate-keys.md"), "{err}");
    // no reason, no forgetting
    assert!(forget::forget(&base, "rotate-keys", "  ").is_err());

    let done = forget::forget(
        &base,
        "rotate-keys",
        "the keys are gone; the procedure with them",
    )
    .unwrap();
    assert!(
        base.join("_archive/wiki/ops/howto/rotate-keys.md")
            .is_file(),
        "{:?}",
        done.steps
    );
    assert!(!base.join("wiki/ops/howto/rotate-keys.md").exists());
    let tomb = fs::read_to_string(base.join(".tombstones.yml")).unwrap();
    assert!(tomb.contains("- id: rotate-keys\n"), "{tomb}");
    assert!(
        tomb.contains("forgotten: \"the keys are gone; the procedure with them\""),
        "{tomb}"
    );
    let index = fs::read_to_string(base.join("wiki/ops/index.md")).unwrap();
    assert!(!index.contains("rotate-keys"), "{index}");
    assert!(
        !base.join(".claude/skills/rotate-keys").exists(),
        "the compiled skill went with the page"
    );
    let ledger = fs::read_to_string(base.join(".forgotten.yml")).unwrap();
    assert!(
        ledger.contains("kind: page\n  path: wiki/ops/howto/rotate-keys.md\n"),
        "{ledger}"
    );
    assert!(errors(&base).is_empty(), "{:?}", errors(&base));
    assert!(
        lookup::lookup(&base, &["rotate".into()])
            .unwrap()
            .is_empty(),
        "forgotten means not found"
    );
    // the identifier is reserved: a new page claiming it collides
    write(
        &base,
        "wiki/ops/howto/rotate-keys.md",
        &format!("---\nid: rotate-keys\ntype: howto\ndomain: ops\nverification: unverified\nupdated: {today}\nsources: [{record}]\n---\n# Rotate keys again\n\nThis page lists steps; read it first.\n\n1. Step.\n"),
    );
    let errs = errors(&base);
    assert!(
        errs.iter()
            .any(|e| e.starts_with("R-061 wiki/ops/howto/rotate-keys.md")),
        "{errs:?}"
    );
    fs::remove_file(base.join("wiki/ops/howto/rotate-keys.md")).unwrap();

    // now the record can go: still a record, never captured again
    let done = forget::forget(&base, &record, "no longer mine").unwrap();
    let moved = format!("raw/_forgotten/inbox/{today}-calendar-rotate-keys.md");
    assert!(base.join(&moved).is_file(), "{:?}", done.steps);
    assert!(errors(&base).is_empty(), "{:?}", errors(&base));
    let again = inbox::add(
        &base,
        &inbox::Provenance {
            source: "calendar".into(),
            source_id: "evt-7".into(),
            title: "Rotate keys".into(),
            url: None,
            date: today.clone(),
        },
        "back again",
    )
    .unwrap();
    assert!(
        again.starts_with("already captured: raw/_forgotten/"),
        "{again}"
    );
    assert!(forget::forget(&base, &moved, "twice")
        .unwrap_err()
        .contains("already forgotten"));
    let s = status::status(&base, Some(&base)).unwrap();
    assert_eq!(s.forgotten, 2);
    assert!(status::render(&s, &base).contains("forgotten: 2"));
    assert_eq!(s.inbox, 0);
    git(&base, &["add", "-A"]);
    git(&base, &["commit", "-q", "-m", "forgotten"]);
    assert!(errors(&base).is_empty(), "{:?}", errors(&base));
}
