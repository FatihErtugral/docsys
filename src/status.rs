//! `docsys status` — what a base or a tree holds right now, derived and never
//! stored (D-080): the inbox, the pages by state, the open items, the consumed
//! namespaces, the compiled skills, and the findings lint would raise. The
//! digest an assistant reads before it says good morning; the tool composes
//! no prose.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::fm::Value;
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
    pub first_errors: Vec<String>,
}

fn count_open(path: &Path) -> usize {
    fs::read_to_string(path)
        .map(|t| t.lines().filter(|l| l.starts_with("- [ ] ")).count())
        .unwrap_or(0)
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
    // the inbox: notes waiting, the oldest by the date its name carries
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
    s.inbox = dates.len();
    dates.retain(|d| crate::model::is_iso_date(d));
    dates.sort();
    s.inbox_oldest = dates.first().cloned();
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
    s.questions_open = count_open(&root.join("work/questions.md"))
        + count_open(&root.join("wiki/open-questions.md"));
    s.debt_open = count_open(&root.join("work/debt.md"));
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
    match (s.inbox, &s.inbox_oldest) {
        (0, _) => out.push_str("inbox: empty\n"),
        (n, Some(d)) => out.push_str(&format!("inbox: {n} note(s), oldest {d}\n")),
        (n, None) => out.push_str(&format!("inbox: {n} note(s)\n")),
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
    format!(
        "{{\"profile\":\"{}\",\"namespace\":{},\"inbox\":{},\"inbox_oldest\":{},\"permanent\":{},\"unverified\":[{}],\"work\":{{{}}},\"questions_open\":{},\"debt_open\":{},\"consumed\":[{}],\"skills_compiled\":{},\"errors\":{},\"warnings\":{},\"by_rule\":{{{}}},\"sources_moved\":{},\"forgotten\":{},\"first_errors\":[{}]}}\n",
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
        first.join(",")
    )
}
