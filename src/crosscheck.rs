//! `docsys crosscheck` — what a cross-check reads, for the agent that runs it
//! (`/docsys-crosscheck`): each page, its `sources:` and the code its pins
//! resolve to now, by the resolver lint uses (D-106). It writes nothing; the
//! reading, the corrections and the items are the agent's.

use crate::fm::Value;
use crate::hook::Json;
use crate::tree::{DocTree, Kind, Page, Profile};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

/// Where a pin's code is now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Region {
    /// the lines, 1-based and inclusive, of the declaration a symbol names
    Lines(usize, usize),
    /// the whole file, this many lines long
    File(usize),
    /// why it does not resolve, in the resolver's own words
    Refused(String),
}

/// One pin and its region now.
#[derive(Debug)]
pub struct PinNow {
    pub label: String,
    pub path: String,
    pub symbol: Option<String>,
    pub region: Region,
}

/// One page as a cross-check reads it.
#[derive(Debug)]
pub struct Entry {
    /// the page's file, from the repository's top
    pub page: String,
    pub id: Option<String>,
    pub sources: Vec<String>,
    pub pins: Vec<PinNow>,
    /// a frontmatter list that never closes: what the page names is unread
    pub unreadable: Option<String>,
}

/// The pages a cross-check reads, in path order.
#[derive(Debug)]
pub struct Crosscheck {
    pub since: Option<String>,
    pub pages: Vec<Entry>,
}

/// The pages named — by id, or by path from the tree or the repository's top
/// — and, with `since`, every permanent page whose file differs between that
/// revision and the working tree, each read once.
pub fn run(
    root: &Path,
    repo: &Path,
    named: &[String],
    since: Option<&str>,
) -> Result<Crosscheck, String> {
    if named.is_empty() && since.is_none() {
        return Err(
            "name the pages — `docsys crosscheck <page>…` — or a revision: `docsys crosscheck --since <ref>`"
                .into(),
        );
    }
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!(
            "`{}` is no documentation tree (no .docmeta.yml)",
            root.display()
        ));
    }
    if !crate::era::Era::of(&tree).declaration_pins() {
        return Err(format!(
            "this tree {}, and a cross-check resolves pins as docsys/0.5 does — `docsys upgrade` moves the tree first (D-118)",
            crate::era::declared(root)
        ));
    }
    let top = crate::fresh::root_rel(repo, root);
    let mut chosen: BTreeSet<&str> = BTreeSet::new();
    for target in named {
        let page = find(&tree, &top, target)
            .ok_or_else(|| format!("no permanent page at `{target}` and none with that id"))?;
        chosen.insert(page.rel.as_str());
    }
    if let Some(rev) = since {
        let changed = changed_since(root, rev)?;
        chosen.extend(
            tree.pages
                .iter()
                .filter(|p| p.kind == Kind::Permanent && changed.contains(&p.rel))
                .map(|p| p.rel.as_str()),
        );
    }
    let pages = chosen
        .iter()
        .filter_map(|rel| tree.pages.iter().find(|p| p.rel == *rel))
        .map(|p| entry(p, repo, &top))
        .collect();
    Ok(Crosscheck {
        since: since.map(str::to_string),
        pages,
    })
}

fn id_of(page: &Page) -> Option<String> {
    page.fm
        .as_ref()?
        .fields
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn find<'a>(tree: &'a DocTree, top: &str, target: &str) -> Option<&'a Page> {
    let target = target.trim();
    let bare = target.trim_start_matches("./");
    let bare = bare.strip_suffix(".md").unwrap_or(bare);
    let from_top = bare
        .strip_prefix(top)
        .and_then(|r| r.strip_prefix('/'))
        .filter(|_| !top.is_empty());
    tree.pages
        .iter()
        .filter(|p| p.kind == Kind::Permanent)
        .find(|p| {
            let rel = p.rel.strip_suffix(".md").unwrap_or(&p.rel);
            rel == bare
                || Some(rel) == from_top
                || (tree.profile == Profile::KnowledgeBase
                    && rel.strip_prefix("wiki/") == Some(bare))
                || id_of(p).as_deref() == Some(target)
        })
}

/// The files under the tree, relative to it, that differ between `rev` and
/// the working tree: committed, staged, edited, or new and not ignored.
fn changed_since(root: &Path, rev: &str) -> Result<BTreeSet<String>, String> {
    if crate::git::toplevel(root).is_none() {
        return Err("--since reads git history, and the tree is in no git repository".into());
    }
    let git = |args: &[&str]| {
        crate::git::cmd(root)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    // a revision is never read as an option
    let commit = (!rev.starts_with('-'))
        .then(|| {
            git(&[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{rev}^{{commit}}"),
            ])
        })
        .flatten()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .ok_or_else(|| format!("`{rev}` names no commit of this repository"))?;
    let diff = git(&[
        "diff",
        "--name-only",
        "-z",
        "--relative",
        &commit,
        "--",
        ".",
    ])
    .ok_or_else(|| format!("git cannot compare the working tree with `{rev}`"))?;
    let new = git(&[
        "ls-files",
        "--others",
        "--exclude-standard",
        "-z",
        "--",
        ".",
    ])
    .ok_or("git cannot list the files it does not track")?;
    Ok(diff
        .split('\0')
        .chain(new.split('\0'))
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect())
}

fn entry(page: &Page, repo: &Path, top: &str) -> Entry {
    let mut e = Entry {
        page: if top.is_empty() {
            page.rel.clone()
        } else {
            format!("{top}/{}", page.rel)
        },
        id: id_of(page),
        sources: Vec::new(),
        pins: Vec::new(),
        unreadable: None,
    };
    let Some(fm) = &page.fm else { return e };
    if let Some((key, line)) = &fm.unclosed {
        e.unreadable = Some(crate::fm::unclosed_refusal(key, *line));
        return e;
    }
    e.sources = fm
        .fields
        .get("sources")
        .and_then(Value::as_list)
        .map(<[String]>::to_vec)
        .unwrap_or_default();
    e.pins = crate::fresh::pins_of(fm)
        .into_iter()
        .map(|p| PinNow {
            region: region(repo, &p.path, p.symbol.as_deref()),
            label: p.label(),
            path: p.path,
            symbol: p.symbol,
        })
        .collect();
    e
}

/// The region a pin covers now: as lint reads it, a symbol's declaration or
/// the whole file; never a guess (R-114).
fn region(repo: &Path, path: &str, symbol: Option<&str>) -> Region {
    let file = repo.join(path);
    let source = match std::fs::read_to_string(&file) {
        Ok(s) => s,
        Err(_) if !file.is_file() => return Region::Refused(format!("`{path}` does not exist")),
        Err(e) => return Region::Refused(format!("`{path}` cannot be read: {e}")),
    };
    match symbol.map(str::trim).filter(|s| !s.is_empty()) {
        None => Region::File(source.lines().count()),
        Some(sym) => match crate::symbols::resolve(&source, path, sym) {
            Ok((first, last)) => Region::Lines(first, last),
            Err(e) => Region::Refused(e),
        },
    }
}

impl Region {
    fn shown(&self, path: &str) -> String {
        match self {
            Region::Lines(first, last) => format!("{path}:L{first}-L{last}"),
            Region::File(0) => format!("{path} (the whole file, empty)"),
            Region::File(n) => format!("{path}:L1-L{n} (the whole file)"),
            Region::Refused(why) => format!("cannot resolve: {why}"),
        }
    }
}

impl Crosscheck {
    /// The text form: each page, its sources, its pins and their regions.
    pub fn render(&self) -> String {
        let mut out = String::new();
        if let (true, Some(rev)) = (self.pages.is_empty(), &self.since) {
            let _ = writeln!(out, "no permanent page changed since `{rev}`");
        }
        for e in &self.pages {
            let id =
                e.id.as_ref()
                    .map(|i| format!(" (id: {i})"))
                    .unwrap_or_default();
            let _ = writeln!(out, "{}{id}", e.page);
            if let Some(why) = &e.unreadable {
                let _ = writeln!(out, "  {why}");
            } else {
                if e.sources.is_empty() {
                    out.push_str("  no `sources:`\n");
                }
                for s in &e.sources {
                    let _ = writeln!(out, "  source: {s}");
                }
                if e.pins.is_empty() {
                    out.push_str("  no pins\n");
                }
                for p in &e.pins {
                    let _ = writeln!(out, "  pin: {} -> {}", p.label, p.region.shown(&p.path));
                }
            }
            out.push('\n');
        }
        let _ = writeln!(out, "-- {} page(s); nothing written", self.pages.len());
        out
    }

    /// The same as data.
    pub fn to_json(&self) -> String {
        let s = |v: &str| Json::Str(v.to_string());
        let n = |v: usize| Json::Num(v.to_string());
        let or_null = |v: Option<&str>| v.map_or(Json::Null, s);
        let pages = self
            .pages
            .iter()
            .map(|e| {
                let mut fields = vec![
                    ("page".to_string(), s(&e.page)),
                    ("id".to_string(), or_null(e.id.as_deref())),
                ];
                if let Some(why) = &e.unreadable {
                    fields.push(("unreadable".to_string(), s(why)));
                }
                fields.push((
                    "sources".to_string(),
                    Json::Arr(e.sources.iter().map(|v| s(v)).collect()),
                ));
                let pins = e
                    .pins
                    .iter()
                    .map(|p| {
                        let mut f = vec![
                            ("pin".to_string(), s(&p.label)),
                            ("path".to_string(), s(&p.path)),
                            ("symbol".to_string(), or_null(p.symbol.as_deref())),
                        ];
                        match &p.region {
                            Region::Lines(first, last) => {
                                f.push(("first".to_string(), n(*first)));
                                f.push(("last".to_string(), n(*last)));
                            }
                            Region::File(lines) => {
                                f.push(("first".to_string(), n(1)));
                                f.push(("last".to_string(), n(*lines)));
                            }
                            Region::Refused(why) => f.push(("refusal".to_string(), s(why))),
                        }
                        Json::Obj(f)
                    })
                    .collect();
                fields.push(("pins".to_string(), Json::Arr(pins)));
                Json::Obj(fields)
            })
            .collect();
        Json::Obj(vec![
            ("since".to_string(), or_null(self.since.as_deref())),
            ("pages".to_string(), Json::Arr(pages)),
        ])
        .render()
    }
}
