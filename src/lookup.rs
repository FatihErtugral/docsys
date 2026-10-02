//! `docsys lookup` — the mechanical first hop of a question: which pages,
//! local or consumed, name these words? Deterministic and read-only:
//! identifiers, titles, tags, summaries and bodies, scored by where a word
//! occurs, and every word must occur somewhere. The model reads the page it
//! picks; the tool never answers. `raw/` is evidence, not an answer, so it is
//! never listed (D-074).

use std::fs;
use std::path::Path;

use crate::fm::Value;
use crate::tree::{DocTree, Kind, Page};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// `<id>` for a local page, `@<namespace>/<id>` for a consumed one
    pub token: String,
    /// `reference`, `howto`, … or the work category for a draft
    pub kind: String,
    pub rel: String,
    pub title: String,
    pub summary: String,
    /// what the reader must know before leaning on it: `status: draft`,
    /// `unverified`
    pub caveat: Option<String>,
    pub score: u32,
}

fn title_of(page: &Page, fallback: &str) -> String {
    page.text
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map_or_else(|| fallback.replace('-', " "), str::to_string)
}

/// Where each word occurs decides its weight; a word found nowhere drops the
/// page — a question's words all matter.
fn score(page: &Page, id: &str, title: &str, summary: &str, words: &[String]) -> Option<u32> {
    let tags = page
        .fm
        .as_ref()
        .and_then(|f| f.fields.get("tags"))
        .and_then(Value::as_list)
        .map(|l| l.join(" "))
        .unwrap_or_default()
        .to_lowercase();
    let id_l = id.to_lowercase();
    let title_l = title.to_lowercase();
    let summary_l = summary.to_lowercase();
    let body_l = page.text.to_lowercase();
    let mut total = 0;
    for w in words {
        let w = w.trim().to_lowercase();
        if w.is_empty() {
            continue;
        }
        total += if id_l.contains(&w) {
            8
        } else if title_l.contains(&w) {
            5
        } else if tags.contains(&w) {
            4
        } else if summary_l.contains(&w) {
            3
        } else if body_l.contains(&w) {
            1
        } else {
            return None;
        };
    }
    Some(total)
}

fn id_of(page: &Page) -> String {
    page.fm
        .as_ref()
        .and_then(|f| f.fields.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            page.rel
                .rsplit('/')
                .next()
                .unwrap_or(&page.rel)
                .trim_end_matches(".md")
                .to_string()
        })
}

fn kind_of(page: &Page) -> String {
    if let Some(t) = page
        .fm
        .as_ref()
        .and_then(|f| f.fields.get("type"))
        .and_then(Value::as_str)
    {
        return t.to_string();
    }
    // a work file: its category, singular
    let dir = page.rel.split('/').nth(1).unwrap_or("work");
    match dir {
        "features" => "feature".to_string(),
        "postmortems" => "postmortem".to_string(),
        other => other.to_string(),
    }
}

/// `blocks` is `Some` in a tree that keeps block records (§21), holding the
/// blocks whose bound pin is stale.
fn caveat_of(page: &Page, blocks: Option<&[String]>) -> Option<String> {
    let fm = page.fm.as_ref()?;
    if page.kind == Kind::Tracked {
        return fm
            .fields
            .get("status")
            .and_then(Value::as_str)
            .map(|s| format!("status: {s}"));
    }
    // how much of a page whose body or pins moved still reads as verified,
    // when a block record says (R-212, R-213)
    let partial = blocks
        .and_then(|stale| crate::blocks::reading(fm, &page.text, stale))
        .filter(crate::blocks::Reading::partial)
        .map(|r| {
            let by = fm
                .fields
                .get("verified_by")
                .and_then(Value::as_str)
                .unwrap_or("?");
            format!(" — {}/{} blocks as verified by {by}", r.found, r.of)
        })
        .unwrap_or_default();
    match fm.fields.get("verification").and_then(Value::as_str) {
        // a record with its body hash says, without history, whether the
        // page still holds what was verified (D-101)
        Some("verified") => {
            let moved = fm
                .fields
                .get("verified_hash")
                .and_then(Value::as_str)
                .is_some_and(|h| {
                    h != crate::fresh::content_hash(&crate::fresh::body_text(&page.text))
                });
            if moved {
                Some(format!("verified, but the body changed since{partial}"))
            } else if !partial.is_empty() {
                Some(format!("verified{partial}"))
            } else {
                None
            }
        }
        None => None,
        // an unverified page a machine checked says so, and whether the check
        // still holds the body (§21, R-214)
        Some(v) => Some(
            match fm.fields.get("checked_hash").and_then(Value::as_str) {
                Some(h)
                    if h == crate::fresh::content_hash(&crate::fresh::body_text(&page.text)) =>
                {
                    let by = fm
                        .fields
                        .get("checked_by")
                        .and_then(Value::as_str)
                        .unwrap_or("?");
                    format!("{v}{partial} · checked by {by}")
                }
                Some(_) => format!("{v}{partial} · check stale"),
                None => format!("{v}{partial}"),
            },
        ),
    }
}

fn hit_of(page: &Page, token: String, words: &[String], blocks: Option<&[String]>) -> Option<Hit> {
    let id = id_of(page);
    let title = title_of(page, &id);
    let summary = crate::export::summary_of(page);
    let score = score(page, &id, &title, &summary, words)?;
    Some(Hit {
        token,
        kind: kind_of(page),
        rel: page.rel.clone(),
        title,
        summary,
        caveat: caveat_of(page, blocks),
        score,
    })
}

/// Every page naming all the words — the tree's own permanent and tracked
/// pages, then each consumed namespace's materialized pages — best first.
pub fn lookup(root: &Path, words: &[String]) -> Result<Vec<Hit>, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    let era = crate::era::Era::of(&tree);
    let repo = era
        .block_records()
        .then(|| crate::repo_of(&tree.root))
        .flatten();
    let mut hits = Vec::new();
    for page in &tree.pages {
        let Some(fm) = page.fm.as_ref() else { continue };
        if !matches!(page.kind, Kind::Permanent | Kind::Tracked) {
            continue;
        }
        // only a page with a block record has blocks a stale pin can take away
        let stale = match &repo {
            Some(r) if crate::blocks::record_of(fm).is_some() => {
                crate::fresh::stale_blocks(&tree.root, r, era, fm)
            }
            _ => Vec::new(),
        };
        let blocks = era.block_records().then_some(stale.as_slice());
        if let Some(h) = hit_of(page, id_of(page), words, blocks) {
            hits.push(h);
        }
    }
    let fed = tree.root.join(".federation");
    let mut namespaces: Vec<_> = fs::read_dir(&fed)
        .map(|it| {
            it.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.is_dir()
                        && !p
                            .file_name()
                            .is_some_and(|n| n.to_string_lossy().starts_with('.'))
                })
                .collect()
        })
        .unwrap_or_default();
    namespaces.sort();
    for ns_dir in namespaces {
        let ns = ns_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut files: Vec<_> = fs::read_dir(&ns_dir)
            .map(|it| {
                it.filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|e| e == "md"))
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        for f in files {
            let Ok(text) = fs::read_to_string(&f) else {
                continue;
            };
            let stem = f
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let page = Page {
                rel: format!(".federation/{ns}/{stem}.md"),
                kind: Kind::Permanent,
                fm: crate::fm::parse(&text),
                text,
            };
            let id = id_of(&page);
            let blocks = era.block_records().then_some(&[][..]);
            if let Some(h) = hit_of(&page, format!("@{ns}/{id}"), words, blocks) {
                hits.push(h);
            }
        }
    }
    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.token.cmp(&b.token)));
    Ok(hits)
}

/// One line per hit: score, token, type, path, title, caveat.
pub fn render(hits: &[Hit], words: &[String]) -> String {
    if hits.is_empty() {
        return format!(
            "no page names: {} — not in the base; a question worth keeping is captured, \
             never answered from memory\n",
            words.join(" ")
        );
    }
    let mut out = String::new();
    for h in hits {
        let caveat = h
            .caveat
            .as_ref()
            .map(|c| format!("  ({c})"))
            .unwrap_or_default();
        out.push_str(&format!(
            "{:>3}  {}  {}  {}  — {}{caveat}\n",
            h.score, h.token, h.kind, h.rel, h.title
        ));
    }
    out.push_str(&format!("-- {} hit(s)\n", hits.len()));
    out
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

pub fn render_json(hits: &[Hit]) -> String {
    let items: Vec<String> = hits
        .iter()
        .map(|h| {
            format!(
                "{{\"token\":\"{}\",\"type\":\"{}\",\"path\":\"{}\",\"title\":\"{}\",\"summary\":\"{}\",\"caveat\":{},\"score\":{}}}",
                esc(&h.token),
                esc(&h.kind),
                esc(&h.rel),
                esc(&h.title),
                esc(&h.summary),
                h.caveat
                    .as_ref()
                    .map_or("null".to_string(), |c| format!("\"{}\"", esc(c))),
                h.score
            )
        })
        .collect();
    format!("{{\"hits\":[{}]}}\n", items.join(","))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn page(rel: &str, text: &str, kind: Kind) -> Page {
        Page {
            rel: rel.into(),
            kind,
            fm: crate::fm::parse(text),
            text: text.into(),
        }
    }

    #[test]
    fn every_word_must_occur_and_the_identifier_weighs_most() {
        let p = page(
            "reference/token-ttl.md",
            "---\nid: token-ttl\ntype: reference\nupdated: 2026-01-01\ntags: [auth]\n---\n# Token lifetime\n\nA token lives one hour.\n\nRenewal is silent.\n",
            Kind::Permanent,
        );
        let w = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        let h = hit_of(&p, "token-ttl".into(), &w("ttl"), None).unwrap();
        assert_eq!(h.score, 8);
        assert_eq!(h.title, "Token lifetime");
        assert_eq!(
            hit_of(&p, "token-ttl".into(), &w("lifetime"), None)
                .unwrap()
                .score,
            5
        );
        assert_eq!(
            hit_of(&p, "token-ttl".into(), &w("auth"), None)
                .unwrap()
                .score,
            4
        );
        assert_eq!(
            hit_of(&p, "token-ttl".into(), &w("renewal"), None)
                .unwrap()
                .score,
            1
        );
        assert!(hit_of(&p, "token-ttl".into(), &w("ttl banana"), None).is_none());
        assert!(h.caveat.is_none());
    }

    #[test]
    fn drafts_and_unverified_pages_carry_a_caveat() {
        let d = page(
            "work/features/jitter.md",
            "---\nid: jitter\nstatus: draft\nupdated: 2026-01-01\n---\n\n## Context\n\nSpread firings.\n",
            Kind::Tracked,
        );
        let h = hit_of(&d, "jitter".into(), &["jitter".into()], None).unwrap();
        assert_eq!(h.kind, "feature");
        assert_eq!(h.caveat.as_deref(), Some("status: draft"));
        let u = page(
            "wiki/ops/howto/x.md",
            "---\nid: x\ntype: howto\ndomain: ops\nverification: unverified\nupdated: 2026-01-01\nsources: []\n---\n# X\n",
            Kind::Permanent,
        );
        let h = hit_of(&u, "x".into(), &["x".into()], None).unwrap();
        assert_eq!(h.caveat.as_deref(), Some("unverified"));
    }
}
