#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! The user documents say what this release does. Each sentence below was
//! found in them saying what a decision of this release retired; none comes
//! back, and the replacements are where a reader looks for them.

const README: &str = include_str!("../README.md");
const CHANGELOG: &str = include_str!("../CHANGELOG.md");
const SPEC: &str = include_str!("../SPEC.md");
const DECISIONS: &str = include_str!("../corpus/DECISIONS.md");

/// A decision's row in the table.
fn decision(id: &str) -> &'static str {
    DECISIONS
        .lines()
        .find(|l| l.starts_with(&format!("| {id} |")))
        .unwrap_or_else(|| panic!("{id}"))
}

#[test]
fn no_document_says_what_this_release_retired() {
    // D-126: a docsys/0.5 tree runs three relays, no post-edit one
    for gone in [
        "installs four hooks",
        "four relay hooks",
        "post-edit hook<br/>demotes",
        "The same four relays",
        "the four relays",
        "four organs, four relays",
    ] {
        assert!(!README.contains(gone), "README: `{gone}`");
    }
    assert!(
        !decision("D-122").contains("The post-edit relay keeps one job"),
        "D-122 against D-126"
    );
    // D-126: the approval rides the description; D-105's modes are 0.4's
    assert!(
        !CHANGELOG.contains("approvals land through a pull request"),
        "CHANGELOG against its D-126 entry"
    );
    assert!(
        CHANGELOG.contains("`description`"),
        "CHANGELOG names the default mode"
    );
    assert!(
        !decision("D-105").contains("`pull-request`, the default for a new adoption"),
        "D-105 against D-126"
    );
    // R-106 is withdrawn: history dates a page, an `updated:` is no error
    assert!(
        !README.contains("an `updated:` behind the\nlast commit"),
        "README: a stale `updated:` as an error"
    );
    // D-124: closing takes the line; the file goes with its last item only
    assert!(
        !README.contains("# the item's file leaves;"),
        "README: closing removes the file"
    );
    // R-103 is withdrawn
    let r035 = SPEC
        .split("**R-035**")
        .nth(1)
        .and_then(|s| s.split("\n**R-").next())
        .unwrap();
    assert!(!r035.contains("R-103"), "R-035 names the withdrawn R-103");
    // D-114 routes intents in the first turn; D-129 explains commands in help;
    // and D-124's ledger is topic files
    assert!(
        !decision("D-129").contains("The rules block keeps routing intents"),
        "D-129 against D-114"
    );
    assert!(
        !decision("D-114").contains("a dated `questions.md` item"),
        "D-114 names the retired ledger"
    );
}
