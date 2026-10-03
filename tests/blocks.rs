#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]
// Block-level verification (R-212, D-103): an approval vouches for the
// body's blocks, so a changed page costs a re-read of the change, not of the
// page. On docsys/0.5 the approval is a commit (D-126); the blocks it read are
// the page as that commit held it. Driven through the binary and plain git.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-blocks-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn commit(dir: &Path, msg: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", msg]);
}

struct Run {
    code: i32,
    out: String,
    err: String,
}

/// `docsys <args>` from the repository's top, the tree at `docs`.
fn docsys(dir: &Path, args: &[&str]) -> Run {
    let out = Command::new(bin())
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    Run {
        code: out.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&out.stdout).into_owned(),
        err: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let r = docsys(dir, args);
    assert_eq!(r.code, 0, "docsys {args:?}: {}{}", r.out, r.err);
    r.out
}

const LIB_RS: &str = "pub fn alpha() -> u32 {\n    1\n}\n\npub fn beta() -> u32 {\n    2\n}\n";

/// Six blocks: [1] the heading, [2] a paragraph, [3]–[6] one bullet each.
const BODY: &str = "\n# Token lifetime\n\nA token is issued at sign-in.\n\n- It lives twelve hours.\n- It renews at half its lifetime.\n- A revoked token fails at once.\n- The clock skew allowed is a minute.\n";

/// A project at `spec`, its one page committed by its maintainer.
fn project(name: &str, spec: &str) -> (PathBuf, PathBuf) {
    let repo = tmp(name);
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "ayse@example.com"]);
    git(&repo, &["config", "user.name", "ayse"]);
    let root = repo.join("docs");
    let write = |rel: &str, text: &str| {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    };
    write(
        ".docmeta.yml",
        &format!("spec: docsys/{spec}\nprofile: project\ndefault_content_language: en\nmaintainers: [ayse <ayse@example.com>]\n"),
    );
    write(
        "index.md",
        "# Docs\n\n- [[reference/token-ttl|Token lifetime]] -- how long a token lives.\n",
    );
    if spec == "0.4" {
        write("work/journal.md", "# Journal\n");
        write("work/debt.md", "# Debt\n");
        write("work/questions.md", "# Questions\n");
        write(
            "reference/token-ttl.md",
            &format!(
                "---\nid: token-ttl\ntype: reference\nupdated: {}\nverification: unverified\nsources: []\n---\n{BODY}",
                docsys::migrate::today()
            ),
        );
    } else {
        // docsys/0.5: a page takes part by its sources, and its date and its
        // verification are history's (D-122, D-126)
        write(
            "reference/token-ttl.md",
            &format!("---\nid: token-ttl\ntype: reference\nsources: [src/lib.rs]\n---\n{BODY}"),
        );
    }
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/lib.rs"), LIB_RS).unwrap();
    commit(&repo, "docs: token lifetime");
    (repo, root)
}

fn edit(root: &Path, from: &str, to: &str) {
    let p = root.join("reference/token-ttl.md");
    let text = fs::read_to_string(&p).unwrap();
    assert!(text.contains(from), "{text}");
    fs::write(&p, text.replace(from, to)).unwrap();
}

/// The lines of `out` whose block is marked `mark` (`changed`, `new`).
fn marked<'a>(out: &'a str, mark: &str) -> Vec<&'a str> {
    let tail = format!(", {mark}:");
    out.lines().filter(|l| l.ends_with(&tail)).collect()
}

#[test]
fn show_prints_exactly_the_bullet_that_changed() {
    let (repo, root) = project("show", "0.5");
    let done = ok(&repo, &["verify", "token-ttl"]);
    assert!(
        done.contains("verified: reference/token-ttl.md by ayse"),
        "{done}"
    );
    // the word is the maintainer's commit; nothing is written into the page
    assert!(git(&repo, &["log", "-1", "--format=%B"])
        .contains("\nApproved-by: ayse <ayse@example.com>"),);
    assert!(git(&repo, &["log", "-1", "--format=%B"]).contains("Verifies: reference/token-ttl.md "));
    let page = fs::read_to_string(root.join("reference/token-ttl.md")).unwrap();
    assert!(!page.contains("verifi"), "{page}");

    edit(
        &root,
        "It renews at half its lifetime.",
        "It renews at a third of its lifetime.",
    );
    let out = ok(&repo, &["verify", "--show", "token-ttl"]);
    let changed = marked(&out, "changed");
    assert_eq!(changed.len(), 1, "{out}");
    assert!(changed[0].starts_with("[4] line "), "{out}");
    assert!(marked(&out, "new").is_empty(), "{out}");
    assert!(!out.contains("removed"), "{out}");
    assert!(
        out.contains("It renews at a third of its lifetime."),
        "{out}"
    );
    assert!(out.contains("5/6 blocks"), "{out}");
    // the edit made it unverified; lookup says how much still reads as verified
    let hits = ok(&repo, &["lookup", "token"]);
    assert!(
        hits.contains("(unverified — 5/6 blocks as verified by ayse)"),
        "{hits}"
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_removed_block_prints_from_what_the_approval_read() {
    let (repo, root) = project("removed", "0.5");
    // the body is approved on a branch
    git(&repo, &["checkout", "-q", "-b", "pr"]);
    edit(
        &root,
        "- The clock skew allowed is a minute.\n",
        "- The clock skew allowed is a minute.\n- A session ends at sign-out.\n",
    );
    commit(&repo, "docs: sessions");
    ok(&repo, &["verify", "token-ttl"]);
    edit(&root, "- A revoked token fails at once.\n", "");
    let out = ok(&repo, &["verify", "--show", "token-ttl"]);
    assert!(out.contains("1 removed:"), "{out}");
    assert!(out.contains("A revoked token fails at once."), "{out}");
    assert!(marked(&out, "changed").is_empty(), "{out}");
    git(&repo, &["checkout", "--", "."]);

    // a squash whose message carries the pull request's description keeps the
    // approval, and what it read, after the branch is gone
    git(&repo, &["checkout", "-q", "main"]);
    git(&repo, &["merge", "-q", "--squash", "pr"]);
    git(
        &repo,
        &[
            "commit",
            "-q",
            "-m",
            "docs: sessions (#2)\n\nApproved-by: ayse <ayse@example.com>",
        ],
    );
    git(&repo, &["branch", "-q", "-D", "pr"]);
    git(&repo, &["reflog", "expire", "--expire=now", "--all"]);
    git(&repo, &["gc", "-q", "--prune=now"]);
    let hits = ok(&repo, &["lookup", "token"]);
    assert!(!hits.contains("unverified"), "{hits}");
    edit(&root, "- A revoked token fails at once.\n", "");
    let out = ok(&repo, &["verify", "--show", "token-ttl"]);
    assert!(out.contains("1 removed:"), "{out}");
    assert!(out.contains("A revoked token fails at once."), "{out}");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_stale_bound_pin_names_its_block_and_only_that_block_stops_reading_as_verified() {
    let (repo, root) = project("bound", "0.5");
    ok(
        &repo,
        &[
            "pin",
            "token-ttl",
            "src/lib.rs",
            "--symbol",
            "alpha",
            "--block",
            "3",
        ],
    );
    let page = fs::read_to_string(root.join("reference/token-ttl.md")).unwrap();
    assert!(
        page.contains("  - path: src/lib.rs\n    symbol: alpha\n    block: "),
        "{page}"
    );
    commit(&repo, "docs: pin");
    ok(&repo, &["verify", "token-ttl"]);
    assert_eq!(docsys(&repo, &["lint"]).code, 0);

    // the region the bullet rests on moves: only that bullet stops counting
    fs::write(
        repo.join("src/lib.rs"),
        LIB_RS.replace("    1\n", "    7\n"),
    )
    .unwrap();
    let lint = docsys(&repo, &["lint"]);
    let r111: Vec<&str> = lint
        .out
        .lines()
        .filter(|l| l.starts_with("ERROR R-111 reference/token-ttl.md [src/lib.rs#alpha]"))
        .collect();
    assert_eq!(r111.len(), 1, "{}", lint.out);
    assert!(r111[0].contains("[3]"), "{}", r111[0]);
    let status = ok(&repo, &["status", "--json"]);
    assert!(status.contains("\"partially_verified\":1"), "{status}");
    let status = ok(&repo, &["status"]);
    assert!(status.contains("reference/token-ttl.md 5/6"), "{status}");
    let hits = ok(&repo, &["lookup", "token"]);
    assert!(
        hits.contains("(verified — 5/6 blocks as verified by ayse)"),
        "{hits}"
    );
    let out = ok(&repo, &["verify", "--show", "token-ttl"]);
    assert!(
        out.contains("src/lib.rs#alpha bound to [3], stale"),
        "{out}"
    );

    // the bullet itself is rewritten: the binding is lost, and reported
    fs::write(repo.join("src/lib.rs"), LIB_RS).unwrap();
    edit(&root, "It lives twelve hours.", "It lives ten hours.");
    let lint = docsys(&repo, &["lint"]);
    assert!(
        lint.out
            .contains("WARN R-212 reference/token-ttl.md [src/lib.rs#alpha]"),
        "{}",
        lint.out
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_0_4_tree_records_no_blocks_and_refuses_a_binding() {
    let (repo, root) = project("v04", "0.4");
    ok(&repo, &["verify", "token-ttl", "--commit"]);
    let page = fs::read_to_string(root.join("reference/token-ttl.md")).unwrap();
    assert!(
        page.contains("verification: verified\nverified_by: ayse\nverified_rev: "),
        "{page}"
    );
    assert!(!page.contains("verified_blocks"), "{page}");
    let refused = docsys(
        &repo,
        &[
            "pin",
            "token-ttl",
            "src/lib.rs",
            "--symbol",
            "alpha",
            "--block",
            "3",
        ],
    );
    assert_eq!(refused.code, 2, "{}{}", refused.out, refused.err);
    assert!(
        refused.err.contains("docsys/0.5") && refused.err.contains("(D-118)"),
        "{}",
        refused.err
    );
    let refused = docsys(&repo, &["verify", "--show", "token-ttl"]);
    assert_eq!(refused.code, 2, "{}{}", refused.out, refused.err);
    assert!(refused.err.contains("(D-118)"), "{}", refused.err);

    // a `block:` written by hand on a 0.4 tree is not read: the findings are
    // the ones the same tree gives without it (the summary counts one more
    // scanned line)
    ok(
        &repo,
        &["pin", "token-ttl", "src/lib.rs", "--symbol", "alpha"],
    );
    commit(&repo, "docs: pin");
    let before = docsys(&repo, &["lint"]);
    let p = root.join("reference/token-ttl.md");
    let text = fs::read_to_string(&p).unwrap();
    fs::write(
        &p,
        text.replace(
            "    symbol: alpha\n",
            "    symbol: alpha\n    block: 065a99e960e7\n",
        ),
    )
    .unwrap();
    commit(&repo, "docs: a hand-written binding");
    fs::write(
        repo.join("src/lib.rs"),
        LIB_RS.replace("    1\n", "    7\n"),
    )
    .unwrap();
    let after = docsys(&repo, &["lint"]);
    assert!(!after.out.contains("R-212"), "{}", after.out);
    assert!(!after.out.contains("[3]"), "{}", after.out);
    fs::write(repo.join("src/lib.rs"), LIB_RS).unwrap();
    let clean = docsys(&repo, &["lint"]);
    let findings = |out: &str| {
        out.lines()
            .filter(|l| !l.starts_with("-- "))
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        (clean.code, findings(&clean.out)),
        (before.code, findings(&before.out))
    );
    let _ = fs::remove_dir_all(&repo);
}

/// The corpus case's R-024 message, which `expected.tsv` cannot carry
/// (D-011): a record left in a docsys/0.5 page names the command that takes
/// it out (D-126).
#[test]
fn the_corpus_record_names_the_upgrade() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus/cases/46-block-record/tree/docs");
    let (report, _) = docsys::lint(&root);
    let r024: Vec<&docsys::model::Finding> = report
        .findings
        .iter()
        .filter(|f| f.rule.0 == "R-024")
        .collect();
    assert!(!r024.is_empty(), "{:?}", report.findings);
    for f in r024 {
        assert!(
            f.message.contains("`docsys upgrade --apply` takes it out"),
            "{}",
            f.message
        );
    }
}
