#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]
// `docsys seed plan` against a synthetic repository: inventory, target
// research, coverage refusal, and the hygiene rules (mega commit, vendored
// path, excluded store).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-seed-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn commit(repo: &Path, msg: &str) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", msg]);
}

fn write(repo: &Path, rel: &str, text: &str) {
    let p = repo.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

/// A small project with two features, one documented, plus noise.
fn build(name: &str) -> PathBuf {
    let repo = tmp(name);
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    docsys::migrate::init_profile(&repo.join("docs"), "en", "project").unwrap();
    commit(&repo, "chore: init");
    write(
        &repo,
        "apps/weather/app.toml",
        "[app]\nslug = \"weather\"\nname = \"Weather\"\n",
    );
    write(
        &repo,
        "apps/weather/WeatherApp.hpp",
        "// WeatherApp — one current reading and a strip of hours.\n// Task model: the app never performs I/O.\n// It owns a Sta lease for its lifetime.\n// Refresh is drawn from an optimistic intent.\n// The data age is read from the snapshot.\n// doc: net-feeds\nclass WeatherApp {};\n",
    );
    commit(
        &repo,
        "feat(weather): first screen\n\nbecause the generic feed viewer could not walk a series",
    );
    let mut hpp = fs::read_to_string(repo.join("apps/weather/WeatherApp.hpp")).unwrap();
    hpp.push_str("int caps_fixed;\n");
    write(&repo, "apps/weather/WeatherApp.hpp", &hpp);
    commit(&repo, "fix(weather): stale caps rejected the app on device");
    write(&repo, "apps/market/Market.cpp", "int market;\n");
    write(&repo, "apps/market/app.toml", "[app]\nslug = \"market\"\n");
    commit(&repo, "feat(market): store card");
    write(&repo, "apps/weather/vendor/lib.c", "int v;\n");
    write(&repo, "restricted/context", "secret\n");
    commit(&repo, "chore(weather): vendored lib + restricted store");
    // a documented feature: a permanent page whose id is the feature
    fs::create_dir_all(repo.join("docs/reference")).unwrap();
    write(
        &repo,
        "docs/reference/market.md",
        "---\nid: market\ntype: reference\nupdated: 2026-08-01\n---\nThis page describes the market; read it when the store changes.\n",
    );
    commit(&repo, "docs(market): reference page");
    // a mega commit: 250 files in one go
    for i in 0..250 {
        write(&repo, &format!("gen/f{i}.txt"), "x\n");
    }
    commit(&repo, "chore: generated 250 files");
    repo
}

fn plan(repo: &Path, target: Option<&str>) -> Result<String, String> {
    docsys::seed::plan(
        repo,
        &repo.join("docs"),
        &docsys::seed::Options {
            target: target.map(str::to_string),
            since: None,
            memory: None,
        },
    )
}

#[test]
fn inventory_lists_features_with_coverage_and_excludes_the_mega_commit() {
    let repo = build("inventory");
    let out = plan(&repo, None).unwrap();
    assert!(out.starts_with("# docsys seed plan"), "{out}");
    assert!(out.contains("# head: "), "{out}");
    assert!(out.contains("excluded by rule"), "{out}");
    assert!(out.contains("250 files — mega-commit"), "{out}");
    // weather: scope + manifest + directory, 2 evidence commits (1 fix), uncovered
    let w = out
        .lines()
        .find(|l| l.starts_with("# feature weather "))
        .expect("weather row");
    assert!(w.contains("directory+manifest+scope"), "{w}");
    assert!(w.contains("· 3 (1) ·"), "{w}");
    assert!(w.ends_with("uncovered"), "{w}");
    // market: covered by its reference page
    let m = out
        .lines()
        .find(|l| l.starts_with("# feature market "))
        .expect("market row");
    assert!(
        m.contains("covered by reference/market.md (page id)"),
        "{m}"
    );
    assert!(out.contains("# vocab: "), "{out}");
    assert!(
        out.contains("2 candidate feature(s), 1 covered, 1 uncovered"),
        "{out}"
    );
}

#[test]
fn a_covered_feature_is_refused_by_name() {
    let repo = build("refused");
    let err = plan(&repo, Some("market")).unwrap_err();
    assert!(
        err.contains("already covered by reference/market.md (page id)"),
        "{err}"
    );
    assert!(err.contains("nothing to seed"), "{err}");
    // spelling variants fold to the same name
    assert!(plan(&repo, Some("Market")).is_err());
}

#[test]
fn a_target_returns_its_history_files_births_comments_and_citations() {
    let repo = build("target");
    let out = plan(&repo, Some("weather")).unwrap();
    assert!(
        out.contains("# feature weather (asked as `weather`)"),
        "{out}"
    );
    assert!(out.contains("# commits 3 (fix 1, revert 0)"), "{out}");
    assert!(out.contains("found by path=3 scope=3 subject=3"), "{out}");
    assert!(out.contains("# birth "), "{out}");
    assert!(out.contains("first file added: apps/weather/"), "{out}");
    // commit bodies come along, quoted
    assert!(
        out.contains("#   | because the generic feed viewer could not walk a series"),
        "{out}"
    );
    // files by touch count; vendored and restricted paths never appear
    assert!(
        out.contains("# file apps/weather/WeatherApp.hpp · commits 2"),
        "{out}"
    );
    assert!(!out.contains("vendor/lib.c"), "{out}");
    assert!(!out.contains("restricted/"), "{out}");
    // the manifest and the code's own comment block, verbatim
    assert!(
        out.contains("# manifest apps/weather/app.toml · name weather"),
        "{out}"
    );
    assert!(
        out.contains("# cites apps/weather/WeatherApp.hpp · doc: net-feeds · dangling"),
        "{out}"
    );
    assert!(out.contains("# next: /docsys-seed"), "{out}");
    // nothing was written anywhere
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(status.stdout.is_empty(), "plan must not touch the tree");
}

#[test]
fn an_unknown_feature_asks_where_it_lives() {
    let repo = build("unknown");
    let out = plan(&repo, Some("telemetry")).unwrap();
    assert!(
        out.contains("nothing in history names this feature"),
        "{out}"
    );
}

#[test]
fn an_active_research_file_reserves_the_feature() {
    let repo = build("reserved");
    write(
        &repo,
        "docs/work/research/weather.md",
        "---\nid: weather\nstatus: active\nupdated: 2026-08-29\n---\n## Question\n## Tried\n## Learned\n## Why no decision\n",
    );
    let err = plan(&repo, Some("weather")).unwrap_err();
    assert!(err.contains("active research (reserved)"), "{err}");
}

fn lint_errors(root: &Path) -> usize {
    let (report, _) = docsys::lint(root);
    report
        .findings
        .iter()
        .filter(|f| f.severity == docsys::model::Severity::Error)
        .count()
}

#[test]
fn gaps_json_lists_features_with_coverage() {
    let repo = build("gaps");
    let json = docsys::seed::gaps_json(
        &repo,
        &repo.join("docs"),
        &docsys::seed::Options {
            target: None,
            since: None,
            memory: None,
        },
    )
    .unwrap();
    assert!(json.starts_with("{\"head\": "), "{json}");
    assert!(json.contains("\"feature\": \"weather\""), "{json}");
    assert!(json.contains("\"covered_by\": null"), "{json}");
    assert!(
        json.contains("\"covered_by\": \"reference/market.md\""),
        "{json}"
    );
    assert!(json.contains("\"how\": \"page id\""), "{json}");
}

#[test]
fn apply_lands_the_approved_rows_under_work_and_is_idempotent() {
    let repo = build("apply");
    let docs = repo.join("docs");
    let sha = String::from_utf8(
        Command::new("git")
            .args(["log", "--format=%h", "--grep=first screen"])
            .current_dir(&repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let sha = sha.trim().to_string();
    let head = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(&repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let plan = repo.join("SEED.tsv");
    fs::write(
        &plan,
        format!(
            "# docsys seed plan\n# head: {}\nresearch\tweather\t{sha}\nanswer\tweather\tthe builder\tOffline reading is a requirement:\\nthe device is used where there is no network.\njournal\t2026-08-02\t{sha}\tweather screen born\npostmortem\tcaps-stale\t{sha}\ndebt\t2026-08-29\tgeocoder attribution missing -- deferred: no OSM yet -- repay when: OSM ships\nquestion\t2026-08-29\tIs the 7-day strip a product decision?\n",
            head.trim()
        ),
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "chore: plan"]);
    let done = docsys::seed::apply(&repo, &docs, &plan, false).unwrap();
    assert!(
        done.iter().any(|d| d.contains("work/research/weather.md")),
        "{done:?}"
    );
    let research = fs::read_to_string(docs.join("work/research/weather.md")).unwrap();
    assert!(
        research.contains("id: weather\nstatus: active\n"),
        "{research}"
    );
    assert!(research.contains("seeded: true"), "{research}");
    assert!(research.contains("covers: [scope:weather]"), "{research}");
    assert!(
        research.contains(&format!("sources: [git:{sha}]")),
        "{research}"
    );
    assert!(research.contains("## Learned\n\n> Offline reading is a requirement:\n> the device is used where there is no network.\n> — the builder, "), "{research}");
    let journal = fs::read_to_string(docs.join("work/journal.md")).unwrap();
    assert!(
        journal.contains(&format!(
            "## 2026-08-02 - weather screen born\n- git: {sha}"
        )),
        "{journal}"
    );
    let heads: Vec<&str> = journal.lines().filter(|l| l.starts_with("## ")).collect();
    assert!(
        heads.len() >= 2 && heads[1].starts_with("## 2026-08-02"),
        "newest first: {heads:?}"
    );
    let pm = fs::read_to_string(docs.join("work/postmortems/caps-stale.md")).unwrap();
    assert!(
        pm.contains("## What happened\n\n> feat(weather): first screen\n"),
        "{pm}"
    );
    assert!(
        pm.contains("> because the generic feed viewer could not walk a series\n"),
        "{pm}"
    );
    assert!(pm.contains(&format!("> — git:{sha}")), "{pm}");
    let debt = fs::read_to_string(docs.join("work/debt.md")).unwrap();
    assert!(debt.contains("- [ ] 2026-08-29 geocoder attribution missing -- deferred: no OSM yet -- repay when: OSM ships"), "{debt}");
    let q = fs::read_to_string(docs.join("work/questions.md")).unwrap();
    assert!(
        q.contains("- [ ] 2026-08-29 Is the 7-day strip a product decision?"),
        "{q}"
    );
    // the seeded tree lints with no errors — seeding never makes a tree red
    assert_eq!(lint_errors(&docs), 0);
    // nothing outside work/ moved
    assert!(!docs.join("reference/weather.md").exists());
    // the feature is now reserved
    let err = plan_for(&repo, "weather").unwrap_err();
    assert!(
        err.contains("already covered by work/research/weather.md"),
        "{err}"
    );
    // idempotent: a second apply changes nothing
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "docs: seeded"]);
    let before: Vec<String> = [
        "work/research/weather.md",
        "work/journal.md",
        "work/postmortems/caps-stale.md",
        "work/debt.md",
        "work/questions.md",
    ]
    .iter()
    .map(|r| fs::read_to_string(docs.join(r)).unwrap())
    .collect();
    let again = docsys::seed::apply(&repo, &docs, &plan, false).unwrap();
    assert!(again.iter().all(|d| d.contains("already")), "{again:?}");
    let after: Vec<String> = [
        "work/research/weather.md",
        "work/journal.md",
        "work/postmortems/caps-stale.md",
        "work/debt.md",
        "work/questions.md",
    ]
    .iter()
    .map(|r| fs::read_to_string(docs.join(r)).unwrap())
    .collect();
    assert_eq!(before, after);
}

fn plan_for(repo: &Path, target: &str) -> Result<String, String> {
    plan(repo, Some(target))
}

#[test]
fn apply_refuses_a_dirty_tree_a_stale_pin_and_a_page_it_did_not_seed() {
    let repo = build("refuse");
    let docs = repo.join("docs");
    let plan_path = repo.join("SEED.tsv");
    fs::write(&plan_path, "research\tweather\t-\n").unwrap();
    // dirty: a source file changed — the untracked plan alone would be fine
    fs::write(repo.join("apps/market/Market.cpp"), "int market = 2;\n").unwrap();
    let err = docsys::seed::apply(&repo, &docs, &plan_path, false).unwrap_err();
    assert!(err.contains("dirty"), "{err}");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "chore: plan"]);
    // stale pin
    fs::write(&plan_path, "# head: 0000000\nresearch\tweather\t-\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "chore: plan 2"]);
    let err = docsys::seed::apply(&repo, &docs, &plan_path, false).unwrap_err();
    assert!(err.contains("re-plan"), "{err}");
    // somebody's research page, not seeded: never touched
    fs::create_dir_all(docs.join("work/research")).unwrap();
    fs::write(
        docs.join("work/research/weather.md"),
        "---\nid: weather\nstatus: active\nupdated: 2026-08-29\n---\nmine\n",
    )
    .unwrap();
    fs::write(&plan_path, "research\tweather\t-\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "chore: plan 3"]);
    let err = docsys::seed::apply(&repo, &docs, &plan_path, false).unwrap_err();
    assert!(err.contains("was not seeded by the tool"), "{err}");
    assert_eq!(
        fs::read_to_string(docs.join("work/research/weather.md")).unwrap(),
        "---\nid: weather\nstatus: active\nupdated: 2026-08-29\n---\nmine\n"
    );
    // an answer without its research row
    fs::write(&plan_path, "answer\tmarket\tx\ty\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "chore: plan 4"]);
    let err = docsys::seed::apply(&repo, &docs, &plan_path, false).unwrap_err();
    assert!(err.contains("`research` row must come first"), "{err}");
}

#[test]
fn git_locators_in_sources_are_checked_by_lint_as_warnings() {
    let repo = build("locators");
    let docs = repo.join("docs");
    let sha = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(&repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let sha = sha.trim().to_string();
    fs::create_dir_all(docs.join("work/research")).unwrap();
    write(
        &repo,
        "docs/work/research/probe.md",
        &format!("---\nid: probe\nstatus: active\nupdated: 2026-08-29\nsources: [git:{sha}, git:0000000, git:{sha}:apps/weather/app.toml@L1-L2, git:{sha}:nope.txt, tag:v9]\n---\nThis page probes locators; read it in the test.\n\n## Question\n\n## Tried\n\n## Learned\n\n## Why no decision\n"),
    );
    let (report, _) = docsys::lint(&docs);
    let r059: Vec<(String, String)> = report
        .findings
        .iter()
        .filter(|f| f.rule.0 == "R-059")
        .map(|f| (f.severity.tag().to_string(), f.subject.clone()))
        .collect();
    assert_eq!(r059.len(), 3, "{r059:?}");
    assert!(
        r059.iter().all(|(s, _)| s == "WARN"),
        "project profile reports, never blocks: {r059:?}"
    );
    let subjects: Vec<&str> = r059.iter().map(|(_, s)| s.as_str()).collect();
    assert!(subjects.contains(&"git:0000000"), "{subjects:?}");
    assert!(
        subjects.contains(&format!("git:{sha}:nope.txt").as_str()),
        "{subjects:?}"
    );
    assert!(subjects.contains(&"tag:v9"), "{subjects:?}");
    assert_eq!(lint_errors(&docs), 0);
}

#[test]
fn memory_notes_become_evidence_lines_without_their_bodies() {
    let repo = build("memory");
    let mem = repo
        .parent()
        .unwrap()
        .join(format!("docsys-seed-memdir-{}", std::process::id()));
    let _ = fs::remove_dir_all(&mem);
    fs::create_dir_all(&mem).unwrap();
    fs::write(mem.join("MEMORY.md"), "- index line\n").unwrap();
    fs::write(
        mem.join("weather-offline.md"),
        "---\nname: weather-offline\ndescription: offline reading was a requirement, not a fallback\nmetadata:\n  type: project\n---\n\nSECRET BODY that must never be printed.\n",
    )
    .unwrap();
    let out = docsys::seed::plan(
        &repo,
        &repo.join("docs"),
        &docsys::seed::Options {
            target: None,
            since: None,
            memory: Some(mem.clone()),
        },
    )
    .unwrap();
    assert!(out.contains("# memory weather-offline · project · offline reading was a requirement, not a fallback — ask: still true, and where should it live?"), "{out}");
    assert!(!out.contains("SECRET BODY"), "{out}");
    let _ = fs::remove_dir_all(&mem);
}
