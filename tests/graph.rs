#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]
// Derived navigation: backlinks, unlinked mentions, graph export — on a small
// tree with a work file graduated to a page and a code file citing it.

use std::fs;
use std::path::{Path, PathBuf};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-graph-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn write(base: &Path, rel: &str, text: &str) {
    let p = base.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn build(name: &str) -> PathBuf {
    let repo = tmp(name);
    let docs = repo.join("docs");
    docsys::migrate::init_profile(&docs, "en", "project").unwrap();
    write(&repo, "docs/index.md", "# Docs\n\n- [[reference/token-ttl|Token TTL]] -- How long a token lives.\n- [[reference/renewal|Renewal]] -- Renewing.\n");
    write(&repo, "docs/reference/token-ttl.md", "---\nid: token-ttl\ntype: reference\nupdated: 2026-08-29\n---\n# Token TTL\n\nThis page describes the token TTL; read it when a token expires.\n\nSee [[reference/renewal]].\n");
    write(&repo, "docs/reference/renewal.md", "---\nid: renewal\ntype: reference\nupdated: 2026-08-29\n---\n# Renewal\n\nThis page describes renewal; read it when the token TTL ends.\n\nThe token TTL decides when renewal starts.\n");
    write(&repo, "docs/work/features/ttl.md", "---\nid: ttl-work\nstatus: graduated\nupdated: 2026-08-29\ngraduated_to: [reference/token-ttl]\n---\n## Context\n## Decision\n## Contract surface\n## Rejected alternatives\n");
    write(&repo, "src/auth.rs", "// doc: token-ttl\nfn ttl() {}\n");
    repo
}

#[test]
fn backlinks_list_pages_and_code_pointing_at_a_page() {
    let repo = build("backlinks");
    let tree = docsys::tree::DocTree::load(&repo.join("docs")).unwrap();
    let out = docsys::graph::backlinks(&tree, Some(&repo), "token-ttl").unwrap();
    assert!(out.contains("index.md:3"), "{out}");
    assert!(
        out.contains("src/auth.rs:1 (code, doc: token-ttl)"),
        "{out}"
    );
    assert!(
        !out.contains("reference/renewal.md"),
        "renewal mentions but does not link: {out}"
    );
    assert!(out.trim_end().ends_with("-- 2 backlink(s)"), "{out}");
    let by_path = docsys::graph::backlinks(&tree, None, "reference/renewal.md").unwrap();
    assert!(
        by_path.contains("reference/token-ttl.md:") && by_path.contains("index.md:4"),
        "{by_path}"
    );
    assert!(docsys::graph::backlinks(&tree, None, "nope").is_err());
}

#[test]
fn unlinked_mentions_name_prose_that_names_a_page_without_a_link() {
    let repo = build("mentions");
    let tree = docsys::tree::DocTree::load(&repo.join("docs")).unwrap();
    let out = docsys::graph::mentions(&tree, Some("token-ttl")).unwrap();
    assert!(
        out.contains("reference/renewal.md:")
            && out.contains("mentions `Token TTL` — link it: [[reference/token-ttl|Token TTL]]"),
        "{out}"
    );
    // the router's own link line is not a mention; the linking page is skipped
    assert!(!out.contains("index.md"), "{out}");
    let all = docsys::graph::mentions(&tree, None).unwrap();
    assert!(all.contains("unlinked mention(s)"), "{all}");
}

#[test]
fn graph_exports_links_graduation_and_citations_in_three_formats() {
    let repo = build("graph");
    let tree = docsys::tree::DocTree::load(&repo.join("docs")).unwrap();
    let (nodes, edges) = docsys::graph::edges(&tree, Some(&repo));
    assert!(nodes.contains(&"code:src/auth.rs".to_string()), "{nodes:?}");
    assert!(
        edges.iter().any(|e| e.from == "reference/token-ttl.md"
            && e.to == "reference/renewal.md"
            && e.kind == "link"),
        "{edges:?}"
    );
    assert!(
        edges.iter().any(|e| e.from == "work/features/ttl.md"
            && e.to == "reference/token-ttl.md"
            && e.kind == "graduated_to"),
        "{edges:?}"
    );
    assert!(
        edges.iter().any(|e| e.from == "code:src/auth.rs"
            && e.to == "reference/token-ttl.md"
            && e.kind == "cites"),
        "{edges:?}"
    );
    let dot = docsys::graph::render(&tree, Some(&repo), "dot").unwrap();
    assert!(
        dot.starts_with("digraph docsys {")
            && dot.contains("\"work/features/ttl.md\" -> \"reference/token-ttl.md\" [style=dashed"),
        "{dot}"
    );
    let json = docsys::graph::render(&tree, None, "json").unwrap();
    assert!(
        json.contains("\"kind\": \"link\"") && !json.contains("cites"),
        "{json}"
    );
    let canvas = docsys::graph::render(&tree, Some(&repo), "jsoncanvas").unwrap();
    assert!(
        canvas.contains("\"type\": \"file\"")
            && canvas.contains("\"file\": \"reference/token-ttl.md\""),
        "{canvas}"
    );
    assert!(
        canvas.contains("\"fromNode\": \"code:src/auth.rs\""),
        "{canvas}"
    );
    // a JSON Canvas node id must be unique and edges reference them
    assert_eq!(
        canvas.matches("\"id\": \"reference/token-ttl.md\"").count(),
        1
    );
    assert!(docsys::graph::render(&tree, None, "svg").is_err());
}
