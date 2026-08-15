//! Conformance corpus harness (R-190/R-191): every case is a tree plus its
//! exact expected findings. Extra findings fail as hard as missing ones —
//! a checker looser OR noisier than the corpus is nonconforming.

use docsys::{lint, Outcome};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn expected_of(case: &Path) -> (Option<u8>, Vec<String>) {
    let text = fs::read_to_string(case.join("expected.tsv")).unwrap_or_default();
    let mut exit = None;
    let mut lines = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(code) = line.strip_prefix("EXIT\t") {
            exit = code.trim().parse::<u8>().ok();
        } else {
            lines.push(line.to_string());
        }
    }
    lines.sort();
    (exit, lines)
}

fn run_case(case: &Path) -> Result<(), String> {
    let (want_exit, want) = expected_of(case);
    let (report, outcome) = lint(&case.join("tree").join("docs"));
    let got_exit: u8 = match outcome {
        Outcome::Clean => 0,
        Outcome::Errors => 1,
        Outcome::Config => 2,
    };
    let mut got: Vec<String> = report
        .findings
        .iter()
        .map(|f| {
            format!(
                "{}\t{}\t{}\t{}",
                f.severity.tag(),
                f.rule,
                f.file,
                f.subject
            )
        })
        .collect();
    got.sort();
    got.dedup();

    let mut errors = String::new();
    if let Some(we) = want_exit {
        if we != got_exit {
            let _ = writeln!(errors, "exit: want {we}, got {got_exit}");
        }
    }
    for missing in want.iter().filter(|w| !got.contains(w)) {
        let _ = writeln!(errors, "missing: {missing}");
    }
    for extra in got.iter().filter(|g| !want.contains(g)) {
        let _ = writeln!(errors, "extra:   {extra}");
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[test]
fn corpus() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus/cases");
    let mut cases: Vec<PathBuf> = fs::read_dir(&root)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    cases.sort();
    assert!(
        !cases.is_empty(),
        "corpus/cases is empty — nothing was tested (R-011)"
    );

    let mut failures = String::new();
    let mut passed = 0usize;
    for case in &cases {
        if !case.is_dir() {
            continue;
        }
        match run_case(case) {
            Ok(()) => passed += 1,
            Err(e) => {
                let name = case.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                let _ = writeln!(failures, "== {name}\n{e}");
            }
        }
    }
    assert!(
        failures.is_empty(),
        "corpus failures ({passed} passed):\n{failures}"
    );
}
