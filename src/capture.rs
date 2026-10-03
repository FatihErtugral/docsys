//! Capture commands — the single-file writes R-097 exempts, made mechanical
//! (D-063): a debt repaid leaves the ledger and lands as a journal line; a
//! journal line lands at its own date; a page is opened from its template.
//! The tool writes structure and the caller's own words; it never composes.

use crate::migrate::{generated_preamble, today, with_preamble, TEMPLATES};
use crate::model::VALID_TYPES;
use crate::seed::insert_journal_entry;
use std::fs;
use std::path::Path;

/// The lists of the tree at `root`: whether it keeps one file per item
/// (D-124), and whether it is a knowledge base.
fn lists_of(root: &Path) -> (bool, bool) {
    (
        crate::era::Era::at(root).item_files(),
        crate::hook::is_knowledge_base(root),
    )
}

/// A ledger with `line` appended — the docsys/0.4 form (D-118).
fn append_to_ledger(root: &Path, ledger: &str, title: &str, line: &str) -> Result<(), String> {
    let path = root.join(ledger);
    let mut text = fs::read_to_string(&path).unwrap_or_else(|_| title.to_string());
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(line);
    text.push('\n');
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(&path, text).map_err(|e| e.to_string())
}

fn item_date(date: Option<&str>) -> Result<String, String> {
    match date {
        Some(d) if crate::model::is_iso_date(d) => Ok(d.to_string()),
        Some(d) => Err(format!("`{d}` is not a YYYY-MM-DD date")),
        None => Ok(today()),
    }
}

/// `debt add`: a deferred debt, dated, with why it waits and what repays it
/// (R-108) — its own file on a docsys/0.5 tree (D-124).
pub fn debt_add(
    root: &Path,
    text: &str,
    deferred: Option<&str>,
    repay_when: Option<&str>,
    date: Option<&str>,
) -> Result<String, String> {
    let text = text.trim();
    let (Some(deferred), Some(repay)) = (
        deferred.map(str::trim).filter(|s| !s.is_empty()),
        repay_when.map(str::trim).filter(|s| !s.is_empty()),
    ) else {
        return Err("a debt says why it waits and what repays it: --deferred <reason> --repay-when <trigger> (R-108)".into());
    };
    if text.is_empty() {
        return Err("nothing to add".into());
    }
    let tree = crate::tree::DocTree::load(root).map_err(|e| e.to_string())?;
    let line = format!(
        "- [ ] {} {text} -- {}: {deferred} -- {}: {repay}",
        item_date(date)?,
        crate::checks::label_of(&tree, "deferred"),
        crate::checks::label_of(&tree, "repay when")
    );
    let (items, kb) = lists_of(root);
    if items {
        let rel = crate::items::add(root, crate::items::List::Debt, kb, &line)?;
        return Ok(format!("added: {rel}"));
    }
    append_to_ledger(root, "work/debt.md", "# Debt\n", &line)?;
    Ok("added: work/debt.md".to_string())
}

/// `question add`: what is not known, dated, never a guess on a page (R-108)
/// — its own file on a docsys/0.5 tree (D-124).
pub fn question_add(
    root: &Path,
    text: &str,
    context: Option<&str>,
    date: Option<&str>,
) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("nothing to add".into());
    }
    let mut line = format!("- [ ] {} {text}", item_date(date)?);
    if let Some(c) = context.map(str::trim).filter(|c| !c.is_empty()) {
        line.push_str(&format!(" -- {c}"));
    }
    let (items, kb) = lists_of(root);
    let list = crate::items::List::Questions;
    if items {
        let rel = crate::items::add(root, list, kb, &line)?;
        return Ok(format!("added: {rel}"));
    }
    append_to_ledger(root, list.ledger(kb), "# Questions\n", &line)?;
    Ok(format!("added: {}", list.ledger(kb)))
}

/// `question close`: an answered question leaves its file and the commit
/// carries `Answered:` (D-124); on a docsys/0.4 tree the n-th open line is
/// checked off with its answer, as R-108 had it.
pub fn question_close(root: &Path, which: &str, answer: Option<&str>) -> Result<String, String> {
    let answer = answer
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .ok_or("an answered question says its answer: --answer <link or one line> (R-108)")?;
    let (items, kb) = lists_of(root);
    let list = crate::items::List::Questions;
    if items {
        return crate::items::close(root, list, kb, which, answer);
    }
    let n: usize = which
        .parse()
        .map_err(|_| format!("`{which}` is not an item number"))?;
    let ledger = list.ledger(kb);
    let path = root.join(ledger);
    let text = fs::read_to_string(&path).map_err(|_| format!("{ledger} does not exist"))?;
    let tree = crate::tree::DocTree::load(root).map_err(|e| e.to_string())?;
    let label = crate::checks::label_of(&tree, "answered");
    let mut seen = 0usize;
    let mut closed = None;
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        if closed.is_none() && line.starts_with("- [ ] ") {
            seen += 1;
            if seen == n {
                let body = line.trim_end().trim_start_matches("- [ ] ");
                closed = Some(body.to_string());
                out.push_str(&format!("- [x] {body} -- {label}: {answer}\n"));
                continue;
            }
        }
        out.push_str(line);
    }
    let item =
        closed.ok_or_else(|| format!("{ledger} has {seen} open item(s); there is no item {n}"))?;
    fs::write(&path, out).map_err(|e| e.to_string())?;
    Ok(format!("answered: {item}"))
}

/// `debt close <item>`: on a docsys/0.5 tree the item's file goes and the
/// commit carries `Resolved:` (D-124); on a 0.4 tree, `debt_close_ledger`.
pub fn debt_close(root: &Path, which: &str, note: Option<&str>) -> Result<String, String> {
    let (items, kb) = lists_of(root);
    if items {
        return crate::items::close(
            root,
            crate::items::List::Debt,
            kb,
            which,
            note.unwrap_or(""),
        );
    }
    let n: usize = which
        .parse()
        .map_err(|_| format!("`{which}` is not an item number — `docsys debt close <n>`"))?;
    debt_close_ledger(root, n, note)
}

/// `debt close <n>` on a docsys/0.4 tree: remove the n-th open item (1-based,
/// in file order) and record the repayment as a journal entry dated today
/// (D-039). The item's own text is the entry; `note` is the caller's one line
/// on top of it.
fn debt_close_ledger(root: &Path, n: usize, note: Option<&str>) -> Result<String, String> {
    let path = root.join("work/debt.md");
    let text = fs::read_to_string(&path).map_err(|_| "work/debt.md does not exist".to_string())?;
    let mut open_seen = 0usize;
    let mut removed: Option<String> = None;
    let mut kept = Vec::new();
    for line in text.lines() {
        if removed.is_none() && line.starts_with("- [ ] ") {
            open_seen += 1;
            if open_seen == n {
                removed = Some(line.trim_start_matches("- [ ] ").to_string());
                continue;
            }
        }
        kept.push(line);
    }
    let Some(item) = removed else {
        return Err(format!(
            "work/debt.md has {open_seen} open item(s); there is no item {n}"
        ));
    };
    let mut ledger = kept.join("\n");
    ledger.push('\n');
    // collapse the blank line the item may have left behind
    while ledger.contains("\n\n\n") {
        ledger = ledger.replace("\n\n\n", "\n\n");
    }
    // the repayment: the item's opening clause is the title, the whole item
    // the body line — nothing rewritten
    let today = today();
    let title = item
        .split(" -- ")
        .next()
        .unwrap_or(&item)
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '-')
        .trim()
        .to_string();
    let mut entry = format!("## {today} - repaid: {title}\n- {item}");
    if let Some(n) = note.map(str::trim).filter(|n| !n.is_empty()) {
        entry.push_str(&format!("\n- {n}"));
    }
    let journal_path = root.join("work/journal.md");
    let journal = fs::read_to_string(&journal_path).unwrap_or_else(|_| "# Journal\n".into());
    fs::write(
        &journal_path,
        insert_journal_entry(&journal, &today, &entry),
    )
    .map_err(|e| e.to_string())?;
    fs::write(&path, ledger).map_err(|e| e.to_string())?;
    Ok(format!(
        "closed: {item}\njournal: {today} - repaid: {title}"
    ))
}

/// `ledger fix` (D-108): R-108's separators are ASCII, and a ledger written
/// with em dashes — ` — deferred: ` — matches no grammar. Each marker at a
/// label position, in the tree's declared `list_labels` form, is rewritten to
/// ` -- `; a dash inside field text stays. The ledgers and their `_archive/`
/// slices; a file is written only when it changed.
pub fn ledger_fix(root: &Path) -> Result<String, String> {
    ledger_fix_with(root, true)
}

/// `ledger_fix`, or with `write: false` only what it would rewrite — the plan
/// `docsys upgrade` prints before anything is written (R-176).
pub fn ledger_fix_with(root: &Path, write: bool) -> Result<String, String> {
    let tree = crate::tree::DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    let mut done = Vec::new();
    for (ledger, labels) in crate::checks::LEDGERS {
        let markers: Vec<(String, String)> = labels
            .iter()
            .map(|l| {
                let l = crate::checks::label_of(&tree, l);
                (format!(" — {l}: "), format!(" -- {l}: "))
            })
            .collect();
        let mut files = vec![ledger.to_string()];
        files.extend(crate::checks::archive_slices(root, ledger));
        // a docsys/0.5 list is a directory of item files (D-124)
        if crate::era::Era::of(&tree).item_files() {
            let dir = ledger.trim_end_matches(".md");
            if let Ok(entries) = fs::read_dir(root.join(dir)) {
                let mut names: Vec<String> = entries
                    .flatten()
                    .filter_map(|e| e.file_name().to_str().map(str::to_string))
                    .filter(|n| n.ends_with(".md"))
                    .map(|n| format!("{dir}/{n}"))
                    .collect();
                names.sort();
                files.extend(names);
            }
        }
        for rel in files {
            let path = root.join(&rel);
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let mut fixed = String::with_capacity(text.len());
            let mut changed = Vec::new();
            for (i, line) in text.split_inclusive('\n').enumerate() {
                let mut line = line.to_string();
                if line.starts_with("- [ ] ") || line.starts_with("- [x] ") {
                    let before = line.clone();
                    for (dashed, ascii) in &markers {
                        if !line.contains(ascii.as_str()) {
                            line = line.replacen(dashed.as_str(), ascii, 1);
                        }
                    }
                    if line != before {
                        changed.push((i + 1).to_string());
                    }
                }
                fixed.push_str(&line);
            }
            if !changed.is_empty() {
                if write {
                    fs::write(&path, fixed).map_err(|e| format!("{rel}: {e}"))?;
                }
                done.push(format!("fixed: {rel} line {}", changed.join(", ")));
            }
        }
    }
    if done.is_empty() {
        done.push("ledger: every field marker is already ASCII".to_string());
    }
    Ok(done.join("\n"))
}

/// An entry's title and body lines from the caller's text: the explicit title,
/// or the first sentence. A single sentence with no title of its own IS the
/// title: repeating it as the first line wrote every one-line entry twice.
fn entry_parts(text: &str, title: Option<&str>) -> (String, Vec<String>) {
    let explicit = title.map(str::trim).filter(|t| !t.is_empty());
    let title = explicit.map(str::to_string).unwrap_or_else(|| {
        let first = text.lines().next().unwrap_or("").trim();
        first
            .split_once(". ")
            .map_or(first, |(a, _)| a)
            .trim_end_matches('.')
            .to_string()
    });
    let text_is_title =
        explicit.is_none() && text.lines().count() == 1 && text.trim_end_matches('.') == title;
    let lines = if text_is_title {
        Vec::new()
    } else {
        text.lines()
            .map(|l| l.trim().trim_start_matches("- ").to_string())
            .filter(|l| !l.is_empty())
            .collect()
    };
    (title, lines)
}

/// `journal add`: one entry at its date (today by default; a retrospective
/// date lands where R-104 puts it), the caller's lines as the body, an
/// optional wiki-link as the pointer R-101 asks for.
pub fn journal_add(
    root: &Path,
    text: &str,
    title: Option<&str>,
    date: Option<&str>,
    link: Option<&str>,
) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("nothing to add".into());
    }
    // on a docsys/0.5 tree the journal is history: the entry is the commit
    // message, handed back for the commit to carry (D-125)
    if crate::era::Era::at(root).journal_from_history() {
        if date.is_some() {
            return Err("on docsys/0.5 an entry's date is its commit's (R-100)".into());
        }
        let (title, lines) = entry_parts(text, title);
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        return Ok(crate::journal::message(&title, &lines, link));
    }
    let date = match date {
        Some(d) if crate::model::is_iso_date(d) => d.to_string(),
        Some(d) => return Err(format!("`{d}` is not a YYYY-MM-DD date")),
        None => today(),
    };
    let (title, lines) = entry_parts(text, title);
    let mut entry = format!("## {date} - {title}");
    for l in &lines {
        entry.push_str(&format!("\n- {l}"));
    }
    if let Some(l) = link.map(str::trim).filter(|l| !l.is_empty()) {
        let target = l.trim_end_matches(".md");
        entry.push_str(&format!("\n- [[{target}]]"));
    }
    let body_lines = entry.lines().count() - 1;
    let path = root.join("work/journal.md");
    let journal = fs::read_to_string(&path).unwrap_or_else(|_| "# Journal\n".into());
    fs::write(&path, insert_journal_entry(&journal, &date, &entry)).map_err(|e| e.to_string())?;
    let mut out = format!("journal: {date} - {title}");
    if body_lines > 5 {
        out.push_str(&format!(
            "\nnote: {body_lines} body lines — R-101 budgets 5; link, do not narrate"
        ));
    }
    Ok(out)
}

/// `page new <category|type> <id>`: a tracked-work file from its `_templates/`
/// file (`feature` → `work/features/<id>.md`, …) or a permanent page with its
/// frontmatter and the R-032 opening left for the author. Refuses to
/// overwrite; never routes — the router line is a sentence only the author
/// can write, and lint names the page until it is written.
pub fn page_new(
    root: &Path,
    kind: &str,
    id: &str,
    title: Option<&str>,
    unverified: bool,
) -> Result<String, String> {
    if !crate::model::is_local_id(id) {
        return Err(format!(
            "`{id}` is not a local-id (lowercase, digits, single hyphens)"
        ));
    }
    let today = today();
    // a docsys/0.5 page's date is history's (D-122)
    let dated = !crate::era::Era::at(root).derived_dates();
    let date = if dated {
        format!("updated: {today}\n")
    } else {
        String::new()
    };
    let pre = generated_preamble(root);
    let title = title
        .map(str::to_string)
        .unwrap_or_else(|| id.replace('-', " "));
    let (rel, text) = if let Some((file, category, sections)) = TEMPLATES
        .iter()
        .find(|(f, c, _)| f.trim_end_matches(".md") == kind || *c == kind)
    {
        let rel = format!("work/{category}/{id}.md");
        let template = fs::read_to_string(root.join("_templates").join(file)).ok();
        let body = match template {
            Some(t) => {
                // the installed template, its placeholders filled
                let t = if dated {
                    t
                } else {
                    crate::fm::without_scalar(&t, "updated").unwrap_or(t)
                };
                let t = t.replace("<id>", id).replace("<YYYY-MM-DD>", &today);
                // drop the template's own instruction comment
                t.lines()
                    .filter(|l| !l.trim_start().starts_with("<!--") || !l.contains("copy to work/"))
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\n"
            }
            None => {
                let mut t = format!("---\nid: {id}\nstatus: draft\n{date}---\n");
                for h in sections {
                    t.push_str(&format!("\n## {h}\n"));
                }
                t
            }
        };
        (rel, body)
    } else if VALID_TYPES.contains(&kind) {
        // --unverified (D-092): a page written from evidence that nobody has
        // vouched for yet — the record fields a maintainer will fill (R-028)
        let verification = if unverified {
            "verification: unverified\nsources: []\n"
        } else {
            ""
        };
        // a directory the index routes already reaches the page (D-123)
        let routed = crate::era::Era::at(root).directory_routes()
            && fs::read_to_string(root.join("index.md"))
                .is_ok_and(|i| i.contains(&format!("[[{kind}/|")));
        let route = if routed {
            ""
        } else {
            " Then route it from index.md."
        };
        (
            format!("{kind}/{id}.md"),
            format!(
                "---\nid: {id}\ntype: {kind}\n{verification}{date}---\n# {title}\n\n<!-- opening: one or two sentences that establish this page's own context — what it describes, when to read it (R-032).{route} -->\n"
            ),
        )
    } else {
        return Err(format!(
            "`{kind}` is not a category (feature | postmortem | research) or a type (reference | howto | explanation | tutorial)"
        ));
    };
    let path = root.join(&rel);
    if path.exists() {
        return Err(format!("{rel} already exists"));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, with_preamble(&text, &pre)).map_err(|e| e.to_string())?;
    Ok(format!("created: {rel}"))
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

    fn tree(name: &str) -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("docsys-capture-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        crate::migrate::init_profile(&root, "en", "project").unwrap();
        root
    }

    /// A docsys/0.4 tree, kept as 0.15.1 kept it (D-118): its journal and
    /// its debt ledger in place.
    fn tree04(name: &str) -> std::path::PathBuf {
        let root = tree(name);
        let meta = root.join(".docmeta.yml");
        let text = fs::read_to_string(&meta).unwrap();
        fs::write(&meta, text.replace("spec: docsys/0.5", "spec: docsys/0.4")).unwrap();
        fs::create_dir_all(root.join("work")).unwrap();
        fs::write(
            root.join("work/journal.md"),
            format!(
                "# Journal\n\n## {} - initialized\n- documentation tree created\n",
                today()
            ),
        )
        .unwrap();
        fs::write(root.join("work/debt.md"), "# Debt\n").unwrap();
        root
    }

    #[test]
    fn on_a_0_5_tree_the_entry_is_the_commit_message_and_nothing_is_written() {
        let root = tree("journal05");
        let out = journal_add(
            &root,
            "Wire format settled. Details on the page.",
            None,
            None,
            Some("reference/wire.md"),
        )
        .unwrap();
        assert_eq!(
            out,
            "Wire format settled\n\nWire format settled. Details on the page.\n\nDocs: reference/wire\n"
        );
        assert!(!root.join("work/journal.md").exists());
        assert!(journal_add(&root, "x", None, Some("2026-01-01"), None).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_item_is_its_own_file_and_closing_it_names_the_trailer() {
        let root = tree("items");
        let out = debt_add(
            &root,
            "Retries are unbounded",
            Some("no owner"),
            Some("next outage"),
            Some("2026-10-01"),
        )
        .unwrap();
        assert_eq!(out, "added: work/debt/retries-are-unbounded.md");
        assert_eq!(
            fs::read_to_string(root.join("work/debt/retries-are-unbounded.md")).unwrap(),
            "- [ ] 2026-10-01 Retries are unbounded -- deferred: no owner -- repay when: next outage\n"
        );
        assert!(
            debt_add(&root, "x", None, Some("y"), None).is_err(),
            "a debt says why it waits"
        );
        let out = question_add(
            &root,
            "Who owns the retry budget?",
            Some("[[reference/retry]]"),
            Some("2026-10-02"),
        )
        .unwrap();
        assert_eq!(out, "added: work/questions/who-owns-the-retry-budget.md");
        let out = debt_close(&root, "retries-are-unbounded", Some("bounded at three")).unwrap();
        assert!(out.ends_with("\nResolved: bounded at three"), "{out}");
        assert!(!root.join("work/debt/retries-are-unbounded.md").exists());
        assert!(
            question_close(&root, "1", None).is_err(),
            "an answer is required"
        );
        let out = question_close(&root, "1", Some("the platform team")).unwrap();
        assert!(out.ends_with("\nAnswered: the platform team"), "{out}");
        assert!(!root
            .join("work/questions/who-owns-the-retry-budget.md")
            .exists());
        assert!(!root.join("work/debt.md").exists() && !root.join("work/questions.md").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_0_4_tree_keeps_its_ledgers() {
        let root = tree04("ledgers");
        debt_add(
            &root,
            "Retries are unbounded",
            Some("no owner"),
            Some("next outage"),
            Some("2026-10-01"),
        )
        .unwrap();
        question_add(&root, "Who owns it?", None, Some("2026-10-02")).unwrap();
        assert!(fs::read_to_string(root.join("work/debt.md")).unwrap().ends_with("- [ ] 2026-10-01 Retries are unbounded -- deferred: no owner -- repay when: next outage\n"));
        question_close(&root, "1", Some("the platform team")).unwrap();
        assert!(fs::read_to_string(root.join("work/questions.md"))
            .unwrap()
            .ends_with("- [x] 2026-10-02 Who owns it? -- answered: the platform team\n"));
        assert!(!root.join("work/debt").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_closed_debt_leaves_the_ledger_and_lands_in_the_journal() {
        let root = tree04("debt");
        fs::write(
            root.join("work/debt.md"),
            "# Debt\n\nPreamble.\n\n- [ ] 2026-08-01 first -- deferred: a -- repay when: b\n- [ ] 2026-08-02 second -- deferred: c -- repay when: d\n",
        )
        .unwrap();
        let out = debt_close(&root, "1", Some("measured twice, held")).unwrap();
        assert!(out.contains("closed: 2026-08-01 first"), "{out}");
        let ledger = fs::read_to_string(root.join("work/debt.md")).unwrap();
        assert!(!ledger.contains("first"), "{ledger}");
        assert!(ledger.contains("- [ ] 2026-08-02 second"), "{ledger}");
        assert!(ledger.contains("Preamble."), "{ledger}");
        let journal = fs::read_to_string(root.join("work/journal.md")).unwrap();
        assert!(journal.contains(&format!("## {} - repaid: first\n- 2026-08-01 first -- deferred: a -- repay when: b\n- measured twice, held", today())), "{journal}");
        // newest first: the repayment sits above the init entry
        let heads: Vec<&str> = journal.lines().filter(|l| l.starts_with("## ")).collect();
        assert!(heads[0].contains("repaid: first"), "{heads:?}");
        assert!(debt_close(&root, "5", None)
            .unwrap_err()
            .contains("no item 5"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn journal_add_lands_at_its_date_with_a_pointer() {
        let root = tree04("journal");
        let out = journal_add(
            &root,
            "Wire format settled. Details on the page.",
            None,
            None,
            Some("reference/wire.md"),
        )
        .unwrap();
        assert!(
            out.starts_with(&format!("journal: {} - Wire format settled", today())),
            "{out}"
        );
        let j = fs::read_to_string(root.join("work/journal.md")).unwrap();
        assert!(
            j.contains("- Wire format settled. Details on the page.\n- [[reference/wire]]"),
            "{j}"
        );
        // a retrospective date lands below newer entries (R-104)
        journal_add(&root, "old news", Some("retro"), Some("2020-01-01"), None).unwrap();
        let j = fs::read_to_string(root.join("work/journal.md")).unwrap();
        assert!(
            j.trim_end().ends_with("## 2020-01-01 - retro\n- old news"),
            "{j}"
        );
        assert!(journal_add(&root, "x", None, Some("not-a-date"), None).is_err());
        assert!(journal_add(&root, "   ", None, None, None).is_err());
        let over = journal_add(&root, "a\nb\nc\nd\ne\nf", None, None, None).unwrap();
        assert!(over.contains("R-101"), "{over}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn page_new_opens_from_the_template_or_with_a_permanent_skeleton() {
        let root = tree("page");
        let out = page_new(&root, "feature", "dark-mode", None, false).unwrap();
        assert_eq!(out, "created: work/features/dark-mode.md");
        // a docsys/0.5 page's date is history's (D-122)
        let f = fs::read_to_string(root.join("work/features/dark-mode.md")).unwrap();
        assert!(
            f.starts_with("---\nid: dark-mode\nstatus: draft\n---\n"),
            "{f}"
        );
        assert!(
            f.contains("## Context") && f.contains("## Rejected alternatives"),
            "{f}"
        );
        assert!(!f.contains("copy to work/"), "{f}");
        let out = page_new(&root, "reference", "token-ttl", Some("Token TTL"), false).unwrap();
        assert_eq!(out, "created: reference/token-ttl.md");
        let p = fs::read_to_string(root.join("reference/token-ttl.md")).unwrap();
        assert!(
            p.contains("type: reference") && p.contains("# Token TTL") && p.contains("R-032"),
            "{p}"
        );
        assert!(page_new(&root, "reference", "token-ttl", None, false)
            .unwrap_err()
            .contains("already exists"));
        assert!(page_new(&root, "novel", "x", None, false)
            .unwrap_err()
            .contains("not a category"));
        assert!(page_new(&root, "feature", "Bad Id", None, false)
            .unwrap_err()
            .contains("local-id"));
        let _ = fs::remove_dir_all(&root);
    }

    /// A docsys/0.4 tree is written as 0.15.1 wrote it (D-118): its pages
    /// carry `updated:`.
    #[test]
    fn a_0_4_page_is_dated_as_before() {
        let root = tree("page04");
        let meta = root.join(".docmeta.yml");
        let text = fs::read_to_string(&meta).unwrap();
        fs::write(&meta, text.replace("spec: docsys/0.5", "spec: docsys/0.4")).unwrap();
        page_new(&root, "reference", "a", None, false).unwrap();
        let p = fs::read_to_string(root.join("reference/a.md")).unwrap();
        assert!(p.contains(&format!("\nupdated: {}\n", today())), "{p}");
        let _ = fs::remove_dir_all(&root);
    }
}
