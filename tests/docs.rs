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
    // D-130: a docsys/0.5 tree runs three relays, no post-edit one
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
        "D-122 against D-130"
    );
    // D-130: a docsys/0.5 page carries no verification; the approval job is
    // a docsys/0.4 tree's alone
    let this_release = CHANGELOG.split("## [0.15.1]").next().unwrap_or("");
    for gone in [
        "approvals land through a pull request",
        "`description`",
        "Approved-by:",
        "verify --show",
        "pin … --block",
        "reads `unverified`",
    ] {
        assert!(!this_release.contains(gone), "CHANGELOG: `{gone}`");
    }
    for gone in [
        "Approved-by",
        "a maintainer verifies",
        "turns unverified",
        "audit the\nwiki",
        "`.verifies/`",
    ] {
        assert!(!README.contains(gone), "README: `{gone}`");
    }
    assert!(
        !decision("D-105").contains("`pull-request`, the default for a new adoption"),
        "D-105 against D-130"
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

/// The rules SPEC withdrew, each by its id.
fn withdrawn() -> Vec<String> {
    SPEC.lines()
        .filter(|l| l.contains("WITHDRAWN"))
        .filter_map(|l| {
            let at = l.find("R-")?;
            let id: String = l[at..]
                .chars()
                .take_while(|c| *c == 'R' || *c == '-' || c.is_ascii_digit())
                .collect();
            (id.len() > 2).then_some(id)
        })
        .collect()
}

/// A live rule cites a withdrawn one only as withdrawn: its reason lives in
/// the rule that absorbed it.
#[test]
fn a_live_rule_cites_no_withdrawn_rule_as_live() {
    let withdrawn = withdrawn();
    assert!(withdrawn.iter().any(|r| r == "R-150"), "{withdrawn:?}");
    let mut cited = Vec::new();
    for rule in SPEC.split("\n**R-").skip(1) {
        let rule = rule.split("\n#").next().unwrap_or(rule);
        let id: String = rule.chars().take_while(char::is_ascii_digit).collect();
        if rule.lines().next().is_some_and(|l| l.contains("WITHDRAWN")) {
            continue;
        }
        for r in &withdrawn {
            for (at, _) in rule.match_indices(r.as_str()) {
                let after = &rule[at + r.len()..];
                let before = rule[..at].trim_end();
                if !after.starts_with(|c: char| c.is_ascii_digit())
                    && !before.ends_with("withdrawn")
                {
                    cited.push(format!("R-{id} cites {r}"));
                }
            }
        }
    }
    assert!(cited.is_empty(), "{cited:?}");
}

/// D-118 says which 0.4 findings 0.16 changes: the reader fixes of D-002
/// reach a docsys/0.4 tree, so it no longer claims 0.15's findings exactly.
#[test]
fn d_118_names_the_reader_fixes_a_0_4_tree_gets() {
    let d118 = decision("D-118");
    assert!(
        !d118.contains("gets exactly the findings 0.15 gave it"),
        "{d118}"
    );
    for named in [
        "D-002",
        "`commit_policy: require   # …` is `require`",
        "R-111 or R-113",
        "R-050 in a page, R-161 in `.docmeta.yml`",
        "R-059",
    ] {
        assert!(d118.contains(named), "D-118 names {named}: {d118}");
    }
}

/// The README cites live rules only: a rule SPEC withdrew is no reason.
#[test]
fn the_readme_cites_no_withdrawn_rule() {
    let withdrawn: Vec<String> = SPEC
        .lines()
        .filter(|l| l.contains("WITHDRAWN"))
        .filter_map(|l| {
            let at = l.find("R-")?;
            let id: String = l[at..]
                .chars()
                .take_while(|c| *c == 'R' || *c == '-' || c.is_ascii_digit())
                .collect();
            (id.len() > 2).then_some(id)
        })
        .collect();
    assert!(withdrawn.iter().any(|r| r == "R-150"), "{withdrawn:?}");
    for r in &withdrawn {
        let cited = README
            .match_indices(r.as_str())
            .any(|(at, _)| !README[at + r.len()..].starts_with(|c: char| c.is_ascii_digit()));
        assert!(!cited, "README cites withdrawn {r}");
    }
}
