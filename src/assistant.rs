//! `docsys assistant` — an assistant's memory from one command (D-081): the
//! knowledge base, its agent layer, the projects it learns from, their pages,
//! their recent history as records, and the digest. Nothing here is new
//! mechanics; it is the order in which the existing commands are run, so a
//! person who wants "my own assistant" types one line and reads what it did.
//! Idempotent: run again to pick up new projects and new commits.

use std::fs;
use std::path::{Path, PathBuf};

use crate::consume;

#[derive(Debug, Default)]
pub struct Outcome {
    pub steps: Vec<String>,
    pub consumed: Vec<String>,
    pub records: usize,
}

fn set_domains(root: &Path, domains: &[String]) -> Result<Option<String>, String> {
    let dm = root.join(".docmeta.yml");
    let text = fs::read_to_string(&dm).map_err(|e| e.to_string())?;
    let declared: Vec<&str> = text
        .lines()
        .find_map(|l| l.strip_prefix("domains:"))
        .map(|v| {
            v.trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if !declared.is_empty() {
        return Ok(None);
    }
    let list = if domains.is_empty() {
        vec!["coding".to_string()]
    } else {
        domains.to_vec()
    };
    let line = format!("domains: [{}]", list.join(", "));
    let out = if text.lines().any(|l| l.starts_with("domains:")) {
        text.lines()
            .map(|l| {
                if l.starts_with("domains:") {
                    line.clone()
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    } else {
        format!("{text}{line}\n")
    };
    fs::write(&dm, out).map_err(|e| e.to_string())?;
    Ok(Some(list.join(", ")))
}

/// The local checkout a consume entry names, if it names one.
fn local_path_of(entry: &str) -> Option<PathBuf> {
    let (_, loc) = entry.split_once('=')?;
    let loc = loc.split('#').next().unwrap_or(loc).trim();
    if crate::export::is_git_url(loc) {
        return None;
    }
    Some(PathBuf::from(loc))
}

/// One command: the base, the layer, the domains, the projects, their pages,
/// their recent commits. `projects` are directories holding docsys trees one
/// level down; another knowledge base found there is skipped — a base learns
/// from projects, not from another memory.
pub fn run(
    root: &Path,
    projects: &[PathBuf],
    domains: &[String],
    since: &str,
    limit: Option<usize>,
) -> Result<Outcome, String> {
    let mut out = Outcome::default();
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    // 1 · a repository: the gate and the record layer's immutability need one
    let repo = crate::repo_of(root).unwrap_or_else(|| root.to_path_buf());
    if crate::repo_of(root).is_none() {
        let ok = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(root)
            .status()
            .is_ok_and(|s| s.success());
        out.steps.push(if ok {
            "git: repository initialized (nothing committed — review, then commit)".into()
        } else {
            "git: not available — the gate and the raw/ immutability check need a repository".into()
        });
    }
    let _ = repo;
    // 2 · the base
    if root.join(".docmeta.yml").is_file() {
        let profile = fs::read_to_string(root.join(".docmeta.yml"))
            .unwrap_or_default()
            .lines()
            .find_map(|l| l.strip_prefix("profile:").map(|v| v.trim().to_string()))
            .unwrap_or_default();
        if profile != "knowledge-base" {
            return Err(format!(
                "`{}` is a `{profile}` tree — an assistant's memory is a knowledge base; \
                 point --root at an empty directory or an existing base",
                root.display()
            ));
        }
        out.steps
            .push("base: kept (already a knowledge base)".into());
    } else {
        crate::migrate::init_profile(root, "en", "knowledge-base")?;
        out.steps
            .push("base: created (raw/inbox/, wiki/, .docmeta.yml)".into());
    }
    if let Some(list) = set_domains(root, domains)? {
        out.steps.push(format!(
            "domains: [{list}]{}",
            if domains.is_empty() {
                " — rename with --domains a,b,c"
            } else {
                ""
            }
        ));
    } else {
        out.steps.push("domains: kept".into());
    }
    // 3 · the agent layer: organs, relays, settings, the gate
    let claude = root.join(".claude");
    let installed = crate::agents::install_kb(&claude, root, false)?;
    out.steps.push(format!(
        "agent layer: {} written, {} kept{}",
        installed.written.len(),
        installed.skipped.len(),
        installed
            .notes
            .iter()
            .map(|n| format!("; {n}"))
            .collect::<String>()
    ));
    // 4 · the projects
    for dir in projects {
        let found = consume::discover(root, dir)?;
        if found.is_empty() {
            out.steps
                .push(format!("projects: none one level under {}", dir.display()));
        }
        for c in found {
            if c.profile == "knowledge-base" {
                out.steps.push(format!(
                    "skipped {} — another knowledge base, not a source of contracts",
                    c.ns
                ));
                continue;
            }
            if c.already {
                out.consumed.push(c.ns.clone());
                continue;
            }
            match consume::add(root, &c.path.to_string_lossy(), Some(&c.ns)) {
                Ok(_) => {
                    out.steps
                        .push(format!("consume: {} ← {}", c.ns, c.path.display()));
                    out.consumed.push(c.ns.clone());
                }
                Err(e) => out.steps.push(format!("consume: {} skipped — {e}", c.ns)),
            }
        }
    }
    // 5 · their pages, then their recent history
    let tree = crate::tree::DocTree::load(root).map_err(|e| e.to_string())?;
    let entries: Vec<String> = tree.docmeta_list("consume").to_vec();
    if entries.is_empty() {
        out.steps.push(
            "consume: nothing yet — `docsys consume add <path|git-url>` names a project".into(),
        );
    } else {
        for line in crate::export::fetch(root)? {
            out.steps.push(format!("fetch: {line}"));
        }
        for entry in &entries {
            let ns = entry.split('=').next().unwrap_or(entry).trim().to_string();
            let path = local_path_of(entry)
                .unwrap_or_else(|| root.join(".federation").join(".checkouts").join(&ns));
            if !path.is_dir() {
                continue;
            }
            match crate::inbox::pull_git(root, &path, since, Some(&ns), limit, false) {
                Ok(lines) => {
                    let new = lines.iter().filter(|l| l.starts_with("captured:")).count();
                    out.records += new;
                    out.steps.push(format!(
                        "records: {ns} — {new} new commit record(s), {} already there",
                        lines.len() - new
                    ));
                }
                Err(e) => out.steps.push(format!("records: {ns} skipped — {e}")),
            }
        }
    }
    Ok(out)
}
