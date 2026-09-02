#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]
// A question's first hop across a tree and what it consumes; a consumer's
// provider list growing by `consume add` and `consume discover`; `adopt`
// naming the tree for its consumers. No registry anywhere else (D-075).

use docsys::{consume, export, lookup};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-lookup-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

/// A provider checkout `<dir>/<name>/docs` with one reference page.
fn provider(hub: &Path, name: &str, id: &str, title: &str, body: &str) -> PathBuf {
    let repo = hub.join(name);
    let docs = repo.join("docs");
    write(
        &docs,
        ".docmeta.yml",
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n",
    );
    write(
        &docs,
        "index.md",
        &format!("# docs\n\n- [[reference/{id}|{title}]] -- {title}.\n"),
    );
    write(
        &docs,
        &format!("reference/{id}.md"),
        &format!(
            "---\nid: {id}\ntype: reference\nupdated: 2026-09-02\n---\n# {title}\n\n\
             This page states {title}; read it before changing it.\n\n{body}\n"
        ),
    );
    repo
}

/// A consumer with its own pages: a reference page, a draft, and — in the
/// knowledge-base variant — a raw record that must never be an answer.
fn consumer(hub: &Path) -> PathBuf {
    let docs = hub.join("app").join("docs");
    write(
        &docs,
        ".docmeta.yml",
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n",
    );
    write(
        &docs,
        "index.md",
        "# docs\n\n- [[reference/token-ttl|Token lifetime]] -- how long a token lives.\n",
    );
    write(
        &docs,
        "reference/token-ttl.md",
        "---\nid: token-ttl\ntype: reference\nupdated: 2026-09-02\ntags: [auth]\n---\n# Token lifetime\n\n\
         This page states how long a token lives; read it before changing renewal.\n\nOne hour, renewed silently.\n",
    );
    write(
        &docs,
        "work/features/jitter.md",
        "---\nid: jitter\nstatus: draft\nupdated: 2026-09-02\n---\n\n## Context\n\nSpread token renewals with jitter.\n\n## Decision\n\n## Contract surface\n\n## Rejected alternatives\n",
    );
    write(
        &docs,
        "work/journal.md",
        "# Journal\n\n## 2026-09-02 - initialized\n- tree created\n",
    );
    docs
}

fn tokens(hits: &[lookup::Hit]) -> Vec<String> {
    hits.iter().map(|h| h.token.clone()).collect()
}

#[test]
fn lookup_ranks_local_and_consumed_pages_and_needs_every_word() {
    let hub = tmp("hub");
    provider(
        &hub,
        "auth",
        "refresh",
        "Token refresh",
        "A token is refreshed at half its lifetime.",
    );
    provider(
        &hub,
        "billing",
        "invoice",
        "Invoice cycle",
        "Invoices close monthly.",
    );
    let docs = consumer(&hub);
    let words = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();

    // before any provider: the local pages only, the draft with its caveat
    let hits = lookup::lookup(&docs, &words("token")).unwrap();
    assert_eq!(tokens(&hits), vec!["token-ttl", "jitter"], "{hits:?}");
    assert_eq!(hits[1].caveat.as_deref(), Some("status: draft"));
    assert_eq!(hits[1].kind, "feature");

    // two providers, added by path; the list lives in the consumer's docmeta
    let msg = consume::add(&docs, hub.join("auth").to_str().unwrap(), None).unwrap();
    assert!(msg.contains("auth"), "{msg}");
    consume::add(&docs, hub.join("billing").to_str().unwrap(), Some("bill")).unwrap();
    let dm = fs::read_to_string(docs.join(".docmeta.yml")).unwrap();
    assert!(dm.contains("consume: [auth="), "{dm}");
    assert!(dm.contains("#docs, bill="), "{dm}");
    let err = consume::add(&docs, hub.join("auth").to_str().unwrap(), None).unwrap_err();
    assert!(err.contains("already"), "{err}");
    let err = consume::add(&docs, hub.to_str().unwrap(), None).unwrap_err();
    assert!(err.contains("no docsys tree"), "{err}");

    export::fetch(&docs).unwrap();
    let hits = lookup::lookup(&docs, &words("token")).unwrap();
    assert_eq!(
        tokens(&hits),
        vec!["token-ttl", "@auth/refresh", "jitter"],
        "{hits:?}"
    );
    assert_eq!(hits[1].rel, ".federation/auth/refresh.md");
    // every word must occur: `token lifetime` keeps the pages naming both
    let hits = lookup::lookup(&docs, &words("token lifetime")).unwrap();
    assert_eq!(
        tokens(&hits),
        vec!["token-ttl", "@auth/refresh"],
        "{hits:?}"
    );
    // a consumed page under its own namespace's name
    let hits = lookup::lookup(&docs, &words("invoice")).unwrap();
    assert_eq!(tokens(&hits), vec!["@bill/invoice"], "{hits:?}");
    // nothing: an empty list, and the rendering says so
    let hits = lookup::lookup(&docs, &words("banana")).unwrap();
    assert!(hits.is_empty());
    assert!(lookup::render(&hits, &words("banana")).contains("not in the base"));
    assert!(lookup::render_json(&hits).contains("\"hits\":[]"));
}

#[test]
fn discover_lists_the_trees_under_a_directory_and_writes_nothing() {
    let hub = tmp("discover");
    provider(&hub, "auth", "refresh", "Token refresh", "Half life.");
    provider(&hub, "billing", "invoice", "Invoice cycle", "Monthly.");
    fs::create_dir_all(hub.join("not-a-tree/src")).unwrap();
    let docs = consumer(&hub);
    consume::add(&docs, hub.join("auth").to_str().unwrap(), None).unwrap();
    let before = fs::read_to_string(docs.join(".docmeta.yml")).unwrap();
    let found = consume::discover(&docs, &hub).unwrap();
    let names: Vec<(String, bool)> = found.iter().map(|c| (c.ns.clone(), c.already)).collect();
    assert_eq!(
        names,
        vec![("auth".to_string(), true), ("billing".to_string(), false)],
        "{found:?}"
    );
    assert_eq!(
        fs::read_to_string(docs.join(".docmeta.yml")).unwrap(),
        before
    );
}

#[test]
fn a_raw_record_is_never_an_answer() {
    let base = tmp("kb");
    docsys::migrate::init_profile(&base, "en", "knowledge-base").unwrap();
    let dm = base.join(".docmeta.yml");
    let text = fs::read_to_string(&dm)
        .unwrap()
        .replace("domains: []", "domains: [finance]");
    fs::write(&dm, text).unwrap();
    write(
        &base,
        "raw/inbox/2026-09-02-fee.md",
        "The FX transfer fee is 0.5 percent.\n",
    );
    write(
        &base,
        "wiki/finance/reference/fx-fee.md",
        "---\nid: fx-fee\ntype: reference\ndomain: finance\nverification: unverified\nupdated: 2026-09-02\nsources: [raw/inbox/2026-09-02-fee.md]\n---\n# FX transfer fee\n\nThis page states the fee; read it before wiring money.\n\n0.5 percent, 20 minimum.\n",
    );
    write(
        &base,
        "wiki/finance/index.md",
        "# finance\n\n- [[finance/reference/fx-fee|FX transfer fee]] -- the fee.\n",
    );
    write(
        &base,
        "wiki/index.md",
        "# Knowledge base\n\n- [[finance/index|Finance]] -- money.\n",
    );
    let hits = lookup::lookup(&base, &["fee".to_string()]).unwrap();
    assert_eq!(tokens(&hits), vec!["fx-fee"], "{hits:?}");
    assert_eq!(hits[0].caveat.as_deref(), Some("unverified"));
}

#[test]
fn adopt_names_the_tree_for_its_consumers() {
    let repo = tmp("adopt-ns");
    let git = |args: &[&str]| {
        assert!(Command::new("git")
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "t@example.invalid"]);
    git(&["config", "user.name", "t"]);
    let docs = repo.join("docs");
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    let dm = fs::read_to_string(docs.join(".docmeta.yml")).unwrap();
    let expected = format!(
        "namespace: {}",
        consume::local_id_of(&repo.file_name().unwrap().to_string_lossy())
    );
    assert!(dm.contains(&expected), "{dm}");
    // idempotent: a second adopt keeps the name
    docsys::adopt::run(&repo, &docs, "en").unwrap();
    assert_eq!(
        fs::read_to_string(docs.join(".docmeta.yml"))
            .unwrap()
            .matches("namespace:")
            .count(),
        1
    );
    let (r, _) = docsys::lint(&docs);
    assert!(
        r.findings.iter().all(|f| f.rule.0 != "R-161"),
        "{:?}",
        r.findings
    );
}
