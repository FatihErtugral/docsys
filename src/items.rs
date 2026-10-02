//! Debt and questions as one file per open item (R-108, D-124): two branches
//! that each add an item never touch the same file, and a closed item leaves
//! with its file — the closing commit's trailer is its record.

use std::fs;
use std::path::Path;

/// One of the two lists an item belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum List {
    Debt,
    Questions,
}

impl List {
    /// The directory of the list's item files, relative to the docs root.
    pub fn dir(self, kb: bool) -> &'static str {
        match (self, kb) {
            (List::Debt, _) => "work/debt",
            (List::Questions, false) => "work/questions",
            (List::Questions, true) => "wiki/open-questions",
        }
    }

    /// The one-file ledger a docsys/0.4 tree keeps instead.
    pub fn ledger(self, kb: bool) -> &'static str {
        match (self, kb) {
            (List::Debt, _) => "work/debt.md",
            (List::Questions, false) => "work/questions.md",
            (List::Questions, true) => "wiki/open-questions.md",
        }
    }

    /// The trailer the closing commit carries (R-108).
    pub fn trailer(self) -> &'static str {
        match self {
            List::Debt => "Resolved",
            List::Questions => "Answered",
        }
    }
}

/// An open item: its file and its one line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// root-relative path of the item file
    pub rel: String,
    /// the file name without `.md`
    pub slug: String,
    /// the `- [ ] YYYY-MM-DD …` line
    pub line: String,
}

impl Item {
    /// The opening date, as the line carries it.
    pub fn date(&self) -> &str {
        self.line.get(6..16).unwrap_or("")
    }
}

/// The open items of a list, oldest first, then by name — the order `debt
/// close <n>` numbers them in.
pub fn open(root: &Path, list: List, kb: bool) -> Vec<Item> {
    let dir = list.dir(kb);
    let Ok(entries) = fs::read_dir(root.join(dir)) else {
        return Vec::new();
    };
    let mut items: Vec<Item> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            let slug = name.strip_suffix(".md")?.to_string();
            let text = fs::read_to_string(e.path()).ok()?;
            let line = text.lines().find(|l| l.starts_with("- [ ] "))?.to_string();
            Some(Item {
                rel: format!("{dir}/{name}"),
                slug,
                line,
            })
        })
        .collect();
    items.sort_by(|a, b| (a.date(), &a.slug).cmp(&(b.date(), &b.slug)));
    items
}

/// The words an item's file is named by: its text after the date, up to its
/// first field marker.
fn words(line: &str) -> &str {
    let rest = line.get(6..).unwrap_or("");
    let rest = rest
        .get(..10)
        .filter(|d| crate::model::is_iso_date(d))
        .map_or(rest, |_| rest.get(10..).unwrap_or(""));
    rest.split(" -- ").next().unwrap_or("").trim()
}

/// A free file name for the item: its words as a slug of at most 48
/// characters, cut at a word boundary; `-2`, `-3` … when the name is taken.
fn slug_for(dir: &Path, line: &str) -> String {
    let full = crate::slug::slug(words(line));
    let mut base = String::new();
    for word in full.split('-') {
        if !base.is_empty() && base.len() + 1 + word.len() > 48 {
            break;
        }
        if !base.is_empty() {
            base.push('-');
        }
        base.push_str(word);
    }
    if base.is_empty() {
        base = "item".to_string();
    }
    let mut name = base.clone();
    let mut n = 2;
    while dir.join(format!("{name}.md")).exists() {
        name = format!("{base}-{n}");
        n += 1;
    }
    name
}

/// Write one open item into its own file; the root-relative path it got.
pub fn add(root: &Path, list: List, kb: bool, line: &str) -> Result<String, String> {
    let dir = root.join(list.dir(kb));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let slug = slug_for(&dir, line);
    let pre = crate::migrate::generated_preamble(root);
    let text = crate::migrate::with_preamble(&format!("{line}\n"), &pre);
    fs::write(dir.join(format!("{slug}.md")), text).map_err(|e| e.to_string())?;
    Ok(format!("{}/{slug}.md", list.dir(kb)))
}

/// One open item, named by its file name or by its 1-based number in the
/// order `open` lists.
pub fn find(root: &Path, list: List, kb: bool, which: &str) -> Result<Item, String> {
    let items = open(root, list, kb);
    let which = which.trim().trim_end_matches(".md");
    let which = which.rsplit('/').next().unwrap_or(which);
    let found = match which.parse::<usize>() {
        Ok(n) => items.get(n.wrapping_sub(1)).cloned(),
        Err(_) => items.iter().find(|i| i.slug == which).cloned(),
    };
    found.ok_or_else(|| {
        let names: Vec<String> = items
            .iter()
            .enumerate()
            .map(|(i, it)| format!("{} {}", i + 1, it.slug))
            .collect();
        if names.is_empty() {
            format!("{} has no open item", list.dir(kb))
        } else {
            format!(
                "no open item `{which}` in {} — open: {}",
                list.dir(kb),
                names.join(", ")
            )
        }
    })
}

/// Close an item: its file goes, and the commit that removes it carries
/// `<trailer>: <record>` — what this returns for the caller to print.
pub fn close(
    root: &Path,
    list: List,
    kb: bool,
    which: &str,
    record: &str,
) -> Result<String, String> {
    let record = record.trim();
    if record.is_empty() {
        return Err(format!(
            "a closed item's record is its commit's `{}:` trailer — say what closed it",
            list.trailer()
        ));
    }
    let item = find(root, list, kb, which)?;
    fs::remove_file(root.join(&item.rel)).map_err(|e| e.to_string())?;
    Ok(format!(
        "closed: {} — the file is removed; history keeps the item\n\
         commit the removal with this trailer in its message (R-108):\n{}: {record}",
        item.rel,
        list.trailer()
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn root(name: &str) -> std::path::PathBuf {
        let r = std::env::temp_dir().join(format!("docsys-items-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&r);
        fs::create_dir_all(&r).unwrap();
        r
    }

    #[test]
    fn an_item_is_named_by_its_words_and_numbered_by_its_date() {
        let r = root("names");
        let late = "- [ ] 2026-10-02 Retry policy is undocumented -- deferred: no owner -- repay when: next outage";
        let early =
            "- [ ] 2026-09-01 Retry policy is undocumented -- deferred: later -- repay when: soon";
        assert_eq!(
            add(&r, List::Debt, false, late).unwrap(),
            "work/debt/retry-policy-is-undocumented.md"
        );
        assert_eq!(
            add(&r, List::Debt, false, early).unwrap(),
            "work/debt/retry-policy-is-undocumented-2.md"
        );
        let items = open(&r, List::Debt, false);
        assert_eq!(items.len(), 2);
        assert_eq!(
            items.first().map(|i| i.line.as_str()),
            Some(early),
            "oldest first"
        );
        assert_eq!(find(&r, List::Debt, false, "1").unwrap().line, early);
        assert_eq!(
            find(&r, List::Debt, false, "retry-policy-is-undocumented")
                .unwrap()
                .line,
            late
        );
        assert!(find(&r, List::Debt, false, "3")
            .unwrap_err()
            .contains("open: 1 retry-policy-is-undocumented-2"));
        let out = close(&r, List::Debt, false, "1", "measured, held").unwrap();
        assert!(out.ends_with("\nResolved: measured, held"), "{out}");
        assert_eq!(open(&r, List::Debt, false).len(), 1);
        assert!(close(&r, List::Debt, false, "1", " ").is_err());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn a_long_question_is_cut_at_a_word() {
        let r = root("long");
        let line = "- [ ] 2026-10-03 Why does the scheduler retry twice when the upstream answers with a timeout?";
        let rel = add(&r, List::Questions, true, line).unwrap();
        assert_eq!(
            rel,
            "wiki/open-questions/why-does-the-scheduler-retry-twice-when-the.md"
        );
        let _ = fs::remove_dir_all(&r);
    }
}
