//! `docsys gate` — the commit-time question, computed by the binary.
//!
//! Field report: `docsys lint` validates the tree's internal consistency and
//! stays green while the actual violation happens — code changes with no
//! documentation change in the same commit. The only thing computing that
//! invariant was a shell script that turned out to be wired to nothing. The
//! binary computes it now; a hook only relays the answer.

use crate::model::Severity;
use std::path::Path;
use std::process::Command;

pub struct GateOutcome {
    pub lint_errors: usize,
    pub lint_warnings: usize,
    /// Changed files outside the docs root ("code moved").
    pub code: Vec<String>,
    /// Changed files inside the docs root.
    pub docs: usize,
    /// Which change set answered: "staged" normally; "working tree" when
    /// nothing is staged yet (`git commit -a` stages at commit time, after
    /// any pre-tool hook has already run).
    pub scope: &'static str,
}

pub fn run(repo: &Path, root: &Path) -> Result<(GateOutcome, crate::checks::Report), String> {
    let (report, _) = crate::lint(root);
    let lint_errors = report
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let lint_warnings = report.findings.len() - lint_errors;
    let repo_canon = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let root_canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let prefix = root_canon
        .strip_prefix(&repo_canon)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let changed = |args: &[&str]| -> Vec<String> {
        // core.quotePath=false: a non-ASCII page name arrives as itself, not
        // as "docs/g\303\274..." — which no docs-root prefix would match, so a
        // real docs change read as a code change (found with a Turkish name).
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
    };
    let mut files = changed(&["diff", "--cached", "--name-only"]);
    let mut scope = "staged";
    if files.is_empty() {
        files = changed(&["diff", "--name-only"]);
        scope = "working tree";
    }
    let mut code = Vec::new();
    let mut docs = 0usize;
    for f in &files {
        let inside = !prefix.is_empty() && (f == &prefix || f.starts_with(&format!("{prefix}/")));
        if inside {
            docs += 1;
        } else {
            code.push(f.clone());
        }
    }
    Ok((
        GateOutcome {
            lint_errors,
            lint_warnings,
            code,
            docs,
            scope,
        },
        report,
    ))
}
