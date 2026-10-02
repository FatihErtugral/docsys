#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// Tests report through panics by design; the production lints stay strict.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-capture-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let p = e.unwrap().path();
        let dest = to.join(p.file_name().unwrap());
        if p.is_dir() {
            copy_tree(&p, &dest);
        } else {
            fs::copy(&p, &dest).unwrap();
        }
    }
}

fn r108(root: &Path) -> Vec<String> {
    let (report, _) = docsys::lint(root);
    report
        .findings
        .iter()
        .filter(|f| f.rule.0 == "R-108")
        .map(|f| format!("{} {}", f.file, f.subject))
        .collect()
}

fn ledger_fix(root: &Path) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args(["ledger", "fix", "--root"])
        .arg(root)
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn ledger_fix_rewrites_dashed_markers_once_and_leaves_field_text() {
    let base = tmp("ledger-fix");
    let root = base.join("docs");
    copy_tree(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("corpus/cases/42-ledger-separators/tree/docs"),
        &root,
    );
    fs::create_dir_all(root.join("_archive/work")).unwrap();
    fs::write(
        root.join("_archive/work/questions-2025.md"),
        "# Questions, 2025\n\n- [x] 2025-03-01 which region first — answered: the nearest one\n",
    )
    .unwrap();
    assert_eq!(
        r108(&root),
        vec!["work/debt.md line-3", "work/questions.md line-4"]
    );

    let said = ledger_fix(&root);
    assert_eq!(
        said.trim_end(),
        "fixed: work/debt.md line 3\nfixed: work/questions.md line 4\n\
         fixed: _archive/work/questions-2025.md line 3"
    );
    assert!(r108(&root).is_empty(), "{:?}", r108(&root));
    let debt = fs::read_to_string(root.join("work/debt.md")).unwrap();
    assert!(debt.contains(
        "- [ ] 2026-09-01 the retry budget is a guess -- deferred: no traffic yet -- repay when: the first week under load\n"
    ));
    let questions = fs::read_to_string(root.join("work/questions.md")).unwrap();
    // the open question's dash is its own text, not a marker
    assert!(
        questions.contains("- [ ] 2026-09-03 does the export keep its order — or only its set?\n")
    );
    assert!(questions.contains(
        "- [x] 2026-09-04 who owns the retry budget -- answered: the service that sends the request\n"
    ));
    let slice = fs::read_to_string(root.join("_archive/work/questions-2025.md")).unwrap();
    assert!(slice.contains("which region first -- answered: the nearest one"));

    // a second run changes nothing, byte for byte
    let before: Vec<String> = [
        "work/debt.md",
        "work/questions.md",
        "_archive/work/questions-2025.md",
    ]
    .iter()
    .map(|f| fs::read_to_string(root.join(f)).unwrap())
    .collect();
    assert_eq!(
        ledger_fix(&root).trim_end(),
        "ledger: every field marker is already ASCII"
    );
    let after: Vec<String> = [
        "work/debt.md",
        "work/questions.md",
        "_archive/work/questions-2025.md",
    ]
    .iter()
    .map(|f| fs::read_to_string(root.join(f)).unwrap())
    .collect();
    assert_eq!(before, after);
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn ledger_fix_reads_the_declared_labels() {
    let root = tmp("ledger-labels");
    fs::create_dir_all(root.join("work")).unwrap();
    fs::write(
        root.join(".docmeta.yml"),
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: xx\nlist_labels: [deferred=WHY, repay when=WHEN]\n",
    )
    .unwrap();
    fs::write(root.join("index.md"), "# Docs\n").unwrap();
    let item =
        "- [ ] 2026-09-01 a guess — WHY: no load yet — WHEN: load arrives — deferred: kept\n";
    fs::write(root.join("work/debt.md"), format!("# Debt\n\n{item}")).unwrap();
    docsys::capture::ledger_fix(&root).unwrap();
    // the local forms are the markers; the canonical word is field text here
    assert_eq!(
        fs::read_to_string(root.join("work/debt.md")).unwrap(),
        "# Debt\n\n- [ ] 2026-09-01 a guess -- WHY: no load yet -- WHEN: load arrives — deferred: kept\n"
    );
    assert!(r108(&root).is_empty(), "{:?}", r108(&root));
    let _ = fs::remove_dir_all(&root);
}
