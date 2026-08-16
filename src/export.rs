//! `docsys export` — the product side of federation's founding goal: a
//! product-level document composed from the tree's permanent pages.
//!
//! Three commands, the migrate/graduate pattern (R-003: composition is
//! judgment):
//!   `export plan`     — a draft product map from the tree's evidence; a
//!                       proposal, never a decision.
//!   `export product`  — compose a whole product from an authored map.
//!   `export feature`  — compose a slice: the identifiers named on the command
//!                       line, optionally widened one hop along their
//!                       wiki-links (`--follow`) — no map file required.
//!
//! Composition is fully mechanical: bodies are carried verbatim (heading
//! levels shift, prose is never rewritten), every composed page carries a
//! source stamp, and the run refuses to half-compose. Regeneration is
//! stateless by design — a cache is state and state drifts (R-002) — but an
//! unchanged result never touches the output file (`write_if_changed`), so
//! nothing downstream re-triggers, and the per-page stamps let downstream
//! consumers re-process only the sections whose hash moved.
//!
//! The map (D-032) is plain markdown: H1 product name, intro prose, H2
//! sections, and under each section R-035-shaped lines whose targets are
//! `doc:` identifiers — `- [[<id>|<title>]] -- <sentence>`. A `@ns/<id>`
//! entry is refused until federation consumption exists (the D-006 pattern:
//! refuse, never half-run). The map lives outside the docs root: its targets
//! are identifiers, not paths, and the link checks must not read them.

use crate::fm::Value;
use crate::migrate::today;
use crate::tree::{DocTree, Kind, Page};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

#[derive(Debug)]
pub struct ProductOutcome {
    pub output: String,
    /// Deliberate-choice reports (an `internal: true` page on the map, R-019;
    /// a declared-language mismatch, R-122).
    pub warnings: Vec<String>,
    pub pages: usize,
}

/// FNV-1a 64 over the composed body — a drift stamp, not the R-113 canonical
/// hash (that lands with the manifest). It answers one question: has the
/// source changed since this document was generated?
fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

fn strip_frontmatter(text: &str) -> &str {
    let Some(rest) = text.strip_prefix("---\n") else {
        return text;
    };
    match rest.find("\n---\n") {
        Some(end) => &rest[end + 5..],
        None => text,
    }
}

/// R-057 defaults: `title` falls back to the first heading's text.
fn title_of(page: &Page) -> String {
    if let Some(t) = page
        .fm
        .as_ref()
        .and_then(|f| f.fields.get("title"))
        .and_then(Value::as_str)
    {
        return t.to_string();
    }
    strip_frontmatter(&page.text)
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .unwrap_or("(untitled)")
        .trim()
        .to_string()
}

/// R-057 defaults: `summary` falls back to the first paragraph, truncated at
/// the first sentence boundary (the algorithm is implementation-defined, §19).
fn summary_of(page: &Page) -> String {
    if let Some(s) = page
        .fm
        .as_ref()
        .and_then(|f| f.fields.get("summary"))
        .and_then(Value::as_str)
    {
        return s.to_string();
    }
    let body = strip_frontmatter(&page.text);
    let mut para = String::new();
    let mut seen_text = false;
    for line in body.lines() {
        // A leading blockquote is presentation, not sentence; comments are
        // not prose at all.
        let t = line.trim().trim_start_matches('>').trim_start();
        if t.starts_with("<!--") {
            continue;
        }
        if t.starts_with('#') || t.is_empty() {
            if seen_text {
                break;
            }
            continue;
        }
        seen_text = true;
        if !para.is_empty() {
            para.push(' ');
        }
        para.push_str(t);
    }
    match para.find(". ") {
        Some(i) => para[..=i].trim().to_string(),
        None => para,
    }
}

/// The page body with its leading H1 dropped (the title is re-set by the
/// composer) and every remaining heading shifted `shift` levels down, fenced
/// code untouched. Prose is carried byte-for-byte — the graduation principle
/// (content moves, it is never rewritten) applied to rendering.
fn shifted_body(text: &str, shift: usize) -> String {
    let mut out = String::new();
    let mut in_fence = false;
    let mut dropped_h1 = false;
    for line in strip_frontmatter(text).lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if !in_fence && line.starts_with('#') {
            if !dropped_h1 && line.starts_with("# ") {
                dropped_h1 = true;
                continue;
            }
            let level = line.chars().take_while(|c| *c == '#').count();
            let add = shift.min(6usize.saturating_sub(level));
            out.push_str(&"#".repeat(add));
            out.push_str(line);
            out.push('\n');
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.trim_matches('\n').to_string()
}

struct Section {
    /// Empty on a sectionless (feature) composition: pages sit one level up
    /// and no table of contents is emitted.
    title: String,
    entries: Vec<String>, // identifiers, map order preserved
}

fn parse_map(text: &str) -> Result<(String, String, Vec<Section>), String> {
    let mut title = None;
    let mut intro = String::new();
    let mut sections: Vec<Section> = Vec::new();
    for line in text.lines() {
        if line.starts_with("<!--") {
            continue;
        }
        if let Some(t) = line.strip_prefix("# ") {
            if title.is_none() {
                title = Some(t.trim().to_string());
                continue;
            }
        }
        if let Some(s) = line.strip_prefix("## ") {
            sections.push(Section {
                title: s.trim().to_string(),
                entries: Vec::new(),
            });
            continue;
        }
        if line.starts_with("- ") && line.contains("[[") {
            let Some(sec) = sections.last_mut() else {
                return Err(format!("map entry before the first `##` section: `{line}`"));
            };
            let inner = line
                .split_once("[[")
                .and_then(|(_, r)| r.split_once("]]"))
                .map(|(i, _)| i)
                .unwrap_or("");
            let target = inner.split('|').next().unwrap_or("").trim();
            if target.is_empty() {
                return Err(format!("map entry has no identifier: `{line}`"));
            }
            sec.entries.push(target.to_string());
            continue;
        }
        if title.is_some() && sections.is_empty() && !line.trim().is_empty() {
            if !intro.is_empty() {
                intro.push('\n');
            }
            intro.push_str(line);
        }
    }
    let title = title.ok_or("the map has no `# <product name>` heading")?;
    if sections.iter().all(|s| s.entries.is_empty()) {
        return Err("the map names no pages — nothing to compose".to_string());
    }
    Ok((title, intro, sections))
}

/// Permanent pages by identifier (aliases included) and the flowing ids, for
/// resolution and refusal messages.
fn index_pages(tree: &DocTree) -> (BTreeMap<&str, &Page>, BTreeMap<&str, &Page>) {
    let mut by_id: BTreeMap<&str, &Page> = BTreeMap::new();
    let mut flowing: BTreeMap<&str, &Page> = BTreeMap::new();
    for page in &tree.pages {
        let Some(fm) = &page.fm else { continue };
        let Some(id) = fm.fields.get("id").and_then(Value::as_str) else {
            continue;
        };
        if page.kind == Kind::Permanent {
            by_id.insert(id, page);
            if let Some(aliases) = fm.fields.get("aliases").and_then(Value::as_list) {
                for a in aliases {
                    by_id.insert(a.as_str(), page);
                }
            }
        } else {
            flowing.insert(id, page);
        }
    }
    (by_id, flowing)
}

/// The shared composer. Every entry must resolve to a permanent page;
/// anything else is refused with the full list — a document with silently
/// missing sections would be the lie R-151 forbids.
///
/// `want_lang` states the document's intended content language. The tool
/// determines no language (R-120's note) and translates nothing — translation
/// is agent work (R-122/R-123) — but it can read declarations: a page whose
/// `lang:` (or the tree's `default_content_language`) differs from the intent
/// is reported, and without an intent a mixed composition is reported once.
fn compose(
    tree: &DocTree,
    source_note: &str,
    title: Option<String>,
    intro: &str,
    sections: &[Section],
    want_lang: Option<&str>,
) -> Result<ProductOutcome, String> {
    let (by_id, flowing) = index_pages(tree);
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    for s in sections {
        for id in &s.entries {
            if id.starts_with('@') {
                errors.push(format!(
                    "`{id}`: federation consumption is not implemented — a foreign page \
                     cannot be composed yet"
                ));
            } else if by_id.contains_key(id.as_str()) {
                // resolves
            } else if flowing.contains_key(id.as_str()) {
                errors.push(format!(
                    "`{id}` is flowing work — distil it into a permanent page first (R-194)"
                ));
            } else if tree.tombstones.iter().any(|t| t == id) {
                errors.push(format!("`{id}` is retired (tombstone ledger)"));
            } else {
                errors.push(format!("`{id}` resolves to no permanent page"));
            }
        }
    }
    if !errors.is_empty() {
        return Err(format!("does not compose:\n  {}", errors.join("\n  ")));
    }
    let sectionless = sections.len() == 1 && sections.first().is_some_and(|s| s.title.is_empty());
    let page_shift = if sectionless { 1 } else { 2 };
    let title = title.unwrap_or_else(|| {
        sections
            .iter()
            .flat_map(|s| s.entries.iter())
            .next()
            .and_then(|id| by_id.get(id.as_str()))
            .map(|p| title_of(p))
            .unwrap_or_else(|| "(untitled)".to_string())
    });

    let mut out = String::new();
    let _ = writeln!(
        out,
        "<!-- generated by docsys export — do not edit; edit the sources and re-run -->"
    );
    let _ = writeln!(out, "<!-- {source_note} · generated: {} -->", today());
    let _ = writeln!(out, "\n# {title}");
    if !intro.is_empty() {
        let _ = writeln!(out, "\n{intro}");
    }
    if !sectionless {
        let _ = writeln!(out);
        for s in sections {
            if !s.entries.is_empty() {
                let _ = writeln!(out, "- {}", s.title);
            }
        }
    }
    let default_lang = tree.docmeta_str("default_content_language").unwrap_or("?");
    let mut langs: BTreeMap<String, Vec<String>> = BTreeMap::new(); // lang → ids
    let mut pages = 0usize;
    for s in sections {
        if s.entries.is_empty() {
            continue;
        }
        if !sectionless {
            let _ = writeln!(out, "\n## {}", s.title);
        }
        for id in &s.entries {
            let Some(page) = by_id.get(id.as_str()) else {
                continue; // unreachable: the resolve pass above refused already
            };
            let internal = page
                .fm
                .as_ref()
                .and_then(|f| f.fields.get("internal"))
                .and_then(Value::as_str)
                == Some("true");
            if internal {
                // R-019: confidentiality is a property of the channel; putting
                // an internal page on a map is the publisher's deliberate call.
                warnings.push(format!(
                    "`{id}` is marked internal — composing it publishes it; make sure the \
                     document's channel may carry it (R-019)"
                ));
            }
            pages += 1;
            let lang = page
                .fm
                .as_ref()
                .and_then(|f| f.fields.get("lang"))
                .and_then(Value::as_str)
                .unwrap_or(default_lang);
            langs.entry(lang.to_string()).or_default().push(id.clone());
            if let Some(want) = want_lang {
                if lang != want {
                    warnings.push(format!(
                        "`{id}` declares content language `{lang}`, the document wants \
                         `{want}` — translation is agent work (R-122; identifiers, product \
                         names and quotations keep their original form, R-123); translate \
                         the source page or set its `lang:`"
                    ));
                }
            }
            let body = shifted_body(&page.text, page_shift);
            let heading = "#".repeat(page_shift + 1);
            let _ = writeln!(out, "\n{heading} {}", title_of(page));
            let _ = writeln!(out, "\n{body}");
            let updated = page
                .fm
                .as_ref()
                .and_then(|f| f.fields.get("updated"))
                .and_then(Value::as_str)
                .unwrap_or("?");
            let _ = writeln!(
                out,
                "\n<!-- source: doc: {id} · {} · fnv:{:016x} · updated: {updated} -->",
                page.rel,
                fnv(body.as_bytes())
            );
        }
    }
    if want_lang.is_none() && langs.len() > 1 {
        let mix = langs
            .iter()
            .map(|(l, ids)| format!("{l}: {}", ids.join(", ")))
            .collect::<Vec<_>>()
            .join(" · ");
        warnings.push(format!(
            "mixed declared content languages composed ({mix}) — declarations only; the tool \
             determines no language (R-120). State the intent with --lang, or translate the \
             odd pages out (agent work, R-122; terms keep their original form, R-123)"
        ));
    }
    Ok(ProductOutcome {
        output: out,
        warnings,
        pages,
    })
}

/// Compose a whole product from an authored map.
pub fn product(
    root: &Path,
    map_path: &Path,
    want_lang: Option<&str>,
) -> Result<ProductOutcome, String> {
    let map_text =
        std::fs::read_to_string(map_path).map_err(|e| format!("{}: {e}", map_path.display()))?;
    let (title, intro, sections) = parse_map(&map_text)?;
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    compose(
        &tree,
        &format!("map: {}", map_path.display()),
        Some(title),
        &intro,
        &sections,
        want_lang,
    )
    .map_err(|e| format!("the map {e}"))
}

/// Compose a slice of the tree — the named identifiers, no map file. With
/// `follow`, the slice widens one hop: pages the named pages wiki-link to
/// (permanent, deduplicated, link order). One hop is mechanical selection;
/// a transitive closure would quietly become the whole tree, which is a
/// product map's job to decide, not a flag's.
pub fn feature(
    root: &Path,
    ids: &[String],
    follow: bool,
    title: Option<String>,
    want_lang: Option<&str>,
) -> Result<ProductOutcome, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    let mut entries: Vec<String> = ids.to_vec();
    if follow {
        let (by_id, _) = index_pages(&tree);
        for id in ids {
            let Some(page) = by_id.get(id.as_str()) else {
                continue; // unresolved: compose refuses with the message below
            };
            for (_, target) in crate::checks::wiki_links(&page.text) {
                let linked = tree.pages.iter().find(|p| {
                    p.kind == Kind::Permanent
                        && crate::checks::link_path_of(&tree, &p.rel).as_deref()
                            == Some(target.as_str())
                });
                let Some(linked) = linked else { continue };
                let Some(lid) = linked
                    .fm
                    .as_ref()
                    .and_then(|f| f.fields.get("id"))
                    .and_then(Value::as_str)
                else {
                    continue;
                };
                if !entries.iter().any(|e| e == lid) {
                    entries.push(lid.to_string());
                }
            }
        }
    }
    let note = format!(
        "ids: {}{}",
        ids.join(", "),
        if follow { " · follow" } else { "" }
    );
    let sections = [Section {
        title: String::new(),
        entries,
    }];
    compose(&tree, &note, title, "", &sections, want_lang).map_err(|e| format!("the slice {e}"))
}

/// Write `output` to `path` only when the composed content differs from what
/// is already there, comparing without the dated header line — an unchanged
/// document keeps its file untouched (and its generation date honest), so
/// watchers, builds and downstream agents are not re-triggered by a no-op.
/// Returns whether the file was written.
pub fn write_if_changed(path: &Path, output: &str) -> std::io::Result<bool> {
    fn undated(text: &str) -> String {
        text.lines()
            .filter(|l| !(l.starts_with("<!--") && l.contains("· generated: ")))
            .collect::<Vec<_>>()
            .join("\n")
    }
    if let Ok(existing) = std::fs::read_to_string(path) {
        if undated(&existing) == undated(output) {
            return Ok(false);
        }
    }
    std::fs::write(path, output)?;
    Ok(true)
}

/// A draft map from the tree's evidence: every permanent page, grouped by
/// type, with its R-057 title and summary. A proposal — keeping, cutting and
/// naming the sections is the judgment the tool never does (R-003).
pub fn plan(root: &Path) -> Result<String, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    let mut groups: BTreeMap<&str, Vec<&Page>> = BTreeMap::new();
    for page in &tree.pages {
        if page.kind != Kind::Permanent {
            continue;
        }
        let Some(fm) = &page.fm else { continue };
        if fm.fields.get("id").and_then(Value::as_str).is_none() {
            continue;
        }
        let ty = fm
            .fields
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("(untyped)");
        groups.entry(ty).or_default().push(page);
    }
    if groups.is_empty() {
        return Err("no permanent pages with an id — nothing to draft from".to_string());
    }
    let mut out = String::new();
    out.push_str(
        "<!-- docsys export plan — a draft product map, not a decision (R-003).\n\
         Rename the title and sections, keep what belongs, delete the rest.\n\
         Targets are doc: identifiers, not paths; keep this file outside the docs root.\n\
         Then: docsys export product <this-file> --root <docs root> -->\n\n",
    );
    out.push_str("# <product name>\n\n<product introduction — one or two sentences>\n");
    for (ty, pages) in &groups {
        let _ = writeln!(out, "\n## {ty}");
        for page in pages {
            let id = page
                .fm
                .as_ref()
                .and_then(|f| f.fields.get("id"))
                .and_then(Value::as_str)
                .unwrap_or("?");
            let _ = writeln!(out, "- [[{id}|{}]] -- {}", title_of(page), summary_of(page));
        }
    }
    Ok(out)
}
