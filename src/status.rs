//! `docsys status` — what a base or a tree holds right now, derived and never
//! stored (D-080): the inbox, the pages by state, the open items, the consumed
//! namespaces, the compiled skills, and the findings lint would raise. The
//! digest an assistant reads before it says good morning; the tool composes
//! no prose.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::fm::Value;
use crate::items;
use crate::model::Severity;
use crate::tree::{DocTree, Kind, Profile};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Namespace {
    pub name: String,
    pub pages: usize,
    pub fetched: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Status {
    pub profile: String,
    pub namespace: Option<String>,
    pub inbox: usize,
    pub inbox_oldest: Option<String>,
    /// a project's records under `raw/` (D-112), and how many no page cites
    pub records: usize,
    pub records_uncited: usize,
    pub permanent: usize,
    pub unverified: Vec<String>,
    /// tracked work files by status (project profile)
    pub work: BTreeMap<String, usize>,
    pub questions_open: usize,
    pub debt_open: usize,
    pub consumed: Vec<Namespace>,
    pub skills_compiled: usize,
    pub errors: usize,
    pub warnings: usize,
    /// error findings by rule: R-111 stale pins, R-106 updated behind history,
    /// R-085 untouched drafts, R-024 verified drift, R-095 stale skills, …
    pub by_rule: BTreeMap<String, usize>,
    /// verified pages whose consumed sources moved since verification (D-082)
    pub sources_moved: usize,
    /// entries in `.forgotten.yml` (D-084)
    pub forgotten: usize,
    /// verified pages anchored by their block record whose `verified_rev` is not in
    /// this history — a squash or a rebase; the hash is the evidence (D-101)
    pub rev_gone: usize,
    /// a 0.5 tree's acknowledgements whose page id pins nothing any more
    /// (D-119) — `None` in a 0.4 tree, which keeps none
    pub orphan_acks: Option<usize>,
    /// pages whose block record holds only part of the body now — a block
    /// edited, or one whose bound pin is stale — with the blocks found again
    /// and the body's count (R-028, R-212); `None` in a 0.4 tree
    pub partially_verified: Option<Vec<(String, usize, usize)>>,
    pub first_errors: Vec<String>,
}

fn count_open(path: &Path) -> usize {
    fs::read_to_string(path)
        .map(|t| t.lines().filter(|l| l.starts_with("- [ ] ")).count())
        .unwrap_or(0)
}

/// A knowledge base's inbox: the notes waiting, and the oldest by the date
/// its name carries.
fn inbox(root: &Path) -> (usize, Option<String>) {
    let mut dates: Vec<String> = fs::read_dir(root.join("raw/inbox"))
        .map(|it| {
            it.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "md"))
                .map(|p| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().chars().take(10).collect::<String>())
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default();
    let n = dates.len();
    dates.retain(|d| crate::model::is_iso_date(d));
    dates.sort();
    (n, dates.first().cloned())
}

/// A project's records (D-112), forgotten ones aside, and how many of them
/// no page's `sources:` names — root-relative, or repository-relative as
/// R-059 also resolves in a project.
fn records(tree: &DocTree, repo: Option<&Path>) -> (usize, usize) {
    let root_rel = repo
        .and_then(|r| {
            let (root, r) = (tree.root.canonicalize().ok()?, r.canonicalize().ok()?);
            let rel = root
                .strip_prefix(r)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            (!rel.is_empty()).then(|| format!("{rel}/"))
        })
        .unwrap_or_default();
    let cited: BTreeSet<&str> = tree
        .pages
        .iter()
        .filter(|p| p.kind != Kind::Raw)
        .filter_map(|p| p.fm.as_ref()?.fields.get("sources")?.as_list())
        .flatten()
        .map(|e| {
            let e = e.trim().trim_start_matches("./");
            e.strip_prefix(root_rel.as_str()).unwrap_or(e)
        })
        .collect();
    let records: Vec<&str> = tree
        .pages
        .iter()
        .filter(|p| p.kind == Kind::Raw && !p.rel.starts_with("raw/_forgotten/"))
        .map(|p| p.rel.as_str())
        .collect();
    let uncited = records.iter().filter(|r| !cited.contains(*r)).count();
    (records.len(), uncited)
}

pub fn status(root: &Path, repo: Option<&Path>) -> Result<Status, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    let mut s = Status {
        profile: match tree.profile {
            Profile::KnowledgeBase => "knowledge-base".into(),
            Profile::Project => "project".into(),
        },
        namespace: tree.docmeta_str("namespace").map(|n| n.trim().to_string()),
        ..Status::default()
    };
    match tree.profile {
        Profile::KnowledgeBase => (s.inbox, s.inbox_oldest) = inbox(root),
        // a project has no ingest organ: its records wait for nothing (D-112)
        Profile::Project => (s.records, s.records_uncited) = records(&tree, repo),
    }
    for page in &tree.pages {
        let Some(fm) = &page.fm else { continue };
        match page.kind {
            Kind::Permanent => {
                s.permanent += 1;
                if fm.fields.get("verification").and_then(Value::as_str) == Some("unverified") {
                    s.unverified.push(page.rel.clone());
                }
            }
            Kind::Tracked => {
                let st = fm
                    .fields
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("?")
                    .to_string();
                *s.work.entry(st).or_insert(0) += 1;
            }
            _ => {}
        }
    }
    // a docsys/0.5 list is a directory of item files (D-124); a ledger a
    // branch from before the move still wrote counts too, until it moves
    let kb = tree.profile == Profile::KnowledgeBase;
    let open_in = |list: items::List| {
        count_open(&root.join(list.ledger(kb))) + items::open(root, list, kb).len()
    };
    s.questions_open = open_in(items::List::Questions);
    s.debt_open = open_in(items::List::Debt);
    // consumed namespaces: the materializations and when they were fetched
    let fed = root.join(".federation");
    let mut ns_dirs: Vec<_> = fs::read_dir(&fed)
        .map(|it| {
            it.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.is_dir()
                        && !p
                            .file_name()
                            .is_some_and(|n| n.to_string_lossy().starts_with('.'))
                })
                .collect()
        })
        .unwrap_or_default();
    ns_dirs.sort();
    for d in ns_dirs {
        let mut ns = Namespace {
            name: d
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            ..Namespace::default()
        };
        if let Ok(files) = fs::read_dir(&d) {
            for f in files.filter_map(Result::ok).map(|e| e.path()) {
                if f.extension().is_some_and(|x| x == "md") {
                    ns.pages += 1;
                } else if f.to_string_lossy().ends_with(".provenance.yml") {
                    if let Some(date) = fs::read_to_string(&f).ok().and_then(|t| {
                        t.lines()
                            .find_map(|l| l.strip_prefix("fetched:").map(|v| v.trim().to_string()))
                    }) {
                        if ns
                            .fetched
                            .as_deref()
                            .is_none_or(|have| date.as_str() > have)
                        {
                            ns.fetched = Some(date);
                        }
                    }
                }
            }
        }
        s.consumed.push(ns);
    }
    // compiled skills beside the tree
    if let Some(repo) = repo {
        if let Ok(dirs) = fs::read_dir(repo.join(".claude/skills")) {
            for d in dirs.filter_map(Result::ok).map(|e| e.path()) {
                if fs::read_to_string(d.join("SKILL.md"))
                    .is_ok_and(|t| t.contains(crate::compile::SOURCE_KEY))
                {
                    s.skills_compiled += 1;
                }
            }
        }
    }
    s.forgotten = crate::forget::count(root);
    if let Some(repo) = repo {
        for page in tree.pages.iter().filter(|p| p.kind == Kind::Permanent) {
            let Some(fm) = &page.fm else { continue };
            let get = |k: &str| fm.fields.get(k).and_then(Value::as_str);
            if get("verification") != Some("verified") || !fm.fields.contains_key("verified_blocks")
            {
                continue;
            }
            let Some(rev) = get("verified_rev") else {
                continue;
            };
            let held = crate::git::cmd(repo)
                .args(["cat-file", "-e", &format!("{}^{{commit}}", rev.trim())])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|st| st.success());
            if !held {
                s.rev_gone += 1;
            }
        }
    }
    let era = crate::era::Era::of(&tree);
    if era.anchored_verification() {
        let mut partial = Vec::new();
        for page in tree.pages.iter().filter(|p| p.kind == Kind::Permanent) {
            let Some(fm) = &page.fm else { continue };
            if crate::blocks::record_of(fm).is_none() {
                continue;
            }
            let stale = repo
                .map(|r| crate::fresh::stale_blocks(root, r, era, fm))
                .unwrap_or_default();
            if let Some(r) = crate::blocks::reading(fm, &page.text, &stale)
                .filter(crate::blocks::Reading::partial)
            {
                partial.push((page.rel.clone(), r.found, r.of));
            }
        }
        s.partially_verified = Some(partial);
    }
    if era.acknowledged_pins() {
        let pinning: std::collections::BTreeSet<String> = tree
            .pages
            .iter()
            .filter(|p| p.kind == Kind::Permanent)
            .filter_map(|p| p.fm.as_ref())
            .filter(|fm| !crate::fresh::pins_of(fm).is_empty())
            .filter_map(|fm| fm.fields.get("id").and_then(Value::as_str))
            .map(|id| id.trim().to_string())
            .collect();
        s.orphan_acks = Some(
            crate::ack::page_ids(root)
                .iter()
                .filter(|id| !pinning.contains(*id))
                .map(|id| crate::ack::names(root, id).len())
                .sum(),
        );
    }
    // what lint would say, once
    let (report, _) = crate::lint_in(root, repo);
    for f in &report.findings {
        if f.severity == Severity::Error {
            s.errors += 1;
            *s.by_rule.entry(f.rule.0.to_string()).or_insert(0) += 1;
            if f.rule.0 == "R-024" && f.subject.starts_with('@') {
                s.sources_moved += 1;
            }
            if s.first_errors.len() < 5 {
                s.first_errors.push(format!(
                    "{} {} [{}] {}",
                    f.rule, f.file, f.subject, f.message
                ));
            }
        } else {
            s.warnings += 1;
        }
    }
    Ok(s)
}

pub fn render(s: &Status, root: &Path) -> String {
    let mut out = String::new();
    let name = s
        .namespace
        .clone()
        .unwrap_or_else(|| root.display().to_string());
    out.push_str(&format!("{name} ({})\n", s.profile));
    if s.profile == "knowledge-base" {
        match (s.inbox, &s.inbox_oldest) {
            (0, _) => out.push_str("inbox: empty\n"),
            (n, Some(d)) => out.push_str(&format!("inbox: {n} note(s), oldest {d}\n")),
            (n, None) => out.push_str(&format!("inbox: {n} note(s)\n")),
        }
    } else if s.records > 0 {
        out.push_str(&format!(
            "records: {} ({} cited by no page)\n",
            s.records, s.records_uncited
        ));
    }
    if s.profile == "knowledge-base" {
        out.push_str(&format!(
            "wiki: {} page(s), {} unverified{}\n",
            s.permanent,
            s.unverified.len(),
            if s.unverified.is_empty() {
                String::new()
            } else {
                format!(" — {}", s.unverified.join(", "))
            }
        ));
    } else {
        let work: Vec<String> = s.work.iter().map(|(k, v)| format!("{v} {k}")).collect();
        out.push_str(&format!(
            "pages: {} permanent{}; work: {}\n",
            s.permanent,
            if s.unverified.is_empty() {
                String::new()
            } else {
                format!(" ({} unverified)", s.unverified.len())
            },
            if work.is_empty() {
                "none".to_string()
            } else {
                work.join(", ")
            }
        ));
    }
    out.push_str(&format!(
        "open: {} question(s), {} debt item(s)\n",
        s.questions_open, s.debt_open
    ));
    if !s.consumed.is_empty() {
        let list: Vec<String> = s
            .consumed
            .iter()
            .map(|n| {
                format!(
                    "{} {} page(s){}",
                    n.name,
                    n.pages,
                    n.fetched
                        .as_ref()
                        .map(|d| format!(" fetched {d}"))
                        .unwrap_or_default()
                )
            })
            .collect();
        out.push_str(&format!("consumed: {}\n", list.join(" · ")));
    }
    out.push_str(&format!(
        "skills: {} compiled, {} stale\n",
        s.skills_compiled,
        s.by_rule.get("R-095").copied().unwrap_or(0)
    ));
    out.push_str(&format!(
        "freshness: {} stale pin(s), {} updated behind history, {} untouched draft(s), {} verified page(s) whose body moved\n",
        s.by_rule.get("R-111").copied().unwrap_or(0),
        s.by_rule.get("R-106").copied().unwrap_or(0),
        s.by_rule.get("R-085").copied().unwrap_or(0),
        s.by_rule
            .get("R-024")
            .copied()
            .unwrap_or(0)
            .saturating_sub(s.sources_moved)
    ));
    if !s.consumed.is_empty() {
        out.push_str(&format!(
            "sources: {} verified page(s) whose consumed sources moved since verification\n",
            s.sources_moved
        ));
    }
    if s.forgotten > 0 {
        out.push_str(&format!(
            "forgotten: {} (see .forgotten.yml)\n",
            s.forgotten
        ));
    }
    if let Some(n) = s.orphan_acks.filter(|n| *n > 0) {
        out.push_str(&format!(
            "pins: {n} acknowledgement(s) no page pins any more — `docsys pin --gc` removes them\n"
        ));
    }
    if let Some(pages) = s.partially_verified.as_ref().filter(|p| !p.is_empty()) {
        let list: Vec<String> = pages
            .iter()
            .map(|(page, found, of)| format!("{page} {found}/{of}"))
            .collect();
        out.push_str(&format!(
            "blocks: {} page(s) partially verified — {} — `docsys verify --show <page>` lists what to re-read\n",
            pages.len(),
            list.join(", ")
        ));
    }
    if s.rev_gone > 0 {
        out.push_str(&format!(
            "verification: {} verified page(s) anchored by their block record; their revision is not in this history (a squash or a rebase) — nothing to do\n",
            s.rev_gone
        ));
    }
    out.push_str(&format!(
        "lint: {} error(s), {} warning(s)\n",
        s.errors, s.warnings
    ));
    for e in &s.first_errors {
        out.push_str(&format!("  {e}\n"));
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

pub fn render_json(s: &Status) -> String {
    let unverified: Vec<String> = s
        .unverified
        .iter()
        .map(|u| format!("\"{}\"", esc(u)))
        .collect();
    let work: Vec<String> = s
        .work
        .iter()
        .map(|(k, v)| format!("\"{}\":{v}", esc(k)))
        .collect();
    let consumed: Vec<String> = s
        .consumed
        .iter()
        .map(|n| {
            format!(
                "{{\"name\":\"{}\",\"pages\":{},\"fetched\":{}}}",
                esc(&n.name),
                n.pages,
                n.fetched
                    .as_ref()
                    .map_or("null".to_string(), |d| format!("\"{}\"", esc(d)))
            )
        })
        .collect();
    let by_rule: Vec<String> = s
        .by_rule
        .iter()
        .map(|(k, v)| format!("\"{k}\":{v}"))
        .collect();
    let first: Vec<String> = s
        .first_errors
        .iter()
        .map(|e| format!("\"{}\"", esc(e)))
        .collect();
    // a project's records (D-112); a knowledge base's output is unchanged
    let records = if s.profile == "knowledge-base" {
        String::new()
    } else {
        format!(
            ",\"records\":{},\"records_uncited\":{}",
            s.records, s.records_uncited
        )
    };
    let acks = s.orphan_acks.map_or(String::new(), |n| {
        format!(",\"acknowledgements_orphaned\":{n}")
    });
    let partial = s.partially_verified.as_ref().map_or(String::new(), |p| {
        format!(",\"partially_verified\":{}", p.len())
    });
    format!(
        "{{\"profile\":\"{}\",\"namespace\":{},\"inbox\":{},\"inbox_oldest\":{}{records},\"permanent\":{},\"unverified\":[{}],\"work\":{{{}}},\"questions_open\":{},\"debt_open\":{},\"consumed\":[{}],\"skills_compiled\":{},\"errors\":{},\"warnings\":{},\"by_rule\":{{{}}},\"sources_moved\":{},\"forgotten\":{},\"rev_gone\":{}{acks}{partial},\"first_errors\":[{}]}}\n",
        esc(&s.profile),
        s.namespace
            .as_ref()
            .map_or("null".to_string(), |n| format!("\"{}\"", esc(n))),
        s.inbox,
        s.inbox_oldest
            .as_ref()
            .map_or("null".to_string(), |d| format!("\"{}\"", esc(d))),
        s.permanent,
        unverified.join(","),
        work.join(","),
        s.questions_open,
        s.debt_open,
        consumed.join(","),
        s.skills_compiled,
        s.errors,
        s.warnings,
        by_rule.join(","),
        s.sources_moved,
        s.forgotten,
        s.rev_gone,
        first.join(",")
    )
}
