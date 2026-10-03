#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// Pins under concurrent change (D-119): branches that change pinned code and
// refresh after a re-read must integrate with no conflict in any docs file,
// whether they merge, squash or rebase — and a region that two branches both
// moved must be stale after the merge, because nobody read the combination.
// Driven through the binary and plain git only, so the same file runs against
// a 0.15.1 build, where every test here fails: a refresh rewrote the page's
// `hash:` lines and `updated:`, and branches refreshed on different days
// conflicted on them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-pins-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn git_out(dir: &Path, args: &[&str]) -> (bool, String) {
    let out = Command::new("git")
        .args(["-c", "commit.gpgsign=false", "-c", "core.quotePath=false"])
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .output()
        .unwrap();
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

fn git(dir: &Path, args: &[&str]) {
    let (ok, out) = git_out(dir, args);
    assert!(ok, "git {args:?}: {out}");
}

/// A commit at a chosen day, so a page's `updated:` and its history agree.
fn commit(dir: &Path, msg: &str, day: &str) {
    git(dir, &["add", "-A"]);
    let stamp = format!("{day}T12:00:00+00:00");
    let out = Command::new("git")
        .args(["-c", "commit.gpgsign=false", "commit", "-q", "-m", msg])
        .current_dir(dir)
        .env("GIT_AUTHOR_DATE", &stamp)
        .env("GIT_COMMITTER_DATE", &stamp)
        .output()
        .unwrap();
    assert!(out.status.success(), "commit {msg}: {out:?}");
}

/// `docsys <args>` in the repository, on a pinned day; must succeed.
fn docsys(dir: &Path, day: &str, args: &[&str]) -> String {
    let out = Command::new(bin())
        .args(args)
        .current_dir(dir)
        .env("DOCSYS_TODAY", day)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "docsys {args:?}: {text}");
    text
}

fn refresh(dir: &Path, day: &str, page: &str) {
    docsys(
        dir,
        day,
        &["pin", "--refresh", page, "--repo", ".", "--root", "docs"],
    );
}

/// The `R-111` findings lint reports, as `page [region]`.
fn stale(dir: &Path) -> Vec<String> {
    let out = Command::new(bin())
        .args(["lint", "--root", "docs", "--repo", "."])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.starts_with("ERROR R-111 "))
        .map(|l| {
            let mut w = l.split_whitespace().skip(2);
            format!("{} {}", w.next().unwrap_or(""), w.next().unwrap_or(""))
        })
        .collect()
}

/// A docsys/0.5 project with the given code files and pages, committed on
/// `2026-09-01`. Each page is `(id, [(path, symbol)])`.
fn project(name: &str, files: &[(&str, &str)], pages: &[(&str, &[(&str, &str)])]) -> PathBuf {
    let repo = tmp(name);
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    let day = "2026-09-01";
    docsys(&repo, day, &["init", "--root", "docs"]);
    let dm = repo.join("docs/.docmeta.yml");
    let text = fs::read_to_string(&dm).unwrap();
    fs::write(&dm, text.replace("spec: docsys/0.4", "spec: docsys/0.5")).unwrap();
    for (path, body) in files {
        let p = repo.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }
    fs::create_dir_all(repo.join("docs/reference")).unwrap();
    let mut index = fs::read_to_string(repo.join("docs/index.md")).unwrap();
    for (id, _) in pages {
        fs::write(
            repo.join(format!("docs/reference/{id}.md")),
            format!(
                "---\nid: {id}\ntype: reference\nupdated: {day}\n---\n# {id}\n\n\
                 This page states what the pinned code does; read it before changing that code.\n"
            ),
        )
        .unwrap();
        index.push_str(&format!(
            "\n- [[reference/{id}|{id}]] -- what the code does.\n"
        ));
    }
    fs::write(repo.join("docs/index.md"), index).unwrap();
    for (id, pins) in pages {
        let page = format!("reference/{id}");
        for (path, symbol) in *pins {
            let mut args = vec!["pin", page.as_str(), path, "--repo", ".", "--root", "docs"];
            if !symbol.is_empty() {
                args.extend(["--symbol", symbol]);
            }
            docsys(&repo, day, &args);
        }
    }
    commit(&repo, "base", day);
    assert!(stale(&repo).is_empty(), "base: {:?}", stale(&repo));
    repo
}

/// A branch from `main` that edits files and refreshes pages on its own day.
fn branch(repo: &Path, name: &str, day: &str, edits: &[(&str, &str, &str)], pages: &[&str]) {
    git(repo, &["checkout", "-q", "-b", name, "main"]);
    for (path, from, to) in edits {
        let p = repo.join(path);
        let text = fs::read_to_string(&p).unwrap();
        assert!(text.contains(from), "{path}: `{from}` not found");
        fs::write(&p, text.replacen(from, to, 1)).unwrap();
    }
    for page in pages {
        refresh(repo, day, page);
    }
    commit(repo, name, day);
    git(repo, &["checkout", "-q", "main"]);
}

#[derive(Debug, Clone, Copy)]
enum Integrate {
    Merge,
    Squash,
    Rebase,
}

/// Integrate the branches into a fresh `target` branch cut from `main`, in
/// order; the paths under `docs/` that conflicted, each time one did.
fn integrate(repo: &Path, target: &str, branches: &[&str], how: Integrate) -> Vec<String> {
    git(repo, &["checkout", "-q", "-b", target, "main"]);
    let mut conflicts = Vec::new();
    for b in branches {
        let ok = match how {
            Integrate::Merge => git_out(repo, &["merge", "-q", "--no-ff", "--no-edit", b]).0,
            Integrate::Squash => {
                let ok = git_out(repo, &["merge", "-q", "--squash", b]).0;
                ok && git_out(repo, &["commit", "-q", "-m", &format!("squash {b}")]).0
            }
            Integrate::Rebase => {
                let work = format!("{b}-on-{target}");
                git(repo, &["branch", "-f", &work, b]);
                let ok = git_out(repo, &["rebase", "-q", target, &work]).0;
                ok && {
                    git(repo, &["checkout", "-q", target]);
                    git_out(repo, &["merge", "-q", "--ff-only", &work]).0
                }
            }
        };
        if !ok {
            let (_, unmerged) = git_out(repo, &["diff", "--name-only", "--diff-filter=U"]);
            conflicts.extend(
                unmerged
                    .lines()
                    .filter(|l| l.starts_with("docs/"))
                    .map(|l| format!("{b}: {l}")),
            );
            let _ = git_out(repo, &["merge", "--abort"]);
            let _ = git_out(repo, &["rebase", "--abort"]);
            let _ = git_out(repo, &["reset", "-q", "--hard"]);
            git(repo, &["checkout", "-q", target]);
        }
    }
    conflicts
}

const LIB_RS: &str = "pub fn alpha() -> u32 {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    a + b + c\n}\n\npub fn beta() -> u32 {\n    2\n}\n";

fn two_pins(name: &str) -> PathBuf {
    project(
        name,
        &[("src/lib.rs", LIB_RS)],
        &[(
            "two-pins",
            &[("src/lib.rs", "alpha"), ("src/lib.rs", "beta")] as &[(&str, &str)],
        )],
    )
}

// ---------------------------------------------------------------- test 1

fn different_pins(how: Integrate, name: &str) {
    let repo = two_pins(name);
    let base_page = fs::read_to_string(repo.join("docs/reference/two-pins.md")).unwrap();
    branch(
        &repo,
        "a",
        "2026-10-01",
        &[("src/lib.rs", "let a = 1;", "let a = 10;")],
        &["reference/two-pins"],
    );
    branch(
        &repo,
        "b",
        "2026-10-02",
        &[("src/lib.rs", "    2\n}", "    20\n}")],
        &["reference/two-pins"],
    );
    let conflicts = integrate(&repo, "m", &["a", "b"], how);
    assert!(
        conflicts.is_empty(),
        "{how:?}: docs conflicts {conflicts:?}"
    );
    assert_eq!(
        fs::read_to_string(repo.join("docs/reference/two-pins.md")).unwrap(),
        base_page,
        "{how:?}: a refresh never writes the page"
    );
    assert!(stale(&repo).is_empty(), "{how:?}: {:?}", stale(&repo));
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn different_pins_merge_clean() {
    different_pins(Integrate::Merge, "t1-merge");
}

#[test]
fn different_pins_merge_clean_squash() {
    different_pins(Integrate::Squash, "t1-squash");
}

#[test]
fn different_pins_merge_clean_rebase() {
    different_pins(Integrate::Rebase, "t1-rebase");
}

#[test]
fn identical_region_identical_file() {
    let repo = two_pins("t1b");
    for (b, day) in [("a", "2026-10-01"), ("b", "2026-10-02")] {
        branch(
            &repo,
            b,
            day,
            &[("src/lib.rs", "let a = 1;", "let a = 10;")],
            &["reference/two-pins"],
        );
    }
    let conflicts = integrate(&repo, "m", &["a", "b"], Integrate::Merge);
    assert!(conflicts.is_empty(), "docs conflicts {conflicts:?}");
    let acks: Vec<_> = fs::read_dir(repo.join("docs/.pins/two-pins"))
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(
        acks.len(),
        2,
        "one acknowledgement per region, added by both"
    );
    for e in acks {
        let name = e.file_name().into_string().unwrap();
        assert_eq!(
            fs::read_to_string(e.path()).unwrap(),
            format!("two-pins {name}\n"),
            "the content is the page id and the region hash"
        );
    }
    assert!(stale(&repo).is_empty(), "{:?}", stale(&repo));
    let _ = fs::remove_dir_all(&repo);
}

// ---------------------------------------------------------------- test 2

fn same_region(how: Integrate, name: &str) {
    let repo = two_pins(name);
    branch(
        &repo,
        "a",
        "2026-10-01",
        &[("src/lib.rs", "let a = 1;", "let a = 10;")],
        &["reference/two-pins"],
    );
    branch(
        &repo,
        "b",
        "2026-10-02",
        &[("src/lib.rs", "let c = 3;", "let c = 30;")],
        &["reference/two-pins"],
    );
    let conflicts = integrate(&repo, "m", &["a", "b"], how);
    assert!(
        conflicts.is_empty(),
        "{how:?}: docs conflicts {conflicts:?}"
    );
    let code = fs::read_to_string(repo.join("src/lib.rs")).unwrap();
    assert!(
        code.contains("let a = 10;") && code.contains("let c = 30;"),
        "{how:?}: both edits landed: {code}"
    );
    assert_eq!(
        stale(&repo),
        vec!["reference/two-pins.md [src/lib.rs#alpha]".to_string()],
        "{how:?}: nobody read the combination"
    );
    refresh(&repo, "2026-10-03", "reference/two-pins");
    assert!(stale(&repo).is_empty(), "{how:?}: {:?}", stale(&repo));
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn same_region_is_stale_after_merge() {
    same_region(Integrate::Merge, "t2-merge");
}

#[test]
fn same_region_is_stale_after_merge_squash() {
    same_region(Integrate::Squash, "t2-squash");
}

#[test]
fn same_region_is_stale_after_merge_rebase() {
    same_region(Integrate::Rebase, "t2-rebase");
}

// ---------------------------------------------------------------- test 3

fn many_rs() -> String {
    (1..=20)
        .map(|i| format!("pub fn f{i}() -> u32 {{\n    {i}\n}}\n\n"))
        .collect()
}

fn fan_in(how: Integrate, name: &str, orders: &[Vec<usize>]) {
    let source = many_rs();
    let symbols: Vec<String> = (1..=20).map(|i| format!("f{i}")).collect();
    let symbol_pins: Vec<(&str, &str)> = symbols
        .iter()
        .map(|s| ("src/many.rs", s.as_str()))
        .collect();
    let whole: [(&str, &str); 1] = [("src/many.rs", "")];
    let pages: Vec<(&str, &[(&str, &str)])> = vec![
        ("p1", &symbol_pins),
        ("p2", &symbol_pins),
        ("p3", &symbol_pins),
        ("p4", &symbol_pins),
        ("p5", &symbol_pins),
        ("w", &whole),
    ];
    let repo = project(name, &[("src/many.rs", &source)], &pages);
    let refreshed = [
        "reference/p1",
        "reference/p2",
        "reference/p3",
        "reference/p4",
        "reference/p5",
        "reference/w",
    ];
    for i in 1..=20 {
        let day = format!("2026-10-{i:02}");
        let from = format!("{{\n    {i}\n}}");
        let to = format!("{{\n    {}\n}}", i * 100);
        branch(
            &repo,
            &format!("b{i}"),
            &day,
            &[("src/many.rs", &from, &to)],
            &refreshed,
        );
    }
    for (n, order) in orders.iter().enumerate() {
        let names: Vec<String> = order.iter().map(|i| format!("b{i}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let target = format!("m{n}");
        let conflicts = integrate(&repo, &target, &refs, how);
        assert!(
            conflicts.is_empty(),
            "{how:?} order {n}: docs conflicts {conflicts:?}"
        );
        assert_eq!(
            stale(&repo),
            vec!["reference/w.md [src/many.rs]".to_string()],
            "{how:?} order {n}: only the whole-file pin reads a combination nobody read"
        );
        git(&repo, &["checkout", "-q", "main"]);
    }
    let _ = fs::remove_dir_all(&repo);
}

fn orders() -> Vec<Vec<usize>> {
    let forward: Vec<usize> = (1..=20).collect();
    let backward: Vec<usize> = (1..=20).rev().collect();
    // a fixed interleaving: odds ascending, then evens descending
    let mixed: Vec<usize> = (1..=20)
        .filter(|i| i % 2 == 1)
        .chain((1..=20).rev().filter(|i| i % 2 == 0))
        .collect();
    vec![forward, backward, mixed]
}

#[test]
fn fan_in_of_twenty() {
    fan_in(Integrate::Merge, "t3-merge", &orders());
}

#[test]
fn fan_in_of_twenty_squash() {
    fan_in(Integrate::Squash, "t3-squash", orders().get(..1).unwrap());
}

#[test]
fn fan_in_of_twenty_rebase() {
    fan_in(Integrate::Rebase, "t3-rebase", orders().get(..1).unwrap());
}
