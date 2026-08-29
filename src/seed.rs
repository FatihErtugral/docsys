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
