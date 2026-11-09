//! Evidence locators — the `sources:` entries that name version-control
//! evidence (D-061): `git:<sha>`, `tag:<ref>`, `git:<sha>:<path>[@L<a>-L<b>]`.
//! One grammar, one resolver, shared by lint (R-059) and by seeding.

use std::path::Path;

use crate::fm::Value;
use crate::model::{Finding, RuleId};

const R059: RuleId = RuleId("R-059");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Locator {
    Commit {
        sha: String,
    },
    Tag {
        name: String,
    },
    Blob {
        sha: String,
        path: String,
        lines: Option<(usize, usize)>,
    },
}

/// Parse a locator; `None` for anything else (a page path, a URL).
pub fn parse(entry: &str) -> Option<Locator> {
    let e = entry.trim();
    if let Some(name) = e.strip_prefix("tag:") {
        let name = name.trim();
        return (!name.is_empty()).then(|| Locator::Tag {
            name: name.to_string(),
        });
    }
    let rest = e.strip_prefix("git:")?;
    let (sha, tail) = match rest.split_once(':') {
        Some((s, t)) => (s, Some(t)),
        None => (rest, None),
    };
    let sha = sha.trim();
    if sha.len() < 7 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let Some(tail) = tail else {
        return Some(Locator::Commit {
            sha: sha.to_string(),
        });
    };
    let (path, lines) = match tail.rsplit_once("@L") {
        Some((p, range)) => {
            let (a, b) = match range.split_once("-L") {
                Some((a, b)) => (a.parse::<usize>().ok()?, b.parse::<usize>().ok()?),
                None => {
                    let a = range.parse::<usize>().ok()?;
                    (a, a)
                }
            };
            if a == 0 || b < a {
                return None;
            }
            (p, Some((a, b)))
        }
        None => (tail, None),
    };
    let path = path.trim();
    (!path.is_empty()).then(|| Locator::Blob {
        sha: sha.to_string(),
        path: path.to_string(),
        lines,
    })
}

fn git_ok(repo: &Path, args: &[&str]) -> bool {
    crate::git::cmd(repo)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The repository a docs root lives in, if any.
pub fn repo_of(root: &Path) -> Option<std::path::PathBuf> {
    crate::git::toplevel(root)
}

/// What a clone that lacks the history says of an entry it cannot check.
pub const SHALLOW: &str =
    "history is shallow here; this entry cannot be checked — check out with fetch-depth: 0";

/// What a clone that lacks a branch commit says of it: a squash or rebase
/// merge leaves the commits a branch cited on no branch.
fn missing_commit(sha: &str) -> String {
    format!(
        "commit {sha} is not in this repository (a squash merge or a rebase drops a branch \
         commit — cite the commit that landed, the pull request URL, or a path)"
    )
}

fn has_commit(repo: &Path, sha: &str) -> bool {
    git_ok(repo, &["cat-file", "-e", &format!("{sha}^{{commit}}")])
}

/// Is `repo` a shallow clone? It holds only the newest commits, so a commit
/// it lacks may exist all the same.
pub fn is_shallow(repo: &Path) -> bool {
    crate::git::cmd(repo)
        .args(["rev-parse", "--is-shallow-repository"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| o.stdout.trim_ascii() == b"true")
}

/// Why `repo` cannot check the locator, or `None` when its history can
/// answer: a commit or a tag that a shallow clone lacks is unknown, never
/// missing.
pub fn unchecked(repo: &Path, loc: &Locator) -> Option<&'static str> {
    let present = match loc {
        Locator::Commit { sha } | Locator::Blob { sha, .. } => has_commit(repo, sha),
        Locator::Tag { name } => git_ok(
            repo,
            &["rev-parse", "-q", "--verify", &format!("refs/tags/{name}")],
        ),
    };
    (!present && is_shallow(repo)).then_some(SHALLOW)
}

/// Does the locator resolve in `repo`? `Err` carries what is missing.
pub fn resolve(repo: &Path, loc: &Locator) -> Result<(), String> {
    match loc {
        Locator::Commit { sha } => {
            if has_commit(repo, sha) {
                Ok(())
            } else {
                Err(missing_commit(sha))
            }
        }
        Locator::Tag { name } => {
            if git_ok(
                repo,
                &["rev-parse", "-q", "--verify", &format!("refs/tags/{name}")],
            ) {
                Ok(())
            } else {
                Err(format!("tag {name} does not exist"))
            }
        }
        Locator::Blob { sha, path, lines } => {
            if !has_commit(repo, sha) {
                return Err(missing_commit(sha));
            }
            let spec = format!("{sha}:{path}");
            let out = crate::git::cmd(repo)
                .args(["cat-file", "-p", &spec])
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(format!("`{path}` does not exist at {sha}"));
            }
            if let Some((a, b)) = lines {
                let n = String::from_utf8_lossy(&out.stdout).lines().count();
                if *b > n {
                    return Err(format!("`{path}` at {sha} has {n} lines, not L{a}-L{b}"));
                }
            }
            Ok(())
        }
    }
}

fn git_text(repo: &Path, args: &[&str]) -> Option<String> {
    crate::git::cmd(repo)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// `sha` is a commit here and no ancestor of `base`. Any other answer — no
/// such commit, no such base — says nothing.
fn only_on_this_branch(repo: &Path, sha: &str, base: &str) -> bool {
    has_commit(repo, sha)
        && crate::git::cmd(repo)
            .args(["merge-base", "--is-ancestor", sha, base])
            .status()
            .is_ok_and(|s| s.code() == Some(1))
}

/// R-059 over a pull request (`gate --range <base>...HEAD`): each `git:`
/// entry the range adds to a page's `sources:` whose commit is not on
/// `base`. It resolves now, and a squash merge leaves it on no branch; a
/// report, because a merge commit keeps it.
pub fn branch_only_citations(repo: &Path, root: &Path, base: &str) -> Vec<Finding> {
    let Ok(tree) = crate::tree::DocTree::load(root) else {
        return Vec::new();
    };
    let prefix = crate::fresh::root_rel(repo, root);
    let mut out = Vec::new();
    for page in &tree.pages {
        if !crate::checks::cites_sources(&tree, page) {
            continue;
        }
        let Some(srcs) = page
            .fm
            .as_ref()
            .and_then(|f| f.fields.get("sources"))
            .and_then(Value::as_list)
        else {
            continue;
        };
        let path = if prefix.is_empty() {
            page.rel.clone()
        } else {
            format!("{prefix}/{}", page.rel)
        };
        let before: Vec<String> = git_text(repo, &["show", &format!("{base}:{path}")])
            .and_then(|t| crate::fm::parse(&t))
            .and_then(|f| {
                f.fields
                    .get("sources")
                    .and_then(Value::as_list)
                    .map(<[String]>::to_vec)
            })
            .unwrap_or_default();
        for s in srcs {
            if before.iter().any(|b| b.trim() == s.trim()) {
                continue;
            }
            let sha = match parse(s) {
                Some(Locator::Commit { sha } | Locator::Blob { sha, .. }) => sha,
                _ => continue,
            };
            if only_on_this_branch(repo, &sha, base) {
                out.push(Finding::warn(
                    R059,
                    &page.rel,
                    s,
                    format!(
                        "sources entry `{s}`: this commit exists only on this branch; a squash \
                         merge drops it — cite the commit that lands, the pull request URL, or a path"
                    ),
                ));
            }
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn grammar() {
        assert_eq!(
            parse("git:deadbeef"),
            Some(Locator::Commit {
                sha: "deadbeef".into()
            })
        );
        assert_eq!(
            parse(" tag:v1.2.0 "),
            Some(Locator::Tag {
                name: "v1.2.0".into()
            })
        );
        assert_eq!(
            parse("git:0123abcd:src/a.rs@L3-L9"),
            Some(Locator::Blob {
                sha: "0123abcd".into(),
                path: "src/a.rs".into(),
                lines: Some((3, 9))
            })
        );
        assert_eq!(
            parse("git:0123abcd:src/a.rs@L7"),
            Some(Locator::Blob {
                sha: "0123abcd".into(),
                path: "src/a.rs".into(),
                lines: Some((7, 7))
            })
        );
        assert_eq!(
            parse("git:0123abcd:docs/x.md"),
            Some(Locator::Blob {
                sha: "0123abcd".into(),
                path: "docs/x.md".into(),
                lines: None
            })
        );
        for bad in [
            "git:",
            "git:xyz",
            "git:abc",
            "git:0123abcd:",
            "git:0123abcd:a@L0-L2",
            "git:0123abcd:a@L5-L2",
            "tag:",
            "reference/x",
            "https://x",
            "",
        ] {
            assert_eq!(parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn resolution_against_a_real_repository() {
        let dir = std::env::temp_dir().join(format!("docsys-locator-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let g = |args: &[&str]| {
            assert!(Command::new("git")
                .args(args)
                .current_dir(&dir)
                .status()
                .unwrap()
                .success())
        };
        g(&["init", "-q"]);
        g(&["config", "user.email", "t@example.invalid"]);
        g(&["config", "user.name", "t"]);
        g(&["config", "tag.gpgsign", "false"]); // the developer's signing setup is not the test's
        std::fs::write(dir.join("a.txt"), "1\n2\n3\n").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-q", "-m", "one"]);
        g(&["tag", "v0.1"]);
        let sha = String::from_utf8(
            Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .current_dir(&dir)
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        assert!(resolve(&dir, &parse(&format!("git:{sha}")).unwrap()).is_ok());
        assert!(resolve(&dir, &parse("tag:v0.1").unwrap()).is_ok());
        assert!(resolve(&dir, &parse(&format!("git:{sha}:a.txt@L1-L3")).unwrap()).is_ok());
        assert!(
            resolve(&dir, &parse(&format!("git:{sha}:a.txt@L2-L9")).unwrap())
                .unwrap_err()
                .contains("has 3 lines")
        );
        assert!(
            resolve(&dir, &parse(&format!("git:{sha}:missing.txt")).unwrap())
                .unwrap_err()
                .contains("does not exist")
        );
        assert!(resolve(&dir, &parse("git:0000000").unwrap())
            .unwrap_err()
            .contains("not in this repository"));
        assert!(resolve(&dir, &parse("tag:v9").unwrap())
            .unwrap_err()
            .contains("does not exist"));
        assert_eq!(
            repo_of(&dir).unwrap().canonicalize().unwrap(),
            dir.canonicalize().unwrap()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
