//! §9 compile — a howto page becomes an executable agent skill (R-094): the
//! page body byte for byte, the source identifier and the source content
//! hash on the skill (R-095), never a step the page did not write (R-096).
//!
//! The judgment — is every step written, with no gap a reader would fill
//! from memory? — is P/R-096, made before this command runs. The tool copies;
//! lint reports the skill stale the moment the page moves (D-073).

use std::fs;
use std::path::Path;

use crate::checks::Report;
use crate::fm::Value;
use crate::fresh::content_hash;
use crate::model::{Finding, RuleId};
use crate::tree::{DocTree, Kind, Page, Profile};

const R095: RuleId = RuleId("R-095");

/// The frontmatter keys a compiled skill carries (R-095).
pub const SOURCE_KEY: &str = "docsys_source";
pub const HASH_KEY: &str = "docsys_source_hash";

/// The body of a page — everything after the closing frontmatter delimiter
/// (R-113); a page without frontmatter is all body.
pub fn body_of(page: &Page) -> String {
    let Some(fm) = &page.fm else {
        return page.text.clone();
    };
    let mut body = page
        .text
        .lines()
        .skip(fm.body_start)
        .collect::<Vec<_>>()
        .join("\n");
    body.push('\n');
    body
}

/// A page by identifier, or by root-relative path with or without `.md`.
fn find_page<'a>(tree: &'a DocTree, page: &str) -> Option<&'a Page> {
    let rel = page.trim_start_matches("./").trim_end_matches(".md");
    tree.pages.iter().find(|p| {
        p.rel.trim_end_matches(".md") == rel
            || p.fm
                .as_ref()
                .and_then(|f| f.fields.get("id"))
                .and_then(Value::as_str)
                == Some(page)
    })
}

fn title_of(page: &Page, id: &str) -> String {
    page.text
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map_or_else(|| id.replace('-', " "), str::to_string)
}

/// `docsys compile <howto>`: the skill file written under
/// `<claude_dir>/skills/<id>/SKILL.md`.
pub fn compile(root: &Path, claude_dir: &Path, page: &str, force: bool) -> Result<String, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    let p = find_page(&tree, page)
        .ok_or_else(|| format!("no page at `{page}` and no page with that id"))?;
    let fm =
        p.fm.as_ref()
            .ok_or_else(|| format!("{}: no frontmatter (R-050)", p.rel))?;
    let id = fm
        .fields
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{}: no `id:` (R-050)", p.rel))?;
    let kind = fm.fields.get("type").and_then(Value::as_str).unwrap_or("?");
    if p.kind != Kind::Permanent || kind != "howto" {
        return Err(format!(
            "{}: a `{kind}` page — only a `howto` compiles into a skill (R-094); a fact \
             page is read, a procedure is run",
            p.rel
        ));
    }
    if tree.profile == Profile::KnowledgeBase
        && fm.fields.get("verification").and_then(Value::as_str) != Some("verified")
    {
        return Err(format!(
            "{}: `verification` is not `verified` — a knowledge-base howto compiles after \
             an independent audit, never from the session that wrote it (R-025, D-073)",
            p.rel
        ));
    }
    let body = body_of(p);
    let hash = content_hash(&body);
    let dir = claude_dir.join("skills").join(id);
    let file = dir.join("SKILL.md");
    if let Ok(existing) = fs::read_to_string(&file) {
        if !existing.contains(SOURCE_KEY) && !force {
            return Err(format!(
                "{} exists and was not compiled by docsys — `--force` overwrites it",
                file.display()
            ));
        }
    }
    let title = title_of(p, id);
    let summary = crate::export::summary_of(p);
    let summary = if summary.trim().is_empty() {
        title.clone()
    } else {
        summary
    };
    let description = format!(
        "{} — compiled from the howto `{id}`; run the steps as written, never improvise one.",
        summary.replace('"', "'")
    );
    let today = crate::migrate::today();
    let text = format!(
        "---\nname: {id}\ndescription: \"{description}\"\n{SOURCE_KEY}: {id}\n{HASH_KEY}: {hash}\n\
         docsys_compiled: {today}\n---\n\
         <!-- compiled by `docsys compile {id}` from {rel}: the body below is the page, byte \
         for byte. A step the page does not write is a gap to report, never to fill from \
         memory (R-096). When the page changes, lint reports this skill stale (R-095): \
         recompile, never edit here. -->\n\n{body}",
        rel = p.rel
    );
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(&file, text).map_err(|e| e.to_string())?;
    Ok(format!("compiled {} → {} ({hash})", p.rel, file.display()))
}

/// R-095 over every compiled skill under `<repo>/.claude/skills/`: the source
/// page must still exist and still hash to what the skill recorded.
pub fn check_compiled(tree: &DocTree, repo: &Path, r: &mut Report) {
    let mut inspected = 0usize;
    let skills = repo.join(".claude").join("skills");
    let mut dirs: Vec<_> = fs::read_dir(&skills)
        .map(|it| it.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    dirs.sort();
    for dir in dirs {
        let file = dir.join("SKILL.md");
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        let Some(fm) = crate::fm::parse(&text) else {
            continue;
        };
        let Some(src) = fm.fields.get(SOURCE_KEY).and_then(Value::as_str) else {
            continue; // an authored skill, not a compiled one
        };
        inspected += 1;
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let rel = format!(".claude/skills/{name}/SKILL.md");
        let recorded = fm
            .fields
            .get(HASH_KEY)
            .and_then(Value::as_str)
            .unwrap_or("");
        match find_page(tree, src) {
            None => r.findings.push(Finding::err(
                R095,
                &rel,
                src,
                format!(
                    "compiled from `{src}`, which no longer exists in the tree — recompile from \
                     its successor or delete the skill"
                ),
            )),
            Some(p) => {
                let now = content_hash(&body_of(p));
                if now != recorded {
                    r.findings.push(Finding::err(
                        R095,
                        &rel,
                        src,
                        format!(
                            "stale: `{src}` changed since this skill was compiled — re-read \
                             the page, then `docsys compile {src}`"
                        ),
                    ));
                }
            }
        }
    }
    r.inspected.insert("compiled-skills", inspected);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn body_is_everything_after_the_frontmatter() {
        let page = Page {
            rel: "howto/x.md".into(),
            kind: Kind::Permanent,
            text: "---\nid: x\ntype: howto\nupdated: 2026-01-01\n---\n# X\n\nStep one.\n".into(),
            fm: crate::fm::parse("---\nid: x\ntype: howto\nupdated: 2026-01-01\n---\n# X\n"),
        };
        assert_eq!(body_of(&page), "# X\n\nStep one.\n");
        assert_eq!(title_of(&page, "x"), "X");
    }
}
