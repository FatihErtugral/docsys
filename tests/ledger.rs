#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// Tests report through panics by design; the production lints stay strict.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-ledger-{name}-{}", std::process::id()));
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

const QUESTIONS: &str = "# Questions\n\n\
- [ ] 2026-09-03 does the export keep its order\n\
- [ ] 2026-09-05 who decides the retry budget\n";

const DEBT: &str = "# Debt\n\n\
- [ ] 2026-09-01 the retry budget is a guess -- deferred: no traffic yet -- repay when: the first week under load\n\
- [ ] 2026-09-02 the cache key ignores the locale -- deferred: one locale today -- repay when: a second locale ships\n";

/// A committed project tree at `<repo>/docs`, its ledgers holding open items.
fn build(name: &str) -> (PathBuf, PathBuf) {
    let repo = tmp(name);
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    let root = repo.join("docs");
    fs::create_dir_all(root.join("work")).unwrap();
    fs::write(
        root.join(".docmeta.yml"),
        "spec: docsys/0.5\nprofile: project\ndefault_content_language: en\n",
    )
    .unwrap();
    fs::write(root.join("index.md"), "# Docs\n").unwrap();
    fs::write(
        root.join("work/journal.md"),
        "# Journal\n\n## 2026-09-01 - started\n- opened the tree\n",
    )
    .unwrap();
    fs::write(root.join("work/questions.md"), QUESTIONS).unwrap();
    fs::write(root.join("work/debt.md"), DEBT).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "ledgers"]);
    (repo, root)
}

fn r045(root: &Path) -> Vec<String> {
    let (report, _) = docsys::lint(root);
    report
        .findings
        .iter()
        .filter(|f| f.rule.0 == "R-045")
        .map(|f| format!("{} {} {}", f.severity.tag(), f.file, f.subject))
        .collect()
}

fn restore(repo: &Path) {
    git(repo, &["checkout", "-q", "--", "."]);
    git(repo, &["clean", "-qfd"]);
}

#[test]
fn an_open_item_that_vanished_is_reported_and_its_counterparts_are_not() {
    let (repo, root) = build("vanished");
    assert!(r045(&root).is_empty());

    // an open question deleted
    fs::write(
        root.join("work/questions.md"),
        QUESTIONS.replace("- [ ] 2026-09-03 does the export keep its order\n", ""),
    )
    .unwrap();
    assert_eq!(
        r045(&root),
        vec!["WARN work/questions.md 2026-09-03 does the export keep its"]
    );
    restore(&repo);

    // the same question closed in place
    fs::write(
        root.join("work/questions.md"),
        QUESTIONS.replace(
            "- [ ] 2026-09-03 does the export keep its order\n",
            "- [x] 2026-09-03 does the export keep its order -- answered: it does\n",
        ),
    )
    .unwrap();
    assert!(r045(&root).is_empty(), "{:?}", r045(&root));
    restore(&repo);

    // reworded, same date
    fs::write(
        root.join("work/questions.md"),
        QUESTIONS.replace(
            "does the export keep its order",
            "is the export order stable across runs",
        ),
    )
    .unwrap();
    assert!(r045(&root).is_empty(), "{:?}", r045(&root));
    restore(&repo);

    // moved to an archive slice
    fs::write(
        root.join("work/questions.md"),
        QUESTIONS.replace("- [ ] 2026-09-03 does the export keep its order\n", ""),
    )
    .unwrap();
    fs::create_dir_all(root.join("_archive/work")).unwrap();
    fs::write(
        root.join("_archive/work/questions-2026.md"),
        "# Questions, 2026\n\n- [x] 2026-09-03 does the export keep its order -- answered: it does\n",
    )
    .unwrap();
    assert!(r045(&root).is_empty(), "{:?}", r045(&root));
    restore(&repo);

    // a debt repaid through `debt close`: the journal carries its text
    let out = Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args(["debt", "close", "1", "--root"])
        .arg(&root)
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert!(r045(&root).is_empty(), "{:?}", r045(&root));
    restore(&repo);

    // a debt deleted by hand
    fs::write(
        root.join("work/debt.md"),
        DEBT.lines()
            .filter(|l| !l.contains("cache key"))
            .map(|l| format!("{l}\n"))
            .collect::<String>(),
    )
    .unwrap();
    assert_eq!(
        r045(&root),
        vec!["WARN work/debt.md 2026-09-02 the cache key ignores the"]
    );
    restore(&repo);

    // an item closed before HEAD is its own counterpart, nobody else's
    let closed = format!("{QUESTIONS}- [x] 2026-09-05 is the cache warm -- answered: yes\n");
    fs::write(root.join("work/questions.md"), &closed).unwrap();
    git(&repo, &["commit", "-qam", "a closed question"]);
    fs::write(
        root.join("work/questions.md"),
        closed.replace("- [ ] 2026-09-05 who decides the retry budget\n", ""),
    )
    .unwrap();
    assert_eq!(
        r045(&root),
        vec!["WARN work/questions.md 2026-09-05 who decides the retry budget"]
    );
    let _ = fs::remove_dir_all(&repo);
}
