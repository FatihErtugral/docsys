//! `docsys migrate` — the mechanical half of brownfield adoption (SPEC mode
//! setup-migrate). The tool inventories and moves; it never classifies —
//! choosing a target type is judgment and stays with a model or a human
//! (R-003). The plan file is the contract between the two.
//!
//! Plan format (D-017), one row per file:
//!     <current-path> <TAB> <target>
//! where target is `reference` | `howto` | `explanation` | `tutorial` |
//! `archive` | `keep`. `keep` leaves the file untouched (generated docs,
//! assets). Inventory writes the skeleton with `TODO` targets plus comment
//! lines carrying the evidence a classifier needs: first heading, inbound and
//! outbound link counts.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// `[text](url)` → `text`; a derived router sentence must not carry links.
fn strip_md_links(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find('[') {
        let Some(mid) = rest.get(start..).and_then(|x| x.find("](").map(|i| start + i)) else {
            break;
        };
        let Some(end) = rest.get(mid..).and_then(|x| x.find(')').map(|i| mid + i)) else {
            break;
        };
        out.push_str(rest.get(..start).unwrap_or(""));
        out.push_str(rest.get(start + 1..mid).unwrap_or(""));
        rest = rest.get(end + 1..).unwrap_or("");
    }
    out.push_str(rest);
    out
}

/// Civil date from the system clock (days-from-epoch algorithm; zero-dep).
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

fn md_files(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        let mut paths: Vec<PathBuf> =
            entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        for p in paths {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if p.is_dir() {
                if name.starts_with('.') || name.starts_with('_') {
                    continue;
                }
                walk(&p, out);
            } else if name.ends_with(".md") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out);
    out
}

fn rel(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn first_heading(text: &str) -> String {
    text.lines()
        .find_map(|l| l.strip_prefix("# "))
        .or_else(|| text.lines().find_map(|l| l.strip_prefix("## ")))
        .unwrap_or("")
        .trim()
        .to_string()
}

/// Markdown link targets `](target)` on a line, unresolved.
pub fn md_link_targets_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(pos) = rest.find("](") {
        let after = rest.get(pos + 2..).unwrap_or("");
        let Some(end) = after.find(')') else { break };
        let target = after.get(..end).unwrap_or("");
        if !target.starts_with("http") && !target.starts_with('#') {
            out.push(target.split('#').next().unwrap_or("").to_string());
        }
        rest = after.get(end + 1..).unwrap_or("");
    }
    out
}

fn md_link_targets(text: &str) -> Vec<String> {
    text.lines().flat_map(md_link_targets_line).collect()
}

/// Resolve a relative link `target` written in file `from` (both root-relative).
pub fn resolve(from: &str, target: &str) -> Option<String> {
    let base: Vec<&str> = from.split('/').collect();
    let mut parts: Vec<String> = base
        .get(..base.len().saturating_sub(1))
        .unwrap_or(&[])
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?; // escapes the tree
            }
            s => parts.push(s.to_string()),
        }
    }
    Some(parts.join("/"))
}

/// Resolve allowing escapes past the root: returns (ups-beyond-root, path).
fn resolve_with_escape(from: &str, target: &str) -> (usize, String) {
    let base: Vec<&str> = from.split('/').collect();
    let mut parts: Vec<String> = base
        .get(..base.len().saturating_sub(1))
        .unwrap_or(&[])
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let mut ups = 0usize;
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    ups += 1;
                }
            }
            s => parts.push(s.to_string()),
        }
    }
    (ups, parts.join("/"))
}

/// Relative path from `from`'s directory to `to` (both root-relative).
fn relative_link(from: &str, to: &str) -> String {
    let from_dir: Vec<&str> = {
        let v: Vec<&str> = from.split('/').collect();
        v.get(..v.len().saturating_sub(1)).unwrap_or(&[]).to_vec()
    };
    let to_parts: Vec<&str> = to.split('/').collect();
    let mut common = 0usize;
    while common < from_dir.len()
        && common + 1 < to_parts.len()
        && from_dir.get(common) == to_parts.get(common)
    {
        common += 1;
    }
    let ups = from_dir.len() - common;
    let mut out: Vec<String> = std::iter::repeat_n("..".to_string(), ups).collect();
    for p in to_parts.get(common..).unwrap_or(&[]) {
        out.push((*p).to_string());
    }
    out.join("/")
}

/// Kebab-case id from a filename: `AppManifests.md` → `app-manifests` (D-018).
pub fn id_from_filename(name: &str) -> String {
    let stem = name.strip_suffix(".md").unwrap_or(name);
    let mut out = String::new();
    let mut prev_lower = false;
    for c in stem.chars() {
        match c {
            'A'..='Z' => {
                if prev_lower {
                    out.push('-');
                }
                out.push(c.to_ascii_lowercase());
                prev_lower = false;
            }
            'a'..='z' | '0'..='9' => {
                out.push(c);
                prev_lower = c.is_ascii_lowercase();
            }
            _ => {
                if !out.ends_with('-') && !out.is_empty() {
                    out.push('-');
                }
                prev_lower = false;
            }
        }
    }
    out.trim_matches('-').to_string()
}

pub fn inventory(root: &Path) -> Result<String, String> {
    if !root.is_dir() {
        return Err(format!("`{}` is not a directory", root.display()));
    }
    let files = md_files(root);
    if files.is_empty() {
        return Err("no markdown files to inventory".to_string());
    }
    // Inbound counts over the resolved link graph.
    let mut inbound: BTreeMap<String, usize> = BTreeMap::new();
    let mut rows = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let from = rel(path, root);
        let outs = md_link_targets(&text);
        for t in &outs {
            if let Some(r) = resolve(&from, t) {
                *inbound.entry(r).or_insert(0) += 1;
            }
        }
        rows.push((from, first_heading(&text), outs.len()));
    }
    let mut plan = String::from(
        "# docsys migration plan (D-017)\n\
         # Fill the second column: reference | howto | explanation | tutorial | archive | keep\n\
         # Evidence per file: first heading · outbound links · inbound links\n",
    );
    for (from, heading, outs) in rows {
        let inn = inbound.get(&from).copied().unwrap_or(0);
        plan.push_str(&format!("# {from} · \"{heading}\" · out:{outs} in:{inn}\n"));
        plan.push_str(&format!("{from}\tTODO\n"));
    }
    Ok(plan)
}

const TARGETS: [&str; 4] = ["reference", "howto", "explanation", "tutorial"];

pub struct ApplyOutcome {
    pub moved: usize,
    pub kept: usize,
    pub archived: usize,
    pub links_rewritten: usize,
    /// (file, rewrites) for inbound path references fixed across the repo.
    pub repo_rewrites: Vec<(String, usize)>,
    /// Inbound references the rewrite could not map (judgment needed).
    pub repo_risks: Vec<String>,
}

/// Apply a filled plan: move files, write frontmatter, rewrite relative links
/// among the moved set, generate `.docmeta.yml` and a router. Content bodies
/// are never touched beyond the link-target rewrite (R-172's boundary).
pub fn apply(
    root: &Path,
    plan_text: &str,
    lang: &str,
    repo: Option<&Path>,
) -> Result<ApplyOutcome, String> {
    let mut mapping: Vec<(String, String)> = Vec::new(); // old rel → target kind
    for (i, line) in plan_text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((path, target)) = line.split_once('\t') else {
            return Err(format!("plan line {}: not `path<TAB>target`", i + 1));
        };
        let target = target.trim();
        if target == "TODO" {
            return Err(format!("plan line {}: `{path}` is still TODO", i + 1));
        }
        if !TARGETS.contains(&target) && target != "keep" && target != "archive" {
            return Err(format!("plan line {}: unknown target `{target}`", i + 1));
        }
        mapping.push((path.trim().to_string(), target.to_string()));
    }

    // New location per old path.
    let mut new_rel: BTreeMap<String, String> = BTreeMap::new();
    for (old, target) in &mapping {
        let filename = old.rsplit('/').next().unwrap_or(old);
        let new = match target.as_str() {
            "keep" => old.clone(),
            "archive" => format!("_archive/{old}"),
            t => format!("{t}/{filename}"),
        };
        new_rel.insert(old.clone(), new);
    }

    let date = today();
    let mut out = ApplyOutcome {
        moved: 0,
        kept: 0,
        archived: 0,
        links_rewritten: 0,
        repo_rewrites: Vec::new(),
        repo_risks: Vec::new(),
    };
    let mut router_entries: Vec<(String, String, String)> = Vec::new(); // path-noext, title, sentence

    for (old, target) in &mapping {
        let old_path = root.join(old);
        let text = fs::read_to_string(&old_path)
            .map_err(|e| format!("{old}: {e}"))?;
        let Some(new) = new_rel.get(old) else { continue };

        // Rewrite relative links whose resolved target also moved.
        let mut body = String::new();
        for (i, line) in text.lines().enumerate() {
            let mut line_out = line.to_string();
            for t in md_link_targets(line) {
                let (ups, resolved) = resolve_with_escape(old, &t);
                let fresh = if ups == 0 {
                    // In-tree target: follow it to its new home when it moved.
                    match new_rel.get(&resolved) {
                        Some(dest_new) => relative_link(new, dest_new),
                        None => relative_link(new, &resolved),
                    }
                } else {
                    // Out-of-tree target: preserve where it pointed — the
                    // migration must not break what existed (R-172); lint
                    // judges the link on its own merits afterwards.
                    let new_depth = new.matches('/').count();
                    let mut s = "../".repeat(new_depth + ups);
                    s.push_str(&resolved);
                    s
                };
                if fresh != t && !t.is_empty() {
                    line_out = line_out.replace(&format!("]({t})"), &format!("]({fresh})"));
                    out.links_rewritten += 1;
                }
            }
            if i > 0 {
                body.push('\n');
            }
            body.push_str(&line_out);
        }
        if text.ends_with('\n') {
            body.push('\n');
        }

        let is_type_target = TARGETS.contains(&target.as_str());
        let final_text = if is_type_target && !body.starts_with("---\n") {
            let filename = old.rsplit('/').next().unwrap_or(old);
            let id = id_from_filename(filename);
            let title = first_heading(&body);
            let raw_sentence = body
                .lines()
                .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
                .unwrap_or("")
                .trim()
                .to_string();
            let sentence = strip_md_links(&raw_sentence);
            router_entries.push((
                new.trim_end_matches(".md").to_string(),
                if title.is_empty() { id.clone() } else { title },
                sentence,
            ));
            format!("---\nid: {id}\ntype: {target}\nupdated: {date}\n---\n{body}")
        } else {
            body
        };

        let new_path = root.join(new);
        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&new_path, final_text).map_err(|e| e.to_string())?;
        if new != old {
            fs::remove_file(&old_path).map_err(|e| e.to_string())?;
        }
        match target.as_str() {
            "keep" => out.kept += 1,
            "archive" => out.archived += 1,
            _ => out.moved += 1,
        }
    }

    // .docmeta.yml — only when absent; migration never clobbers configuration.
    let docmeta = root.join(".docmeta.yml");
    if !docmeta.exists() {
        fs::write(
            &docmeta,
            format!(
                "spec: docsys/0.4\nprofile: project\ndefault_content_language: {lang}\ncreated: {date}\n"
            ),
        )
        .map_err(|e| e.to_string())?;
    }

    // Router: generated entries are derived (title + first sentence, R-057);
    // appended to an existing router, created otherwise (R-035 append rule).
    let index = root.join("index.md");
    let mut router = fs::read_to_string(&index).unwrap_or_else(|_| "# Documentation\n\n".to_string());
    if !router.ends_with('\n') {
        router.push('\n');
    }
    for (path, title, sentence) in &router_entries {
        let sentence = sentence
            .split(". ")
            .next()
            .unwrap_or(sentence)
            .trim_end_matches('.');
        router.push_str(&format!("- [[{path}|{title}]] -- {sentence}.\n"));
    }
    fs::write(&index, router).map_err(|e| e.to_string())?;

    // work/ skeleton per R-043: journal and debt only.
    let work = root.join("work");
    fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let journal = work.join("journal.md");
    if !journal.exists() {
        fs::write(
            &journal,
            format!("# Journal\n\n## {date} - migrated\n- brownfield tree migrated into the docsys layout\n"),
        )
        .map_err(|e| e.to_string())?;
    }
    let debt = work.join("debt.md");
    if !debt.exists() {
        fs::write(&debt, "# Debt\n").map_err(|e| e.to_string())?;
    }

    if let Some(repo_root) = repo {
        rewrite_repo_references(repo_root, root, &new_rel, &mut out);
    }

    Ok(out)
}

/// `docsys init` — the greenfield skeleton (R-043: journal and debt only).
pub fn init(root: &Path, lang: &str) -> Result<(), String> {
    if root.join(".docmeta.yml").exists() {
        return Err("already initialized (.docmeta.yml exists)".to_string());
    }
    let date = today();
    fs::create_dir_all(root.join("work")).map_err(|e| e.to_string())?;
    fs::write(
        root.join(".docmeta.yml"),
        format!("spec: docsys/0.4\nprofile: project\ndefault_content_language: {lang}\ncreated: {date}\n"),
    )
    .map_err(|e| e.to_string())?;
    fs::write(root.join("index.md"), "# Documentation\n").map_err(|e| e.to_string())?;
    fs::write(
        root.join("work/journal.md"),
        format!("# Journal\n\n## {date} - initialized\n- documentation tree created\n"),
    )
    .map_err(|e| e.to_string())?;
    fs::write(root.join("work/debt.md"), "# Debt\n").map_err(|e| e.to_string())?;
    Ok(())
}

const REPO_SKIP_DIRS: [&str; 6] = [".git", "node_modules", "target", "build", "dist", ".venv"];

fn repo_text_files(repo: &Path, docs_root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, docs_root: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        for p in paths {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if p.is_dir() {
                if REPO_SKIP_DIRS.contains(&name) || p == docs_root {
                    continue;
                }
                walk(&p, docs_root, out);
            } else if fs::metadata(&p).map(|m| m.len() <= 2_000_000).unwrap_or(false) {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(repo, docs_root, &mut out);
    out
}

/// Inbound references from the repo into the docs tree, as inventory evidence
/// (SPEC setup-migrate Phase A: the links that a move would break).
pub fn inbound_report(repo: &Path, docs_root: &Path) -> Vec<(String, usize)> {
    let prefix = rel(docs_root, repo);
    let needle = format!("{prefix}/");
    let mut out = Vec::new();
    for file in repo_text_files(repo, docs_root) {
        let Ok(text) = fs::read_to_string(&file) else { continue };
        let hits = text.matches(&needle).count();
        if hits > 0 {
            out.push((rel(&file, repo), hits));
        }
    }
    out
}

/// Rewrite inbound path references after a migration (Phase C: grep the repo
/// and fix links to moved files). Purely mechanical: exact old→new strings.
pub fn rewrite_repo_references(
    repo: &Path,
    docs_root: &Path,
    moves: &BTreeMap<String, String>,
    out: &mut ApplyOutcome,
) {
    let prefix = rel(docs_root, repo);
    for file in repo_text_files(repo, docs_root) {
        let Ok(text) = fs::read_to_string(&file) else { continue };
        if !text.contains(&format!("{prefix}/")) {
            continue;
        }
        let mut fresh = text.clone();
        let mut count = 0usize;
        for (old, new) in moves {
            if old == new {
                continue;
            }
            let from = format!("{prefix}/{old}");
            let to = format!("{prefix}/{new}");
            let n = fresh.matches(&from).count();
            if n > 0 {
                fresh = fresh.replace(&from, &to);
                count += n;
            }
        }
        let frel = rel(&file, repo);
        if count > 0
            && fs::write(&file, &fresh).is_ok() {
                out.repo_rewrites.push((frel.clone(), count));
            }
        // Leftovers: references into the docs tree that map to nothing now —
        // directory-level links, globs, prose. Judgment, so they are reported.
        for (i, line) in fresh.lines().enumerate() {
            if let Some(pos) = line.find(&format!("{prefix}/")) {
                // A match preceded by `/` or a word character is a segment of
                // some other path or URL (arm.com/documentation/…), not a
                // reference into this tree.
                let preceded = line
                    .get(..pos)
                    .and_then(|s| s.chars().last())
                    .is_some_and(|c| c == '/' || c.is_ascii_alphanumeric());
                if preceded {
                    continue;
                }
                let tail = line.get(pos..).unwrap_or("");
                let token: String = tail
                    .chars()
                    .take_while(|c| !c.is_whitespace() && !"()[]<>\"'`,".contains(*c) || *c == '.')
                    .collect();
                let candidate = token.trim_end_matches('.');
                let rel_in_docs = candidate.strip_prefix(&format!("{prefix}/")).unwrap_or("");
                if !rel_in_docs.is_empty() && !docs_root.join(rel_in_docs).exists() {
                    out.repo_risks.push(format!("{frel}:{}: {candidate}", i + 1));
                }
            }
        }
    }
    out.repo_risks.sort();
    out.repo_risks.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_from_filenames() {
        assert_eq!(id_from_filename("AppManifests.md"), "app-manifests");
        assert_eq!(id_from_filename("OTA.md"), "ota");
        assert_eq!(id_from_filename("fbt.md"), "fbt");
        assert_eq!(id_from_filename("FuriHalBus.md"), "furi-hal-bus");
        assert_eq!(id_from_filename("file_formats.md"), "file-formats");
    }

    #[test]
    fn link_resolution_and_relativization() {
        assert_eq!(resolve("a/b.md", "../x.md").as_deref(), Some("x.md"));
        assert_eq!(resolve("a/b.md", "c.md").as_deref(), Some("a/c.md"));
        assert_eq!(resolve("a.md", "../out.md"), None);
        assert_eq!(relative_link("howto/a.md", "reference/b.md"), "../reference/b.md");
        assert_eq!(relative_link("a.md", "reference/b.md"), "reference/b.md");
        assert_eq!(relative_link("reference/a.md", "reference/b.md"), "b.md");
    }
}

