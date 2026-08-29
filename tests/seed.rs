#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
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
