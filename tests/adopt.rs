#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// Tests report through panics by design; the production lints stay strict.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-adopt-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn git_init(dir: &Path) {
    assert!(Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir)
        .status()
        .unwrap()
        .success());
}

#[test]
fn greenfield_adopt_scaffolds_everything_and_is_idempotent() {
    let repo = tmp("green");
    git_init(&repo);
    let docs = repo.join("docs");

    let out = docsys::adopt::run(&repo, &docs, "en").unwrap();
    // init skeleton, not a bare config file
    assert!(docs.join(".docmeta.yml").exists());
    assert!(docs.join("index.md").exists());
    // settings.json created when absent, with the hook wires
    let settings = fs::read_to_string(repo.join(".claude/settings.json")).unwrap();
    assert!(settings.contains("UserPromptSubmit"), "{settings}");
    // AGENTS.md managed block + report with the judgment checklist
    let agents = fs::read_to_string(repo.join("AGENTS.md")).unwrap();
    assert!(agents.contains("docsys:rules:begin"));
    let report = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    assert!(report.contains("Judgment checklist"), "{report}");
    assert!(report.contains("doc-extensions.md"), "{report}");
    // no .githooks, no configured hooksPath → gate falls back to .git/hooks
    let gate = fs::read_to_string(repo.join(".git/hooks/pre-commit")).unwrap();
    assert!(gate.contains("docsys documentation gate"), "{gate}");
    assert!(
        out.summary.iter().any(|s| s.contains("created")),
        "{:?}",
        out.summary
    );

    // Second run: everything already in place, nothing rewritten destructively.
    let again = docsys::adopt::run(&repo, &docs, "en").unwrap();
    assert!(
        again
            .summary
            .iter()
            .any(|s| s.contains(".docmeta.yml: kept")),
        "{:?}",
        again.summary
    );
    assert!(
        again.summary.iter().any(|s| s.contains("gate: kept")),
        "{:?}",
        again.summary
    );
    // settings now exists → untouched, and the merge snippet moves to the checklist
    assert!(again
        .summary
        .iter()
        .any(|s| s.contains("settings.json: untouched")));
    let report = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    assert!(report.contains("Merge the docsys hook wires"), "{report}");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn adopt_refuses_an_unmigrated_tree() {
    let repo = tmp("unmig");
    git_init(&repo);
    fs::create_dir_all(repo.join("docs")).unwrap();
    fs::write(repo.join("docs/old.md"), "# legacy page\n").unwrap();

    let err = docsys::adopt::run(&repo, &repo.join("docs"), "en").unwrap_err();
    assert!(err.contains("migrate inventory"), "{err}");
    // init must not have clobbered anything
    assert!(!repo.join("docs/index.md").exists());
    assert!(!repo.join("docs/.docmeta.yml").exists());
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn gate_prefers_a_tracked_githooks_dir_and_sets_hookspath() {
    let repo = tmp("githooks");
    git_init(&repo);
    fs::create_dir_all(repo.join(".githooks")).unwrap();

    docsys::adopt::run(&repo, &repo.join("docs"), "en").unwrap();
    let gate = fs::read_to_string(repo.join(".githooks/pre-commit")).unwrap();
    assert!(gate.contains("docsys documentation gate"), "{gate}");
    let out = Command::new("git")
        .args(["config", "core.hooksPath"])
        .current_dir(&repo)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), ".githooks");

    // Idempotent: the marker prevents a second append.
    docsys::adopt::run(&repo, &repo.join("docs"), "en").unwrap();
    let gate2 = fs::read_to_string(repo.join(".githooks/pre-commit")).unwrap();
    assert_eq!(gate.matches("docsys documentation gate").count(), 1);
    assert_eq!(gate2.matches("docsys documentation gate").count(), 1);
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn docmeta_upgrade_prepends_missing_keys_and_keeps_owner_lines() {
    let repo = tmp("upgrade");
    git_init(&repo);
    let docs = repo.join("docs");
    fs::create_dir_all(&docs).unwrap();
    fs::write(docs.join(".docmeta.yml"), "custom_key: kept-verbatim\n").unwrap();

    let out = docsys::adopt::run(&repo, &docs, "en").unwrap();
    let meta = fs::read_to_string(docs.join(".docmeta.yml")).unwrap();
    assert!(meta.starts_with("spec: docsys/0.4\n"), "{meta}");
    assert!(meta.contains("profile: project\n"));
    assert!(meta.contains("default_content_language: en\n"));
    assert!(meta.ends_with("custom_key: kept-verbatim\n"), "{meta}");
    assert!(
        out.summary.iter().any(|s| s.contains("upgraded")),
        "{:?}",
        out.summary
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn adopt_keeps_the_owners_comment_header_on_the_report() {
    let repo = tmp("header");
    git_init(&repo);
    let docs = repo.join("docs");
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let report = repo.join("ADOPTION.md");
    let body = fs::read_to_string(&report).unwrap();
    fs::write(
        &report,
        format!(
            "<!-- restricted-context:public -->\n<!-- reviewed:\n     2026-08-26 -->\n\n{body}"
        ),
    )
    .unwrap();
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let again = fs::read_to_string(&report).unwrap();
    assert!(
        again.starts_with("<!-- restricted-context:public -->\n<!-- reviewed:\n     2026-08-26 -->\n\n<!-- docsys:adoption:begin"),
        "{again}"
    );
    assert_eq!(again.matches("# docsys adoption report").count(), 1);
}

#[test]
fn adopt_names_hooks_behind_the_binary_template() {
    let repo = tmp("stale");
    git_init(&repo);
    let docs = repo.join("docs");
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let hook = repo.join(".claude/hooks/stop-docs-reminder.sh");
    let text = fs::read_to_string(&hook).unwrap();
    assert!(
        text.lines()
            .nth(1)
            .unwrap()
            .starts_with("# docsys-template: "),
        "{text}"
    );
    // an older stamp, and a hook with none at all
    fs::write(
        &hook,
        text.replace(docsys::agents::TEMPLATE_VERSION, "0.4.4"),
    )
    .unwrap();
    let pre = repo.join(".claude/hooks/pre-commit-docs.sh");
    let old = fs::read_to_string(&pre)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with("# docsys-template"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&pre, old).unwrap();
    let out = docsys::adopt::run(&repo, &docs, "en").unwrap();
    let line = out
        .summary
        .iter()
        .find(|l| l.starts_with("hooks:"))
        .expect("staleness line");
    assert!(line.contains("2 template(s) behind"), "{line}");
    assert!(line.contains("stop-docs-reminder.sh: 0.4.4"), "{line}");
    assert!(line.contains("pre-commit-docs.sh: unversioned"), "{line}");
    assert!(line.contains("agents --force"), "{line}");
    // doctor says the same, as information, not failure
    let d = docsys::doctor::run(&repo, &docs, &repo.join(".claude"));
    assert!(
        d.lines
            .iter()
            .any(|l| l.starts_with("info hooks/stop-docs-reminder.sh template 0.4.4")),
        "{:?}",
        d.lines
    );
    // --force refreshes, and the line is gone
    docsys::agents::install(&repo.join(".claude"), true).unwrap();
    let out = docsys::adopt::run(&repo, &docs, "en").unwrap();
    assert!(
        !out.summary.iter().any(|l| l.starts_with("hooks:")),
        "{:?}",
        out.summary
    );
}

#[test]
fn every_generated_markdown_file_opens_with_the_declared_preamble() {
    let repo = tmp("preamble");
    git_init(&repo);
    let docs = repo.join("docs");
    docsys::migrate::init_profile(&docs, "en", "project").unwrap();
    let mut dm = fs::read_to_string(docs.join(".docmeta.yml")).unwrap();
    dm.push_str("generated_preamble: \"<!-- restricted-context:public -->\"\n");
    fs::write(docs.join(".docmeta.yml"), dm).unwrap();
    // the tree existed before the key: templates and the ledger come from adopt's scaffold
    let _ = fs::remove_dir_all(docs.join("_templates"));
    let _ = fs::remove_file(docs.join("work/questions.md"));
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    for rel in [
        "ADOPTION.md",
        "docs/work/questions.md",
        "docs/_templates/feature.md",
        ".claude/commands/docsys-sync.md",
        ".claude/skills/docsys/SKILL.md",
    ] {
        let text = fs::read_to_string(repo.join(rel)).unwrap();
        let marker = "<!-- restricted-context:public -->";
        assert!(text.contains(marker), "{rel} lacks the preamble: {text}");
        assert_eq!(text.matches(marker).count(), 1, "{rel} carries it twice");
        if text.starts_with("---\n") {
            let after_fm = text.split("\n---\n").nth(1).unwrap();
            assert!(
                after_fm.starts_with(marker),
                "{rel}: preamble must follow the frontmatter"
            );
        } else {
            assert!(text.starts_with(marker), "{rel}: preamble must be first");
        }
    }
    // hooks never
    let hook = fs::read_to_string(repo.join(".claude/hooks/pre-commit-docs.sh")).unwrap();
    assert!(!hook.contains("restricted-context"));
    // regeneration keeps exactly one
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let report = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    assert_eq!(
        report.matches("<!-- restricted-context:public -->").count(),
        1,
        "{report}"
    );
}

#[test]
fn adopt_never_deletes_what_the_owner_wrote_in_the_report() {
    let repo = tmp("authored");
    git_init(&repo);
    let docs = repo.join("docs");
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let report = repo.join("ADOPTION.md");
    let mut text = fs::read_to_string(&report).unwrap();
    text.push_str(
        "\n## Closing note — 2026-08-26\n\nTwo deliberate deviations from the recipe, and why.\n",
    );
    fs::write(&report, &text).unwrap();
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let again = fs::read_to_string(&report).unwrap();
    assert!(
        again.contains("## Closing note — 2026-08-26\n\nTwo deliberate deviations"),
        "{again}"
    );
    assert_eq!(
        again.matches("# docsys adoption report").count(),
        1,
        "{again}"
    );
    assert_eq!(again.matches(docsys::adopt::REPORT_BEGIN).count(), 1);
    fs::write(&report, "# docsys adoption report\n\n## Done\n- old\n\n## Triage — 2026-08-26\n| item | decision |\n").unwrap();
    let out = docsys::adopt::run(&repo, &docs, "en").unwrap();
    let third = fs::read_to_string(&report).unwrap();
    assert!(
        third.contains("## Triage — 2026-08-26\n| item | decision |"),
        "{third}"
    );
    assert!(
        out.summary.iter().any(|l| l.contains("kept verbatim")),
        "{:?}",
        out.summary
    );
}
