//! Documentation tree loader: walks the root, classifies every markdown file,
//! and parses frontmatter once so checks never re-read the disk.

use crate::fm::{self, Frontmatter};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// What a file is, per SPEC §4. Classification is by path, never by content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// reference/ howto/ explanation/ tutorial/ — or wiki/<domain>/<type>/
    Permanent,
    /// work/features/ work/postmortems/ work/research/ + declared categories
    Tracked,
    /// journal.md, debt.md, questions.md, journal slices
    ListFile,
    /// docs/index.md — or wiki/index.md and the domain indexes (D-030)
    Router,
    /// README.md — exempt (R-050)
    Readme,
    /// knowledge-base `raw/` — the content-immutable record layer (R-023)
    Raw,
    /// anything else outside reserved dirs
    Other,
}

/// The declared profile (R-020). A profile selects the layer names; everything
/// else is shared. An unknown or missing value falls back to `project` so the
/// shared checks still run (check_docmeta reports the declaration itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Project,
    KnowledgeBase,
}

#[derive(Debug)]
pub struct Page {
    /// Path relative to the documentation root, with `/` separators.
    pub rel: String,
    pub kind: Kind,
    pub text: String,
    pub fm: Option<Frontmatter>,
}

#[derive(Debug)]
pub struct DocTree {
    pub root: PathBuf,
    pub profile: Profile,
    pub pages: Vec<Page>,
    /// Parsed `.docmeta.yml` fields (empty map when the file is absent).
    pub docmeta: BTreeMap<String, fm::Value>,
    pub docmeta_present: bool,
    pub docmeta_problems: Vec<String>,
    /// Parsed `.tombstones.yml` ids (D-003 registers the v0 ledger format).
    pub tombstones: Vec<String>,
}

/// Reserved directory names skipped by the walk (R-044). `_archive/` is walked
/// for nothing in v0 — a registered narrowing (D-007).
const SKIP_DIRS: [&str; 4] = ["_archive", "_templates", "_unsorted", ".federation"];

/// Tracked-work categories of the core layout (R-040); `.docmeta.yml`
/// `work_categories` extends this list (R-042).
const CORE_TRACKED: [&str; 3] = ["features", "postmortems", "research"];

const PERMANENT_DIRS: [&str; 4] = ["reference", "howto", "explanation", "tutorial"];

/// Knowledge-base layout (R-020 table): `raw/` is the flowing record layer;
/// permanent pages live at `wiki/<domain>/<type>/`; navigation is
/// `wiki/index.md` plus the domain indexes (registered decision D-030).
fn classify_kb(rel: &str) -> Kind {
    let parts: Vec<&str> = rel.split('/').collect();
    match parts.as_slice() {
        ["README.md"] => Kind::Readme,
        ["raw", ..] => Kind::Raw,
        ["wiki", "index.md"] | ["wiki", _, "index.md"] => Kind::Router,
        ["wiki", _, ty, ..] if PERMANENT_DIRS.contains(ty) => Kind::Permanent,
        _ => Kind::Other,
    }
}

fn classify(rel: &str, extra_tracked: &[String], profile: Profile) -> Kind {
    if profile == Profile::KnowledgeBase {
        return classify_kb(rel);
    }
    let mut parts = rel.split('/');
    let first = parts.next().unwrap_or("");
    match first {
        "index.md" => Kind::Router,
        "README.md" => Kind::Readme,
        _ if PERMANENT_DIRS.contains(&first) => Kind::Permanent,
        "work" => {
            let second = parts.next().unwrap_or("");
            match second {
                "journal.md" | "debt.md" | "questions.md" => Kind::ListFile,
                "journal" => Kind::ListFile,
                _ if CORE_TRACKED.contains(&second)
                    || extra_tracked.iter().any(|c| c == second) =>
                {
                    Kind::Tracked
                }
                _ => Kind::Other,
            }
        }
        _ => Kind::Other,
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort(); // deterministic order: findings must not depend on the OS
    for path in paths {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if SKIP_DIRS.contains(&name) || name.starts_with('.') {
                continue;
            }
            walk(&path, out);
        } else if name.ends_with(".md") {
            out.push(path);
        }
    }
}

fn rel_of(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Minimal ledger parser (D-003): entries are `- id: <local-id>` lines; the
/// indented `date:` / `superseded_by:` lines are tolerated and ignored in v0.
fn parse_tombstones(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| l.strip_prefix("- id: "))
        .map(|s| s.trim().to_string())
        .collect()
}

impl DocTree {
    pub fn load(root: &Path) -> std::io::Result<DocTree> {
        let docmeta_path = root.join(".docmeta.yml");
        let (docmeta, docmeta_present, docmeta_problems) = match fs::read_to_string(&docmeta_path) {
            Ok(text) => {
                // .docmeta.yml is pure frontmatter fields without the fences.
                let framed = format!("---\n{text}---\n");
                match fm::parse(&framed) {
                    Some(f) => (f.fields, true, f.problems),
                    None => (BTreeMap::new(), true, vec!["unreadable".to_string()]),
                }
            }
            Err(_) => (BTreeMap::new(), false, Vec::new()),
        };

        let extra_tracked: Vec<String> = docmeta
            .get("work_categories")
            .and_then(fm::Value::as_list)
            .map(<[String]>::to_vec)
            .unwrap_or_default();

        let profile = match docmeta.get("profile").and_then(fm::Value::as_str) {
            Some("knowledge-base") => Profile::KnowledgeBase,
            _ => Profile::Project,
        };

        // R-077's `scan_exclude` is the owner's word on tooling and archived
        // sub-projects; the docs-side walk honors it too (D-030) — a template
        // library inside a knowledge base is not documentation to be linted.
        let excludes: Vec<String> = docmeta
            .get("scan_exclude")
            .and_then(fm::Value::as_list)
            .map(|l| l.iter().filter_map(|e| scan_prefix(e).ok()).collect())
            .unwrap_or_default();

        let tombstones = fs::read_to_string(root.join(".tombstones.yml"))
            .map(|t| parse_tombstones(&t))
            .unwrap_or_default();

        let mut files = Vec::new();
        walk(root, &mut files);

        let mut pages = Vec::new();
        for path in files {
            let rel = rel_of(&path, root);
            if excludes
                .iter()
                .any(|e| !e.is_empty() && rel.starts_with(e.as_str()))
            {
                continue;
            }
            let text = fs::read_to_string(&path)?;
            let kind = classify(&rel, &extra_tracked, profile);
            let fm = fm::parse(&text);
            pages.push(Page {
                rel,
                kind,
                text,
                fm,
            });
        }

        Ok(DocTree {
            root: root.to_path_buf(),
            profile,
            pages,
            docmeta,
            docmeta_present,
            docmeta_problems,
            tombstones,
        })
    }

    pub fn docmeta_str(&self, key: &str) -> Option<&str> {
        self.docmeta.get(key).and_then(fm::Value::as_str)
    }

    pub fn docmeta_list(&self, key: &str) -> &[String] {
        self.docmeta
            .get(key)
            .and_then(fm::Value::as_list)
            .unwrap_or(&[])
    }
}

/// The path prefix a `scan_exclude` entry names (R-077). The common spellings
/// of "this directory" — `spec`, `spec/`, `./spec`, `spec/**` — all reduce to
/// `spec`; what the prefix form cannot express (glob syntax, `..`) is `Err`
/// carrying the entry, so the caller can report it instead of ignoring it.
pub fn scan_prefix(entry: &str) -> Result<String, String> {
    let mut e = entry.trim();
    while let Some(rest) = e.strip_prefix("./") {
        e = rest;
    }
    loop {
        let t = e.trim_end_matches('/');
        let t = t.strip_suffix("/**").unwrap_or(t);
        if t.len() == e.len() {
            break;
        }
        e = t;
    }
    if e.is_empty()
        || e.contains(['*', '?', '['])
        || e == ".."
        || e.starts_with("../")
        || e.contains("/../")
    {
        return Err(entry.to_string());
    }
    Ok(e.to_string())
}

/// Component-boundary prefix test: `spec` excludes `spec` and `spec/x`, never
/// `specification.md`.
pub fn under_prefix(path: &str, prefix: &str) -> bool {
    path == prefix || path.starts_with(&format!("{prefix}/"))
}
