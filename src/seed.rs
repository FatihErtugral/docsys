//! `docsys seed plan` — brownfield seeding, first slice (D-053, D-054).
//!
//! A project whose documentation does not exist keeps its knowledge in three
//! places: version-control history, the code's own comment blocks, and the
//! builder's head. This command reads the first two and prints EVIDENCE —
//! never prose — for a conversation in which the builder confirms, corrects
//! and supplies the third. Two modes:
//!
//! * no target: the feature inventory — every candidate feature the history
//!   and the tree name (commit scopes, package manifests, directories), with
//!   its span, its size and whether a page already covers it;
//! * `--target <feature>`: the research for one feature — the commits, files,
//!   births, manifests, comment blocks, citations and tests that carry it —
//!   refused when a page already covers the feature (the system is trusted
//!   to keep it current from there; the seed is for what nothing owns).
//!
//! Everything printed is re-derivable with one git command and names its
//! sha, path or line. The noise is excluded by rule (D-054): merge and mega
//! commits, vendored trees, restricted stores, and a delete-and-restore pair
//! that would otherwise date every file at the day of the accident.

use crate::checks::heading_map;
use crate::fm::Value;
use crate::tree::{DocTree, Kind};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

pub struct Options {
    pub target: Option<String>,
    pub since: Option<String>,
}

/// Files above this count in one commit make it a tree-wide operation
/// (a rename, a vendoring, an accident), not a change to a feature.
pub const MEGA_COMMIT_FILES: usize = 200;
/// Path components that are never evidence for the project's own features.
pub const EXCLUDED_COMPONENTS: [&str; 6] = [
    "vendor",
    "node_modules",
    "third_party",
    "restricted",
    ".git",
    ".federation",
];
/// The word-based "fix" vocabulary the tool knows without a declaration —
/// English conventional-commit types. Anything else counts as work until a
/// vocabulary file maps it (a later slice).
pub const FIX_TYPES: [&str; 4] = ["fix", "hotfix", "bugfix", "revert"];

// ───────────────────────────── git

fn git(repo: &Path, args: &[&str]) -> Vec<String> {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "core.quotePath=false"])
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone)]
pub struct Commit {
    pub sha: String,
    pub date: String,
    pub subject: String,
    pub body: String,
    /// (status letter, path) — `A`dded, `M`odified, `D`eleted, `R`enamed…
    pub files: Vec<(char, String)>,
    pub kind: Option<String>,
    pub scope: Option<String>,
    /// Excluded from feature evidence and dating, with the reason.
    pub excluded: Option<&'static str>,
}

/// `feat(weather)!: subject` → (`feat`, `weather`). Unicode-aware: the type is
/// a run of alphabetic characters, so a repository whose types are not
/// English still counts them (D-053).
pub fn prefix_of(subject: &str) -> (Option<String>, Option<String>) {
    let mut chars = subject.char_indices().peekable();
    let mut end = 0;
    for (i, c) in chars.by_ref() {
        if c.is_alphabetic() {
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        return (None, None);
    }
    let kind = subject.get(..end).unwrap_or("").to_lowercase();
    let rest = subject.get(end..).unwrap_or("");
    let (scope, rest) = match rest.strip_prefix('(') {
        Some(r) => match r.split_once(')') {
            Some((s, after)) => (Some(s.trim().to_lowercase()), after),
            None => return (None, None),
        },
        None => (None, rest),
    };
    let rest = rest.strip_prefix('!').unwrap_or(rest);
    if rest.starts_with(':') {
        (Some(kind), scope.filter(|s| !s.is_empty()))
    } else {
        (None, None)
    }
}

/// `svc_weather`, `weather-app`, `WeatherService` name the feature `weather`:
/// a component carries the name when one of its folded words is the name.
fn component_names(comp: &str, want: &str) -> bool {
    let f = fold(comp);
    f == want || f.split('-').any(|w| w == want)
}

fn excluded_path(p: &str) -> bool {
    p.split('/').any(|c| EXCLUDED_COMPONENTS.contains(&c))
}

/// The history as commits, newest first, merges dropped, each one either
/// evidence or excluded-with-a-reason.
pub fn load_commits(repo: &Path, since: Option<&str>) -> Vec<Commit> {
    let mut args = vec![
        "log",
        "--no-merges",
        "--date=short",
        "--name-status",
        "--format=%x1e%h%x1f%ad%x1f%s%x1f%b%x1f",
    ];
    let since_arg;
    if let Some(s) = since {
        since_arg = format!("--since={s}");
        args.push(&since_arg);
    }
    let text = git(repo, &args).join("\n");
    let tracked_now = git(repo, &["ls-files"]).len().max(1);
    let mut out = Vec::new();
    for rec in text.split('\x1e').skip(1) {
        let mut parts = rec.splitn(5, '\x1f');
        let sha = parts.next().unwrap_or("").trim().to_string();
        let date = parts.next().unwrap_or("").trim().to_string();
        let subject = parts.next().unwrap_or("").trim().to_string();
        let body = parts.next().unwrap_or("").trim().to_string();
        let tail = parts.next().unwrap_or("");
        let mut files = Vec::new();
        for line in tail.lines() {
            let mut it = line.split('\t');
            let status = it.next().unwrap_or("").chars().next().unwrap_or(' ');
            let path = it.next_back().unwrap_or("").to_string();
            if !path.is_empty() && status != ' ' {
                files.push((status, path));
            }
        }
        let (kind, scope) = prefix_of(&subject);
        let deleted = files.iter().filter(|(s, _)| *s == 'D').count();
        let excluded = if files.len() > MEGA_COMMIT_FILES {
            Some("mega-commit")
        } else if deleted * 2 >= tracked_now && deleted > 50 {
            Some("delete-restore hazard")
        } else {
            None
        };
        out.push(Commit {
            sha,
            date,
            subject,
            body,
            files,
            kind,
            scope,
            excluded,
        });
    }
    // The other half of a delete/restore pair: the restore that re-adds what
    // the hazard commit deleted (found live: 3,561 files, one accident).
    let hazards: Vec<usize> = out
        .iter()
        .enumerate()
        .filter(|(_, c)| c.excluded == Some("delete-restore hazard"))
        .map(|(i, _)| i)
        .collect();
    for i in hazards {
        if i > 0 {
            if let Some(prev) = out.get_mut(i - 1) {
                let added = prev.files.iter().filter(|(s, _)| *s == 'A').count();
                if added * 2 >= tracked_now && prev.excluded.is_none() {
                    prev.excluded = Some("delete-restore hazard (restore)");
                }
            }
        }
    }
    out
}

fn is_fix(c: &Commit) -> bool {
    c.kind.as_deref().is_some_and(|k| FIX_TYPES.contains(&k))
        || c.subject.to_lowercase().starts_with("revert")
}

// ───────────────────────────── names and coverage

/// `Weather Screen`, `weather_screen`, `weather-screen` → `weather-screen`.
pub fn fold(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.trim().chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

fn has_word(text: &str, word: &str) -> bool {
    let t = text.to_lowercase();
    let mut start = 0;
    while let Some(pos) = t.get(start..).and_then(|s| s.find(word)) {
        let abs = start + pos;
        let before = t.get(..abs).and_then(|s| s.chars().last());
        let after = t.get(abs + word.len()..).and_then(|s| s.chars().next());
        let b_ok = before.is_none_or(|c| !c.is_alphanumeric());
        let a_ok = after.is_none_or(|c| !c.is_alphanumeric());
        if b_ok && a_ok {
            return true;
        }
        start = abs + word.len().max(1);
    }
    false
}

/// How a page covers a feature name: by identity, by declaration, or by an
/// active research reservation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coverage {
    pub page: String,
    pub how: &'static str,
}

fn title_of(text: &str) -> String {
    text.lines()
        .find_map(|l| l.strip_prefix("# "))
        .unwrap_or("")
        .trim()
        .to_string()
}

/// The page that covers `name`, if any (D-053): `id`/`aliases`/title equal
/// to the folded name; a `covers:` entry `scope:<name>` or a path prefix the
/// feature lives under; or a research file for the name with `status:
/// active` — a reservation another seeding must respect.
pub fn coverage(tree: &DocTree, name: &str, paths: &[String]) -> Option<Coverage> {
    let want = fold(name);
    let mut reservation: Option<Coverage> = None;
    for page in &tree.pages {
        if !matches!(page.kind, Kind::Permanent | Kind::Tracked) {
            continue;
        }
        let Some(fm) = &page.fm else { continue };
        let id = fm.fields.get("id").and_then(Value::as_str).unwrap_or("");
        let status = fm
            .fields
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("");
        let aliases: Vec<String> = fm
            .fields
            .get("aliases")
            .and_then(Value::as_list)
            .map(|l| l.iter().map(|a| fold(a)).collect())
            .unwrap_or_default();
        let by_name =
            fold(id) == want || aliases.contains(&want) || fold(&title_of(&page.text)) == want;
        if by_name && page.kind == Kind::Permanent {
            return Some(Coverage {
                page: page.rel.clone(),
                how: "page id",
            });
        }
        if let Some(list) = fm.fields.get("covers").and_then(Value::as_list) {
            for entry in list {
                let e = entry.trim();
                if let Some(scope) = e.strip_prefix("scope:") {
                    if fold(scope) == want {
                        return Some(Coverage {
                            page: page.rel.clone(),
                            how: "covers: scope",
                        });
                    }
                } else {
                    let prefix = e.trim_end_matches("/**").trim_end_matches('/');
                    if !prefix.is_empty()
                        && paths
                            .iter()
                            .any(|p| p == prefix || p.starts_with(&format!("{prefix}/")))
                    {
                        return Some(Coverage {
                            page: page.rel.clone(),
                            how: "covers: path",
                        });
                    }
                }
            }
        }
        if by_name && page.kind == Kind::Tracked && status == "active" {
            reservation = Some(Coverage {
                page: page.rel.clone(),
                how: "active research (reserved)",
            });
        }
    }
    reservation
}

// ───────────────────────────── inventory

struct Candidate {
    name: String,
    kinds: BTreeSet<&'static str>,
    paths: BTreeSet<String>,
}

fn manifest_name(path: &str, text: &str) -> Option<String> {
    let base = path.rsplit('/').next().unwrap_or(path);
    let key = match base {
        "app.toml" => "slug",
        "Cargo.toml" | "pyproject.toml" => "name",
        "package.json" => "\"name\"",
        _ => return None,
    };
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix(key) {
            let rest = rest.trim_start().trim_start_matches([':', '=']).trim();
            let v = rest
                .trim_matches(|c| c == '"' || c == '\'' || c == ',')
                .trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

const FEATURE_ROOTS: [&str; 9] = [
    "apps",
    "services",
    "components",
    "packages",
    "crates",
    "modules",
    "src",
    "lib",
    "features",
];

fn candidates(repo: &Path, commits: &[Commit]) -> BTreeMap<String, Candidate> {
    let mut out: BTreeMap<String, Candidate> = BTreeMap::new();
    let mut add = |name: String, kind: &'static str, path: Option<String>| {
        let key = fold(&name);
        if key.is_empty() {
            return;
        }
        let c = out.entry(key.clone()).or_insert_with(|| Candidate {
            name: key,
            kinds: BTreeSet::new(),
            paths: BTreeSet::new(),
        });
        c.kinds.insert(kind);
        if let Some(p) = path {
            c.paths.insert(p);
        }
    };
    // scopes with at least two commits
    let mut scope_counts: BTreeMap<String, usize> = BTreeMap::new();
    for c in commits.iter().filter(|c| c.excluded.is_none()) {
        if let Some(s) = &c.scope {
            *scope_counts.entry(s.clone()).or_insert(0) += 1;
        }
    }
    for (s, n) in scope_counts {
        if n >= 2 {
            add(s, "scope", None);
        }
    }
    // manifests and feature directories from the tracked file list
    let files = git(repo, &["ls-files"]);
    for f in &files {
        if excluded_path(f) {
            continue;
        }
        let base = f.rsplit('/').next().unwrap_or(f);
        if matches!(
            base,
            "app.toml" | "Cargo.toml" | "package.json" | "pyproject.toml"
        ) {
            if let Ok(text) = std::fs::read_to_string(repo.join(f)) {
                if let Some(name) = manifest_name(f, &text) {
                    let dir = f.rsplit_once('/').map(|(d, _)| d.to_string());
                    if dir.is_some() {
                        add(name, "manifest", dir);
                    }
                }
            }
        }
        let mut parts = f.split('/');
        if let (Some(top), Some(sub), Some(_)) = (parts.next(), parts.next(), parts.next()) {
            if FEATURE_ROOTS.contains(&top) && !sub.starts_with('.') {
                add(sub.to_string(), "directory", Some(format!("{top}/{sub}")));
            }
        }
    }
    out
}

fn span(commits: &[&Commit]) -> (String, String) {
    let mut dates: Vec<&str> = commits.iter().map(|c| c.date.as_str()).collect();
    dates.sort_unstable();
    (
        dates.first().map_or("", |d| d).to_string(),
        dates.last().map_or("", |d| d).to_string(),
    )
}

fn header(repo: &Path, commits: &[Commit], opts: &Options) -> String {
    let head = git(repo, &["rev-parse", "--short", "HEAD"])
        .into_iter()
        .next()
        .unwrap_or_else(|| "none".into());
    let tags = git(repo, &["tag", "--list"]).len();
    let evidence: Vec<&Commit> = commits.iter().filter(|c| c.excluded.is_none()).collect();
    let (first, last) = span(&evidence);
    let mut s =
        String::from("# docsys seed plan (D-053) — evidence for a conversation, never prose\n");
    s.push_str(&format!("# head: {head}\n"));
    s.push_str(&format!(
        "# history: {} commits ({} excluded by rule) · {first}..{last} · {tags} tag(s){}\n",
        commits.len(),
        commits.len() - evidence.len(),
        opts.since
            .as_deref()
            .map(|d| format!(" · since {d}"))
            .unwrap_or_default()
    ));
    for c in commits.iter().filter(|c| c.excluded.is_some()) {
        s.push_str(&format!(
            "# excluded: {} {} {} files — {}\n",
            c.sha,
            c.date,
            c.files.len(),
            c.excluded.unwrap_or("")
        ));
    }
    // vocabulary
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut prefixed = 0usize;
    for c in &evidence {
        if let Some(k) = &c.kind {
            *kinds.entry(k.clone()).or_insert(0) += 1;
            prefixed += 1;
        }
    }
    let mut kinds: Vec<(String, usize)> = kinds.into_iter().collect();
    kinds.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let list: Vec<String> = kinds
        .iter()
        .take(12)
        .map(|(k, n)| format!("{k}={n}"))
        .collect();
    s.push_str(&format!(
        "# vocab: {prefixed}/{} subjects carry a `type(scope):` prefix · {}\n",
        evidence.len(),
        list.join(" ")
    ));
    let unknown: Vec<&str> = kinds
        .iter()
        .map(|(k, _)| k.as_str())
        .filter(|k| {
            !FIX_TYPES.contains(k)
                && !matches!(
                    *k,
                    "feat"
                        | "docs"
                        | "chore"
                        | "refactor"
                        | "test"
                        | "build"
                        | "ci"
                        | "perf"
                        | "style"
                )
        })
        .collect();
    if !unknown.is_empty() {
        s.push_str(&format!(
            "# vocab-note: types outside the tool's English set are counted as work, not fixes: {} — say which are fixes\n",
            unknown.join(", ")
        ));
    }
    s
}

pub fn inventory(repo: &Path, root: &Path, opts: &Options) -> Result<String, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    let commits = load_commits(repo, opts.since.as_deref());
    if commits.is_empty() {
        return Err("no history to read — is this a git repository with commits?".into());
    }
    let mut out = header(repo, &commits, opts);
    let cands = candidates(repo, &commits);
    out.push_str("#\n# features — name · found by · commits (fix) · span · coverage\n");
    let mut covered = 0usize;
    for cand in cands.values() {
        let mine: Vec<&Commit> = commits
            .iter()
            .filter(|c| c.excluded.is_none())
            .filter(|c| {
                c.scope.as_deref().is_some_and(|s| fold(s) == cand.name)
                    || cand.paths.iter().any(|p| {
                        c.files
                            .iter()
                            .any(|(_, f)| f == p || f.starts_with(&format!("{p}/")))
                    })
            })
            .collect();
        let fixes = mine.iter().filter(|c| is_fix(c)).count();
        let (first, last) = span(&mine);
        let paths: Vec<String> = cand.paths.iter().cloned().collect();
        let cov = coverage(&tree, &cand.name, &paths);
        if cov.is_some() {
            covered += 1;
        }
        let kinds: Vec<&str> = cand.kinds.iter().copied().collect();
        out.push_str(&format!(
            "# feature {} · {} · {} ({}) · {}..{} · {}\n",
            cand.name,
            kinds.join("+"),
            mine.len(),
            fixes,
            first,
            last,
            cov.map_or_else(
                || "uncovered".to_string(),
                |c| format!("covered by {} ({})", c.page, c.how)
            )
        ));
    }
    out.push_str(&format!(
        "#\n# {} candidate feature(s), {} covered, {} uncovered — pick one: `docsys seed plan --target <name>`\n",
        cands.len(),
        covered,
        cands.len() - covered
    ));
    Ok(out)
}

// ───────────────────────────── one feature

fn comment_blocks(text: &str, min_lines: usize) -> Vec<(usize, Vec<String>)> {
    let mut out = Vec::new();
    let mut run: Vec<String> = Vec::new();
    let mut start = 0usize;
    let is_comment = |l: &str| {
        let t = l.trim_start();
        t.starts_with("//")
            || t.starts_with('#')
            || t.starts_with("--")
            || t.starts_with("* ")
            || t.starts_with("/*")
            || t.starts_with(";;")
    };
    for (i, line) in text.lines().enumerate() {
        if is_comment(line) {
            if run.is_empty() {
                start = i + 1;
            }
            run.push(line.trim().to_string());
        } else {
            if run.len() >= min_lines {
                out.push((start, run.clone()));
            }
            run.clear();
        }
    }
    if run.len() >= min_lines {
        out.push((start, run));
    }
    out
}

pub fn target(repo: &Path, root: &Path, name: &str, opts: &Options) -> Result<String, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    let want = fold(name);
    if want.is_empty() {
        return Err("empty target".into());
    }
    let commits = load_commits(repo, opts.since.as_deref());
    // the research: scope, subject word, path component, symbol
    let mut how: BTreeMap<&str, usize> = BTreeMap::new();
    let mut mine: Vec<&Commit> = Vec::new();
    let mut touches = 0usize;
    for c in commits.iter().filter(|c| c.excluded.is_none()) {
        let by_scope = c.scope.as_deref().is_some_and(|s| fold(s) == want);
        let by_subject = has_word(&c.subject, &want) || has_word(&c.subject, name);
        // A path match counts when the feature is what the commit is ABOUT:
        // at least a quarter of its files live under the feature. A commit
        // brushing one file of sixteen apps touches the feature; it is not
        // its history (measured live: store-wide commits listed under one app).
        let under: usize = c
            .files
            .iter()
            .filter(|(_, f)| f.split('/').any(|comp| component_names(comp, &want)))
            .count();
        let by_path = under > 0 && under * 4 >= c.files.len().max(1);
        if under > 0 && !by_path && !by_scope && !by_subject {
            touches += 1;
        }
        if by_scope || by_subject || by_path {
            if by_scope {
                *how.entry("scope").or_insert(0) += 1;
            }
            if by_subject {
                *how.entry("subject").or_insert(0) += 1;
            }
            if by_path {
                *how.entry("path").or_insert(0) += 1;
            }
            mine.push(c);
        }
    }
    let symbol_hits = git(
        repo,
        &["log", "--no-merges", "--format=%h", "-S", name, "--", "."],
    );
    let symbol_hits: Vec<&String> = symbol_hits.iter().take(200).collect();
    // files by touch count
    let mut file_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut file_scopes: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let docs_rel = root
        .canonicalize()
        .ok()
        .zip(repo.canonicalize().ok())
        .and_then(|(r, p)| {
            r.strip_prefix(&p)
                .ok()
                .map(|x| x.to_string_lossy().replace('\\', "/"))
        })
        .unwrap_or_default();
    for c in &mine {
        for (_, f) in &c.files {
            // the documentation tree is the destination, never evidence
            let in_docs =
                !docs_rel.is_empty() && (f == &docs_rel || f.starts_with(&format!("{docs_rel}/")));
            if excluded_path(f) || in_docs {
                continue;
            }
            *file_counts.entry(f.clone()).or_insert(0) += 1;
        }
    }
    for c in commits.iter().filter(|c| c.excluded.is_none()) {
        if let Some(s) = &c.scope {
            for (_, f) in &c.files {
                if file_counts.contains_key(f) {
                    *file_scopes
                        .entry(f.clone())
                        .or_default()
                        .entry(s.clone())
                        .or_insert(0) += 1;
                }
            }
        }
    }
    let paths: Vec<String> = file_counts.keys().cloned().collect();
    let tracked: BTreeSet<String> = git(repo, &["ls-files"]).into_iter().collect();
    if let Some(cov) = coverage(&tree, name, &paths) {
        return Err(format!(
            "`{name}` is already covered by {} ({}) — nothing to seed; the system keeps that page current (`/docsys-sync` names what drifted)",
            cov.page, cov.how
        ));
    }
    let mut out = header(repo, &commits, opts);
    out.push_str(&format!("#\n# feature {want} (asked as `{name}`)\n"));
    if mine.is_empty() && symbol_hits.is_empty() {
        out.push_str("# nothing in history names this feature — ask the builder where it lives (a path, a scope, a symbol)\n");
        return Ok(out);
    }
    let fixes = mine.iter().filter(|c| is_fix(c)).count();
    let reverts = mine
        .iter()
        .filter(|c| c.subject.to_lowercase().starts_with("revert"))
        .count();
    let (first, last) = span(&mine);
    let hows: Vec<String> = how.iter().map(|(k, v)| format!("{k}={v}")).collect();
    out.push_str(&format!(
        "# commits {} (fix {fixes}, revert {reverts}) · {first}..{last} · found by {} · symbol `{name}` in {} commit(s) · {touches} other commit(s) touch a file of it in passing\n",
        mine.len(),
        hows.join(" "),
        symbol_hits.len()
    ));
    // birth: earliest --diff-filter=A among the feature's files (never "last touch")
    let mut births: Vec<(String, String, String)> = Vec::new();
    let mut top_files: Vec<(&String, &usize)> = file_counts.iter().collect();
    top_files.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    // birth from the files that carry the feature's name; every touched file
    // only when none does (a build file touched by the feature is not its birth)
    let own: Vec<&String> = top_files
        .iter()
        .filter(|(f, _)| f.split('/').any(|comp| component_names(comp, &want)))
        .map(|(f, _)| *f)
        .collect();
    let birth_files: Vec<&String> = if own.is_empty() {
        top_files.iter().take(20).map(|(f, _)| *f).collect()
    } else {
        own.into_iter().take(20).collect()
    };
    for f in birth_files {
        let added = git(
            repo,
            &[
                "log",
                "--diff-filter=A",
                "--date=short",
                "--format=%ad %h",
                "--",
                f,
            ],
        );
        if let Some(line) = added.last() {
            let (d, sha) = line.split_once(' ').unwrap_or((line, ""));
            births.push((d.to_string(), sha.to_string(), f.clone()));
        }
    }
    births.sort();
    if let Some((d, sha, f)) = births.first() {
        out.push_str(&format!("# birth {d} {sha} — first file added: {f}\n"));
    }
    for c in mine.iter().take(30) {
        out.push_str(&format!("# commit {} {} {}\n", c.sha, c.date, c.subject));
        for line in c.body.lines().filter(|l| !l.trim().is_empty()).take(3) {
            out.push_str(&format!("#   | {}\n", line.trim()));
        }
    }
    if mine.len() > 30 {
        out.push_str(&format!("# … {} more commit(s)\n", mine.len() - 30));
    }
    for (f, n) in top_files.iter().take(20) {
        let also: Vec<String> = file_scopes
            .get(*f)
            .map(|m| {
                let mut v: Vec<(&String, &usize)> =
                    m.iter().filter(|(s, _)| fold(s) != want).collect();
                v.sort_by(|a, b| b.1.cmp(a.1));
                v.iter().take(3).map(|(s, n)| format!("{s}={n}")).collect()
            })
            .unwrap_or_default();
        let is_test = f.split('/').any(|c| c.contains("test"));
        let gone = !tracked.contains(*f);
        out.push_str(&format!(
            "# file {f} · commits {n}{}{}{}
",
            if gone {
                " · not at HEAD (moved or deleted)"
            } else {
                ""
            },
            if is_test { " · test" } else { "" },
            if also.is_empty() {
                String::new()
            } else {
                format!(" · also under {}", also.join(","))
            }
        ));
    }
    // manifests, comment blocks, citations — the code's own rationale
    let idx = crate::checks::build_index(&tree);
    let mut blocks_shown = 0usize;
    for (f, _) in top_files.iter().take(20) {
        let Ok(text) = std::fs::read_to_string(repo.join(f)) else {
            continue;
        };
        if let Some(mname) = manifest_name(f, &text) {
            out.push_str(&format!("# manifest {f} · name {mname}\n"));
            for line in text
                .lines()
                .filter(|l| {
                    let t = l.trim();
                    !t.is_empty() && !t.starts_with('#') && !t.starts_with("//")
                })
                .take(12)
            {
                out.push_str(&format!("#   | {}\n", line.trim()));
            }
        }
        for line_text in text.lines() {
            for token in crate::checks::doc_tokens_on_line(line_text) {
                let state = match crate::checks::resolve_doc_token(&idx, &token) {
                    Ok(_) => "resolves",
                    Err(crate::checks::DocRefFail::Foreign) => "foreign",
                    Err(_) => "dangling",
                };
                out.push_str(&format!("# cites {f} · doc: {token} · {state}\n"));
            }
        }
        if blocks_shown < 6 {
            for (line_no, block) in comment_blocks(&text, 5) {
                if blocks_shown >= 6 {
                    break;
                }
                blocks_shown += 1;
                out.push_str(&format!(
                    "# comment {f}@L{line_no}-L{} (verbatim)\n",
                    line_no + block.len() - 1
                ));
                for l in block.iter().take(12) {
                    out.push_str(&format!("#   | {l}\n"));
                }
                if block.len() > 12 {
                    out.push_str(&format!("#   | … {} more line(s)\n", block.len() - 12));
                }
            }
        }
    }
    // tags inside the span, dated by the tagged commit
    for line in git(
        repo,
        &["for-each-ref", "refs/tags", "--format=%(refname:short)\t%(*committerdate:short)%(committerdate:short)\t%(creatordate:short)"],
    ) {
        let mut it = line.split('\t');
        let (tag, dated, created) = (it.next().unwrap_or(""), it.next().unwrap_or(""), it.next().unwrap_or(""));
        let d = dated.get(..10).unwrap_or(dated);
        if !first.is_empty() && d >= first.as_str() && d <= last.as_str() {
            out.push_str(&format!("# tag {tag} · {d} (tagged commit) · created {created}\n"));
        }
    }
    out.push_str("#\n# next: /docsys-seed presents this to the builder — confirm, correct, then say what history cannot: why, what is still open, what comes next. Nothing is written before that.\n");
    let _ = heading_map(&tree);
    Ok(out)
}

/// Entry point: inventory without a target, research with one.
pub fn plan(repo: &Path, root: &Path, opts: &Options) -> Result<String, String> {
    match &opts.target {
        Some(t) => target(repo, root, t, opts),
        None => inventory(repo, root, opts),
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    #[test]
    fn conventional_prefixes_in_any_alphabet() {
        assert_eq!(
            prefix_of("feat(weather): x"),
            (Some("feat".into()), Some("weather".into()))
        );
        assert_eq!(prefix_of("fix: x"), (Some("fix".into()), None));
        assert_eq!(
            prefix_of("feat(api)!: breaking"),
            (Some("feat".into()), Some("api".into()))
        );
        assert_eq!(
            prefix_of("düzeltme(ağ): x"),
            (Some("düzeltme".into()), Some("ağ".into()))
        );
        assert_eq!(prefix_of("Merge branch 'x'"), (None, None));
        assert_eq!(prefix_of("no prefix here"), (None, None));
        assert_eq!(prefix_of("feat(unclosed: x"), (None, None));
        assert_eq!(prefix_of(""), (None, None));
    }

    #[test]
    fn names_fold_to_one_form() {
        assert_eq!(fold("Weather Screen"), "weather-screen");
        assert_eq!(fold("weather_screen"), "weather-screen");
        assert_eq!(fold("  Sub-GHz  "), "sub-ghz");
        assert_eq!(fold("Çekirdek"), "çekirdek");
        assert_eq!(fold("---"), "");
    }

    #[test]
    fn subject_words_are_bounded() {
        assert!(has_word("feat: the weather screen", "weather"));
        assert!(!has_word("feat: weatherproofing", "weather"));
        assert!(has_word("weather", "weather"));
        assert!(!has_word("", "weather"));
    }

    #[test]
    fn a_component_names_the_feature_by_one_of_its_words() {
        assert!(component_names("svc_weather", "weather"));
        assert!(component_names("weather-app", "weather"));
        assert!(component_names("WeatherService", "weatherservice"));
        assert!(!component_names("weatherproof", "weather"));
        assert!(!component_names("apps", "weather"));
    }

    #[test]
    fn excluded_components_at_any_depth() {
        assert!(excluded_path("apps/x/vendor/lib.c"));
        assert!(excluded_path("restricted/context"));
        assert!(!excluded_path("apps/vendorish/lib.c"));
    }

    #[test]
    fn manifests_yield_their_names() {
        assert_eq!(
            manifest_name(
                "apps/w/app.toml",
                "[app]\nslug = \"weather\"\nname = \"Weather\"\n"
            )
            .as_deref(),
            Some("weather")
        );
        assert_eq!(
            manifest_name("x/Cargo.toml", "[package]\nname = \"docsys\"\n").as_deref(),
            Some("docsys")
        );
        assert_eq!(
            manifest_name(
                "x/package.json",
                "{\n  \"name\": \"web\",\n  \"version\": \"1\"\n}"
            )
            .as_deref(),
            Some("web")
        );
        assert_eq!(manifest_name("x/README.md", "name = x"), None);
    }

    #[test]
    fn comment_blocks_of_five_or_more_lines() {
        let text =
            "// a\n// b\ncode\n// 1\n// 2\n// 3\n// 4\n// 5\ncode\n# x\n# y\n# z\n# w\n# v\n";
        let got = comment_blocks(text, 5);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, 4);
        assert_eq!(got[0].1.len(), 5);
        assert_eq!(got[1].0, 10);
    }
}

// ───────────────────────────── gaps (machine-readable, for the interview)

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The inventory as JSON — one object per candidate feature with its
/// evidence counts and coverage — for the interview command to pick from.
pub fn gaps_json(repo: &Path, root: &Path, opts: &Options) -> Result<String, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    let commits = load_commits(repo, opts.since.as_deref());
    if commits.is_empty() {
        return Err("no history to read — is this a git repository with commits?".into());
    }
    let head = git(repo, &["rev-parse", "--short", "HEAD"])
        .into_iter()
        .next()
        .unwrap_or_else(|| "none".into());
    let cands = candidates(repo, &commits);
    let mut items = Vec::new();
    for cand in cands.values() {
        let mine: Vec<&Commit> = commits
            .iter()
            .filter(|c| c.excluded.is_none())
            .filter(|c| {
                c.scope.as_deref().is_some_and(|s| fold(s) == cand.name)
                    || cand.paths.iter().any(|p| {
                        c.files
                            .iter()
                            .any(|(_, f)| f == p || f.starts_with(&format!("{p}/")))
                    })
            })
            .collect();
        let fixes = mine.iter().filter(|c| is_fix(c)).count();
        let (first, last) = span(&mine);
        let paths: Vec<String> = cand.paths.iter().cloned().collect();
        let cov = coverage(&tree, &cand.name, &paths);
        let kinds: Vec<String> = cand.kinds.iter().map(|k| json_str(k)).collect();
        let path_list: Vec<String> = paths.iter().map(|p| json_str(p)).collect();
        items.push(format!(
            "  {{\"feature\": {}, \"found_by\": [{}], \"paths\": [{}], \"commits\": {}, \"fixes\": {}, \"first\": {}, \"last\": {}, \"covered_by\": {}, \"how\": {}}}",
            json_str(&cand.name),
            kinds.join(", "),
            path_list.join(", "),
            mine.len(),
            fixes,
            json_str(&first),
            json_str(&last),
            cov.as_ref().map_or("null".to_string(), |c| json_str(&c.page)),
            cov.as_ref().map_or("null".to_string(), |c| json_str(c.how)),
        ));
    }
    Ok(format!(
        "{{\"head\": {}, \"commits\": {}, \"features\": [\n{}\n]}}\n",
        json_str(&head),
        commits.len(),
        items.join(",\n")
    ))
}

// ───────────────────────────── apply (after the builder's approval)

/// One approved row of a seed plan. TAB-separated; `#` lines are evidence
/// and are skipped. Kinds and their columns:
///
/// * `journal <date> <sha> <title>` — a retrospective entry at its own date
///   (R-104) whose body is the provenance line `- git: <sha>`
/// * `research <feature> <sha,...|->` — `work/research/<feature>.md`,
///   `status: active`, `covers: [scope:<feature>]`, `sources:` from the shas,
///   the R-048 headings, empty
/// * `answer <feature> <who> <text>` — the builder's words, verbatim, as a
///   blockquote under `## Learned` of that research file (`\n` in the text
///   is a line break — a row is one line)
/// * `postmortem <slug> <sha>` — `work/postmortems/<slug>.md` quoting the
///   commit subject and body verbatim under `## What happened`
/// * `debt <date> <text>` — a dated open item in `work/debt.md` (the text
///   carries `-- deferred: … -- repay when: …`)
/// * `question <date> <text>` — a dated open item in `work/questions.md`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    Journal {
        date: String,
        sha: String,
        title: String,
    },
    Research {
        feature: String,
        shas: Vec<String>,
    },
    Answer {
        feature: String,
        who: String,
        text: String,
    },
    Postmortem {
        slug: String,
        sha: String,
    },
    Debt {
        date: String,
        text: String,
    },
    Question {
        date: String,
        text: String,
    },
}

#[derive(Debug)]
pub struct Plan {
    pub head: Option<String>,
    pub rows: Vec<Row>,
}

pub fn parse_plan(text: &str) -> Result<Plan, String> {
    let mut head = None;
    let mut rows = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        if let Some(h) = line.strip_prefix("# head:") {
            head = Some(h.trim().to_string());
            continue;
        }
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        let col = |k: usize| cols.get(k).map(|s| s.trim()).unwrap_or("");
        if cols.iter().any(|c| c.trim() == "TODO") {
            return Err(format!("line {n}: a TODO row — fill it or delete it"));
        }
        let row = match col(0) {
            "journal" => {
                if !crate::model::is_iso_date(col(1)) || col(2).is_empty() || col(3).is_empty() {
                    return Err(format!(
                        "line {n}: journal rows are `journal\\t<YYYY-MM-DD>\\t<sha>\\t<title>`"
                    ));
                }
                Row::Journal {
                    date: col(1).into(),
                    sha: col(2).into(),
                    title: col(3).into(),
                }
            }
            "research" => {
                let feature = fold(col(1));
                if feature.is_empty() {
                    return Err(format!(
                        "line {n}: research rows are `research\\t<feature>\\t<sha,...|->`"
                    ));
                }
                let shas = col(2)
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty() && *s != "-")
                    .map(String::from)
                    .collect();
                Row::Research { feature, shas }
            }
            "answer" => {
                let feature = fold(col(1));
                if feature.is_empty() || col(2).is_empty() || col(3).is_empty() {
                    return Err(format!(
                        "line {n}: answer rows are `answer\\t<feature>\\t<who>\\t<text>`"
                    ));
                }
                Row::Answer {
                    feature,
                    who: col(2).into(),
                    text: col(3).replace("\\n", "\n"),
                }
            }
            "postmortem" => {
                let slug = fold(col(1));
                if slug.is_empty() || col(2).is_empty() {
                    return Err(format!(
                        "line {n}: postmortem rows are `postmortem\\t<slug>\\t<sha>`"
                    ));
                }
                Row::Postmortem {
                    slug,
                    sha: col(2).into(),
                }
            }
            "debt" | "question" => {
                if !crate::model::is_iso_date(col(1)) || col(2).is_empty() {
                    return Err(format!(
                        "line {n}: {} rows are `{}\\t<YYYY-MM-DD>\\t<text>`",
                        col(0),
                        col(0)
                    ));
                }
                if col(0) == "debt" {
                    Row::Debt {
                        date: col(1).into(),
                        text: col(2).into(),
                    }
                } else {
                    Row::Question {
                        date: col(1).into(),
                        text: col(2).into(),
                    }
                }
            }
            other => return Err(format!("line {n}: unknown row kind `{other}`")),
        };
        rows.push(row);
    }
    Ok(Plan { head, rows })
}

/// Insert a dated entry into a journal, newest first (R-104): before the
/// first entry older than it, after the preamble; appended when none is.
pub fn insert_journal_entry(journal: &str, date: &str, entry: &str) -> String {
    let lines: Vec<&str> = journal.lines().collect();
    let mut at = None;
    for (i, l) in lines.iter().enumerate() {
        if let Some(rest) = l.strip_prefix("## ") {
            let d = rest.get(..10).unwrap_or("");
            if crate::model::is_iso_date(d) && d < date {
                at = Some(i);
                break;
            }
        }
    }
    let mut out = String::new();
    match at {
        Some(i) => {
            for l in lines.iter().take(i) {
                out.push_str(l);
                out.push('\n');
            }
            out = out.trim_end_matches('\n').to_string();
            out.push_str("\n\n");
            out.push_str(entry.trim_end());
            out.push_str("\n\n");
            for l in lines.iter().skip(i) {
                out.push_str(l);
                out.push('\n');
            }
        }
        None => {
            out.push_str(journal.trim_end_matches('\n'));
            out.push_str("\n\n");
            out.push_str(entry.trim_end());
            out.push('\n');
        }
    }
    out
}

const SEEDED: &str = "seeded: true";

fn frontmatter_line(text: &str, key: &str) -> Option<String> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    rest.get(..end)?
        .lines()
        .find(|l| l.starts_with(&format!("{key}:")))
        .map(str::to_string)
}

/// Land the approved plan (D-058). Writes only under `work/` and the list
/// files; refuses a dirty tree, a stale HEAD pin, a TODO row and a file the
/// tool did not seed. Idempotent: every write carries a token it checks first.
pub fn apply(
    repo: &Path,
    root: &Path,
    plan_path: &Path,
    force: bool,
) -> Result<Vec<String>, String> {
    let text =
        std::fs::read_to_string(plan_path).map_err(|e| format!("{}: {e}", plan_path.display()))?;
    let plan = parse_plan(&text)?;
    if !force {
        // The plan itself may be the one dirty file: it is the input, not a
        // second authoritative copy of anything (R-097's concern).
        let plan_rel = plan_path
            .canonicalize()
            .ok()
            .zip(repo.canonicalize().ok())
            .and_then(|(p, r)| {
                p.strip_prefix(&r)
                    .ok()
                    .map(|x| x.to_string_lossy().replace('\\', "/"))
            })
            .unwrap_or_default();
        let dirty: Vec<String> = git(repo, &["status", "--porcelain"])
            .into_iter()
            .filter(|l| l.get(3..).map(str::trim) != Some(plan_rel.as_str()))
            .collect();
        if !dirty.is_empty() {
            return Err(
                "working tree is dirty — commit or stash first, or pass --force (R-097)".into(),
            );
        }
    }
    let head = git(repo, &["rev-parse", "--short", "HEAD"])
        .into_iter()
        .next()
        .unwrap_or_default();
    if let Some(pin) = plan.head.as_deref().filter(|p| !p.is_empty()) {
        // The evidence has moved when the pinned commit is no longer behind
        // HEAD — a rebase, a reset. Commits on top (the plan's own) are fine.
        let ancestor = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["merge-base", "--is-ancestor", pin, "HEAD"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ancestor {
            return Err(format!(
                "plan was made at {pin}, which is not behind HEAD {head} — re-plan (the evidence may have moved)"
            ));
        }
    }
    let today = crate::migrate::today();
    let pre = crate::migrate::generated_preamble(root);
    let mut done = Vec::new();
    let commit_text = |sha: &str| -> Result<(String, String), String> {
        let lines = git(repo, &["show", "-s", "--format=%s%n%b", sha]);
        if lines.is_empty() {
            return Err(format!("`{sha}` does not resolve to a commit"));
        }
        let subject = lines.first().cloned().unwrap_or_default();
        let body = lines
            .iter()
            .skip(1)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n");
        Ok((subject, body.trim().to_string()))
    };
    for row in &plan.rows {
        match row {
            Row::Journal { date, sha, title } => {
                let path = root.join("work/journal.md");
                let journal =
                    std::fs::read_to_string(&path).unwrap_or_else(|_| "# Journal\n".into());
                let token = format!("- git: {sha}");
                if journal.contains(&token) {
                    done.push(format!("journal: {sha} already present"));
                    continue;
                }
                let entry = format!("## {date} - {title}\n{token}");
                std::fs::write(&path, insert_journal_entry(&journal, date, &entry))
                    .map_err(|e| e.to_string())?;
                done.push(format!("journal: {date} — {title} ({sha})"));
            }
            Row::Research { feature, shas } => {
                let dir = root.join("work/research");
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let path = dir.join(format!("{feature}.md"));
                if path.exists() {
                    let existing = std::fs::read_to_string(&path).unwrap_or_default();
                    if frontmatter_line(&existing, "seeded").is_none() {
                        return Err(format!("work/research/{feature}.md exists and was not seeded by the tool — it is somebody's page; add answers to it by hand or pick another name"));
                    }
                    done.push(format!("research: {feature} already seeded"));
                    continue;
                }
                let sources: Vec<String> = shas.iter().map(|s| format!("git:{s}")).collect();
                let mut text = format!(
                    "---\nid: {feature}\nstatus: active\nupdated: {today}\n{SEEDED}\ncovers: [scope:{feature}]\nsources: [{}]\n---\nThis page holds what the builder said about `{feature}` during seeding, verbatim; read it before writing the permanent page.\n",
                    sources.join(", ")
                );
                for h in ["Question", "Tried", "Learned", "Why no decision"] {
                    text.push_str(&format!("\n## {h}\n"));
                }
                std::fs::write(&path, crate::migrate::with_preamble(&text, &pre))
                    .map_err(|e| e.to_string())?;
                done.push(format!(
                    "research: work/research/{feature}.md (active, reserved)"
                ));
            }
            Row::Answer {
                feature,
                who,
                text: answer,
            } => {
                let path = root.join(format!("work/research/{feature}.md"));
                let existing = std::fs::read_to_string(&path).map_err(|_| format!("answer for `{feature}` but work/research/{feature}.md does not exist — a `research` row must come first"))?;
                let quote: String = answer
                    .lines()
                    .map(|l| format!("> {l}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                let block = format!("{quote}\n> — {who}, {today}\n");
                if existing.contains(&quote) {
                    done.push(format!("answer: already recorded in {feature}"));
                    continue;
                }
                let marker = "\n## Learned\n";
                let new = match existing.find(marker) {
                    Some(pos) => {
                        let (a, b) = existing.split_at(pos + marker.len());
                        // append after whatever is already under Learned, before the next heading
                        let next = b.find("\n## ").map_or(b.len(), |i| i + 1);
                        let (under, rest) = b.split_at(next);
                        let under = under.trim_end_matches('\n');
                        if under.is_empty() {
                            format!("{a}\n{block}\n{rest}")
                        } else {
                            format!("{a}{under}\n\n{block}\n{rest}")
                        }
                    }
                    None => format!("{existing}\n## Learned\n\n{block}"),
                };
                std::fs::write(&path, new).map_err(|e| e.to_string())?;
                done.push(format!("answer: {feature} ← {who}"));
            }
            Row::Postmortem { slug, sha } => {
                let dir = root.join("work/postmortems");
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let path = dir.join(format!("{slug}.md"));
                if path.exists() {
                    done.push(format!("postmortem: {slug} already exists — left alone"));
                    continue;
                }
                let (subject, body) = commit_text(sha)?;
                let mut quote = format!("> {subject}\n");
                for l in body.lines() {
                    quote.push_str(&format!("> {l}\n"));
                }
                quote.push_str(&format!("> — git:{sha}\n"));
                let text = format!(
                    "---\nid: {slug}\nstatus: draft\nupdated: {today}\n{SEEDED}\nsources: [git:{sha}]\n---\nThis page holds a commit's own account of an incident, verbatim; the builder fills the rest.\n\n## What happened\n\n{quote}\n## Root cause\n\n## Recurrence\n\n## Lesson\n"
                );
                std::fs::write(&path, crate::migrate::with_preamble(&text, &pre))
                    .map_err(|e| e.to_string())?;
                done.push(format!("postmortem: work/postmortems/{slug}.md ({sha})"));
            }
            Row::Debt { date, text: item } | Row::Question { date, text: item } => {
                let (file, title) = if matches!(row, Row::Debt { .. }) {
                    ("work/debt.md", "# Debt\n")
                } else {
                    ("work/questions.md", "# Questions\n")
                };
                let path = root.join(file);
                let existing = std::fs::read_to_string(&path).unwrap_or_else(|_| title.to_string());
                let line = format!("- [ ] {date} {item}");
                if existing.lines().any(|l| l.trim() == line) {
                    done.push(format!("{file}: item already present"));
                    continue;
                }
                let mut new = existing.trim_end_matches('\n').to_string();
                new.push('\n');
                if !new.ends_with("\n\n") && !new.lines().last().unwrap_or("").starts_with("- [") {
                    new.push('\n');
                }
                new.push_str(&line);
                new.push('\n');
                std::fs::write(&path, new).map_err(|e| e.to_string())?;
                done.push(format!(
                    "{file}: {date} {}",
                    item.chars().take(60).collect::<String>()
                ));
            }
        }
    }
    Ok(done)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests_apply {
    use super::*;

    #[test]
    fn rows_parse_and_todo_or_unknown_refuse() {
        let p = parse_plan("# head: abc123\n# evidence\njournal\t2026-08-01\tdeadbee\tfirst screen\nresearch\tWeather\tdeadbee,cafe\nanswer\tweather\towner\tit is a requirement\npostmortem\tcaps-stale\tdeadbee\ndebt\t2026-08-02\tx -- deferred: y -- repay when: z\nquestion\t2026-08-03\twhy?\n").unwrap();
        assert_eq!(p.head.as_deref(), Some("abc123"));
        assert_eq!(p.rows.len(), 6);
        assert_eq!(
            p.rows[1],
            Row::Research {
                feature: "weather".into(),
                shas: vec!["deadbee".into(), "cafe".into()]
            }
        );
        assert!(matches!(&p.rows[2], Row::Answer { who, .. } if who == "owner"));
        assert!(parse_plan("journal\t2026-08-01\tTODO\tx\n")
            .unwrap_err()
            .contains("TODO"));
        assert!(parse_plan("bogus\tx\n")
            .unwrap_err()
            .contains("unknown row kind"));
        assert!(parse_plan("journal\tnot-a-date\tsha\tt\n")
            .unwrap_err()
            .contains("journal rows"));
        assert!(parse_plan("research\t-\t-\n")
            .unwrap_err()
            .contains("research rows"));
        assert!(parse_plan("debt\t2026-01-01\t\n")
            .unwrap_err()
            .contains("debt rows"));
        assert!(
            parse_plan("research\tx\t-\n").unwrap().rows[0]
                == Row::Research {
                    feature: "x".into(),
                    shas: vec![]
                }
        );
    }

    #[test]
    fn journal_entries_land_newest_first() {
        let j = "# Journal\n\n## 2026-08-20 - new\n- a\n\n## 2026-08-01 - old\n- b\n";
        let out = insert_journal_entry(j, "2026-08-10", "## 2026-08-10 - mid\n- git: abc");
        let heads: Vec<&str> = out.lines().filter(|l| l.starts_with("## ")).collect();
        assert_eq!(
            heads,
            vec![
                "## 2026-08-20 - new",
                "## 2026-08-10 - mid",
                "## 2026-08-01 - old"
            ]
        );
        let out = insert_journal_entry(j, "2026-07-01", "## 2026-07-01 - oldest\n- git: x");
        assert!(out.trim_end().ends_with("- git: x"));
        let out = insert_journal_entry(j, "2026-08-20", "## 2026-08-20 - same day\n- git: y");
        let heads: Vec<&str> = out.lines().filter(|l| l.starts_with("## ")).collect();
        assert_eq!(heads[0], "## 2026-08-20 - new");
        assert_eq!(heads[1], "## 2026-08-20 - same day");
        let out = insert_journal_entry(
            "# Journal\n",
            "2026-01-01",
            "## 2026-01-01 - only\n- git: z",
        );
        assert_eq!(out, "# Journal\n\n## 2026-01-01 - only\n- git: z\n");
    }

    #[test]
    fn json_strings_escape() {
        assert_eq!(json_str("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
        assert_eq!(json_str("çekirdek"), "\"çekirdek\"");
    }
}
