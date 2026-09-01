//! Capture commands — the single-file writes R-097 exempts, made mechanical
//! (D-063): a debt repaid leaves the ledger and lands as a journal line; a
//! journal line lands at its own date; a page is opened from its template.
//! The tool writes structure and the caller's own words; it never composes.

use crate::migrate::{generated_preamble, today, with_preamble, TEMPLATES};
use crate::model::VALID_TYPES;
use crate::seed::insert_journal_entry;
use std::fs;
use std::path::Path;

/// `debt close <n>`: remove the n-th open item (1-based, in file order) and
/// record the repayment as a journal entry dated today (D-039). The item's
/// own text is the entry; `note` is the caller's one line on top of it.
pub fn debt_close(root: &Path, n: usize, note: Option<&str>) -> Result<String, String> {
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
    let date = match date {
        Some(d) if crate::model::is_iso_date(d) => d.to_string(),
        Some(d) => return Err(format!("`{d}` is not a YYYY-MM-DD date")),
        None => today(),
    };
    let explicit = title.map(str::trim).filter(|t| !t.is_empty());
    let title = explicit.map(str::to_string).unwrap_or_else(|| {
        let first = text.lines().next().unwrap_or("").trim();
        first
            .split_once(". ")
            .map_or(first, |(a, _)| a)
            .trim_end_matches('.')
            .to_string()
    });
    // A single sentence with no title of its own IS the title: repeating it
    // as the first bullet wrote every one-line entry twice. R-101 blesses a
    // one-line entry; the links, when given, are its body.
    let text_is_title =
        explicit.is_none() && text.lines().count() == 1 && text.trim_end_matches('.') == title;
    let mut entry = format!("## {date} - {title}");
    if !text_is_title {
        for line in text.lines() {
            let l = line.trim();
            if !l.is_empty() {
                entry.push_str(&format!("\n- {}", l.trim_start_matches("- ")));
            }
        }
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
pub fn page_new(root: &Path, kind: &str, id: &str, title: Option<&str>) -> Result<String, String> {
    if !crate::model::is_local_id(id) {
        return Err(format!(
            "`{id}` is not a local-id (lowercase, digits, single hyphens)"
        ));
    }
    let today = today();
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
                let t = t.replace("<id>", id).replace("<YYYY-MM-DD>", &today);
                // drop the template's own instruction comment
                t.lines()
                    .filter(|l| !l.trim_start().starts_with("<!--") || !l.contains("copy to work/"))
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\n"
            }
            None => {
                let mut t = format!("---\nid: {id}\nstatus: draft\nupdated: {today}\n---\n");
                for h in sections {
                    t.push_str(&format!("\n## {h}\n"));
                }
                t
            }
        };
        (rel, body)
    } else if VALID_TYPES.contains(&kind) {
        (
            format!("{kind}/{id}.md"),
            format!(
                "---\nid: {id}\ntype: {kind}\nupdated: {today}\n---\n# {title}\n\n<!-- opening: one or two sentences that establish this page's own context — what it describes, when to read it (R-032). Then route it from index.md. -->\n"
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

    #[test]
    fn a_closed_debt_leaves_the_ledger_and_lands_in_the_journal() {
        let root = tree("debt");
        fs::write(
            root.join("work/debt.md"),
            "# Debt\n\nPreamble.\n\n- [ ] 2026-08-01 first -- deferred: a -- repay when: b\n- [ ] 2026-08-02 second -- deferred: c -- repay when: d\n",
        )
        .unwrap();
        let out = debt_close(&root, 1, Some("measured twice, held")).unwrap();
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
        assert!(debt_close(&root, 5, None)
            .unwrap_err()
            .contains("no item 5"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn journal_add_lands_at_its_date_with_a_pointer() {
        let root = tree("journal");
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
        let out = page_new(&root, "feature", "dark-mode", None).unwrap();
        assert_eq!(out, "created: work/features/dark-mode.md");
        let f = fs::read_to_string(root.join("work/features/dark-mode.md")).unwrap();
        assert!(
            f.starts_with(&format!(
                "---\nid: dark-mode\nstatus: draft\nupdated: {}\n---\n",
                today()
            )),
            "{f}"
        );
        assert!(
            f.contains("## Context") && f.contains("## Rejected alternatives"),
            "{f}"
        );
        assert!(!f.contains("copy to work/"), "{f}");
        let out = page_new(&root, "reference", "token-ttl", Some("Token TTL")).unwrap();
        assert_eq!(out, "created: reference/token-ttl.md");
        let p = fs::read_to_string(root.join("reference/token-ttl.md")).unwrap();
        assert!(
            p.contains("type: reference") && p.contains("# Token TTL") && p.contains("R-032"),
            "{p}"
        );
        assert!(page_new(&root, "reference", "token-ttl", None)
            .unwrap_err()
            .contains("already exists"));
        assert!(page_new(&root, "novel", "x", None)
            .unwrap_err()
            .contains("not a category"));
        assert!(page_new(&root, "feature", "Bad Id", None)
            .unwrap_err()
            .contains("local-id"));
        let _ = fs::remove_dir_all(&root);
    }
}
