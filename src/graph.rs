//! Derived navigation (D-064): backlinks, unlinked mentions, and the graph of
//! page→page links, work→permanent graduation and code→page citations.
//! Derived artifacts only — nothing here is ever written into a page (R-156).

use crate::checks::{build_index, doc_tokens_on_line, link_path_of, resolve_doc_token, wiki_links};
use crate::fm::Value;
use crate::tree::{DocTree, Kind};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

fn id_of(page: &crate::tree::Page) -> Option<String> {
    page.fm
        .as_ref()
        .and_then(|f| f.fields.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn title_of(page: &crate::tree::Page) -> String {
    page.text
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(|t| t.trim().to_string())
        .unwrap_or_default()
}

/// The page named by `what`: a root-relative path (with or without `.md`)
/// or an `id`.
fn find_page<'a>(tree: &'a DocTree, what: &str) -> Option<&'a crate::tree::Page> {
    let w = what.trim().trim_end_matches(".md");
    tree.pages
        .iter()
        .find(|p| p.rel.trim_end_matches(".md") == w)
        .or_else(|| tree.pages.iter().find(|p| id_of(p).as_deref() == Some(w)))
}

/// Pages linking to `what`, and — with a repository — code files citing its id.
pub fn backlinks(tree: &DocTree, repo: Option<&Path>, what: &str) -> Result<String, String> {
    let page = find_page(tree, what).ok_or_else(|| format!("no page at or with id `{what}`"))?;
    let target = link_path_of(tree, &page.rel).unwrap_or_default();
    let id = id_of(page);
    let mut out = format!(
        "# backlinks of {}{}\n",
        page.rel,
        id.as_ref().map_or(String::new(), |i| format!(" (id {i})"))
    );
    let mut n = 0usize;
    for p in &tree.pages {
        if p.rel == page.rel {
            continue;
        }
        for (line, t) in wiki_links(&p.text) {
            let t = t.split('#').next().unwrap_or("");
            if t == target {
                out.push_str(&format!("{}:{}\n", p.rel, line + 1));
                n += 1;
            }
        }
        if let Some(id) = &id {
            for (i, l) in p.text.lines().enumerate() {
                if doc_tokens_on_line(l).iter().any(|tok| tok == id) {
                    out.push_str(&format!("{}:{} (doc: {id})\n", p.rel, i + 1));
                    n += 1;
                }
            }
        }
    }
    if let (Some(repo), Some(id)) = (repo, &id) {
        for file in crate::migrate::repo_text_files(repo, &tree.root) {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let rel = file
                .strip_prefix(repo)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            for (i, l) in text.lines().enumerate() {
                if doc_tokens_on_line(l).iter().any(|tok| tok == id) {
                    out.push_str(&format!("{rel}:{} (code, doc: {id})\n", i + 1));
                    n += 1;
                }
            }
        }
    }
    out.push_str(&format!("-- {n} backlink(s)\n"));
    Ok(out)
}

/// Prose that names a page — its title or its id as a whole word — without
/// linking to it. A suggestion, never an edit.
pub fn mentions(tree: &DocTree, what: Option<&str>) -> Result<String, String> {
    let targets: Vec<&crate::tree::Page> = match what {
        Some(w) => vec![find_page(tree, w).ok_or_else(|| format!("no page at or with id `{w}`"))?],
        None => tree
            .pages
            .iter()
            .filter(|p| p.kind == Kind::Permanent)
            .collect(),
    };
    let mut out = String::new();
    let mut n = 0usize;
    for page in targets {
        let path = link_path_of(tree, &page.rel).unwrap_or_default();
        let mut names: Vec<String> = Vec::new();
        let title = title_of(page);
        if title.chars().count() >= 4 {
            names.push(title.clone());
        }
        if let Some(id) = id_of(page) {
            if id.len() >= 4 {
                names.push(id);
            }
        }
        if names.is_empty() {
            continue;
        }
        for p in &tree.pages {
            if p.rel == page.rel {
                continue;
            }
            let links: BTreeSet<String> = wiki_links(&p.text)
                .into_iter()
                .map(|(_, t)| t.split('#').next().unwrap_or("").to_string())
                .collect();
            if links.contains(&path) {
                continue;
            }
            for (i, l) in crate::checks::scannable_lines_pub(&p.text) {
                if l.contains("[[") || !doc_tokens_on_line(l).is_empty() {
                    continue;
                }
                for name in &names {
                    if has_word_ci(l, name) {
                        out.push_str(&format!(
                            "{}:{} mentions `{name}` — link it: [[{path}|{name}]]\n",
                            p.rel,
                            i + 1
                        ));
                        n += 1;
                        break;
                    }
                }
            }
        }
    }
    out.push_str(&format!("-- {n} unlinked mention(s)\n"));
    Ok(out)
}

fn has_word_ci(text: &str, word: &str) -> bool {
    let t = text.to_lowercase();
    let w = word.to_lowercase();
    let mut start = 0;
    while let Some(pos) = t.get(start..).and_then(|s| s.find(&w)) {
        let abs = start + pos;
        let before = t.get(..abs).and_then(|s| s.chars().last());
        let after = t.get(abs + w.len()..).and_then(|s| s.chars().next());
        if before.is_none_or(|c| !c.is_alphanumeric()) && after.is_none_or(|c| !c.is_alphanumeric())
        {
            return true;
        }
        start = abs + w.len().max(1);
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: &'static str, // link | graduated_to | cites
}

/// Nodes (pages, plus code files when a repository is given) and edges.
pub fn edges(tree: &DocTree, repo: Option<&Path>) -> (Vec<String>, Vec<Edge>) {
    let mut nodes: BTreeSet<String> = tree.pages.iter().map(|p| p.rel.clone()).collect();
    let mut edges = Vec::new();
    let by_path: BTreeMap<String, String> = tree
        .pages
        .iter()
        .filter_map(|p| link_path_of(tree, &p.rel).map(|lp| (lp, p.rel.clone())))
        .collect();
    let by_id: BTreeMap<String, String> = tree
        .pages
        .iter()
        .filter_map(|p| id_of(p).map(|i| (i, p.rel.clone())))
        .collect();
    for p in &tree.pages {
        for (_, t) in wiki_links(&p.text) {
            let t = t.split('#').next().unwrap_or("");
            if let Some(to) = by_path.get(t) {
                edges.push(Edge {
                    from: p.rel.clone(),
                    to: to.clone(),
                    kind: "link",
                });
            }
        }
        if let Some(fm) = &p.fm {
            if let Some(list) = fm.fields.get("graduated_to").and_then(Value::as_list) {
                for g in list {
                    let g = g.trim().trim_end_matches(".md");
                    if let Some(to) = by_path.get(g).or_else(|| by_id.get(g)) {
                        edges.push(Edge {
                            from: p.rel.clone(),
                            to: to.clone(),
                            kind: "graduated_to",
                        });
                    }
                }
            }
        }
    }
    if let Some(repo) = repo {
        let idx = build_index(tree);
        for file in crate::migrate::repo_text_files(repo, &tree.root) {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let rel = file
                .strip_prefix(repo)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            for l in text.lines() {
                for tok in doc_tokens_on_line(l) {
                    if resolve_doc_token(&idx, &tok).is_ok() {
                        if let Some(to) = by_id.get(&tok) {
                            nodes.insert(format!("code:{rel}"));
                            edges.push(Edge {
                                from: format!("code:{rel}"),
                                to: to.clone(),
                                kind: "cites",
                            });
                        }
                    }
                }
            }
        }
    }
    edges.sort_by(|a, b| {
        (a.from.as_str(), a.to.as_str(), a.kind).cmp(&(b.from.as_str(), b.to.as_str(), b.kind))
    });
    edges.dedup();
    (nodes.into_iter().collect(), edges)
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `dot`, `json`, or `jsoncanvas` (JSON Canvas 1.0 — nodes laid out in
/// columns by layer, so Obsidian opens it as a readable map).
pub fn render(tree: &DocTree, repo: Option<&Path>, format: &str) -> Result<String, String> {
    let (nodes, edges) = edges(tree, repo);
    match format {
        "dot" => {
            let mut s = String::from(
                "digraph docsys {\n  rankdir=LR;\n  node [shape=box, fontname=\"Helvetica\"];\n",
            );
            for n in &nodes {
                let shape = if n.starts_with("code:") {
                    ", shape=note"
                } else {
                    ""
                };
                s.push_str(&format!(
                    "  \"{n}\" [label=\"{}\"{shape}];\n",
                    n.trim_start_matches("code:")
                ));
            }
            for e in &edges {
                let style = match e.kind {
                    "graduated_to" => " [style=dashed, label=\"graduated\"]",
                    "cites" => " [color=gray, label=\"doc:\"]",
                    _ => "",
                };
                s.push_str(&format!("  \"{}\" -> \"{}\"{style};\n", e.from, e.to));
            }
            s.push_str("}\n");
            Ok(s)
        }
        "json" => {
            let ns: Vec<String> = nodes.iter().map(|n| json_str(n)).collect();
            let es: Vec<String> = edges
                .iter()
                .map(|e| {
                    format!(
                        "{{\"from\": {}, \"to\": {}, \"kind\": {}}}",
                        json_str(&e.from),
                        json_str(&e.to),
                        json_str(e.kind)
                    )
                })
                .collect();
            Ok(format!(
                "{{\"nodes\": [{}],\n \"edges\": [\n  {}\n ]}}\n",
                ns.join(", "),
                es.join(",\n  ")
            ))
        }
        "jsoncanvas" => {
            // columns: code | work | permanent | router
            let column = |n: &str| -> usize {
                if n.starts_with("code:") {
                    0
                } else if n.starts_with("work/") {
                    1
                } else if n == "index.md" || n.ends_with("/index.md") {
                    3
                } else {
                    2
                }
            };
            let mut rows: BTreeMap<usize, usize> = BTreeMap::new();
            let mut ns = Vec::new();
            for n in &nodes {
                let c = column(n);
                let y = *rows.get(&c).unwrap_or(&0);
                rows.insert(c, y + 1);
                let (kind, body) = if n.starts_with("code:") {
                    (
                        "text",
                        format!("\"text\": {}", json_str(n.trim_start_matches("code:"))),
                    )
                } else {
                    ("file", format!("\"file\": {}", json_str(n)))
                };
                ns.push(format!(
                    "  {{\"id\": {}, \"type\": \"{kind}\", \"x\": {}, \"y\": {}, \"width\": 320, \"height\": 60, {body}}}",
                    json_str(n),
                    c * 400,
                    y * 90
                ));
            }
            let es: Vec<String> = edges
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    format!(
                        "  {{\"id\": \"e{i}\", \"fromNode\": {}, \"toNode\": {}, \"fromSide\": \"right\", \"toSide\": \"left\", \"label\": {}}}",
                        json_str(&e.from),
                        json_str(&e.to),
                        json_str(e.kind)
                    )
                })
                .collect();
            Ok(format!(
                "{{\"nodes\": [\n{}\n],\n\"edges\": [\n{}\n]}}\n",
                ns.join(",\n"),
                es.join(",\n")
            ))
        }
        other => Err(format!(
            "`{other}` is not a format (dot | json | jsoncanvas)"
        )),
    }
}
