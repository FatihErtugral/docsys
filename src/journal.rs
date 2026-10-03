//! The journal is version-control history (§10, D-125): a commit that changes
//! the documentation tree, or carries a `Docs:` trailer, is an entry. Nothing
//! is written beside history; the journal files a docsys/0.4 tree kept stay
//! readable, frozen under `_archive/journal/`.

use std::path::Path;

/// The trailer that makes a commit that changed no page a journal entry, and
/// answers `commit_policy: require` (R-209).
pub const DOCS: &str = "Docs";

/// One entry: the commit's date, its subject and its body lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub date: String,
    pub title: String,
    pub lines: Vec<String>,
}

/// Whether a commit message carries `<key>: …` on a line of its own after
/// the subject. Read leniently: a squash merge stacks the messages of every
/// commit it carries, trailers included.
pub fn has_trailer(message: &str, key: &str) -> bool {
    let prefix = format!("{key}:");
    message
        .lines()
        .skip(1)
        .any(|l| l.trim_start().starts_with(&prefix))
}

/// The commits `git log` lists with `args`: (committer time, date, hash,
/// message), newest first.
fn log(repo: &Path, args: &[&str]) -> Option<Vec<(i64, String, String, String)>> {
    let out = crate::git::cmd(repo)
        .args([
            "log",
            "--no-merges",
            "-z",
            "--format=%ct%x1f%cs%x1f%H%x1f%B",
        ])
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let text = String::from_utf8_lossy(&out.stdout);
    Some(
        text.split('\0')
            .filter_map(|rec| {
                let mut f = rec.trim_start_matches('\n').splitn(4, '\u{1f}');
                let time = f.next()?.trim().parse().ok()?;
                let date = f.next()?.to_string();
                let hash = f.next()?.to_string();
                let msg = f.next().unwrap_or("").to_string();
                Some((time, date, hash, msg))
            })
            .collect(),
    )
}

/// The entries history holds, newest first; `None` outside a repository.
pub fn entries(repo: &Path, root: &Path, since: Option<&str>) -> Option<Vec<Entry>> {
    let scope = match crate::fresh::root_rel(repo, root) {
        r if r.is_empty() => ".".to_string(),
        r => r,
    };
    let mut all = log(repo, &["--", &scope])?;
    let grep = format!("--grep=^{DOCS}:");
    for c in log(repo, &[&grep]).unwrap_or_default() {
        if has_trailer(&c.3, DOCS) && !all.iter().any(|a| a.2 == c.2) {
            all.push(c);
        }
    }
    all.sort_by_key(|c| std::cmp::Reverse(c.0));
    Some(
        all.into_iter()
            .filter(|c| since.is_none_or(|s| c.1.as_str() >= s))
            .map(|(_, date, _, msg)| {
                let mut lines = msg.lines();
                let title = lines.next().unwrap_or("").trim().to_string();
                Entry {
                    date,
                    title,
                    lines: lines
                        .map(str::trim_end)
                        .filter(|l| !l.trim().is_empty())
                        .map(str::to_string)
                        .collect(),
                }
            })
            .collect(),
    )
}

/// The frozen journal files, newest first: the active file as the move left
/// it, then the slices by name. Root-relative paths and their text.
pub fn frozen(root: &Path) -> Vec<(String, String)> {
    let dir = root.join("_archive/journal");
    let Ok(read) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = read
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| n.ends_with(".md"))
        .collect();
    // a slice is named by its dates; the active file and its re-runs are not
    let slice = |n: &str| n.as_bytes().first().is_some_and(u8::is_ascii_digit);
    names.sort_by(|a, b| slice(a).cmp(&slice(b)).then(b.cmp(a)));
    names
        .into_iter()
        .filter_map(|n| {
            let text = std::fs::read_to_string(dir.join(&n)).ok()?;
            Some((format!("_archive/journal/{n}"), text))
        })
        .collect()
}

/// `docsys journal`: history's entries newest first (R-100, R-104), then the
/// frozen files as written; `since` keeps history's entries from that day on
/// and leaves the frozen files out.
pub fn render(repo: Option<&Path>, root: &Path, since: Option<&str>) -> String {
    let mut out = String::from("# Journal\n");
    match repo.and_then(|r| entries(r, root, since)) {
        Some(list) => {
            for e in list {
                out.push_str(&format!("\n## {} - {}\n", e.date, e.title));
                for l in e.lines {
                    out.push_str(&l);
                    out.push('\n');
                }
            }
        }
        None => out.push_str("\nhistory: unknown — the tree is not inside a repository\n"),
    }
    if since.is_none() {
        for (rel, text) in frozen(root) {
            out.push_str(&format!("\n<!-- frozen: {rel} -->\n"));
            out.push_str(&text);
            if !text.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    out
}

/// The commit message `journal add` hands back on a docsys/0.5 tree: the
/// title, the lines, and the `Docs:` trailer naming the page or the why.
pub fn message(title: &str, lines: &[&str], link: Option<&str>) -> String {
    let mut out = format!("{title}\n");
    if !lines.is_empty() {
        out.push('\n');
        for l in lines {
            out.push_str(l);
            out.push('\n');
        }
    }
    let docs = link
        .map(|l| l.trim().trim_end_matches(".md").to_string())
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| title.to_string());
    out.push_str(&format!("\n{DOCS}: {docs}\n"));
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_trailer_is_read_after_the_subject_wherever_a_squash_put_it() {
        assert!(has_trailer(
            "subject\n\nbody\n\nDocs: reference/retry\n",
            DOCS
        ));
        assert!(has_trailer("squash\n\n* one\n\nDocs: why\n\n* two\n", DOCS));
        assert!(!has_trailer("Docs: in the subject only\n", DOCS));
        assert!(!has_trailer("subject\n\nDocsify: no\n", DOCS));
    }

    #[test]
    fn a_message_carries_its_title_lines_and_trailer() {
        assert_eq!(
            message(
                "Retry bounded",
                &["three attempts"],
                Some("reference/retry.md")
            ),
            "Retry bounded\n\nthree attempts\n\nDocs: reference/retry\n"
        );
        assert_eq!(
            message("Retry bounded", &[], None),
            "Retry bounded\n\nDocs: Retry bounded\n"
        );
    }
}
