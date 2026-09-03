//! `docsys inbox` — the write gate a connector uses (§20, EXPERIMENTAL).
//!
//! A connector never classifies and never writes a page: it lands one record
//! in `raw/inbox/` with its provenance — which source, which item there,
//! when, where — and the same item twice lands once (D-079). Ingest does the
//! rest, in a session, with judgment. `pull` is the first connector, built in
//! because docsys already reads git: the commits of a repository since a
//! date, one record each.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::consume::local_id_of;

/// A record's provenance, the fields every connector fills (R-201).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub source: String,
    pub source_id: String,
    pub title: String,
    pub url: Option<String>,
    pub date: String,
}

/// Every record under `raw/` carrying provenance, as (source, source_id, path).
fn records(root: &Path) -> Vec<(String, String, PathBuf)> {
    fn walk(dir: &Path, out: &mut Vec<(String, String, PathBuf)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|e| e == "md") {
                let Ok(text) = fs::read_to_string(&p) else {
                    continue;
                };
                let Some(fm) = crate::fm::parse(&text) else {
                    continue;
                };
                let get = |k: &str| {
                    fm.fields
                        .get(k)
                        .and_then(crate::fm::Value::as_str)
                        .map(str::to_string)
                };
                if let (Some(s), Some(i)) = (get("source"), get("source_id")) {
                    out.push((s, i, p));
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join("raw"), &mut out);
    out
}

/// `docsys inbox add`: one record, or the path of the one already there.
/// The `--since` bound git receives. A bare `YYYY-MM-DD` means "that day at
/// this hour" to git — `--since <today>` would land nothing — so a bare date
/// becomes the start of that day; every other spelling (`30.days`,
/// `2 weeks ago`, a full timestamp) passes through.
pub fn since_bound(since: &str) -> String {
    let s = since.trim();
    let bare = s.len() == 10
        && s.as_bytes().get(4) == Some(&b'-')
        && s.as_bytes().get(7) == Some(&b'-')
        && s.chars()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    if bare {
        format!("{s}T00:00:00")
    } else {
        s.to_string()
    }
}

pub fn add(root: &Path, p: &Provenance, body: &str) -> Result<String, String> {
    if !root.join("raw").is_dir() {
        return Err(format!(
            "`{}` has no raw/ — a knowledge base (`docsys init --profile knowledge-base`) receives records",
            root.display()
        ));
    }
    let source = local_id_of(&p.source);
    if source.is_empty() || p.source_id.trim().is_empty() {
        return Err("a record needs `--source <name>` and `--id <item id at the source>`".into());
    }
    if let Some((_, _, path)) = records(root)
        .into_iter()
        .find(|(s, i, _)| *s == source && *i == p.source_id.trim())
    {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        return Ok(format!("already captured: {rel}"));
    }
    let date = if crate::model::is_iso_date(&p.date) {
        p.date.clone()
    } else {
        return Err(format!("`{}` is not a YYYY-MM-DD date", p.date));
    };
    let slug = {
        let s = local_id_of(&p.title);
        let s = if s.is_empty() {
            local_id_of(&p.source_id)
        } else {
            s
        };
        s.chars().take(60).collect::<String>()
    };
    let slug = slug.trim_end_matches('-').to_string();
    let inbox = root.join("raw").join("inbox");
    fs::create_dir_all(&inbox).map_err(|e| e.to_string())?;
    let mut rel = format!("raw/inbox/{date}-{source}-{slug}.md");
    let mut n = 2;
    while root.join(&rel).exists() {
        rel = format!("raw/inbox/{date}-{source}-{slug}-{n}.md");
        n += 1;
    }
    let mut text = format!(
        "---\nsource: {source}\nsource_id: {}\ntitle: \"{}\"\n",
        p.source_id.trim(),
        p.title.replace('"', "'")
    );
    if let Some(u) = p.url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
        text.push_str(&format!("url: {u}\n"));
    }
    text.push_str(&format!("captured: {date}\n---\n"));
    let body = body.trim_end();
    if !body.is_empty() {
        text.push_str(body);
        text.push('\n');
    }
    fs::write(root.join(&rel), text).map_err(|e| e.to_string())?;
    Ok(format!("captured: {rel}"))
}

/// One commit as a connector sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub sha: String,
    pub date: String,
    pub subject: String,
    pub body: String,
    pub files: Vec<String>,
}

/// The commits of `repo` since `since` (inclusive), newest first; merges and
/// empty subjects skipped.
pub fn commits_since(repo: &Path, since: &str) -> Result<Vec<Commit>, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "-c",
            "core.quotePath=false",
            "log",
            "--no-merges",
            "--name-only",
            &format!("--since={}", since_bound(since)),
            "--format=%x1e%H%x1f%cs%x1f%s%x1f%b%x1f",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "`{}` is not a git repository, or git failed: {}",
            repo.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut commits = Vec::new();
    for rec in text.split('\u{1e}').skip(1) {
        let mut parts = rec.split('\u{1f}');
        let sha = parts.next().unwrap_or("").trim().to_string();
        let date = parts.next().unwrap_or("").trim().to_string();
        let subject = parts.next().unwrap_or("").trim().to_string();
        let body = parts.next().unwrap_or("").trim().to_string();
        let files: Vec<String> = parts
            .next()
            .unwrap_or("")
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        if sha.is_empty() || subject.is_empty() {
            continue;
        }
        commits.push(Commit {
            sha,
            date,
            subject,
            body,
            files,
        });
    }
    Ok(commits)
}

/// Bookkeeping, not knowledge (D-079): no body, and nothing touched outside
/// documentation, manifests and the agent layer — an `updated:` bump, a
/// manifest regeneration, a template refresh. A commit with a body, or one
/// that touched code, is kept whatever it touched.
fn is_bookkeeping(c: &Commit) -> bool {
    c.body.trim().is_empty()
        && !c.files.is_empty()
        && c.files.iter().all(|f| {
            f.ends_with(".md")
                || f.starts_with("docs/")
                || f.starts_with(".claude/")
                || f.ends_with("manifest.docsys")
                || f.ends_with(".docmeta.yml")
        })
}

/// `docsys inbox pull <repo> --since <date> [--limit <n>] [--as <ns>] [--all]`:
/// one record per commit, idempotent by `(git, <ns>@<sha>)`; bookkeeping
/// commits are skipped unless `all`. Returns what landed and what was skipped.
pub fn pull_git(
    root: &Path,
    repo: &Path,
    since: &str,
    ns: Option<&str>,
    limit: Option<usize>,
    all: bool,
) -> Result<Vec<String>, String> {
    let repo_c = repo
        .canonicalize()
        .map_err(|_| format!("`{}` does not exist", repo.display()))?;
    let ns = ns.map(str::to_string).unwrap_or_else(|| {
        local_id_of(
            &repo_c
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        )
    });
    let mut out = Vec::new();
    // newest first; the noise filter runs before `--limit`, so the limit
    // counts commits worth reading
    for c in commits_since(&repo_c, since)?
        .into_iter()
        .filter(|c| all || !is_bookkeeping(c))
        .take(limit.unwrap_or(usize::MAX))
    {
        let short: String = c.sha.chars().take(12).collect();
        let mut body = String::new();
        if !c.body.is_empty() {
            body.push_str(&c.body);
            body.push_str("\n\n");
        }
        body.push_str(&format!("commit {} in {ns}, {}\n", c.sha, c.date));
        if !c.files.is_empty() {
            body.push_str("files:\n");
            for f in &c.files {
                body.push_str(&format!("- {f}\n"));
            }
        }
        body.push_str("\nWhy it is worth keeping: a change in a project this base follows; distil what it decided, not what it did.\n");
        let msg = add(
            root,
            &Provenance {
                source: "git".into(),
                source_id: format!("{ns}@{short}"),
                title: format!("{ns}: {}", c.subject),
                url: None,
                date: c.date.clone(),
            },
            &body,
        )?;
        out.push(msg);
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn base(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("docsys-inbox-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("raw/inbox")).unwrap();
        dir
    }

    #[test]
    fn a_record_lands_once_with_its_provenance() {
        let root = base("once");
        let p = Provenance {
            source: "Calendar".into(),
            source_id: "evt-42".into(),
            title: "Dentist, 09:30".into(),
            url: Some("https://cal.example/evt-42".into()),
            date: "2026-09-02".into(),
        };
        let first = add(&root, &p, "Bring the referral.\n").unwrap();
        assert_eq!(
            first,
            "captured: raw/inbox/2026-09-02-calendar-dentist-09-30.md"
        );
        let text = fs::read_to_string(root.join("raw/inbox/2026-09-02-calendar-dentist-09-30.md"))
            .unwrap();
        assert!(text.starts_with("---\nsource: calendar\nsource_id: evt-42\ntitle: \"Dentist, 09:30\"\nurl: https://cal.example/evt-42\ncaptured: 2026-09-02\n---\nBring the referral.\n"), "{text}");
        let again = add(&root, &p, "different body").unwrap();
        assert_eq!(
            again,
            "already captured: raw/inbox/2026-09-02-calendar-dentist-09-30.md"
        );
        assert_eq!(fs::read_dir(root.join("raw/inbox")).unwrap().count(), 1);
        // same title, another item: a second file, never an overwrite
        let other = Provenance {
            source_id: "evt-43".into(),
            ..p.clone()
        };
        let second = add(&root, &other, "").unwrap();
        assert_eq!(
            second,
            "captured: raw/inbox/2026-09-02-calendar-dentist-09-30-2.md"
        );
        assert!(add(
            &root,
            &Provenance {
                source_id: "".into(),
                ..p.clone()
            },
            ""
        )
        .is_err());
        assert!(add(
            &root,
            &Provenance {
                source_id: "evt-99".into(),
                date: "today".into(),
                ..p
            },
            ""
        )
        .is_err());
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod since_tests {
    use super::since_bound;

    #[test]
    fn a_bare_date_starts_at_midnight_and_other_spellings_pass_through() {
        assert_eq!(since_bound("2026-09-03"), "2026-09-03T00:00:00");
        assert_eq!(since_bound(" 2026-09-03 "), "2026-09-03T00:00:00");
        assert_eq!(since_bound("30.days"), "30.days");
        assert_eq!(since_bound("2026-09-03T12:00:00"), "2026-09-03T12:00:00");
        assert_eq!(since_bound("2 weeks ago"), "2 weeks ago");
    }
}
