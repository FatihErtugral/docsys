//! Debt and questions grouped by topic (R-108, D-124): each open item is one
//! line in the file of the page or feature it concerns — the `[topic]` tag
//! the line carries after its date, `general` when it carries none. A closed
//! item leaves its file, and the commit that removes it carries its trailer;
//! a topic with no open item left has no file.

use std::fs;
use std::path::Path;

/// One of the two lists an item belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum List {
    Debt,
    Questions,
}

impl List {
    /// The directory of the list's topic files, relative to the docs root.
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

/// The topic of an item with no tag of its own.
pub const GENERAL: &str = "general";

/// An open item: its topic file and its line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// root-relative path of the topic file
    pub rel: String,
    /// the file name without `.md`
    pub topic: String,
    /// the `- [ ] YYYY-MM-DD …` line
    pub line: String,
}

/// The topic a line names: the `[tag]` right after its date when the tag is
/// a local id, else `general`.
pub fn topic_of(line: &str) -> String {
    line.get(16..)
        .map(str::trim_start)
        .and_then(|rest| rest.strip_prefix('['))
        .and_then(|rest| rest.split_once(']'))
        .map(|(tag, _)| tag.trim())
        .filter(|tag| crate::model::is_local_id(tag))
        .map_or_else(|| GENERAL.to_string(), str::to_string)
}

/// The open items of a list, topic by topic in name order, each topic's in
/// the order its file holds them — the order `close <n>` numbers them in.
pub fn open(root: &Path, list: List, kb: bool) -> Vec<Item> {
    let dir = list.dir(kb);
    let Ok(entries) = fs::read_dir(root.join(dir)) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| n.ends_with(".md"))
        .collect();
    names.sort();
    let mut items = Vec::new();
    for name in names {
        let Ok(text) = fs::read_to_string(root.join(dir).join(&name)) else {
            continue;
        };
        let topic = name.trim_end_matches(".md").to_string();
        for line in text.lines().filter(|l| l.starts_with("- [ ] ")) {
            items.push(Item {
                rel: format!("{dir}/{name}"),
                topic: topic.clone(),
                line: line.to_string(),
            });
        }
    }
    items
}

/// Append one open item to its topic's file, the file made with the first;
/// the root-relative path of the file.
pub fn add(root: &Path, list: List, kb: bool, line: &str) -> Result<String, String> {
    fs::create_dir_all(root.join(list.dir(kb))).map_err(|e| e.to_string())?;
    let rel = format!("{}/{}.md", list.dir(kb), topic_of(line));
    let path = root.join(&rel);
    let text = match fs::read_to_string(&path) {
        Ok(mut text) => {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(line);
            text.push('\n');
            text
        }
        Err(_) => {
            let pre = crate::migrate::generated_preamble(root);
            crate::migrate::with_preamble(&format!("{line}\n"), &pre)
        }
    };
    fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(rel)
}

/// One open item: its 1-based number in the order `open` lists, or a piece
/// of its text no other open item holds.
pub fn find(root: &Path, list: List, kb: bool, which: &str) -> Result<Item, String> {
    let items = open(root, list, kb);
    let which = which.trim();
    if items.is_empty() {
        return Err(format!("{} has no open item", list.dir(kb)));
    }
    if let Ok(n) = which.parse::<usize>() {
        return items.get(n.wrapping_sub(1)).cloned().ok_or_else(|| {
            format!(
                "{} has {} open item(s); there is no item {n}",
                list.dir(kb),
                items.len()
            )
        });
    }
    let hits: Vec<&Item> = items.iter().filter(|i| i.line.contains(which)).collect();
    match hits.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(format!("no open item in {} holds `{which}`", list.dir(kb))),
        many => Err(format!(
            "{} open items hold `{which}` — name one by its number or by more of its words",
            many.len()
        )),
    }
}

/// Close an item: its line leaves its topic file — the file goes with its
/// last item — and the commit that removes it carries `<trailer>: <record>`,
/// which this returns for the caller to print.
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
    let path = root.join(&item.rel);
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut removed = false;
    let rest: String = text
        .split_inclusive('\n')
        .filter(|l| {
            if !removed && l.trim_end_matches('\n') == item.line {
                removed = true;
                return false;
            }
            true
        })
        .collect();
    if rest.lines().any(|l| l.starts_with("- [ ] ")) {
        fs::write(&path, rest).map_err(|e| e.to_string())?;
    } else {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(format!(
        "closed: {}\n  from {}; history keeps it\n\
         commit the change with this trailer in its message (R-108):\n{}: {record}",
        item.line,
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
    fn an_item_joins_the_topic_its_tag_names_and_general_without_one() {
        let r = root("topics");
        let a = "- [ ] 2026-10-01 [retry-policy] Retries are unbounded -- deferred: no owner -- repay when: next outage";
        let b = "- [ ] 2026-10-02 [retry-policy] The backoff is fixed -- deferred: later -- repay when: soon";
        let c = "- [ ] 2026-10-03 The CI cache is cold -- deferred: later -- repay when: soon";
        assert_eq!(
            add(&r, List::Debt, false, a).unwrap(),
            "work/debt/retry-policy.md"
        );
        assert_eq!(
            add(&r, List::Debt, false, b).unwrap(),
            "work/debt/retry-policy.md"
        );
        assert_eq!(
            add(&r, List::Debt, false, c).unwrap(),
            "work/debt/general.md"
        );
        assert_eq!(
            fs::read_to_string(r.join("work/debt/retry-policy.md")).unwrap(),
            format!("{a}\n{b}\n")
        );
        let lines: Vec<String> = open(&r, List::Debt, false)
            .into_iter()
            .map(|i| i.line)
            .collect();
        assert_eq!(lines, [c, a, b], "topic by topic, each in file order");
        assert_eq!(topic_of("- [ ] 2026-10-01 [Not An Id] x"), GENERAL);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn closing_takes_the_line_out_and_the_last_takes_the_file() {
        let r = root("close");
        let a =
            "- [ ] 2026-10-01 [retry-policy] Retries are unbounded -- deferred: a -- repay when: b";
        let b =
            "- [ ] 2026-10-02 [retry-policy] The backoff is fixed -- deferred: c -- repay when: d";
        add(&r, List::Debt, false, a).unwrap();
        add(&r, List::Debt, false, b).unwrap();
        let out = close(&r, List::Debt, false, "backoff", "made exponential").unwrap();
        assert!(out.ends_with("\nResolved: made exponential"), "{out}");
        assert_eq!(
            fs::read_to_string(r.join("work/debt/retry-policy.md")).unwrap(),
            format!("{a}\n")
        );
        assert!(find(&r, List::Debt, false, "2")
            .unwrap_err()
            .contains("there is no item 2"));
        assert!(
            close(&r, List::Debt, false, "1", " ").is_err(),
            "a record is said"
        );
        close(&r, List::Debt, false, "1", "bounded at three").unwrap();
        assert!(!r.join("work/debt/retry-policy.md").exists());
        assert!(find(&r, List::Debt, false, "1")
            .unwrap_err()
            .contains("no open item"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn a_piece_of_text_names_one_item_or_says_how_many() {
        let r = root("text");
        add(
            &r,
            List::Questions,
            true,
            "- [ ] 2026-10-01 [cache] Who owns the cache?",
        )
        .unwrap();
        add(
            &r,
            List::Questions,
            true,
            "- [ ] 2026-10-02 [cache] Who warms the cache?",
        )
        .unwrap();
        assert!(find(&r, List::Questions, true, "Who")
            .unwrap_err()
            .contains("2 open items hold"));
        assert_eq!(
            find(&r, List::Questions, true, "warms").unwrap().rel,
            "wiki/open-questions/cache.md"
        );
        let _ = fs::remove_dir_all(&r);
    }
}
