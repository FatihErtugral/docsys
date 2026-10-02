#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// §11 freshness against a real repository: pins recomputed from the code,
// history dating every page, the gate over a commit range, and adoption
// writing the CI workflow and a gate that is hard when the tree is clean.
// A corpus tree cannot carry git state, so these live here.

use docsys::{fresh, gate, lint_in, model::Severity};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-fresh-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn git(dir: &Path, args: &[&str]) {
    git_dated(dir, args, None);
}

/// A commit at a chosen calendar day: history is what the checks read.
fn git_dated(dir: &Path, args: &[&str], date: Option<&str>) {
    let mut cmd = Command::new("git");
    cmd.args(["-c", "commit.gpgsign=false", "-c", "core.quotePath=false"])
        .args(args)
        .current_dir(dir);
    if let Some(d) = date {
        let stamp = format!("{d}T12:00:00+00:00");
        cmd.env("GIT_AUTHOR_DATE", &stamp)
            .env("GIT_COMMITTER_DATE", &stamp);
    }
    assert!(cmd.status().unwrap().success(), "git {args:?}");
}

const AUTH_RS: &str = "use std::time::Duration;\n\npub fn other(x: u32) -> u32 {\n    x + 1\n}\n\n/// doc: refresh\npub fn refresh_token(ttl: Duration) -> Duration {\n    ttl / 2\n}\n";

/// A project repository with one routed reference page and one code file,
/// committed on a chosen day.
fn repo(name: &str, day: &str) -> (PathBuf, PathBuf) {
    let repo = tmp(name);
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    let docs = repo.join("docs");
    docsys::migrate::init_profile(&docs, "en", "project").unwrap();
    fs::create_dir_all(docs.join("reference")).unwrap();
    fs::write(
        docs.join("reference/refresh.md"),
        format!(
            "---\nid: refresh\ntype: reference\nupdated: {day}\n---\n# Refresh\n\n\
             This page states how a token is refreshed; read it before changing the TTL.\n\n\
             A token is refreshed at half its TTL.\n"
        ),
    )
    .unwrap();
    let index = docs.join("index.md");
    let mut text = fs::read_to_string(&index).unwrap();
    text.push_str("\n- [[reference/refresh|Refresh]] -- when a token is refreshed.\n");
    fs::write(&index, text).unwrap();
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/auth.rs"), AUTH_RS).unwrap();
    git(&repo, &["add", "-A"]);
    git_dated(&repo, &["commit", "-q", "-m", "init"], Some(day));
    (repo, docs)
}

/// The tree declares a spec version (D-118): 0.5 rules switch on with it.
fn declare_spec(docs: &Path, spec: &str) {
    let dm = docs.join(".docmeta.yml");
    let text = fs::read_to_string(&dm).unwrap();
    assert!(text.contains("spec: docsys/0.4\n"), "{text}");
    fs::write(
        &dm,
        text.replace("spec: docsys/0.4\n", &format!("spec: {spec}\n")),
    )
    .unwrap();
}

fn errors(docs: &Path, repo: &Path) -> Vec<String> {
    let (r, _) = lint_in(docs, Some(repo));
    r.findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| format!("{} {} {}", f.rule, f.file, f.subject))
        .collect()
}

#[test]
fn a_pinned_symbol_detects_drift_in_its_region_only() {
    let today = docsys::migrate::today();
    let (repo, docs) = repo("pins", &today);
    let msg = fresh::pin(
        &docs,
        &repo,
        "reference/refresh",
        "src/auth.rs",
        Some("refresh_token"),
    )
    .unwrap();
    assert!(msg.contains("sha256:"), "{msg}");
    let page = fs::read_to_string(docs.join("reference/refresh.md")).unwrap();
    assert!(
        page.contains(
            "verifies:\n  - path: src/auth.rs\n    symbol: refresh_token\n    hash: \"sha256:"
        ),
        "{page}"
    );
    assert!(page.contains(&format!("updated: {today}")), "{page}");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "pin"]);
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );

    // the other function moves: the pinned region did not
    fs::write(repo.join("src/auth.rs"), AUTH_RS.replace("x + 1", "x + 2")).unwrap();
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );

    // the pinned function moves: stale, an error, naming page and region
    fs::write(
        repo.join("src/auth.rs"),
        AUTH_RS.replace("ttl / 2", "ttl / 3"),
    )
    .unwrap();
    let errs = errors(&docs, &repo);
    assert!(
        errs.contains(&"R-111 reference/refresh.md src/auth.rs#refresh_token".to_string()),
        "{errs:?}"
    );

    // the author re-reads the page and refreshes: current again (by id)
    let msg = fresh::refresh(&docs, &repo, "refresh").unwrap();
    assert!(msg.contains("refreshed"), "{msg}");
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );

    // the region is gone: still an error, saying so
    fs::remove_file(repo.join("src/auth.rs")).unwrap();
    let (r, _) = lint_in(&docs, Some(&repo));
    assert!(
        r.findings
            .iter()
            .any(|f| f.rule.0 == "R-111" && f.message.contains("no longer exists")),
        "{:?}",
        r.findings
    );
}

#[test]
fn an_ambiguous_symbol_or_a_malformed_hash_is_an_error_not_a_guess() {
    let today = docsys::migrate::today();
    let (repo, docs) = repo("ambiguous", &today);
    fs::write(
        repo.join("src/dup.rs"),
        "fn f() {\n    1\n}\nmod inner {\n    pub fn f() {\n        2\n    }\n}\n",
    )
    .unwrap();
    let err = fresh::pin(&docs, &repo, "reference/refresh", "src/dup.rs", Some("f")).unwrap_err();
    assert!(err.contains("ambiguous"), "{err}");
    let err = fresh::pin(
        &docs,
        &repo,
        "reference/refresh",
        "src/dup.rs",
        Some("nope"),
    )
    .unwrap_err();
    assert!(err.contains("not found"), "{err}");
    // the whole file is always resolvable
    fresh::pin(&docs, &repo, "reference/refresh", "src/dup.rs", None).unwrap();

    // a hash written by hand in the wrong form is reported under R-113
    let page = docs.join("reference/refresh.md");
    let text = fs::read_to_string(&page).unwrap();
    let start = text.find("hash: ").unwrap();
    let end = start + text[start..].find('\n').unwrap();
    let broken = format!("{}hash: \"sha256:zz\"{}", &text[..start], &text[end..]);
    fs::write(&page, broken).unwrap();
    let errs = errors(&docs, &repo);
    assert!(
        errs.iter()
            .any(|e| e.starts_with("R-113 reference/refresh.md")),
        "{errs:?}"
    );
}

/// `(from, to)`: one textual edit.
type Edit = (&'static str, &'static str);

/// One file per family: (path, symbol, source, an edit inside the declaration,
/// an edit at a use site only). Each use site is a shape D-069 took for the
/// declaration: a block-opening `if`, a template, a `{{ }}` in a Vue template.
const FAMILIES: &[(&str, &str, &str, Edit, Edit)] = &[
    (
        "src/ledger.rs",
        "settle",
        "pub fn settle(total: u64) -> u64 {\n    total / 2\n}\n\npub fn run(total: u64) -> u64 {\n    if settle(total) > 3 {\n        return 1;\n    }\n    0\n}\n",
        ("total / 2", "total / 4"),
        ("> 3", "> 5"),
    ),
    (
        "web/settle.ts",
        "settle",
        "export const settle = (x: number): boolean => x > 0;\n\nexport function run(x: number) {\n  if (ready && settle(x)) {\n    return 1;\n  }\n  return 0;\n}\n",
        ("x > 0", "x > 1"),
        ("return 1", "return 2"),
    ),
    (
        "web/Panel.vue",
        "settle",
        "<template>\n  <button @click=\"settle\">{{ label }}</button>\n</template>\n\n<script setup lang=\"ts\">\nconst label = 'settle'\nfunction settle() {\n  return label.length\n}\n</script>\n",
        ("label.length", "label.length + 1"),
        ("@click=", "@click.stop="),
    ),
    (
        "tool/settle.py",
        "settle",
        "def settle(\n    total,\n):\n    return total // 2\n\n\ndef run(total):\n    return settle(total)\n",
        ("total // 2", "total // 4"),
        ("return settle(total)", "return settle(total) + 1"),
    ),
    (
        "cmd/settle.go",
        "Settle",
        "package cmd\n\nfunc Settle(total int) int {\n\treturn total / 2\n}\n\nfunc Run(total int) int {\n\tif Settle(total) > 3 {\n\t\treturn 1\n\t}\n\treturn 0\n}\n",
        ("total / 2", "total / 4"),
        ("return 1", "return 2"),
    ),
    (
        "app/Settle.java",
        "settle",
        "public class Settle {\n    public static int settle(int total)\n    {\n        return total / 2;\n    }\n\n    public static int run(int total) {\n        if (settle(total) > 3) {\n            return 1;\n        }\n        return 0;\n    }\n}\n",
        ("total / 2", "total / 4"),
        ("return 1", "return 2"),
    ),
];

#[test]
fn a_symbol_pin_covers_its_declaration_in_every_family_and_no_use() {
    let today = docsys::migrate::today();
    let (repo, docs) = repo("families", &today);
    declare_spec(&docs, "docsys/0.5");
    for (path, _, source, _, _) in FAMILIES {
        let file = repo.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, source).unwrap();
    }
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "code"]);
    let mut wrong = Vec::new();
    for (path, symbol, _, _, _) in FAMILIES {
        if let Err(e) = fresh::pin(&docs, &repo, "reference/refresh", path, Some(symbol)) {
            wrong.push(format!("{path}#{symbol}: pin refused: {e}"));
        }
    }
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "pins"]);
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?} {wrong:?}",
        errors(&docs, &repo)
    );
    for (path, symbol, source, (decl_from, decl_to), (use_from, use_to)) in FAMILIES {
        let file = repo.join(path);
        let stale = format!("R-111 reference/refresh.md {path}#{symbol}");
        let at_use = source.replacen(use_from, use_to, 1);
        assert_ne!(&at_use, source, "{path}: the use-site edit changes nothing");
        fs::write(&file, &at_use).unwrap();
        if errors(&docs, &repo).contains(&stale) {
            wrong.push(format!(
                "{path}#{symbol}: a use site moved and the pin went stale"
            ));
        }
        let in_decl = source.replacen(decl_from, decl_to, 1);
        assert_ne!(
            &in_decl, source,
            "{path}: the declaration edit changes nothing"
        );
        fs::write(&file, &in_decl).unwrap();
        if !errors(&docs, &repo).contains(&stale) {
            wrong.push(format!(
                "{path}#{symbol}: the declaration moved and the pin stayed current"
            ));
        }
        fs::write(&file, source).unwrap();
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_0_4_tree_keeps_the_0_15_1_resolution() {
    let today = docsys::migrate::today();
    let (repo, docs) = repo("era-0-4", &today);
    // the TypeScript file: its only block-opening line naming `settle` is a use
    let (path, symbol, source, _, (use_from, use_to)) = *FAMILIES.get(1).unwrap();
    fs::create_dir_all(repo.join("web")).unwrap();
    fs::write(repo.join(path), source).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "code"]);
    fresh::pin(&docs, &repo, "reference/refresh", path, Some(symbol)).unwrap();
    let d069 = fresh::region(source, path, Some(symbol)).unwrap();
    assert!(d069.starts_with("  if (ready && settle(x)) {"), "{d069}");
    let page = fs::read_to_string(docs.join("reference/refresh.md")).unwrap();
    assert!(page.contains(&fresh::content_hash(&d069)), "{page}");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "pin"]);
    let (r, _) = lint_in(&docs, Some(&repo));
    assert!(r.findings.is_empty(), "{:?}", r.findings);
    // as 0.15.1 had it: the use site is the pinned region
    fs::write(repo.join(path), source.replacen(use_from, use_to, 1)).unwrap();
    let errs = errors(&docs, &repo);
    assert!(
        errs.contains(&format!("R-111 reference/refresh.md {path}#{symbol}")),
        "{errs:?}"
    );
}

#[test]
fn pinning_a_large_file_whole_prints_a_note_not_a_finding() {
    let today = docsys::migrate::today();
    let (repo, docs) = repo("large", &today);
    declare_spec(&docs, "docsys/0.5");
    let big: String = (0..301)
        .map(|i| format!("const A{i}: u32 = {i};\n"))
        .collect();
    fs::write(repo.join("src/big.rs"), big).unwrap();
    let msg = fresh::pin(&docs, &repo, "reference/refresh", "src/big.rs", None).unwrap();
    assert!(
        msg.contains("note:") && msg.contains("301 lines") && msg.contains("pin a symbol"),
        "{msg}"
    );
    let msg = fresh::pin(&docs, &repo, "reference/refresh", "src/auth.rs", None).unwrap();
    assert!(!msg.contains("note:"), "{msg}");
    let msg = fresh::pin(&docs, &repo, "reference/refresh", "src/big.rs", Some("A7")).unwrap();
    assert!(!msg.contains("note:"), "{msg}");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "pins"]);
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );
}

#[test]
fn updated_behind_history_and_an_untouched_draft_are_errors() {
    let (repo, docs) = repo("history", "2026-08-01");
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );

    // a hand edit that skipped the tooling: body changed, `updated` not
    let page = docs.join("reference/refresh.md");
    let text = fs::read_to_string(&page).unwrap();
    fs::write(&page, text.replace("half its TTL", "a third of its TTL")).unwrap();
    git(&repo, &["add", "-A"]);
    git_dated(
        &repo,
        &["commit", "-q", "-m", "hand edit"],
        Some("2026-08-20"),
    );
    let errs = errors(&docs, &repo);
    assert!(
        errs.contains(&"R-106 reference/refresh.md updated".to_string()),
        "{errs:?}"
    );

    // the field catches up with history: clean
    let text = fs::read_to_string(&page).unwrap();
    fs::write(
        &page,
        text.replace("updated: 2026-08-01", "updated: 2026-08-20"),
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    git_dated(&repo, &["commit", "-q", "-m", "bump"], Some("2026-08-20"));
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );

    // a draft nobody touched since 2025: undeclared abandonment
    fs::create_dir_all(docs.join("work/features")).unwrap();
    fs::write(
        docs.join("work/features/old.md"),
        "---\nid: old\nstatus: draft\nupdated: 2025-01-01\n---\n\n## Context\n\n## Decision\n\n## Contract surface\n\n## Rejected alternatives\n",
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    git_dated(
        &repo,
        &["commit", "-q", "-m", "old draft"],
        Some("2025-01-01"),
    );
    let errs = errors(&docs, &repo);
    assert!(
        errs.contains(&"R-085 work/features/old.md status".to_string()),
        "{errs:?}"
    );

    // the tree declares a longer patience: clean
    let dm = docs.join(".docmeta.yml");
    let mut text = fs::read_to_string(&dm).unwrap();
    text.push_str("stale_active_days: 100000\n");
    fs::write(&dm, text).unwrap();
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );
}

#[test]
fn the_gate_over_a_range_fails_code_without_docs() {
    let today = docsys::migrate::today();
    let (repo, docs) = repo("range", &today);
    let base = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    fs::write(repo.join("src/auth.rs"), AUTH_RS.replace("x + 1", "x + 3")).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "code only"]);
    let (g, _) = gate::run_range(&repo, &docs, &format!("{base}..HEAD")).unwrap();
    assert_eq!(g.scope, "range");
    assert_eq!(g.code, vec!["src/auth.rs".to_string()]);
    assert_eq!(g.docs, 0);

    let journal = docs.join("work/journal.md");
    let mut text = fs::read_to_string(&journal).unwrap();
    text = text.replacen(
        "# Journal\n",
        &format!("# Journal\n\n## {today} - other changed\n- `other` adds three now\n"),
        1,
    );
    fs::write(&journal, text).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "journal"]);
    let (g, _) = gate::run_range(&repo, &docs, &format!("{base}..HEAD")).unwrap();
    assert!(g.docs > 0, "docs={} code={:?}", g.docs, g.code);
}

#[test]
fn adopt_writes_the_ci_workflow_and_hardens_the_gate_once_clean() {
    let repo = tmp("adopt-ci");
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    fs::create_dir_all(repo.join(".github")).unwrap();
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "init"]);

    // a tree with debt: the gate warns, the workflow lands
    let docs = repo.join("docs");
    docsys::migrate::init_profile(&docs, "en", "project").unwrap();
    let index = docs.join("index.md");
    let mut text = fs::read_to_string(&index).unwrap();
    text.push_str("\n- [[reference/ghost|Ghost]] -- dangling on purpose.\n");
    fs::write(&index, text).unwrap();
    let done = docsys::adopt::run(&repo, &docs, "en").unwrap();
    assert!(
        done.summary
            .iter()
            .any(|s| s.contains("git pre-commit gate") && s.contains("warn-mode")),
        "{:?}",
        done.summary
    );
    assert!(
        done.summary
            .iter()
            .any(|s| s.contains("ci workflow") && s.contains("written")),
        "{:?}",
        done.summary
    );
    let wf = fs::read_to_string(repo.join(".github/workflows/docsys.yml")).unwrap();
    assert!(wf.contains("docsys lint --root docs --repo ."), "{wf}");
    assert!(
        wf.contains("--range \"origin/${{ github.base_ref }}...HEAD\""),
        "{wf}"
    );
    // D-095: the merge job verifies under each declared approver
    assert!(wf.contains("verify-on-approval:"), "{wf}");
    assert!(wf.contains("docsys verify --range"), "{wf}");
    assert!(wf.contains("--by \"@$login\" --commit"), "{wf}");
    let hook = fs::read_to_string(repo.join(".git/hooks/pre-commit")).unwrap();
    assert!(hook.contains("docsys_gate_exit=0"), "{hook}");

    // the debt is repaid: the next adopt hardens the gate in place
    let text = fs::read_to_string(&index).unwrap();
    fs::write(
        &index,
        text.replace(
            "\n- [[reference/ghost|Ghost]] -- dangling on purpose.\n",
            "",
        ),
    )
    .unwrap();
    let done = docsys::adopt::run(&repo, &docs, "en").unwrap();
    assert!(
        done.summary.iter().any(|s| s.contains("hardened")),
        "{:?}",
        done.summary
    );
    let hook = fs::read_to_string(repo.join(".git/hooks/pre-commit")).unwrap();
    assert!(hook.contains("docsys_gate_exit=1"), "{hook}");
    assert!(hook.contains("Hard gate"), "{hook}");
    assert_eq!(
        fs::read_to_string(repo.join(".github/workflows/docsys.yml")).unwrap(),
        wf,
        "the workflow is the project's file after the first write"
    );
}

// ------------------------------------------- acknowledged pins (D-119, a 0.5 tree)

fn ack_names(docs: &Path, id: &str) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(docs.join(".verifies").join(id))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn a_0_5_pin_is_an_acknowledgement_and_a_refresh_never_writes_the_page() {
    let today = docsys::migrate::today();
    let (repo, docs) = repo("ack", &today);
    declare_spec(&docs, "docsys/0.5");
    let page = docs.join("reference/refresh.md");
    fresh::pin(
        &docs,
        &repo,
        "reference/refresh",
        "src/auth.rs",
        Some("refresh_token"),
    )
    .unwrap();
    let pinned = fs::read_to_string(&page).unwrap();
    assert!(pinned.contains("    symbol: refresh_token\n"), "{pinned}");
    assert!(
        !pinned.contains("hash:"),
        "the page holds no hash: {pinned}"
    );
    let first = ack_names(&docs, "refresh");
    assert_eq!(first.len(), 1, "{first:?}");
    let name = first.first().unwrap();
    assert_eq!(
        fs::read_to_string(docs.join(".verifies/refresh").join(name)).unwrap(),
        format!("refresh {name}\n")
    );
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "pin"]);
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );

    // layout and a citation inside the region move nothing
    let auth = repo.join("src/auth.rs");
    fs::write(
        &auth,
        AUTH_RS.replace(
            "    ttl / 2\n",
            "    // doc: refresh\n        ttl   /   2 // half\n",
        ),
    )
    .unwrap();
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );

    // meaning moves it: stale until the re-read is acknowledged
    fs::write(&auth, AUTH_RS.replace("ttl / 2", "ttl / 3")).unwrap();
    assert_eq!(
        errors(&docs, &repo),
        vec!["R-111 reference/refresh.md src/auth.rs#refresh_token".to_string()]
    );
    let msg = fresh::refresh(&docs, &repo, "reference/refresh").unwrap();
    assert!(
        msg.contains("acknowledged src/auth.rs#refresh_token"),
        "{msg}"
    );
    assert!(msg.contains("1 superseded"), "{msg}");
    assert_eq!(
        fs::read_to_string(&page).unwrap(),
        pinned,
        "a refresh never writes the page"
    );
    let second = ack_names(&docs, "refresh");
    assert_eq!(
        second.len(),
        1,
        "the superseded acknowledgement left: {second:?}"
    );
    assert_ne!(second, first);
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );
    let again = fresh::refresh(&docs, &repo, "reference/refresh").unwrap();
    assert!(again.contains("all acknowledged"), "{again}");
    assert_eq!(ack_names(&docs, "refresh"), second);
}

#[test]
fn pin_gc_removes_what_no_current_region_needs() {
    let today = docsys::migrate::today();
    let (repo, docs) = repo("gc", &today);
    declare_spec(&docs, "docsys/0.5");
    fresh::pin(
        &docs,
        &repo,
        "reference/refresh",
        "src/auth.rs",
        Some("refresh_token"),
    )
    .unwrap();
    let kept = ack_names(&docs, "refresh");
    let stray = "c".repeat(64);
    docsys::ack::write(&docs, "refresh", &stray).unwrap();
    docsys::ack::write(&docs, "retired-page", &stray).unwrap();
    let st = docsys::status::status(&docs, Some(&repo)).unwrap();
    assert_eq!(
        st.orphan_acks,
        Some(1),
        "the retired page's acknowledgement"
    );
    assert!(
        docsys::status::render(&st, &docs).contains("1 acknowledgement(s) no page pins any more"),
        "{}",
        docsys::status::render(&st, &docs)
    );
    let done = fresh::gc(&docs, &repo).unwrap();
    assert_eq!(done.len(), 2, "{done:?}");
    assert!(
        done.iter().any(|l| l.contains(".verifies/retired-page/")),
        "{done:?}"
    );
    assert!(
        done.iter()
            .any(|l| l.ends_with(&format!("refresh/{stray}"))),
        "{done:?}"
    );
    assert_eq!(ack_names(&docs, "refresh"), kept);
    assert!(!docs.join(".verifies/retired-page").exists());
    assert!(
        fresh::gc(&docs, &repo).unwrap().is_empty(),
        "a second run removes nothing"
    );
}

#[test]
fn the_legacy_conversion_acknowledges_only_what_0_15_recorded_as_fresh() {
    let day = "2026-09-01";
    let (repo, docs) = repo("convert", day);
    fs::create_dir_all(repo.join("web")).unwrap();
    // the TypeScript file whose block-opening use D-069 took for `settle`
    let (ts_path, ts_symbol, ts_source, _, _) = *FAMILIES.get(1).unwrap();
    fs::write(repo.join(ts_path), ts_source).unwrap();
    fs::write(
        repo.join("web/use.ts"),
        "export function run(x: number) {\n  if (ready(x)) {\n    return 1;\n  }\n  return 0;\n}\n",
    )
    .unwrap();
    fs::write(
        repo.join("src/gone.rs"),
        "pub fn moved() -> u32 {\n    1\n}\n",
    )
    .unwrap();
    git_dated(&repo, &["add", "-A"], None);
    git_dated(&repo, &["commit", "-q", "-m", "code"], Some(day));
    // pinned as 0.15 did, in the 0.4 tree
    for (path, symbol) in [
        ("src/auth.rs", Some("refresh_token")),
        ("src/gone.rs", Some("moved")),
        (ts_path, Some(ts_symbol)),
        ("web/use.ts", Some("ready")),
        ("src/auth.rs", None),
    ] {
        fresh::pin(&docs, &repo, "reference/refresh", path, symbol).unwrap();
    }
    // one region moved after it was pinned: stale as recorded
    fs::write(
        repo.join("src/gone.rs"),
        "pub fn moved() -> u32 {\n    2\n}\n",
    )
    .unwrap();
    git_dated(&repo, &["add", "-A"], None);
    git_dated(&repo, &["commit", "-q", "-m", "pins"], Some(day));
    let page = docs.join("reference/refresh.md");
    let before = fs::read_to_string(&page).unwrap();
    assert_eq!(before.matches("    hash: ").count(), 5, "{before}");

    declare_spec(&docs, "docsys/0.5");
    let plan = fresh::convert_legacy(&docs, &repo, false).unwrap();
    assert_eq!(
        fs::read_to_string(&page).unwrap(),
        before,
        "a plan writes nothing"
    );
    assert!(!docs.join(".verifies").exists());
    let outcome = |pin: &str| {
        plan.iter()
            .find(|c| c.pin == pin)
            .map(|c| c.outcome.clone())
            .unwrap_or_else(|| panic!("{pin} missing from {plan:?}"))
    };
    use fresh::Conversion;
    assert_eq!(
        outcome("src/auth.rs#refresh_token"),
        Conversion::Acknowledged
    );
    assert_eq!(outcome("src/auth.rs"), Conversion::Acknowledged);
    assert_eq!(outcome("src/gone.rs#moved"), Conversion::StaleAsRecorded);
    assert!(
        matches!(outcome(&format!("{ts_path}#{ts_symbol}")), Conversion::Reresolved { first, .. } if first > 0),
        "{plan:?}"
    );
    assert!(
        matches!(outcome("web/use.ts#ready"), Conversion::Unresolvable(_)),
        "{plan:?}"
    );

    let done = fresh::convert_legacy(&docs, &repo, true).unwrap();
    assert_eq!(done, plan);
    let after = fs::read_to_string(&page).unwrap();
    assert!(!after.contains("hash:"), "{after}");
    assert_eq!(
        after,
        before
            .lines()
            .filter(|l| !l.starts_with("    hash: "))
            .map(|l| format!("{l}\n"))
            .collect::<String>(),
        "only the hash lines leave; `updated:` stays"
    );
    assert_eq!(
        ack_names(&docs, "refresh").len(),
        2,
        "the two proven regions"
    );
    git_dated(&repo, &["add", "-A"], None);
    git_dated(&repo, &["commit", "-q", "-m", "upgrade the pins"], None);
    let errs = errors(&docs, &repo);
    assert!(
        !errs.iter().any(|e| e.starts_with("R-106")),
        "a hash is bookkeeping: {errs:?}"
    );
    assert!(
        errs.contains(&"R-111 reference/refresh.md src/gone.rs#moved".to_string()),
        "{errs:?}"
    );
    assert!(
        errs.contains(&format!("R-111 reference/refresh.md {ts_path}#{ts_symbol}")),
        "{errs:?}"
    );
    assert!(
        errs.contains(&"R-114 reference/refresh.md web/use.ts#ready".to_string()),
        "{errs:?}"
    );
    assert!(!errs.iter().any(|e| e.contains("src/auth.rs")), "{errs:?}");
    assert!(
        fresh::convert_legacy(&docs, &repo, true)
            .unwrap()
            .is_empty(),
        "a second run finds nothing"
    );
}

#[test]
fn a_0_4_tree_keeps_hash_lines_and_never_writes_verifies() {
    let day = "2026-09-01";
    let (repo, docs) = repo("ack-control", day);
    fresh::pin(
        &docs,
        &repo,
        "reference/refresh",
        "src/auth.rs",
        Some("refresh_token"),
    )
    .unwrap();
    let page = docs.join("reference/refresh.md");
    let pinned = fs::read_to_string(&page).unwrap();
    assert!(pinned.contains("    hash: \"sha256:"), "{pinned}");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "pin"]);
    fs::write(
        repo.join("src/auth.rs"),
        AUTH_RS.replace("    ttl / 2\n", "    // a comment\n    ttl / 2\n"),
    )
    .unwrap();
    assert_eq!(
        errors(&docs, &repo),
        vec!["R-111 reference/refresh.md src/auth.rs#refresh_token".to_string()],
        "a 0.4 tree hashes the canonical text, as 0.15.1 did"
    );
    fresh::refresh(&docs, &repo, "reference/refresh").unwrap();
    assert_ne!(
        fs::read_to_string(&page).unwrap(),
        pinned,
        "the page carries the new hash"
    );
    assert!(!docs.join(".verifies").exists());
    assert!(
        errors(&docs, &repo).is_empty(),
        "{:?}",
        errors(&docs, &repo)
    );
    let st = docsys::status::status(&docs, Some(&repo)).unwrap();
    assert_eq!(st.orphan_acks, None, "a 0.4 tree keeps no acknowledgements");
    assert!(!docsys::status::render_json(&st).contains("acknowledgements_orphaned"));
}

// ------------------------------------- I3 acceptance (binary only; runs on 0.15.1)

fn cli_git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

fn cli_docsys(dir: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args(args)
        .current_dir(dir)
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

fn cli_stale(dir: &Path) -> Vec<String> {
    let out = Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args(["lint", "--root", "docs", "--repo", "."])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.starts_with("ERROR R-111 "))
        .map(|l| {
            l.split_whitespace()
                .skip(2)
                .take(2)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

/// A committed docsys/0.5 project: the code files, and pages pinning
/// `(page, path, symbol)` — an empty symbol pins the file.
fn cli_project(name: &str, files: &[(&str, &str)], pins: &[(&str, &str, &str)]) -> PathBuf {
    let repo = std::env::temp_dir().join(format!("docsys-i3-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(&repo).unwrap();
    cli_git(&repo, &["init", "-q"]);
    cli_git(&repo, &["config", "user.email", "t@example.invalid"]);
    cli_git(&repo, &["config", "user.name", "t"]);
    cli_docsys(&repo, &["init", "--root", "docs"]);
    let dm = repo.join("docs/.docmeta.yml");
    let text = fs::read_to_string(&dm).unwrap();
    fs::write(&dm, text.replace("spec: docsys/0.4", "spec: docsys/0.5")).unwrap();
    for (path, body) in files {
        let p = repo.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }
    let today = docsys::migrate::today();
    fs::create_dir_all(repo.join("docs/reference")).unwrap();
    let mut index = fs::read_to_string(repo.join("docs/index.md")).unwrap();
    let mut pages: Vec<&str> = pins.iter().map(|(p, _, _)| *p).collect();
    pages.dedup();
    for id in &pages {
        fs::write(
            repo.join(format!("docs/reference/{id}.md")),
            format!(
                "---\nid: {id}\ntype: reference\nupdated: {today}\n---\n# {id}\n\n\
                 This page states what the pinned code does; read it before changing that code.\n"
            ),
        )
        .unwrap();
        index.push_str(&format!(
            "\n- [[reference/{id}|{id}]] -- what the code does.\n"
        ));
    }
    fs::write(repo.join("docs/index.md"), index).unwrap();
    for (id, path, symbol) in pins {
        let page = format!("reference/{id}");
        let mut args = vec!["pin", page.as_str(), path, "--repo", ".", "--root", "docs"];
        if !symbol.is_empty() {
            args.extend(["--symbol", symbol]);
        }
        cli_docsys(&repo, &args);
    }
    cli_git(&repo, &["add", "-A"]);
    cli_git(&repo, &["commit", "-q", "-m", "base"]);
    assert!(cli_stale(&repo).is_empty(), "base: {:?}", cli_stale(&repo));
    repo
}

const I3_LIB_RS: &str = "pub fn alpha(x: u32) -> u32 {\n    let s = 'a';\n    x + s as u32\n}\n";
const I3_G_TS: &str =
    "export function g(x: number): string {\n  const s = 'a';\n  return s + x;\n}\n";
const I3_H_PY: &str = "def h(x):\n    s = 'a'\n    return s + str(x)\n";
const I3_C_VUE: &str = "<template>\n  <p>{{ label }}</p>\n</template>\n<script setup lang=\"ts\">\nfunction save(x: number) {\n  return x + 1;\n}\n</script>\n";

#[test]
fn layout_comments_and_citations_stale_nothing() {
    let repo = cli_project(
        "i3-layout",
        &[
            ("src/lib.rs", I3_LIB_RS),
            ("web/g.ts", I3_G_TS),
            ("py/h.py", I3_H_PY),
            ("web/c.vue", I3_C_VUE),
        ],
        &[
            ("four", "src/lib.rs", "alpha"),
            ("four", "src/lib.rs", ""),
            ("four", "web/g.ts", "g"),
            ("four", "py/h.py", "h"),
            ("four", "web/c.vue", "save"),
        ],
    );
    let edits: [(&str, [(&str, &str); 4]); 3] = [
        (
            "a reformat",
            [
                ("src/lib.rs", "pub fn alpha(\n    x: u32,\n) -> u32 {\n        let s = 'a';\n        x + s as u32\n}\n"),
                ("web/g.ts", "export function g(\n    x: number,\n): string {\n    const s = \"a\";\n    return s + x;\n}\n"),
                ("py/h.py", "def h(x):\n  s = \"a\"\n  return s + str(\n      x\n  )\n"),
                ("web/c.vue", "<template>\n  <p>{{ label }}</p>\n</template>\n<script setup lang=\"ts\">\nfunction save(\n  x: number,\n) {\n  return x + 1;\n}\n</script>\n"),
            ],
        ),
        (
            "a comment",
            [
                ("src/lib.rs", "pub fn alpha(x: u32) -> u32 {\n    // the letter\n    let s = 'a'; /* why */\n    x + s as u32\n}\n"),
                ("web/g.ts", "export function g(x: number): string {\n  // the letter\n  const s = 'a';\n  return s + x; // why\n}\n"),
                ("py/h.py", "def h(x):\n    # the letter\n    s = 'a'\n    return s + str(x)  # why\n"),
                ("web/c.vue", "<template>\n  <!-- the label -->\n  <p>{{ label }}</p>\n</template>\n<script setup lang=\"ts\">\nfunction save(x: number) {\n  // one more\n  return x + 1;\n}\n</script>\n"),
            ],
        ),
        (
            "a doc: citation",
            [
                ("src/lib.rs", "// doc: four\npub fn alpha(x: u32) -> u32 {\n    // doc: four\n    let s = 'a';\n    x + s as u32\n}\n"),
                ("web/g.ts", "// doc: four\nexport function g(x: number): string {\n  // doc: four\n  const s = 'a';\n  return s + x;\n}\n"),
                ("py/h.py", "# doc: four\ndef h(x):\n    # doc: four\n    s = 'a'\n    return s + str(x)\n"),
                ("web/c.vue", "<template>\n  <p>{{ label }}</p>\n</template>\n<script setup lang=\"ts\">\n// doc: four\nfunction save(x: number) {\n  // doc: four\n  return x + 1;\n}\n</script>\n"),
            ],
        ),
    ];
    for (what, files) in edits {
        for (path, text) in files {
            fs::write(repo.join(path), text).unwrap();
        }
        assert!(
            cli_stale(&repo).is_empty(),
            "{what}: {:?}",
            cli_stale(&repo)
        );
        let out = Command::new("git")
            .args(["status", "--porcelain", "docs/"])
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(
            out.stdout.is_empty(),
            "{what}: nothing to refresh, nothing written"
        );
        cli_git(&repo, &["checkout", "--", "."]);
    }
    // the control: meaning moves the pins
    fs::write(repo.join("web/g.ts"), I3_G_TS.replace("s + x", "s + x + 1")).unwrap();
    assert_eq!(
        cli_stale(&repo),
        vec!["reference/four.md [web/g.ts#g]".to_string()]
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_member_change_stales_only_that_member() {
    let store_ts = "export class Store {\n  a(): number {\n    return 1;\n  }\n\n  b(): number {\n    return 2;\n  }\n}\n";
    let store_rs = "pub struct Store;\n\nimpl Store {\n    pub fn a(&self) -> u32 {\n        1\n    }\n\n    pub fn b(&self) -> u32 {\n        2\n    }\n}\n";
    let repo = cli_project(
        "i3-member",
        &[("web/store.ts", store_ts), ("src/store.rs", store_rs)],
        &[
            ("x", "web/store.ts", "Store.a"),
            ("x", "src/store.rs", "Store::a"),
            ("y", "web/store.ts", "Store"),
        ],
    );
    fs::write(
        repo.join("web/store.ts"),
        store_ts.replace("return 2;", "return 20;"),
    )
    .unwrap();
    fs::write(
        repo.join("src/store.rs"),
        store_rs.replace("        2\n", "        20\n"),
    )
    .unwrap();
    assert_eq!(
        cli_stale(&repo),
        vec!["reference/y.md [web/store.ts#Store]".to_string()],
        "a claim about `a` rests on `a`; a claim about the class rests on the class"
    );
    let _ = fs::remove_dir_all(&repo);
}
