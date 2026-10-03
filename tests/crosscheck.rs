#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! `docsys crosscheck`: what a cross-check reads — each page, its `sources:`
//! and the code its pins resolve to now, by the resolver lint uses — for the
//! pages named or changed since a revision; it writes nothing. And
//! `/docsys-crosscheck`, the agent's procedure, installed with the agent layer
//! of a docsys/0.5 tree in either profile. A corpus tree cannot carry git
//! state, so these live here.

use docsys::hook::{parse_json, Json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-crosscheck-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Runs `docsys` in `dir`: the exit code, standard output and standard error.
fn docsys(dir: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args(args)
        .current_dir(dir)
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .output()
        .unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

const AUTH_RS: &str = "use std::time::Duration;\n\npub fn other(x: u32) -> u32 {\n    x + 1\n}\n\n/// The token is refreshed at half its lifetime.\npub fn refresh_token(ttl: Duration) -> Duration {\n    ttl / 2\n}\n";

const LIMITS_RS: &str =
    "pub fn max_retries() -> u32 {\n    3\n}\n\npub fn min_wait() -> u32 {\n    1\n}\n";

fn page(id: &str, sources: &str, body: &str) -> String {
    format!(
        "---\nid: {id}\ntype: reference\nsources: [{sources}]\n---\n# {id}\n\n\
         This page states {id}; read it before changing it.\n\n{body}\n"
    )
}

/// A docsys/0.5 project: a page with `sources:`, a symbol pin, a file pin and
/// a second symbol pin; two pages with neither; everything committed.
fn project(name: &str) -> (PathBuf, PathBuf) {
    let repo = tmp(name);
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    let docs = repo.join("docs");
    docsys::migrate::init_profile(&docs, "en", "project").unwrap();
    write(&repo, "src/auth.rs", AUTH_RS);
    write(&repo, "src/limits.rs", LIMITS_RS);
    write(
        &docs,
        "reference/refresh.md",
        &page(
            "refresh",
            "src/auth.rs, reference/limits",
            "A token is refreshed at half its TTL.",
        ),
    );
    write(
        &docs,
        "reference/limits.md",
        &page("limits", "", "A call is retried three times."),
    );
    write(
        &docs,
        "explanation/why.md",
        &page("why", "", "Retries stay few on purpose."),
    );
    for args in [
        &[
            "pin",
            "reference/refresh",
            "src/auth.rs",
            "--symbol",
            "refresh_token",
        ][..],
        &["pin", "reference/refresh", "src/limits.rs"],
        &[
            "pin",
            "reference/refresh",
            "src/limits.rs",
            "--symbol",
            "min_wait",
        ],
    ] {
        let (code, _, err) = docsys(&repo, args);
        assert_eq!(code, Some(0), "{args:?}: {err}");
    }
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "init"]);
    (repo, docs)
}

fn field<'a>(j: &'a Json, key: &str) -> &'a Json {
    match j {
        Json::Obj(fields) => {
            &fields
                .iter()
                .find(|(k, _)| k == key)
                .unwrap_or_else(|| panic!("no `{key}` in {j:?}"))
                .1
        }
        _ => panic!("not an object: {j:?}"),
    }
}

fn items(j: &Json) -> &[Json] {
    match j {
        Json::Arr(items) => items,
        _ => panic!("not an array: {j:?}"),
    }
}

fn text(j: &Json) -> &str {
    match j {
        Json::Str(s) => s,
        _ => panic!("not a string: {j:?}"),
    }
}

fn num(j: &Json) -> usize {
    match j {
        Json::Num(n) => n.parse().unwrap(),
        _ => panic!("not a number: {j:?}"),
    }
}

/// The pages a JSON answer lists, in its order.
fn pages_of(out: &str) -> Vec<String> {
    let j = parse_json(out).unwrap_or_else(|| panic!("not JSON: {out}"));
    items(field(&j, "pages"))
        .iter()
        .map(|p| text(field(p, "page")).to_string())
        .collect()
}

/// A page is read with its `sources:` and the lines each pin resolves to
/// now, by the 0.5 resolver lint uses — a symbol's declaration, a whole
/// file, or the resolver's own refusal — and nothing is written.
#[test]
fn a_page_shows_its_sources_and_the_code_its_pins_resolve_to_now() {
    let (repo, _) = project("show");
    let (first, last) = docsys::symbols::resolve(AUTH_RS, "src/auth.rs", "refresh_token").unwrap();
    assert!(first <= 8 && last >= 10, "{first}-{last}");
    // the code under one pin moves in the working tree: its symbol is gone
    let moved = LIMITS_RS.replace("min_wait", "min_delay");
    write(&repo, "src/limits.rs", &moved);
    let refusal = docsys::symbols::resolve(&moved, "src/limits.rs", "min_wait").unwrap_err();
    let before = git(&repo, &["status", "--porcelain", "--untracked-files=all"]);
    let (code, out, err) = docsys(&repo, &["crosscheck", "reference/refresh"]);
    assert_eq!(code, Some(0), "{err}");
    let want = format!(
        "docs/reference/refresh.md (id: refresh)\n\
         \x20 source: src/auth.rs\n\
         \x20 source: reference/limits\n\
         \x20 pin: src/auth.rs#refresh_token -> src/auth.rs:L{first}-L{last}\n\
         \x20 pin: src/limits.rs -> src/limits.rs:L1-L{} (the whole file)\n\
         \x20 pin: src/limits.rs#min_wait -> cannot resolve: {refusal}\n",
        moved.lines().count()
    );
    assert!(out.starts_with(&want), "{out}");
    assert!(out.ends_with("-- 1 page(s); nothing written\n"), "{out}");
    // the id, the path with `.md` and the path from the repository's top
    // name the same page
    for named in [
        "refresh",
        "reference/refresh.md",
        "docs/reference/refresh.md",
    ] {
        let (code, again, err) = docsys(&repo, &["crosscheck", named]);
        assert_eq!(code, Some(0), "{named}: {err}");
        assert_eq!(again, out, "{named}");
    }
    // a page with neither says so
    let (code, out, _) = docsys(&repo, &["crosscheck", "limits"]);
    assert_eq!(code, Some(0));
    assert!(
        out.starts_with("docs/reference/limits.md (id: limits)\n  no `sources:`\n  no pins\n"),
        "{out}"
    );
    // the same as data
    let (code, out, err) = docsys(&repo, &["crosscheck", "reference/refresh", "--json"]);
    assert_eq!(code, Some(0), "{err}");
    let j = parse_json(&out).unwrap_or_else(|| panic!("{out}"));
    let [p] = items(field(&j, "pages")) else {
        panic!("one page: {out}")
    };
    assert_eq!(text(field(p, "page")), "docs/reference/refresh.md");
    assert_eq!(text(field(p, "id")), "refresh");
    let sources: Vec<&str> = items(field(p, "sources")).iter().map(text).collect();
    assert_eq!(sources, ["src/auth.rs", "reference/limits"]);
    let [symbol, whole, refused] = items(field(p, "pins")) else {
        panic!("three pins: {out}")
    };
    assert_eq!(text(field(symbol, "pin")), "src/auth.rs#refresh_token");
    assert_eq!(text(field(symbol, "path")), "src/auth.rs");
    assert_eq!(text(field(symbol, "symbol")), "refresh_token");
    assert_eq!(
        (num(field(symbol, "first")), num(field(symbol, "last"))),
        (first, last)
    );
    assert_eq!(field(whole, "symbol"), &Json::Null);
    assert_eq!(
        (num(field(whole, "first")), num(field(whole, "last"))),
        (1, moved.lines().count())
    );
    assert_eq!(text(field(refused, "refusal")), refusal);
    // it wrote nothing
    assert_eq!(
        git(&repo, &["status", "--porcelain", "--untracked-files=all"]),
        before
    );
    let _ = fs::remove_dir_all(&repo);
}

/// `--since <ref>` reads every permanent page whose file changed between the
/// revision and the working tree — committed, staged, edited or new — and no
/// other; named pages join them, each once, in one order.
#[test]
fn since_picks_exactly_the_pages_changed_since_the_revision() {
    let (repo, docs) = project("since");
    let base = git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    // committed after the base
    write(
        &docs,
        "reference/limits.md",
        &page("limits", "", "A call is retried four times."),
    );
    git(&repo, &["commit", "-q", "-am", "limits"]);
    // staged, edited and new, none committed; a router and code change too
    write(
        &docs,
        "explanation/why.md",
        &page("why", "", "Retries stay few, on purpose."),
    );
    git(&repo, &["add", "docs/explanation/why.md"]);
    write(
        &docs,
        "reference/fresh.md",
        &page("fresh", "", "A new page, not yet added."),
    );
    let index = fs::read_to_string(docs.join("index.md")).unwrap();
    write(&docs, "index.md", &format!("{index}\nMore.\n"));
    write(&repo, "src/auth.rs", &format!("{AUTH_RS}// end\n"));
    let (code, out, err) = docsys(&repo, &["crosscheck", "--since", &base, "--json"]);
    assert_eq!(code, Some(0), "{err}");
    assert_eq!(
        pages_of(&out),
        [
            "docs/explanation/why.md",
            "docs/reference/fresh.md",
            "docs/reference/limits.md"
        ]
    );
    // since the last commit: only what is not committed
    let (code, out, err) = docsys(&repo, &["crosscheck", "--since", "HEAD", "--json"]);
    assert_eq!(code, Some(0), "{err}");
    assert_eq!(
        pages_of(&out),
        ["docs/explanation/why.md", "docs/reference/fresh.md"]
    );
    // a page named joins them; one named twice, or changed too, is read once
    let (code, out, err) = docsys(
        &repo,
        &[
            "crosscheck",
            "refresh",
            "explanation/why",
            "--since",
            "HEAD",
            "--json",
        ],
    );
    assert_eq!(code, Some(0), "{err}");
    assert_eq!(
        pages_of(&out),
        [
            "docs/explanation/why.md",
            "docs/reference/fresh.md",
            "docs/reference/refresh.md"
        ]
    );
    // from inside the tree, the same answer
    let (code, again, err) = docsys(&docs, &["crosscheck", "--since", "HEAD", "--json"]);
    assert_eq!(code, Some(0), "{err}");
    assert_eq!(
        pages_of(&again),
        pages_of(&docsys(&repo, &["crosscheck", "--since", "HEAD", "--json"]).1)
    );
    // nothing changed since the revision: said, and no page
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "rest"]);
    let (code, out, err) = docsys(&repo, &["crosscheck", "--since", "HEAD"]);
    assert_eq!(code, Some(0), "{err}");
    assert_eq!(
        out,
        "no permanent page changed since `HEAD`\n-- 0 page(s); nothing written\n"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// A bad invocation exits 2, prints nothing on standard output and names
/// what is wrong, its command once: no page and no revision, a page the tree
/// does not hold, a revision git does not know, a tree before docsys/0.5.
#[test]
fn a_bad_invocation_exits_2_and_names_what_is_wrong() {
    let (repo, docs) = project("refusals");
    for (args, named) in [
        (&["crosscheck"][..], "`docsys crosscheck --since <ref>`"),
        (&["crosscheck", "--json"], "`docsys crosscheck <page>…`"),
        (&["crosscheck", "nope"], "`nope`"),
        (&["crosscheck", "refresh", "nope", "--json"], "`nope`"),
        (&["crosscheck", "work/nope"], "`work/nope`"),
        (&["crosscheck", "--since", "no-such-rev"], "`no-such-rev`"),
        (&["crosscheck", "--since"], "--since"),
        (&["crosscheck", "--force"], "`--force`"),
    ] {
        let (code, out, err) = docsys(&repo, args);
        assert_eq!(code, Some(2), "{args:?}: {err}");
        assert!(out.is_empty(), "{args:?}: {out}");
        assert!(err.contains(named), "{args:?}: {err}");
        assert!(!err.contains("crosscheck: crosscheck"), "{args:?}: {err}");
    }
    // both forms are named when neither is given
    let (_, _, err) = docsys(&repo, &["crosscheck"]);
    assert!(
        err.contains("`docsys crosscheck <page>…`")
            && err.contains("`docsys crosscheck --since <ref>`"),
        "{err}"
    );
    // a docsys/0.4 tree resolves pins another way: it moves first
    let dm = docs.join(".docmeta.yml");
    let text = fs::read_to_string(&dm).unwrap();
    fs::write(
        &dm,
        text.replace(
            &format!("spec: docsys/{}", docsys::rules::spec_version()),
            "spec: docsys/0.4",
        ),
    )
    .unwrap();
    let (code, out, err) = docsys(&repo, &["crosscheck", "refresh"]);
    assert_eq!(code, Some(2), "{err}");
    assert!(out.is_empty(), "{out}");
    assert!(
        err.contains("crosscheck: this tree declares docsys/0.4")
            && err.contains("`docsys upgrade` moves the tree first"),
        "{err}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// `/docsys-crosscheck` comes with the agent layer of a docsys/0.5 tree, a
/// project's and a knowledge base's alike; `upgrade` writes it where it is
/// absent; a docsys/0.4 tree keeps the layer 0.15.1 wrote. Its text runs the
/// helper and records what it cannot settle as items, never as a
/// verification.
#[test]
fn the_agent_layer_carries_the_crosscheck_command_for_both_profiles() {
    let command = "commands/docsys-crosscheck.md";
    for kb in [false, true] {
        let owned = docsys::agents::owned_assets(kb);
        assert!(
            owned.iter().any(|(a, _, new)| *a == command && *new),
            "kb {kb}: {owned:?}"
        );
    }
    let (_, text, _) = docsys::agents::owned_assets(false)
        .into_iter()
        .find(|(a, _, _)| *a == command)
        .unwrap();
    for must in [
        "docsys crosscheck",
        "--since",
        "docsys question add",
        "docsys debt add",
    ] {
        assert!(text.contains(must), "{must}: {text}");
    }
    for never in ["docsys verify", "Approved-by", "Verifies:", "verified"] {
        assert!(!text.contains(never), "{never}: {text}");
    }
    assert!(text.lines().count() <= 40, "{} lines", text.lines().count());
    // a project's layer
    let base = tmp("layer");
    write(
        &base,
        "docs/.docmeta.yml",
        "spec: docsys/0.5\nprofile: project\n",
    );
    let dir = base.join(".claude");
    let done = docsys::agents::install(&dir, false).unwrap();
    assert!(
        done.written.iter().any(|f| f == command),
        "{:?}",
        done.written
    );
    assert_eq!(fs::read_to_string(dir.join(command)).unwrap(), text);
    // a knowledge base's layer
    let kb = tmp("layer-kb");
    docsys::migrate::init_profile(&kb, "en", "knowledge-base").unwrap();
    let done = docsys::agents::install_kb(&kb.join(".claude"), &kb, false).unwrap();
    assert!(
        done.written.iter().any(|f| f == command),
        "{:?}",
        done.written
    );
    assert_eq!(
        fs::read_to_string(kb.join(".claude").join(command)).unwrap(),
        text
    );
    // a docsys/0.4 tree's layer has none
    let old = tmp("layer-04");
    write(
        &old,
        "docs/.docmeta.yml",
        "spec: docsys/0.4\nprofile: project\n",
    );
    let done = docsys::agents::install(&old.join(".claude"), false).unwrap();
    assert!(
        !old.join(".claude").join(command).exists(),
        "{:?}",
        done.written
    );
    for d in [base, kb, old] {
        let _ = fs::remove_dir_all(d);
    }
}
