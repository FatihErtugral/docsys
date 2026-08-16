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
//! entry composes from the namespace's local materialization under
//! `.federation/` (`docsys fetch`, D-034) — never from a live provider; an
//! unfetched or tampered materialization is refused by name. The map lives
//! outside the docs root: its targets are identifiers, not paths, and the
//! link checks must not read them.

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

/// A page's declared audience; undeclared reads as `developer` (D-033) —
/// every existing tree is developer documentation, so the default costs no
/// migration and an audience-filtered export only ever includes a page that
/// says who it is for.
fn audience_of(page: &Page) -> &str {
    page.fm
        .as_ref()
        .and_then(|f| f.fields.get("audience"))
        .and_then(Value::as_str)
        .unwrap_or("developer")
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

/// Load a foreign entry from its local materialization (R-136/R-148): the
/// last verified fetch is what composes — never a live provider query. Both
/// halves are checked: a missing sidecar and a body that no longer hashes to
/// its provenance are refusals (R-137/R-147/R-149).
fn load_foreign(tree: &DocTree, token: &str) -> Result<(Page, String), String> {
    let Some((ns, id)) = token.trim_start_matches('@').split_once('/') else {
        return Err(format!("`{token}` is not `@namespace/<id>`"));
    };
    let base = tree.root.join(".federation").join(ns);
    let Ok(text) = std::fs::read_to_string(base.join(format!("{id}.md"))) else {
        return Err(format!(
            "`{token}`: not materialized — declare the provider in .docmeta.yml \
             `consume: [{ns}=<path>]` and run `docsys fetch`"
        ));
    };
    let Ok(side) = std::fs::read_to_string(base.join(format!("{id}.provenance.yml"))) else {
        return Err(format!(
            "`{token}`: materialized page has no provenance sidecar (R-149) — \
             hand-placed content cannot masquerade as federated; re-run `docsys fetch`"
        ));
    };
    let get = |k: &str| {
        side.lines()
            .find_map(|l| l.strip_prefix(k).and_then(|r| r.strip_prefix(": ")))
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let body = strip_frontmatter(&text).replace("\r\n", "\n");
    let body = body.trim_matches('\n');
    let got = format!("fnv:{:016x}", fnv(body.as_bytes()));
    if get("hash") != got {
        return Err(format!(
            "`{token}`: body no longer matches its provenance hash — edited locally or \
             half-fetched (R-137); re-run `docsys fetch`"
        ));
    }
    let fm = crate::fm::parse(&text);
    Ok((
        Page {
            rel: format!(".federation/{ns}/{id}.md"),
            kind: Kind::Permanent,
            text,
            fm,
        },
        get("fetched"),
    ))
}

/// The manifest (R-133): what this namespace exports, without the bodies
/// (R-134 — title and summary are identity metadata, the deliberate
/// exception). One small file per provider, so a consumer can ask "what
/// changed?" without cloning anything (D-038). Line format, not JSON: the
/// parser is the one already in this binary, and a hand-readable manifest is
/// one people can diff in a review.
pub fn manifest(root: &Path) -> Result<String, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    let ns = tree.docmeta_str("namespace").unwrap_or("").trim();
    let mut out = String::new();
    let _ = writeln!(out, "manifest: 1");
    if !ns.is_empty() {
        let _ = writeln!(out, "namespace: {ns}");
    }
    let _ = writeln!(
        out,
        "spec: {}",
        tree.docmeta_str("spec").unwrap_or("docsys/0.4")
    );
    let _ = writeln!(out, "generated: {}", today());
    let mut pages: Vec<&Page> = tree
        .pages
        .iter()
        .filter(|p| p.kind == Kind::Permanent)
        .collect();
    pages.sort_by(|a, b| a.rel.cmp(&b.rel));
    for page in pages {
        let Some(fm) = &page.fm else { continue };
        let Some(id) = fm.fields.get("id").and_then(Value::as_str) else {
            continue;
        };
        if fm.fields.get("internal").and_then(Value::as_str) == Some("true") {
            continue; // R-135: excluded from the manifest entirely
        }
        let field = |k: &str| fm.fields.get(k).and_then(Value::as_str).unwrap_or("");
        let body = strip_frontmatter(&page.text).replace("\r\n", "\n");
        let _ = writeln!(out, "\n- id: {id}");
        let _ = writeln!(out, "  type: {}", field("type"));
        let _ = writeln!(out, "  title: {}", title_of(page));
        let _ = writeln!(out, "  summary: {}", summary_of(page));
        let _ = writeln!(
            out,
            "  hash: fnv:{:016x}",
            fnv(body.trim_matches('\n').as_bytes())
        );
        let _ = writeln!(out, "  updated: {}", field("updated"));
        for k in ["lang", "audience", "owner"] {
            let v = field(k);
            if !v.is_empty() {
                let _ = writeln!(out, "  {k}: {v}");
            }
        }
        let _ = writeln!(out, "  path: {}", page.rel);
    }
    // R-133: retired identifiers travel too, or every tombstone would drop on
    // the next export and break the deprecation window when it is needed.
    for t in &tree.tombstones {
        let _ = writeln!(out, "\n- id: {t}\n  state: withdrawn");
    }
    Ok(out)
}

/// One manifest entry, as a consumer reads it.
struct ManifestEntry {
    id: String,
    hash: String,
    path: String,
    withdrawn: bool,
}

fn parse_manifest(text: &str) -> Result<(u32, Vec<ManifestEntry>), String> {
    let mut version = None;
    let mut entries: Vec<ManifestEntry> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("manifest: ") {
            version = v.trim().parse::<u32>().ok();
        } else if let Some(id) = t.strip_prefix("- id: ") {
            entries.push(ManifestEntry {
                id: id.trim().to_string(),
                hash: String::new(),
                path: String::new(),
                withdrawn: false,
            });
        } else if let Some(last) = entries.last_mut() {
            if let Some(h) = t.strip_prefix("hash: ") {
                last.hash = h.trim().to_string();
            } else if let Some(p) = t.strip_prefix("path: ") {
                last.path = p.trim().to_string();
            } else if t == "state: withdrawn" {
                last.withdrawn = true;
            }
        }
    }
    let version = version.ok_or("not a docsys manifest (no `manifest:` version line)")?;
    // R-182: an unimplemented MAJOR version is refused by name, never skipped.
    if version != 1 {
        return Err(format!(
            "manifest format version {version} is not implemented by this build"
        ));
    }
    Ok((version, entries))
}

fn is_git_url(loc: &str) -> bool {
    loc.starts_with("git@")
        || loc.starts_with("http://")
        || loc.starts_with("https://")
        || loc.starts_with("ssh://")
        || loc.starts_with("file://")
        || loc.ends_with(".git")
}

/// Shallow clone or update a provider checkout. The cache is never edited
/// locally, so a hard reset to what the remote serves is always correct.
fn git_sync(url: &str, cache: &Path) -> Result<(), String> {
    use std::process::Command;
    let run = |args: &[&str], cwd: Option<&Path>| -> Result<(), String> {
        let mut c = Command::new("git");
        if let Some(d) = cwd {
            c.arg("-C").arg(d);
        }
        let out = c.args(args).output().map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "git {} failed for `{url}`: {}",
                args.first().unwrap_or(&"?"),
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    };
    if cache.join(".git").is_dir() {
        run(&["fetch", "-q", "--depth", "1", "origin"], Some(cache))?;
        run(&["reset", "-q", "--hard", "FETCH_HEAD"], Some(cache))
    } else {
        if let Some(parent) = cache.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        run(
            &["clone", "-q", "--depth", "1", url, &cache.to_string_lossy()],
            None,
        )
    }
}

/// Materialize every consumed namespace under `.federation/` (filesystem
/// transport, R-145's simplest conformant channel). The provider tree is read
/// directly; each exported page lands as reconstructed frontmatter (R-136) +
/// verbatim body, with a provenance sidecar (R-149). `internal: true` pages
/// are never materialized (R-135).
pub fn fetch(root: &Path) -> Result<Vec<String>, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    let entries = tree.docmeta_list("consume");
    if entries.is_empty() {
        return Err("nothing to fetch — declare providers in .docmeta.yml: a \
             `consume_base:` template plus `consume: [<name>, …]`, or explicit \
             `consume: [<namespace>=<location>, …]` (D-034)"
            .to_string());
    }
    // At estate scale nobody maintains three hundred locations by hand
    // (R-002): one template names them all, `{ns}` substituted per entry.
    let base = tree.docmeta_str("consume_base");
    let mut summary = Vec::new();
    for entry in entries {
        let (ns, loc) = match entry.split_once('=') {
            Some((n, l)) => (n.trim(), l.trim().to_string()),
            None => {
                let Some(base) = base else {
                    return Err(format!(
                        "consume entry `{entry}` names no location and no `consume_base:` \
                         template is declared"
                    ));
                };
                (entry.trim(), base.replace("{ns}", entry.trim()))
            }
        };
        // `<location>#<subdir>` — the docs root inside the repository.
        let (loc, sub) = match loc.split_once('#') {
            Some((l, s)) => (l.trim().to_string(), s.trim().to_string()),
            None => (loc, String::new()),
        };
        let provider_root = if is_git_url(&loc) {
            // The checkout cache lives under a dot-directory: the tree walk
            // never reads it, only fetch does.
            let cache = root.join(".federation").join(".checkouts").join(ns);
            git_sync(&loc, &cache)?;
            cache.join(if sub.is_empty() { "docs" } else { sub.as_str() })
        } else {
            let p = Path::new(&loc);
            let base_dir = if p.is_absolute() {
                p.to_path_buf()
            } else {
                root.join(p)
            };
            if sub.is_empty() {
                base_dir
            } else {
                base_dir.join(&sub)
            }
        };
        let provider = DocTree::load(&provider_root)
            .map_err(|e| format!("{ns}: cannot read `{}`: {e}", provider_root.display()))?;
        if !provider.docmeta_present {
            return Err(format!(
                "{ns}: `{}` has no .docmeta.yml — not a docsys tree",
                provider_root.display()
            ));
        }
        let dir = root.join(".federation").join(ns);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        // Manifest first (D-038): the provider's index answers "what changed?"
        // before a single body is read. A provider that publishes none is
        // still consumable — the tree itself is the index then.
        let mut unchanged = 0usize;
        let published: BTreeMap<String, String> =
            std::fs::read_to_string(provider_root.join("manifest.docsys"))
                .ok()
                .map(|text| parse_manifest(&text))
                .transpose()
                .map_err(|e| format!("{ns}: {e}"))?
                .map(|(_, entries)| {
                    entries
                        .into_iter()
                        .filter(|e| !e.withdrawn)
                        .map(|e| (e.id, e.hash))
                        .collect()
                })
                .unwrap_or_default();
        let mut count = 0usize;
        for page in &provider.pages {
            if page.kind != Kind::Permanent {
                continue;
            }
            let Some(fm) = &page.fm else { continue };
            let Some(id) = fm.fields.get("id").and_then(Value::as_str) else {
                continue;
            };
            if fm.fields.get("internal").and_then(Value::as_str) == Some("true") {
                continue; // R-135
            }
            let body = strip_frontmatter(&page.text).replace("\r\n", "\n");
            let body = body.trim_matches('\n');
            // Nothing to do when the published hash equals what we hold: the
            // file keeps its bytes and its fetch date, so nothing downstream
            // re-triggers on an unchanged page (the write_if_changed rule,
            // applied to materialization).
            let held = std::fs::read_to_string(dir.join(format!("{id}.provenance.yml")))
                .ok()
                .and_then(|s| {
                    s.lines()
                        .find_map(|l| l.trim().strip_prefix("hash: ").map(str::to_string))
                });
            let publish_hash = published.get(id);
            let current = format!("fnv:{:016x}", fnv(body.as_bytes()));
            if let (Some(h), Some(p)) = (held.as_deref(), publish_hash) {
                if h == p && h == current && dir.join(format!("{id}.md")).is_file() {
                    unchanged += 1;
                    count += 1;
                    continue;
                }
            }
            let mut head = String::from("---\n");
            let _ = writeln!(head, "id: {id}");
            for k in ["type", "updated", "lang", "audience"] {
                if let Some(v) = fm.fields.get(k).and_then(Value::as_str) {
                    let _ = writeln!(head, "{k}: {v}");
                }
            }
            head.push_str("---\n\n");
            std::fs::write(dir.join(format!("{id}.md")), format!("{head}{body}\n"))
                .map_err(|e| e.to_string())?;
            let sidecar = format!(
                "namespace: {ns}\nid: {id}\nhash: fnv:{:016x}\nfetched: {}\n",
                fnv(body.as_bytes()),
                today()
            );
            std::fs::write(dir.join(format!("{id}.provenance.yml")), sidecar)
                .map_err(|e| e.to_string())?;
            count += 1;
        }
        let note = if published.is_empty() {
            " (no manifest published — read from the tree)".to_string()
        } else if unchanged > 0 {
            format!(" ({unchanged} unchanged, skipped)")
        } else {
            String::new()
        };
        summary.push(format!(
            "{ns}: {count} page(s) under .federation/{ns}/{note}"
        ));
    }
    Ok(summary)
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
    want_audience: Option<&str>,
) -> Result<ProductOutcome, String> {
    let (by_id, flowing) = index_pages(tree);
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    // token → (materialized page, fetched date). Foreign entries compose from
    // the last verified fetch, never a live query (R-136/R-148).
    let mut foreign: BTreeMap<String, (Page, String)> = BTreeMap::new();
    for s in sections {
        for id in &s.entries {
            if id.starts_with('@') {
                if foreign.contains_key(id.as_str()) {
                    continue;
                }
                match load_foreign(tree, id) {
                    Ok(pf) => {
                        if let Some(want) = want_audience {
                            let aud = audience_of(&pf.0);
                            if aud != want {
                                errors.push(format!(
                                    "`{id}` is a `{aud}` page — this document wants `{want}`; \
                                     author a `{want}` page (agent work) or drop the entry"
                                ));
                            }
                        }
                        foreign.insert(id.clone(), pf);
                    }
                    Err(e) => errors.push(e),
                }
            } else if let Some(page) = by_id.get(id.as_str()) {
                // Resolves. An explicitly named page of the wrong audience is
                // refused, not skipped: a document for one reader with another
                // reader's page silently inside is the half-compose R-151
                // forbids.
                if let Some(want) = want_audience {
                    let aud = audience_of(page);
                    if aud != want {
                        errors.push(format!(
                            "`{id}` is a `{aud}` page — this document wants `{want}`; \
                             author a `{want}` page (agent work) or drop the entry"
                        ));
                    }
                }
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
            let (page, fetched): (&Page, Option<&str>) =
                if let Some((p, f)) = foreign.get(id.as_str()) {
                    (p, Some(f.as_str()))
                } else if let Some(p) = by_id.get(id.as_str()) {
                    (p, None)
                } else {
                    continue; // unreachable: the resolve pass above refused already
                };
            let declared = tree.docmeta_list("audiences");
            let aud = audience_of(page);
            if !declared.is_empty() && aud != "developer" && !declared.iter().any(|a| a == aud) {
                warnings.push(format!(
                    "`{id}` declares audience `{aud}`, not in .docmeta.yml `audiences:` — \
                     a misspelling would silently hide the page from its readers"
                ));
            }
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
            let fetched_note = fetched
                .map(|f| format!(" · fetched: {f}"))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "\n<!-- source: doc: {id} · {} · fnv:{:016x} · updated: {updated}{fetched_note} -->",
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
    want_audience: Option<&str>,
) -> Result<ProductOutcome, String> {
    let map_text =
        std::fs::read_to_string(map_path).map_err(|e| format!("{}: {e}", map_path.display()))?;
    let (title, intro, sections) = parse_map(&map_text)?;
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    let note = match want_audience {
        Some(a) => format!("map: {} · audience: {a}", map_path.display()),
        None => format!("map: {}", map_path.display()),
    };
    compose(
        &tree,
        &note,
        Some(title),
        &intro,
        &sections,
        want_lang,
        want_audience,
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
    want_audience: Option<&str>,
) -> Result<ProductOutcome, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    let mut entries: Vec<String> = ids.to_vec();
    let mut gaps: Vec<String> = Vec::new();
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
                // A followed page of another audience is a gap, not an error:
                // the walk found related material this reader cannot use yet.
                if let Some(want) = want_audience {
                    let aud = audience_of(linked);
                    if aud != want {
                        gaps.push(format!(
                            "gap: linked page `{lid}` is `{aud}` — no `{want}` counterpart \
                             exists; authoring one is agent work"
                        ));
                        continue;
                    }
                }
                if !entries.iter().any(|e| e == lid) {
                    entries.push(lid.to_string());
                }
            }
        }
    }
    let note = format!(
        "ids: {}{}{}",
        ids.join(", "),
        if follow { " · follow" } else { "" },
        want_audience
            .map(|a| format!(" · audience: {a}"))
            .unwrap_or_default()
    );
    let sections = [Section {
        title: String::new(),
        entries,
    }];
    let mut done = compose(&tree, &note, title, "", &sections, want_lang, want_audience)
        .map_err(|e| format!("the slice {e}"))?;
    done.warnings.extend(gaps);
    Ok(done)
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
pub fn plan(root: &Path, want_audience: Option<&str>) -> Result<String, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    // type → (identifier as it goes on the map, title, summary)
    let mut groups: BTreeMap<String, Vec<(String, String, String)>> = BTreeMap::new();
    let mut add = |page: &Page, id_on_map: String| {
        if want_audience.is_some_and(|want| audience_of(page) != want) {
            return;
        }
        let ty = page
            .fm
            .as_ref()
            .and_then(|f| f.fields.get("type"))
            .and_then(Value::as_str)
            .unwrap_or("(untyped)")
            .to_string();
        groups
            .entry(ty)
            .or_default()
            .push((id_on_map, title_of(page), summary_of(page)));
    };
    for page in &tree.pages {
        if page.kind != Kind::Permanent {
            continue;
        }
        let Some(id) = page
            .fm
            .as_ref()
            .and_then(|f| f.fields.get("id"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        add(page, id.to_string());
    }
    // Fetched namespaces draft too: an estate repo owns no pages of its own,
    // and its draft must still show everything the estate can compose.
    let fed = tree.root.join(".federation");
    if let Ok(entries) = std::fs::read_dir(&fed) {
        let mut dirs: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        dirs.sort();
        for d in dirs {
            let Some(ns) = d.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !d.is_dir() || ns.starts_with('.') {
                continue;
            }
            let Ok(files) = std::fs::read_dir(&d) else {
                continue;
            };
            let mut files: Vec<_> = files.filter_map(Result::ok).map(|e| e.path()).collect();
            files.sort();
            for f in files {
                if f.extension().is_none_or(|x| x != "md") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&f) else {
                    continue;
                };
                let fm = crate::fm::parse(&text);
                let page = Page {
                    rel: format!(".federation/{ns}"),
                    kind: Kind::Permanent,
                    text,
                    fm,
                };
                let Some(id) = page
                    .fm
                    .as_ref()
                    .and_then(|x| x.fields.get("id"))
                    .and_then(Value::as_str)
                else {
                    continue;
                };
                let id_on_map = format!("@{ns}/{id}");
                add(&page, id_on_map);
            }
        }
    }
    if groups.is_empty() {
        return Err(match want_audience {
            // The whole-tree gap, named: the draft cannot propose pages that
            // were never written.
            Some(want) => format!(
                "no permanent page declares `audience: {want}` — the pages do not exist \
                 yet; authoring them is agent work (undeclared pages read as `developer`, \
                 D-033)"
            ),
            None => "no permanent pages with an id — nothing to draft from".to_string(),
        });
    }
    let mut out = String::new();
    out.push_str(
        "<!-- docsys export plan — a draft product map, not a decision (R-003).\n\
         Rename the title and sections, keep what belongs, delete the rest.\n\
         Targets are doc: identifiers, not paths; keep this file outside the docs root.\n\
         Then: docsys export product <this-file> --root <docs root> -->\n\n",
    );
    out.push_str("# <product name>\n\n<product introduction — one or two sentences>\n");
    for (ty, rows) in &groups {
        let _ = writeln!(out, "\n## {ty}");
        for (id, title, summary) in rows {
            let _ = writeln!(out, "- [[{id}|{title}]] -- {summary}");
        }
    }
    Ok(out)
}
