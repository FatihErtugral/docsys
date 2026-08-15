//! `docsys refs` — validate `doc: <id>` references in the code base against
//! the documentation tree (R-072/R-073/R-076/R-079). This is the check that
//! makes the identifier a real contract: a typo in a code comment stops being
//! invisible the day this runs in CI.

use crate::checks::{self, doc_tokens_on_line, resolve_doc_token, DocRefFail, Report, Resolved};
use crate::migrate::repo_text_files;
use crate::model::{Finding, RuleId};
use crate::tree::DocTree;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const R073: RuleId = RuleId("R-073");
const R076: RuleId = RuleId("R-076");
const R194: RuleId = RuleId("R-194");

/// Scan the repository (excluding the docs tree itself — lint owns that side)
/// and resolve every `doc:` token against the tree's identifiers.
pub fn run(repo: &Path, tree: &DocTree) -> Report {
    let mut r = Report {
        findings: Vec::new(),
        inspected: BTreeMap::new(),
    };
    let idx = checks::build_index(tree);
    // R-077: the tree declares its scan scope; `scan_exclude` prefixes are the
    // owner's word on what is out of bounds (archived sub-projects, tooling).
    let excludes: Vec<String> = tree.docmeta_list("scan_exclude").to_vec();
    let mut files_scanned = 0usize;
    let mut tokens = 0usize;

    for file in repo_text_files(repo, &tree.root) {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        files_scanned += 1;
        let frel = file
            .strip_prefix(repo)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        if excludes
            .iter()
            .any(|p| frel.starts_with(p.trim_end_matches('/')))
        {
            files_scanned -= 1;
            continue;
        }
        for (i, line) in text.lines().enumerate() {
            for token in doc_tokens_on_line(line) {
                tokens += 1;
                match resolve_doc_token(&idx, &token) {
                    Ok(Resolved::Permanent) => {}
                    // Code asking a graduated page for an answer is the sharp
                    // case: "no longer the source of truth" and "the code reads
                    // it" cannot both be true (R-194).
                    Ok(Resolved::Graduated) => r.findings.push(Finding::err(
                        R194,
                        &frel,
                        &token,
                        format!(
                            "line {}: `doc: {token}` points at a graduated page — cite the \
                             destination it graduated to",
                            i + 1
                        ),
                    )),
                    Ok(Resolved::Flowing) => r.findings.push(Finding::warn(
                        R194,
                        &frel,
                        &token,
                        format!(
                            "line {}: `doc: {token}` cites the flowing layer — the page is real \
                             but temporary; distil it",
                            i + 1
                        ),
                    )),
                    Err(DocRefFail::Foreign) => r.findings.push(Finding::warn(
                        R076,
                        &frel,
                        &token,
                        "foreign reference — unresolvable here (federation is experimental)"
                            .to_string(),
                    )),
                    Err(DocRefFail::BadGrammar) => r.findings.push(Finding::warn(
                        R073,
                        &frel,
                        &token,
                        format!("line {}: `{token}` is not a local-id", i + 1),
                    )),
                    Err(DocRefFail::Dangling) => r.findings.push(Finding::err(
                        R076,
                        &frel,
                        &token,
                        format!(
                            "line {}: `doc: {token}` resolves to no id, alias, or family member",
                            i + 1
                        ),
                    )),
                }
            }
        }
    }

    // Stray docs page: a markdown file OUTSIDE the docs tree carrying an
    // `id:` frontmatter claim — it looks like a page, but no check governs it
    // (behavior carried over from the founding estate's check_stray_docs).
    for file in repo_text_files(repo, &tree.root) {
        let frel = file
            .strip_prefix(repo)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        if !frel.ends_with(".md")
            || excludes
                .iter()
                .any(|p| frel.starts_with(p.trim_end_matches('/')))
        {
            continue;
        }
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        if let Some(fm) = crate::fm::parse(&text) {
            if fm.fields.contains_key("id") {
                r.findings.push(Finding::warn(
                    RuleId("R-020"),
                    &frel,
                    "stray",
                    "markdown page with an `id:` living outside the docs tree —                      no check governs it there; move it in, or drop the identity claim"
                        .to_string(),
                ));
            }
        }
    }

    r.inspected.insert("code-files", files_scanned);
    r.inspected.insert("code-doc-refs", tokens);
    r.findings.sort();
    r.findings.dedup();
    r
}
