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
    run_scoped(repo, root, None)
}

/// The same question over a commit range (`origin/main...HEAD`): what CI asks
/// of a pull request. No marker, no asking once — the answer is the answer.
pub fn run_range(
    repo: &Path,
    root: &Path,
    range: &str,
) -> Result<(GateOutcome, crate::checks::Report), String> {
    run_scoped(repo, root, Some(range))
}

fn run_scoped(
    repo: &Path,
    root: &Path,
    range: Option<&str>,
) -> Result<(GateOutcome, crate::checks::Report), String> {
    let (report, _) = crate::lint_in(root, Some(repo));
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
    let (files, scope) = match range {
        Some(r) => (changed(&["diff", "--name-only", r]), "range"),
        None => {
            let staged = changed(&["diff", "--cached", "--name-only"]);
            if staged.is_empty() {
                (changed(&["diff", "--name-only"]), "working tree")
            } else {
                (staged, "staged")
            }
        }
    };
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) {
        assert!(Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap()
            .success());
    }

    #[test]
    fn a_non_ascii_docs_page_is_a_docs_change_and_a_non_ascii_source_is_code() {
        let repo = std::env::temp_dir().join(format!("docsys-gate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q"]);
        git(&repo, &["config", "user.email", "t@example.invalid"]);
        git(&repo, &["config", "user.name", "t"]);
        // quotePath ON explicitly — the default the field hit
        git(&repo, &["config", "core.quotePath", "true"]);
        let docs = repo.join("docs");
        crate::migrate::init_profile(&docs, "tr", "project").unwrap();
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "init"]);
        fs::create_dir_all(docs.join("reference")).unwrap();
        fs::write(docs.join("reference/kılavuz.md"), "---\nid: kilavuz\ntype: reference\nupdated: 2026-08-26\n---\nThis page describes the guide; read it when the guide changes.\n").unwrap();
        fs::write(repo.join("çekirdek.rs"), "fn main() {}\n").unwrap();
        git(&repo, &["add", "-A"]);
        let (g, _) = super::run(&repo, &docs).unwrap();
        assert_eq!(g.scope, "staged");
        assert_eq!(g.docs, 1, "the Turkish-named page must count as docs");
        assert_eq!(
            g.code,
            vec!["çekirdek.rs".to_string()],
            "unquoted, as itself"
        );
        // nothing staged: the working tree answers, same rules
        git(&repo, &["commit", "-q", "-m", "both"]);
        fs::write(repo.join("çekirdek.rs"), "fn main() { run() }\n").unwrap();
        let (g, _) = super::run(&repo, &docs).unwrap();
        assert_eq!(g.scope, "working tree");
        assert_eq!((g.docs, g.code.len()), (0, 1));
        let _ = fs::remove_dir_all(&repo);
    }
}
