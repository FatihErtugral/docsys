#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! What a reader cannot read is never read as something else: a list that
//! never closes declares nothing anyone may act on, a file that is not UTF-8
//! is named, and a flag's value is never another flag (D-002, D-129).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-readers-{name}-{}", std::process::id()));
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

fn docsys(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args(args)
        .current_dir(dir)
        .env_remove("DOCSYS_DISPATCHED")
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .output()
        .unwrap()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn head(repo: &Path) -> String {
    String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
}

/// Every file under `dir`, with its bytes.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else {
                out.insert(p.clone(), fs::read(&p).unwrap());
            }
        }
    }
    out
}

const PAGE: &str = "---\nid: retry\ntype: reference\nsources: [src/retry.rs]\n---\nThis page states the retry policy; read it before changing it.\n\nThree attempts.\n";

/// A committed repository whose tree declares `docmeta`, with one page that
/// rests on a source; the git identity is `t`, nobody's maintainer handle.
fn project(name: &str, docmeta: &str) -> (PathBuf, PathBuf) {
    let repo = tmp(name);
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    let root = repo.join("docs");
    fs::create_dir_all(root.join("reference")).unwrap();
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/retry.rs"), "pub fn retry() {}\n").unwrap();
    fs::write(root.join(".docmeta.yml"), docmeta).unwrap();
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

const SPEC_05: &str = "spec: docsys/0.5\nprofile: project\ndefault_content_language: en\n";
const SPEC_04: &str = "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n";

/// A docsys/0.4 tree keeps 0.15.1's verification, and an unclosed
/// `maintainers:` names nobody there: `verify` refuses, by its line (D-002).
#[test]
fn an_unclosed_maintainers_list_lets_nobody_verify() {
    let unclosed = format!("{SPEC_04}maintainers: [ayse <ayse@example.com> @ayse-gh\n");
    let (repo, _) = project("maintainers", &unclosed);
    let refusal = ".docmeta.yml: `maintainers` on line 4 opens a list that never closes with `]`";
    let before = head(&repo);
    for args in [
        &["verify", "retry"][..],
        &["verify", "retry", "--by", "t"],
        &["verify", "retry", "--by", "@ayse-gh"],
    ] {
        let out = docsys(&repo, args);
        assert_eq!(
            out.status.code(),
            Some(2),
            "{args:?} must refuse while the list cannot be read: {out:?}"
        );
        assert!(stderr(&out).contains(refusal), "{args:?}: {out:?}");
        assert_eq!(head(&repo), before, "{args:?} committed nothing");
    }
    let _ = fs::remove_dir_all(&repo);
}

/// A tree of `spec`, outside git, at `<dir>/docs`.
fn tree(name: &str, docmeta: &str) -> (PathBuf, PathBuf) {
    let dir = tmp(name);
    let root = dir.join("docs");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join(".docmeta.yml"), docmeta).unwrap();
    fs::write(root.join("index.md"), "# Docs\n").unwrap();
    (dir, root)
}

#[test]
fn a_single_dash_flag_is_never_a_flags_value() {
    let (dir, root) = tree("dash", SPEC_05);
    // `-h` after a value flag asks for help, as `--help` there does
    let out = docsys(&dir, &["rules", "--agents-md", "--write", "-h"]);
    assert!(
        !dir.join("-h").exists(),
        "`-h` was taken as the file to write: {out:?}"
    );
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(stdout(&out).starts_with("docsys rules"), "{out:?}");
    let out = docsys(
        &dir,
        &["debt", "add", "x", "--deferred", "-h", "--repay-when", "z"],
    );
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(stdout(&out).starts_with("docsys debt add"), "{out:?}");
    assert!(!root.join("work").exists(), "{out:?}");
    // any other single-dash flag is refused by name
    for (args, cmd, flag) in [
        (
            &["page", "new", "reference", "z", "--title", "-V"][..],
            "page new",
            "-V",
        ),
        (&["rules", "--agents-md", "--write", "-x"], "rules", "-x"),
        (
            &["export", "manifest", "--out", "-x"],
            "export manifest",
            "-x",
        ),
        (&["journal", "--since", "-x"], "journal", "-x"),
    ] {
        let out = docsys(&dir, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {out:?}");
        assert!(
            stderr(&out).starts_with(&format!("`{flag}` is no flag of {cmd}")),
            "{args:?}: {out:?}"
        );
    }
    assert!(!dir.join("-x").exists() && !root.join("reference/z.md").exists());
    // a dash before a digit, or alone, is a value
    for (id, title) in [("minus", "-1"), ("dash", "-")] {
        let out = docsys(&dir, &["page", "new", "reference", id, "--title", title]);
        assert!(out.status.success(), "{title}: {out:?}");
        assert!(root.join(format!("reference/{id}.md")).is_file(), "{out:?}");
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_unreadable_file_is_named_and_blocks_only_the_command_that_reads_it() {
    let (dir, root) = tree("utf8", SPEC_05);
    let debt = [
        "--deferred",
        "no load yet",
        "--repay-when",
        "the next outage",
    ];
    // a page elsewhere that is not UTF-8: lint names it, `debt add` is not its reader
    fs::create_dir_all(root.join("reference")).unwrap();
    fs::write(root.join("reference/x.md"), b"---\nid: x\n---\ncaf\xe9\n").unwrap();
    let out = docsys(&dir, &["lint"]);
    assert!(
        stdout(&out).contains("reference/x.md") || stderr(&out).contains("reference/x.md"),
        "the tree's read error names the file: {out:?}"
    );
    let out = docsys(
        &dir,
        &[&["debt", "add", "retries are unbounded"][..], &debt].concat(),
    );
    assert!(out.status.success(), "{out:?}");
    assert!(
        stdout(&out).contains("added: work/debt/general.md"),
        "{out:?}"
    );
    fs::remove_file(root.join("reference/x.md")).unwrap();
    // its own topic file unreadable: refused by name, the bytes kept
    let bytes =
        b"- [ ] 2026-10-01 caf\xe9 limits -- deferred: no load -- repay when: first outage\n";
    fs::write(root.join("work/debt/general.md"), bytes).unwrap();
    let out = docsys(
        &dir,
        &[&["debt", "add", "the cache is cold"][..], &debt].concat(),
    );
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        stderr(&out).contains("work/debt/general.md: ")
            && stderr(&out).contains("the file is left as it is"),
        "{out:?}"
    );
    // another topic is another file
    let out = docsys(
        &dir,
        &[
            &["debt", "add", "the backoff is fixed", "--topic", "retry"][..],
            &debt,
        ]
        .concat(),
    );
    assert!(out.status.success(), "{out:?}");
    assert!(
        stdout(&out).contains("added: work/debt/retry.md"),
        "{out:?}"
    );
    // closing numbers every topic file's items: an unread one is named
    for (list, flag, file) in [
        ("debt", "--note", "work/debt/general.md"),
        ("question", "--answer", "work/questions/general.md"),
    ] {
        if list == "question" {
            fs::create_dir_all(root.join("work/questions")).unwrap();
            fs::write(root.join(file), bytes).unwrap();
        }
        for which in ["1", "limits"] {
            let out = docsys(&dir, &[list, "close", which, flag, "done"]);
            assert_eq!(out.status.code(), Some(2), "{out:?}");
            assert!(
                !stderr(&out).contains("no open item") && stderr(&out).contains(file),
                "{list} close {which}: {out:?}"
            );
            assert_eq!(fs::read(root.join(file)).unwrap(), bytes);
        }
    }
    let _ = fs::remove_dir_all(&dir);
    // a docsys/0.4 tree's ledger it cannot read is named, never written over
    let (dir, root) = tree(
        "utf8-04",
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n",
    );
    fs::create_dir_all(root.join("work")).unwrap();
    fs::write(root.join("work/debt.md"), bytes).unwrap();
    let out = docsys(
        &dir,
        &[&["debt", "add", "the cache is cold"][..], &debt].concat(),
    );
    assert_eq!(
        fs::read(root.join("work/debt.md")).unwrap(),
        bytes,
        "{out:?}"
    );
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        stderr(&out).contains("work/debt.md: ")
            && stderr(&out).contains("the file is left as it is"),
        "{out:?}"
    );
    // and its questions ledger alike
    fs::write(root.join("work/questions.md"), bytes).unwrap();
    let out = docsys(&dir, &["question", "add", "who owns the cache"]);
    assert_eq!(
        fs::read(root.join("work/questions.md")).unwrap(),
        bytes,
        "{out:?}"
    );
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        stderr(&out).contains("work/questions.md: ")
            && stderr(&out).contains("the file is left as it is"),
        "{out:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_docmeta_problem_is_named_at_its_own_line() {
    let (dir, _) = tree(
        "docmeta-line",
        &format!("{SPEC_05}profile: project\nmaintainers: [ayse\n"),
    );
    let out = docsys(&dir, &["lint"]);
    let text = format!("{}{}", stdout(&out), stderr(&out));
    assert!(
        text.contains("parse: line 4: duplicate key `profile`"),
        "{text}"
    );
    assert!(
        text.contains("parse: line 5: the inline list `maintainers` never closes"),
        "{text}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn raw_move_and_forget_refuse_while_a_page_s_sources_never_close() {
    let dir = tmp("raw-move");
    let out = docsys(
        &dir,
        &["init", "--root", ".", "--profile", "knowledge-base"],
    );
    assert!(out.status.success(), "{out:?}");
    let meta = fs::read_to_string(dir.join(".docmeta.yml")).unwrap();
    assert!(meta.contains("domains: []"), "{meta}");
    fs::write(
        dir.join(".docmeta.yml"),
        meta.replace("domains: []", "domains: [notes]"),
    )
    .unwrap();
    let record = "raw/inbox/2026-10-03-notes-note-one.md";
    fs::create_dir_all(dir.join("raw/inbox")).unwrap();
    fs::write(dir.join(record), "Note one.\n").unwrap();
    let page = "wiki/notes/reference/note-facts.md";
    fs::create_dir_all(dir.join("wiki/notes/reference")).unwrap();
    fs::write(
        dir.join(page),
        format!("---\nid: note-facts\ntype: reference\nsources: [{record},\nlang: en\n---\n# Note facts\n\nBody.\n"),
    )
    .unwrap();
    let before = snapshot(&dir);
    let refusal = format!("{page}: `sources` on line 4 opens a list that never closes with `]`");
    let out = docsys(&dir, &["raw", "move", record, "notes", "--root", "."]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "a page whose `sources:` cannot be read may cite it: {out:?}"
    );
    assert!(stderr(&out).contains(&refusal), "{out:?}");
    assert_eq!(snapshot(&dir), before, "nothing moved, nothing written");
    let out = docsys(
        &dir,
        &["forget", record, "--reason", "wrong source", "--root", "."],
    );
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(stderr(&out).contains(&refusal), "{out:?}");
    assert_eq!(snapshot(&dir), before, "nothing forgotten, nothing written");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_writer_refuses_a_frontmatter_whose_list_never_closes() {
    let (repo, root) = project("writers", SPEC_05);
    fs::create_dir_all(repo.join("scripts")).unwrap();
    fs::write(repo.join("scripts/y.sh"), "#!/bin/sh\necho y\n").unwrap();
    fs::write(repo.join("scripts/z.sh"), "#!/bin/sh\necho z\n").unwrap();
    let page = root.join("reference/token-ttl.md");
    fs::write(
        &page,
        "---\nid: token-ttl\ntype: reference\nsources: [a.md]\n---\nTokens live an hour.\n",
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "a page"]);
    let out = docsys(&repo, &["pin", "reference/token-ttl", "scripts/y.sh"]);
    assert!(out.status.success(), "{out:?}");
    let text = fs::read_to_string(&page).unwrap();
    fs::write(&page, text.replace("sources: [a.md]", "sources: [a.md,")).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "pinned, its list left open"]);
    let refusal =
        "reference/token-ttl.md: `sources` on line 4 opens a list that never closes with `]`";
    let before = snapshot(&root);
    for args in [
        &["pin", "reference/token-ttl", "scripts/z.sh"][..],
        &["pin", "--refresh", "reference/token-ttl"],
        &["pin", "--gc"],
    ] {
        let out = docsys(&repo, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {out:?}");
        assert!(stderr(&out).contains(refusal), "{args:?}: {out:?}");
        assert_eq!(snapshot(&root), before, "{args:?} wrote nothing");
    }
    let _ = fs::remove_dir_all(&repo);
    // a docsys/0.4 tree's record writes
    let (repo, root) = project(
        "writers-04",
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n",
    );
    let page = root.join("reference/retry.md");
    fs::write(
        &page,
        PAGE.replace(
            "sources: [src/retry.rs]\n",
            "sources: [src/retry.rs,\nverification: unverified\nupdated: 2026-01-01\n",
        ),
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "its list left open"]);
    let before = snapshot(&root);
    let refusal = "reference/retry.md: `sources` on line 4 opens a list that never closes with `]`";
    for args in [&["verify", "retry"][..], &["verify", "retry", "--revoke"]] {
        let out = docsys(&repo, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {out:?}");
        assert!(stderr(&out).contains(refusal), "{args:?}: {out:?}");
        assert_eq!(snapshot(&root), before, "{args:?} wrote nothing");
    }
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn fetch_refuses_a_provider_list_that_never_closes() {
    let (dir, _) = tree(
        "fetch",
        &format!("{SPEC_05}consume: [up=/nonexistent/up#docs,\n"),
    );
    let out = docsys(&dir, &["fetch"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        stderr(&out)
            .contains(".docmeta.yml: `consume` on line 4 opens a list that never closes with `]`"),
        "{out:?}"
    );
    assert!(!dir.join("docs/.federation").exists(), "{out:?}");
    let _ = fs::remove_dir_all(&dir);
}
