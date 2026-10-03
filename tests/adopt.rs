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
    // an existing project's order, each step naming its command (backlog M2)
    let order = [
        "1. [ ] Collect what exists",
        "`/docsys-seed <feature>`",
        "`/docsys-interview`",
        "2. [ ] Write pages by type",
        "`docsys page new <type> <id> --unverified`",
        "3. [ ] Bind each page about code",
        "`docsys pin`",
        "4. [ ] Name the maintainers",
        "5. [ ] Start the verify flow",
        "`docsys verify <page>`",
    ];
    let mut at = 0;
    for step in order {
        let found = report.get(at..).and_then(|r| r.find(step)).map(|i| at + i);
        assert!(
            found.is_some(),
            "`{step}` missing or out of order:\n{report}"
        );
        at = found.unwrap_or(at);
    }
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
    // settings now carries the wires → nothing to add, nothing on the checklist (D-086)
    assert!(again
        .summary
        .iter()
        .any(|s| s == "settings.json: already wired"));
    let report = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    assert!(!report.contains("Merge the docsys hook wires"), "{report}");
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
    // a tree that declared no spec was read as docsys/0.4 (D-118); it is
    // stamped as the spec it was read as, and `docsys upgrade` moves it on
    assert!(meta.starts_with("spec: docsys/0.4\n"), "{meta}");
    assert_eq!(
        docsys::era::Era::at(&docs),
        docsys::era::Era::of_spec(None),
        "{meta}"
    );
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
    // the tree existed before the key: templates come from adopt's scaffold
    let _ = fs::remove_dir_all(docs.join("_templates"));
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    // an item file is generated too (D-124)
    docsys::capture::question_add(&docs, "Who owns it?", None, None, None).unwrap();
    let marker = "<!-- restricted-context:public -->";
    for rel in [
        "docs/work/questions/general.md",
        "docs/_templates/feature.md",
        ".claude/commands/docsys-sync.md",
        ".claude/commands/docsys-seed.md",
        ".claude/skills/docsys/SKILL.md",
    ] {
        let text = fs::read_to_string(repo.join(rel)).unwrap();
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
    // managed blocks carry it INSIDE, so every regeneration's diff has it
    let report = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    assert!(
        report.contains(&format!("{}\n{marker}\n", docsys::adopt::REPORT_BEGIN)),
        "{report}"
    );
    assert_eq!(report.matches(marker).count(), 1, "{report}");
    let agents = fs::read_to_string(repo.join("AGENTS.md")).unwrap();
    assert!(
        agents.contains(&format!(
            "<!-- docsys:rules:begin — generated, do not edit inside -->\n{marker}\n"
        )),
        "{agents}"
    );
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

#[test]
fn adopt_obsidian_writes_the_vault_settings_and_a_stale_work_view_once() {
    let repo = tmp("obsidian");
    git_init(&repo);
    let docs = repo.join("docs");
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let written = docsys::adopt::obsidian(&docs).unwrap();
    assert_eq!(
        written,
        vec![
            ".obsidian/app.json",
            ".obsidian/templates.json",
            "_templates/stale-work.base"
        ]
    );
    let app = fs::read_to_string(docs.join(".obsidian/app.json")).unwrap();
    assert!(
        app.contains("\"newLinkFormat\": \"absolute\"") && app.contains("_archive/"),
        "{app}"
    );
    assert!(fs::read_to_string(docs.join(".obsidian/templates.json"))
        .unwrap()
        .contains("_templates"));
    let base = fs::read_to_string(docs.join("_templates/stale-work.base")).unwrap();
    assert!(
        base.contains("status == \"active\"") && base.contains("direction: ASC"),
        "{base}"
    );
    // second run touches nothing; the tree still lints clean (dot-dirs and _templates are not pages)
    assert!(docsys::adopt::obsidian(&docs).unwrap().is_empty());
    let (report, _) = docsys::lint(&docs);
    assert_eq!(
        report
            .findings
            .iter()
            .filter(|f| f.severity == docsys::model::Severity::Error)
            .count(),
        0
    );
}

#[test]
fn adopt_merges_the_hook_wires_into_an_existing_settings_file() {
    let repo = tmp("settings-merge");
    git_init(&repo);
    let claude = repo.join(".claude");
    fs::create_dir_all(&claude).unwrap();
    fs::write(
        claude.join("settings.json"),
        "{\n  \"permissions\": { \"allow\": [\"Bash(cargo test:*)\"] }\n}\n",
    )
    .unwrap();
    let docs = repo.join("docs");
    let out = docsys::adopt::run(&repo, &docs, "en").unwrap();
    assert!(
        out.summary
            .iter()
            .any(|s| s.contains("settings.json: merged 3 docsys hook wire(s)")),
        "{:?}",
        out.summary
    );
    let text = fs::read_to_string(claude.join("settings.json")).unwrap();
    assert!(
        text.contains("Bash(cargo test:*)"),
        "permissions kept:\n{text}"
    );
    for hook in [
        "session-intent.sh",
        "pre-commit-docs.sh",
        "stop-docs-reminder.sh",
    ] {
        assert!(text.contains(hook), "{hook} missing:\n{text}");
    }
    // a docsys/0.5 tree runs no post-edit relay (D-126)
    assert!(!text.contains("post-edit-updated.sh"), "{text}");
    assert!(!claude.join("hooks/post-edit-updated.sh").exists());
    assert!(
        docsys::hook::parse_json(&text).is_some(),
        "still JSON:\n{text}"
    );
    let report = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    assert!(!report.contains("Merge the docsys hook wires"), "{report}");

    // second run: nothing to add, nothing rewritten
    let again = docsys::adopt::run(&repo, &docs, "en").unwrap();
    assert!(
        again
            .summary
            .iter()
            .any(|s| s == "settings.json: already wired"),
        "{:?}",
        again.summary
    );
    assert_eq!(
        fs::read_to_string(claude.join("settings.json")).unwrap(),
        text
    );

    // a file that is not JSON is never touched; the snippet goes to the checklist
    fs::write(
        claude.join("settings.json"),
        "// comments are not JSON\n{}\n",
    )
    .unwrap();
    let broken = docsys::adopt::run(&repo, &docs, "en").unwrap();
    assert!(
        broken.summary.iter().any(|s| s.contains("not valid JSON")),
        "{:?}",
        broken.summary
    );
    assert_eq!(
        fs::read_to_string(claude.join("settings.json")).unwrap(),
        "// comments are not JSON\n{}\n"
    );
    let report = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    assert!(report.contains("Merge the docsys hook wires"), "{report}");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn the_gate_mode_follows_lint_and_refs_together() {
    // D-088: a dangling `doc:` in code blocks a commit at the gate, so a
    // repository carrying one gets a warn-mode gate, not a hard one
    let repo = tmp("gate-refs");
    git_init(&repo);
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(
        repo.join("src/lib.rs"),
        "// doc: nowhere-yet\npub fn f() {}\n",
    )
    .unwrap();
    let docs = repo.join("docs");
    let out = docsys::adopt::run(&repo, &docs, "en").unwrap();
    let gate_line = out
        .summary
        .iter()
        .find(|s| s.starts_with("git pre-commit gate:"))
        .cloned()
        .unwrap_or_default();
    assert!(
        gate_line.contains("warn"),
        "a dangling doc: must leave the gate in warn mode: {gate_line} / {:?}",
        out.summary
    );
    fs::write(repo.join("src/lib.rs"), "pub fn f() {}\n").unwrap();
    let again = docsys::adopt::run(&repo, &docs, "en").unwrap();
    let gate_line = again
        .summary
        .iter()
        .find(|s| s.starts_with("git pre-commit gate:"))
        .cloned()
        .unwrap_or_default();
    assert!(
        gate_line.contains("hard"),
        "clean lint and refs → hard gate: {gate_line} / {:?}",
        again.summary
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn the_seed_command_names_the_absent_builder_case() {
    // finding 29: a repository whose people are gone must still be seedable
    let repo = tmp("seed-text");
    git_init(&repo);
    docsys::adopt::run(&repo, &repo.join("docs"), "en").unwrap();
    let text = fs::read_to_string(repo.join(".claude/commands/docsys-seed.md")).unwrap();
    for needle in [
        "rows wait for a builder",
        "a `question` row that names the",
        "never committed",
    ] {
        assert!(text.contains(needle), "docsys-seed.md lacks `{needle}`");
    }
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_re_run_keeps_the_adoption_record_and_reports_itself_as_the_last_run() {
    // D-097: "created" must not become "kept" in the only file that says what adoption did
    let repo = tmp("record");
    git_init(&repo);
    let docs = repo.join("docs");
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let first = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    assert!(first.contains("## Done — adoption ("), "{first}");
    assert!(
        first.contains("settings.json: created with docsys hook wires"),
        "{first}"
    );
    assert!(!first.contains("## Last run"), "{first}");
    // a re-run that changes nothing leaves the report as it was (docsys/0.5)
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    assert_eq!(fs::read_to_string(repo.join("ADOPTION.md")).unwrap(), first);
    // a re-run that writes something reports itself
    let asset = repo.join(".claude/commands/docsys-sync.md");
    fs::remove_file(&asset).unwrap();
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let second = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    assert!(
        second.contains("settings.json: created with docsys hook wires"),
        "the adoption record survives: {second}"
    );
    assert!(second.contains("## Last run — "), "{second}");
    assert!(second.contains("settings.json: already wired"), "{second}");
    assert_eq!(second.matches("## Done — adoption").count(), 1, "{second}");
    assert_eq!(second.matches("## Last run").count(), 1, "{second}");
    // a third run replaces the last-run block, not the adoption one
    fs::remove_file(&asset).unwrap();
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let third = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    assert_eq!(third.matches("## Last run").count(), 1, "{third}");
    assert!(
        third.contains("settings.json: created with docsys hook wires"),
        "{third}"
    );
    // a report written before D-097 (a plain `## Done` inside the managed block) is taken as the record
    let start = third.find("## Done — adoption (").unwrap();
    let end = start + third[start..].find(')').unwrap() + 1;
    let legacy = format!("{}## Done{}", &third[..start], &third[end..]);
    fs::write(repo.join("ADOPTION.md"), legacy).unwrap();
    fs::remove_file(&asset).unwrap();
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let fourth = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    assert_eq!(fourth.matches("## Done — adoption").count(), 1, "{fourth}");
    assert!(
        fourth.contains("settings.json: created with docsys hook wires"),
        "{fourth}"
    );
    assert_eq!(fourth.matches("## Last run").count(), 1, "{fourth}");
    let _ = fs::remove_dir_all(&repo);
}

fn git(dir: &Path, args: &[&str]) {
    assert!(Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap()
        .success());
}

/// The binary, run from the repository's top level — the flags live in main.rs.
fn docsys(repo: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap()
}

const RULES_BEGIN: &str = "docsys:rules:begin";

#[test]
fn adopt_on_pages_without_docmeta_names_three_ways_on() {
    // D-110: classification is judgment (R-003) — adopt stops and says how on
    let repo = tmp("three-ways");
    git_init(&repo);
    fs::create_dir_all(repo.join("docs/guides")).unwrap();
    let index = "# Our docs\n\nThe owner's own index.\n";
    fs::write(repo.join("docs/index.md"), index).unwrap();
    fs::write(repo.join("docs/guides/setup.md"), "# Setup\n").unwrap();
    let out = docsys(&repo, &["adopt"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(
        err.contains("`docs` holds 2 page(s) and no .docmeta.yml"),
        "{err}"
    );
    for way in [
        "docsys migrate inventory --root docs > plan.tsv",
        "docsys migrate apply --plan plan.tsv --root docs",
        "docsys init --root docs",
        "D-016",
        "docsys adopt --root <dir>",
    ] {
        assert!(err.contains(way), "`{way}` missing:\n{err}");
    }
    assert!(!repo.join("docs/.docmeta.yml").exists());
    // the second way keeps every page where it is: init writes only what is absent
    assert!(docsys(&repo, &["init", "--root", "docs"]).status.success());
    assert_eq!(
        fs::read_to_string(repo.join("docs/index.md")).unwrap(),
        index
    );
    let out = docsys(&repo, &["adopt"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn the_rules_block_goes_to_claude_md_when_git_ignores_agents_md() {
    let repo = tmp("rules-ignored");
    git_init(&repo);
    fs::write(repo.join(".gitignore"), "AGENTS.md\n").unwrap();
    let docs = repo.join("docs");
    let out = docsys::adopt::run(&repo, &docs, "en").unwrap();
    assert!(!repo.join("AGENTS.md").exists());
    let claude = fs::read_to_string(repo.join("CLAUDE.md")).unwrap();
    assert_eq!(claude.matches(RULES_BEGIN).count(), 1, "{claude}");
    assert!(
        out.summary
            .iter()
            .any(|s| s.starts_with("CLAUDE.md: managed block written")),
        "{:?}",
        out.summary
    );
    // a re-adopt updates it where it is
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    assert!(!repo.join("AGENTS.md").exists());
    let claude = fs::read_to_string(repo.join("CLAUDE.md")).unwrap();
    assert_eq!(claude.matches(RULES_BEGIN).count(), 1, "{claude}");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn the_rules_block_stays_where_its_markers_are() {
    let repo = tmp("rules-stay");
    git_init(&repo);
    let owner = "# Agents\n\nThe owner's rules.\n";
    fs::write(repo.join("AGENTS.md"), owner).unwrap();
    fs::write(repo.join("CLAUDE.md"), "# Claude\n").unwrap();
    docsys::rules::write_agents_block(&repo.join("CLAUDE.md")).unwrap();
    git(&repo, &["add", "AGENTS.md", "CLAUDE.md"]);
    docsys::adopt::run(&repo, &repo.join("docs"), "en").unwrap();
    assert_eq!(fs::read_to_string(repo.join("AGENTS.md")).unwrap(), owner);
    let claude = fs::read_to_string(repo.join("CLAUDE.md")).unwrap();
    assert!(claude.starts_with("# Claude\n"), "{claude}");
    assert_eq!(claude.matches(RULES_BEGIN).count(), 1, "{claude}");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn rules_file_names_the_file_and_a_re_adopt_finds_it_there() {
    let repo = tmp("rules-file");
    git_init(&repo);
    let out = docsys(&repo, &["adopt", "--rules-file", "docs/AGENTS.md"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let block = |rel: &str| {
        fs::read_to_string(repo.join(rel))
            .unwrap_or_default()
            .matches(RULES_BEGIN)
            .count()
    };
    assert_eq!(block("docs/AGENTS.md"), 1);
    assert!(!repo.join("AGENTS.md").exists());
    let out = docsys(&repo, &["adopt"]);
    assert!(out.status.success());
    assert_eq!(block("docs/AGENTS.md"), 1);
    assert!(!repo.join("AGENTS.md").exists());
    assert!(!repo.join("CLAUDE.md").exists());
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn with_both_rules_files_ignored_the_block_is_printed_and_the_checklist_names_it() {
    let repo = tmp("rules-nowhere");
    git_init(&repo);
    fs::write(repo.join(".gitignore"), "AGENTS.md\nCLAUDE.md\n").unwrap();
    let out = docsys(&repo, &["adopt"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.matches(RULES_BEGIN).count(), 1, "{stdout}");
    assert!(!repo.join("AGENTS.md").exists());
    assert!(!repo.join("CLAUDE.md").exists());
    let report = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    assert!(
        report.contains("- [ ] Git ignores both `AGENTS.md` and `CLAUDE.md`"),
        "{report}"
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn report_dir_places_the_report_and_a_re_adopt_finds_it_there() {
    let repo = tmp("report-dir");
    git_init(&repo);
    let out = docsys(&repo, &["adopt", "--report-dir", ".github"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains(".github/ADOPTION.md"));
    assert!(!repo.join("ADOPTION.md").exists());
    let first = fs::read_to_string(repo.join(".github/ADOPTION.md")).unwrap();
    assert!(first.contains("## Done — adoption ("), "{first}");
    let out = docsys(&repo, &["adopt"]);
    assert!(out.status.success());
    assert!(!repo.join("ADOPTION.md").exists());
    let again = fs::read_to_string(repo.join(".github/ADOPTION.md")).unwrap();
    assert!(again.contains("## Last run — "), "{again}");
    assert_eq!(again.matches(docsys::adopt::REPORT_BEGIN).count(), 1);
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn no_report_prints_the_report_and_writes_nothing() {
    let repo = tmp("no-report");
    git_init(&repo);
    let out = docsys(&repo, &["adopt", "--no-report"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("# docsys adoption report"), "{stdout}");
    assert!(stdout.contains("Judgment checklist"), "{stdout}");
    assert!(!repo.join("ADOPTION.md").exists());
    let listed = Command::new("git")
        .args(["ls-files", "-o", "--", "*ADOPTION.md"])
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(listed.stdout.is_empty(), "{listed:?}");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn an_ignored_report_is_updated_where_it_is() {
    // a report kept out of git through .gitignore — at the root, or moved by hand
    let repo = tmp("report-ignored");
    git_init(&repo);
    fs::write(repo.join(".gitignore"), "ADOPTION.md\n.notes/\n").unwrap();
    let docs = repo.join("docs");
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let at_root = repo.join("ADOPTION.md");
    let asset = repo.join(".claude/commands/docsys-sync.md");
    fs::remove_file(&asset).unwrap();
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let text = fs::read_to_string(&at_root).unwrap();
    assert!(text.contains("## Last run — "), "{text}");
    fs::create_dir_all(repo.join(".notes")).unwrap();
    fs::rename(&at_root, repo.join(".notes/ADOPTION.md")).unwrap();
    fs::remove_file(&asset).unwrap();
    let out = docsys::adopt::run(&repo, &docs, "en").unwrap();
    assert!(!at_root.exists(), "the report was written back to the root");
    let moved = fs::read_to_string(repo.join(".notes/ADOPTION.md")).unwrap();
    assert_eq!(moved.matches("## Done — adoption").count(), 1, "{moved}");
    assert_eq!(moved.matches("## Last run").count(), 1, "{moved}");
    assert!(
        out.report_path.ends_with(".notes/ADOPTION.md"),
        "{}",
        out.report_path
    );
    let _ = fs::remove_dir_all(&repo);
}

/// D-123: the index `adopt` writes routes the four type directories, so a new
/// page is reachable without a line of its own, and `page new` stops asking
/// for one.
#[test]
fn a_new_page_under_a_routed_directory_needs_no_index_line() {
    let repo = tmp("routes");
    git_init(&repo);
    let docs = repo.join("docs");
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let index = fs::read_to_string(docs.join("index.md")).unwrap();
    for dir in ["reference", "howto", "explanation", "tutorial"] {
        assert!(index.contains(&format!("- [[{dir}/|")), "{index}");
    }
    let made = docsys::capture::page_new(&docs, "reference", "limits", None, false).unwrap();
    assert!(made.contains("reference/limits.md"), "{made}");
    let page = docs.join("reference/limits.md");
    let text = fs::read_to_string(&page).unwrap();
    assert!(!text.contains("route it from index.md"), "{text}");
    let text = text.split("<!-- opening").next().unwrap().to_string();
    fs::write(
        &page,
        format!("{text}This page states the limits; read it before raising one.\n"),
    )
    .unwrap();
    let (r, _) = docsys::lint_in(&docs, Some(&repo));
    let orphans: Vec<_> = r.findings.iter().filter(|f| f.rule.0 == "R-034").collect();
    assert!(orphans.is_empty(), "{orphans:?}");
    assert_eq!(fs::read_to_string(docs.join("index.md")).unwrap(), index);
    let _ = fs::remove_dir_all(&repo);
}

fn adopt_run(repo: &Path, args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args(args)
        .current_dir(repo)
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr),
    )
}

/// On a docsys/0.5 tree adopt says only what is so: the inventory lists the
/// team's own agent files, never docsys's; a clean adoption carries no
/// checklist item about errors it does not have; a re-adopt that changes
/// nothing says "kept", leaves ADOPTION.md and every byte outside the rules
/// block as they were; `--no-report` prints the status once, and a
/// `--report-dir` it would ignore is refused.
#[test]
fn adoption_on_a_0_5_tree_says_only_what_is_so() {
    let repo = tmp("adopt-says");
    git_init(&repo);
    fs::create_dir_all(repo.join(".claude/commands")).unwrap();
    fs::write(
        repo.join(".claude/commands/team-docs.md"),
        "---\nallowed-tools: Bash(mkdocs build)\n---\nBuild the docs site.\n",
    )
    .unwrap();
    let (code, out) = adopt_run(&repo, &["adopt"]);
    assert_eq!(code, 0, "{out}");
    let report = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    let inventory = report
        .split("## Existing agent layer")
        .nth(1)
        .and_then(|s| s.split("\n## ").next())
        .unwrap_or_default();
    assert!(inventory.contains("team-docs.md"), "{inventory}");
    assert!(
        !inventory.contains("docsys-") && !inventory.contains("session-intent"),
        "{inventory}"
    );
    assert!(
        !report.contains("Triage the error findings"),
        "a clean adoption: {report}"
    );
    assert!(!report.contains("When errors reach zero"), "{report}");
    // the owner's text right after the block, a blank line between
    let holder = ["AGENTS.md", "CLAUDE.md"]
        .iter()
        .map(|f| repo.join(f))
        .find(|p| fs::read_to_string(p).is_ok_and(|t| t.contains("docsys:rules:end")))
        .unwrap();
    let text = fs::read_to_string(&holder).unwrap();
    fs::write(&holder, format!("{text}\nOwner text after.\n")).unwrap();
    let held = fs::read_to_string(&holder).unwrap();
    let report = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    let (code, out) = adopt_run(&repo, &["adopt"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("managed block kept"), "{out}");
    assert_eq!(
        fs::read_to_string(&holder).unwrap(),
        held,
        "outside the block"
    );
    assert_eq!(
        fs::read_to_string(repo.join("ADOPTION.md")).unwrap(),
        report,
        "a re-run that changed nothing"
    );
    let (code, out) = adopt_run(&repo, &["adopt", "--no-report"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(out.matches("agent assets:").count(), 1, "{out}");
    let (code, out) = adopt_run(&repo, &["adopt", "--no-report", "--report-dir", "x"]);
    assert_eq!(code, 2, "{out}");
    let _ = fs::remove_dir_all(&repo);
}

/// A tree 0.15.1 adopted, committed: `corpus/adopt-0.4/before`, made by
/// docsys 0.15.1, in a repository named as it was, `project`.
fn tree_04(name: &str) -> (PathBuf, Vec<String>) {
    let case = Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/adopt-0.4");
    let repo = tmp(name).join("project");
    fs::create_dir_all(&repo).unwrap();
    fn files(dir: &Path, base: &Path, out: &mut Vec<String>) {
        for e in fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                files(&p, base, out);
            } else {
                out.push(p.strip_prefix(base).unwrap().to_string_lossy().into_owned());
            }
        }
    }
    let mut before = Vec::new();
    files(&case.join("before"), &case.join("before"), &mut before);
    git_init(&repo);
    for rel in &before {
        let to = real_04(&repo, rel);
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::copy(case.join("before").join(rel), &to).unwrap();
    }
    let who = [
        "-c",
        "user.email=t@example.invalid",
        "-c",
        "user.name=t",
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.hooksPath=/dev/null",
    ];
    git(&repo, &[&who[..], &["add", "-A"]].concat());
    git(
        &repo,
        &[&who[..], &["commit", "-qm", "adopted by 0.15.1"]].concat(),
    );
    (repo, before)
}

/// Where a stored name of `corpus/adopt-0.4` lives in the repository.
fn real_04(repo: &Path, rel: &str) -> PathBuf {
    match rel.strip_prefix("git-hooks/") {
        Some(hook) => repo.join(".git/hooks").join(hook),
        None => repo.join(rel.replacen("dot-claude/", ".claude/", 1)),
    }
}

/// D-118: a docsys/0.4 tree is adopted again as 0.15.1 adopts it again — every
/// file and the git gate, byte for byte. `corpus/adopt-0.4` was made by
/// docsys 0.15.1: `before/` after its adopt, `after/` the files its second
/// adopt changed.
#[test]
fn adopt_on_a_0_4_tree_writes_what_0_15_1_writes() {
    let case = Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/adopt-0.4");
    let (repo, before) = tree_04("v04");
    let out = docsys(&repo, &["adopt"]);
    assert!(out.status.success(), "{out:?}");
    for rel in &before {
        let changed = case.join("after").join(rel);
        let want = fs::read(if changed.is_file() {
            changed
        } else {
            case.join("before").join(rel)
        })
        .unwrap();
        let got = fs::read(real_04(&repo, rel)).unwrap();
        assert!(
            got == want,
            "{rel} differs from what 0.15.1 writes:\n{}",
            String::from_utf8_lossy(&got)
        );
    }
    let status = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(&repo)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&status.stdout),
        " M ADOPTION.md\n",
        "nothing 0.15.1 does not write"
    );
    // and the block `rules --agents-md` prints there is the same one
    let printed = docsys(&repo, &["rules", "--agents-md"]);
    let agents = fs::read_to_string(case.join("before/AGENTS.md")).unwrap();
    assert!(
        agents.contains(&*String::from_utf8_lossy(&printed.stdout)),
        "{printed:?}"
    );
    let _ = fs::remove_dir_all(repo.parent().unwrap());
}

/// A docsys/0.4 tree that lost an asset is not given this docsys's text of it,
/// which would teach it 0.5: `adopt` and `agents` name it and leave it to the
/// upgrade (D-118).
#[test]
fn a_0_4_tree_missing_an_asset_is_told_the_upgrade_writes_it() {
    let (repo, _) = tree_04("v04-missing");
    let relay = repo.join(".claude/hooks/stop-docs-reminder.sh");
    fs::remove_file(&relay).unwrap();
    for args in [&["adopt"][..], &["agents"], &["agents", "--force"]] {
        let out = docsys(&repo, args);
        assert!(out.status.success(), "{args:?}: {out:?}");
        let said = String::from_utf8_lossy(&out.stdout);
        assert!(
            said.contains("this docsys/0.4 tree is missing .claude/hooks/stop-docs-reminder.sh; `docsys upgrade` moves the tree to 0.5 and writes it"),
            "{args:?}: {said}"
        );
        assert!(!relay.exists(), "{args:?}");
    }
    let _ = fs::remove_dir_all(repo.parent().unwrap());
}
