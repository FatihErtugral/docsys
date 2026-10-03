//! `docsys graduate` — the mechanical half of graduation (§9). The command
//! moves bytes and maintains links and frontmatter; it never chooses what has
//! permanent value and never retypes text (R-090). The plan file is the
//! contract: the model fills the mapping, a human approves, apply executes.
//!
//! Plan format (D-024):
//!   # source: <work-file relative to the docs root>
//!   # block <n> · L<a>-L<b> · fnv:<16hex> · "<first-line snippet>"
//!   <n>\t<action>
//! Actions: `keep` — stays in the source · `link:<dest-path>` — the content
//! already exists on that permanent page, the block is replaced by a link
//! (P/R-092's "yes" branch) · `move:<dest-path>` — the block's bytes are
//! appended to that page verbatim and replaced by a link in the source.
//! The destination page must already exist with `id:` — preparing it is
//! R-099's authored step and is never done blindly by this command.

use crate::fm;
use crate::migrate::today;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

/// Plan-integrity checksum (FNV-1a 64). Not the R-113 content hash — this only
/// detects "the source changed between plan and apply", where refusing and
/// re-planning is the correct outcome (D-024).
fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

/// A block per R-098: a heading section (heading + content until the next
/// heading of equal-or-higher level), or the pre-heading body as block 0.
/// `body_start`/`body_end` are line indices; for a required template section
/// the heading line stays in place and only the body range moves.
pub struct Block {
    pub index: usize,
    pub start: usize,
    /// First moved line (== start except for kept template headings).
    pub body_start: usize,
    pub end: usize, // exclusive
    pub keep_heading: bool,
    pub snippet: String,
    pub checksum: u64,
}

const TEMPLATE_HEADINGS: [&str; 12] = [
    "## Context",
    "## Decision",
    "## Contract surface",
    "## Rejected alternatives",
    "## What happened",
    "## Root cause",
    "## Recurrence",
    "## Lesson",
    "## Question",
    "## Tried",
    "## Learned",
    "## Why no decision",
];

fn heading_level(line: &str) -> Option<usize> {
    let n = line.chars().take_while(|c| *c == '#').count();
    if n > 0 && line.get(n..).is_some_and(|r| r.starts_with(' ')) {
        Some(n)
    } else {
        None
    }
}

/// `.docmeta.yml` `headings:` — each template section's canonical name and the
/// name the tree shows (R-120).
fn heading_map(root: &Path) -> Vec<(String, String)> {
    crate::tree::docmeta_at(root)
        .and_then(|f| {
            f.fields
                .get("headings")
                .and_then(fm::Value::as_list)
                .map(<[String]>::to_vec)
        })
        .unwrap_or_default()
        .iter()
        .filter_map(|e| e.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

fn load_heading_map(root: &Path) -> Vec<String> {
    // Displayed forms of the template headings: canonical + any mapped names.
    let mut shown: Vec<String> = TEMPLATE_HEADINGS.iter().map(|s| (*s).to_string()).collect();
    shown.extend(
        heading_map(root)
            .into_iter()
            .map(|(_, v)| format!("## {v}")),
    );
    shown
}

/// The sections R-049 retains with the file: what history keeps when
/// graduation removes it (D-127).
const RETAINED: [&str; 4] = ["Context", "What happened", "Question", "Why no decision"];

fn retained_headings(root: &Path) -> Vec<String> {
    let mut shown: Vec<String> = RETAINED.iter().map(|s| format!("## {s}")).collect();
    shown.extend(
        heading_map(root)
            .into_iter()
            .filter(|(k, _)| RETAINED.contains(&k.as_str()))
            .map(|(_, v)| format!("## {v}")),
    );
    shown
}

/// A line `apply` wrote where a block left (R-091).
fn is_graduation_link(line: &str) -> bool {
    let line = line.trim();
    (line.starts_with("Moved to [[") || line.starts_with("Already documented: [["))
        && line.ends_with("]].")
}

pub fn blocks_with(text: &str, template_headings: &[String]) -> Vec<Block> {
    let lines: Vec<&str> = text.lines().collect();
    // Skip frontmatter.
    let mut i = 0usize;
    if lines.first() == Some(&"---") {
        i = 1;
        while i < lines.len() && lines.get(i) != Some(&"---") {
            i += 1;
        }
        i = (i + 1).min(lines.len());
    }
    let mut out = Vec::new();
    let cursor = i;
    let body_of = |a: usize, b: usize| -> String { lines.get(a..b).unwrap_or(&[]).join("\n") };
    // Block 0: content before the first heading (if any non-blank line).
    let first_heading = (i..lines.len())
        .find(|&j| lines.get(j).is_some_and(|l| heading_level(l).is_some()))
        .unwrap_or(lines.len());
    if lines
        .get(cursor..first_heading)
        .unwrap_or(&[])
        .iter()
        .any(|l| !l.trim().is_empty())
    {
        let body = body_of(cursor, first_heading);
        out.push(Block {
            index: out.len(),
            start: cursor,
            body_start: cursor,
            end: first_heading,
            keep_heading: false,
            snippet: body.lines().next().unwrap_or("").trim().to_string(),
            checksum: fnv(body.as_bytes()),
        });
    }
    // Every heading's section is a block (R-098) — nested sections included,
    // so a `##` inside a page-title `#` is addressable on its own. Overlapping
    // selections are rejected at apply time.
    for j in first_heading..lines.len() {
        let Some(line) = lines.get(j) else { continue };
        let Some(level) = heading_level(line) else {
            continue;
        };
        let mut end = j + 1;
        while end < lines.len() {
            if let Some(l) = lines.get(end) {
                if heading_level(l).is_some_and(|n| n <= level) {
                    break;
                }
            }
            end += 1;
        }
        let keep = template_headings.iter().any(|h| h == line.trim_end());
        let body_start = if keep { j + 1 } else { j };
        let body = body_of(body_start, end);
        out.push(Block {
            index: out.len(),
            start: j,
            body_start,
            end,
            keep_heading: keep,
            snippet: line.trim().to_string(),
            checksum: fnv(body.as_bytes()),
        });
    }
    out
}

pub fn blocks(text: &str) -> Vec<Block> {
    let canonical: Vec<String> = TEMPLATE_HEADINGS.iter().map(|s| (*s).to_string()).collect();
    blocks_with(text, &canonical)
}

/// R-092: in the knowledge-base profile graduation is distillation — an
/// authored rewrite of raw notes into a wiki page — not byte movement. A
/// command that moved bytes there would fake the one step that is judgment.
fn refuse_knowledge_base(root: &Path) -> Result<(), String> {
    if crate::hook::is_knowledge_base(root) {
        return Err(
            "knowledge-base graduation is distillation, not movement (R-092) — author the \
             wiki page, list the raw notes in `sources:`; nothing is moved or removed"
                .to_string(),
        );
    }
    Ok(())
}

pub fn plan(root: &Path, source_rel: &str) -> Result<String, String> {
    refuse_knowledge_base(root)?;
    let text = fs::read_to_string(root.join(source_rel)).map_err(|e| e.to_string())?;
    let bs = blocks_with(&text, &load_heading_map(root));
    if bs.is_empty() {
        return Err("the source has no blocks to graduate".to_string());
    }
    let mut out = String::from("# docsys graduation plan (D-024)\n");
    let _ = writeln!(out, "# source: {source_rel}");
    out.push_str(
        "# Fill the second column: keep | link:<dest-path> | move:<dest-path>\n\
         # A `move:` destination must already exist with an id (prepare it first, R-099).\n",
    );
    for b in &bs {
        let kept = if b.keep_heading {
            " · heading stays (R-048)"
        } else {
            ""
        };
        let _ = writeln!(
            out,
            "# block {} · L{}-L{} · fnv:{:016x}{} · \"{}\"",
            b.index,
            b.start + 1,
            b.end,
            b.checksum,
            kept,
            b.snippet
        );
        let _ = writeln!(out, "{}\tkeep", b.index);
    }
    Ok(out)
}

enum Action {
    Keep,
    Link(String),
    Move(String),
}

pub struct Outcome {
    pub moved: usize,
    pub linked: usize,
    pub dest_files: Vec<String>,
    /// the work file graduation removed (D-127)
    pub removed: Option<String>,
    /// the message the change is committed with when the file was removed
    pub message: Option<String>,
}

fn page_path(dest: &str) -> String {
    let dest = dest.trim();
    dest.strip_suffix(".md").unwrap_or(dest).to_string()
}

fn dest_id(root: &Path, dest: &str) -> Result<String, String> {
    let path = root.join(format!("{dest}.md"));
    let text = fs::read_to_string(&path)
        .map_err(|_| format!("destination `{dest}` does not exist — prepare it first (R-099)"))?;
    let parsed = fm::parse(&text)
        .ok_or_else(|| format!("destination `{dest}` has no frontmatter (R-050)"))?;
    parsed
        .fields
        .get("id")
        .and_then(fm::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("destination `{dest}` has no `id:` (R-050)"))
}

/// Update the source frontmatter: add `graduated_to` entries (R-091). The
/// field may sit on an `active` file — partial graduation is legal.
fn add_graduated_to(text: &str, ids: &[String]) -> Result<String, String> {
    if ids.is_empty() {
        return Ok(text.to_string());
    }
    let Some(parsed) = fm::parse(text).filter(|f| f.body_start > 0) else {
        return Ok(text.to_string());
    };
    fm::refuse_unclosed(&parsed)?;
    let mut items: Vec<String> = parsed
        .fields
        .get("graduated_to")
        .and_then(fm::Value::as_list)
        .map(<[String]>::to_vec)
        .unwrap_or_default();
    for id in ids {
        if !items.iter().any(|x| x == id) {
            items.push(id.clone());
        }
    }
    let line = format!("graduated_to: [{}]", items.join(", "));
    // the field's own lines give way to the one line; absent, it goes last
    let span = parsed
        .spans
        .get("graduated_to")
        .cloned()
        .unwrap_or(parsed.body_start - 1..parsed.body_start - 1);
    let mut out: Vec<String> = Vec::new();
    for (i, l) in text.lines().enumerate() {
        if i == span.start {
            out.push(line.clone());
        }
        if !span.contains(&i) {
            out.push(l.to_string());
        }
    }
    let mut out = out.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// R-093's question, asked mechanically before graduation removes the file
/// (D-127): every line of value outside the sections R-049 retains leaves
/// with a moved or linked block, or left before as a link. The innermost
/// block holding the first line that would stay.
fn staying_block(
    lines: &[&str],
    bs: &[Block],
    leaving: &[(usize, usize)],
    retained: &[String],
) -> Option<usize> {
    let from = bs.iter().map(|b| b.start).min()?;
    let retained_ranges: Vec<(usize, usize)> = bs
        .iter()
        .filter(|b| {
            lines
                .get(b.start)
                .is_some_and(|h| retained.iter().any(|r| r == h.trim_end()))
        })
        .map(|b| (b.start, b.end))
        .collect();
    let within = |ranges: &[(usize, usize)], i: usize| ranges.iter().any(|&(a, b)| a <= i && i < b);
    let stays = lines.iter().enumerate().skip(from).find(|(i, line)| {
        !(line.trim().is_empty()
            || heading_level(line).is_some()
            || is_graduation_link(line)
            || within(leaving, *i)
            || within(&retained_ranges, *i))
    });
    let (i, _) = stays?;
    bs.iter()
        .filter(|b| b.start <= i && i < b.end)
        .max_by_key(|b| b.start)
        .map(|b| b.index)
}

/// Graduation as before: the blocks move, the source keeps its links and
/// `graduated_to` (R-091).
pub fn apply(root: &Path, plan_text: &str, force: bool) -> Result<Outcome, String> {
    run(root, plan_text, force, None)
}

/// Graduation's end on a docsys/0.5 tree (D-127): the blocks move and the
/// work file is removed on `who`'s word (R-081), once nothing of value would
/// leave with it (R-093).
pub fn apply_confirmed(
    root: &Path,
    plan_text: &str,
    force: bool,
    who: &str,
) -> Result<Outcome, String> {
    run(root, plan_text, force, Some(who.trim()))
}

fn run(
    root: &Path,
    plan_text: &str,
    force: bool,
    confirmed: Option<&str>,
) -> Result<Outcome, String> {
    refuse_knowledge_base(root)?;
    let tree = match confirmed {
        Some(who) => {
            if !crate::era::Era::at(root).graduation_removes() {
                return Err(format!(
                    "this tree {}, where a graduated file stays: graduate without --confirmed and \
                     record `confirmed:` on the file (R-081); removing it is docsys/0.5's end of \
                     graduation (D-127)",
                    crate::era::served(root)
                ));
            }
            let tree = crate::tree::DocTree::load(root).map_err(|e| e.to_string())?;
            let maintainers = crate::checks::maintainer_handles(&tree);
            maintainers.readable()?;
            if crate::approval::maintainer_of(&maintainers, who).is_none() {
                return Err(format!(
                    "`{who}` names no maintainer in .docmeta.yml — the word that graduates a file is a maintainer's (R-208)"
                ));
            }
            Some(tree)
        }
        None => None,
    };
    // R-097: refuse a dirty tree unless forced (only when git is present).
    if !force {
        let dirty = crate::git::cmd(root)
            .args(["status", "--porcelain"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);
        if dirty {
            return Err(
                "working tree is dirty — commit or stash first, or pass --force (R-097)"
                    .to_string(),
            );
        }
    }

    let source_rel = plan_text
        .lines()
        .find_map(|l| l.strip_prefix("# source: "))
        .ok_or("plan has no `# source:` line")?
        .trim()
        .to_string();
    let source_path = root.join(&source_rel);
    let text = fs::read_to_string(&source_path).map_err(|e| e.to_string())?;
    let bs = blocks_with(&text, &load_heading_map(root));

    // Parse and validate actions against the current blocks.
    let mut actions: Vec<(usize, Action)> = Vec::new();
    for (ln, line) in plan_text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((idx, act)) = line.split_once('\t') else {
            return Err(format!("plan line {}: not `<n><TAB><action>`", ln + 1));
        };
        let idx: usize = idx
            .trim()
            .parse()
            .map_err(|_| format!("plan line {}: bad block index", ln + 1))?;
        if idx >= bs.len() {
            return Err(format!(
                "plan line {}: block {idx} does not exist — the source changed; re-plan",
                ln + 1
            ));
        }
        let action = match act.trim() {
            "keep" => Action::Keep,
            // a destination is a page path, with its `.md` or without
            a if a.starts_with("link:") => Action::Link(page_path(&a[5..])),
            a if a.starts_with("move:") => Action::Move(page_path(&a[5..])),
            other => return Err(format!("plan line {}: unknown action `{other}`", ln + 1)),
        };
        actions.push((idx, action));
    }
    // Checksums in plan comments guard against drift.
    for line in plan_text.lines() {
        if let Some(rest) = line.strip_prefix("# block ") {
            let idx: usize = rest
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(usize::MAX);
            if let Some(hex) = rest.split("fnv:").nth(1).and_then(|s| s.get(..16)) {
                let planned = u64::from_str_radix(hex, 16).unwrap_or(0);
                if let Some(b) = bs.get(idx) {
                    if b.checksum != planned {
                        return Err(format!(
                            "block {idx} changed since the plan was written — re-plan (D-024)"
                        ));
                    }
                }
            }
        }
    }

    // Overlapping selections (a parent section and its child both acted on)
    // are a conflict, not a merge (R-098 nested enumeration).
    {
        let mut chosen: Vec<(usize, usize)> = actions
            .iter()
            .filter(|(_, a)| !matches!(a, Action::Keep))
            .filter_map(|(i, _)| bs.get(*i).map(|b| (b.body_start, b.end)))
            .collect();
        chosen.sort_unstable();
        for w in chosen.windows(2) {
            if let [a, b] = w {
                if b.0 < a.1 {
                    return Err(
                        "plan selects overlapping blocks (a section and its subsection) — pick one"
                            .to_string(),
                    );
                }
            }
        }
    }

    // Stage everything in memory first (R-097: all writes land or none do).
    let lines: Vec<&str> = text.lines().collect();
    let mut dest_appends: Vec<(String, String)> = Vec::new(); // dest rel → bytes
    let mut replacements: Vec<(usize, usize, String)> = Vec::new(); // line range → text
    let mut new_ids: Vec<String> = Vec::new();
    let mut out = Outcome {
        moved: 0,
        linked: 0,
        dest_files: Vec::new(),
        removed: None,
        message: None,
    };

    for (idx, action) in &actions {
        let Some(b) = bs.get(*idx) else { continue };
        match action {
            Action::Keep => {}
            Action::Link(dest) => {
                let id = dest_id(root, dest)?;
                let link = format!("Already documented: [[{dest}|{id}]].");
                replacements.push((b.body_start, b.end, link));
                if !new_ids.contains(&id) {
                    new_ids.push(id);
                }
                out.linked += 1;
            }
            Action::Move(dest) => {
                let id = dest_id(root, dest)?;
                let body = lines.get(b.body_start..b.end).unwrap_or(&[]).join("\n");
                dest_appends.push((dest.clone(), body));
                let link = format!("Moved to [[{dest}|{id}]].");
                replacements.push((b.body_start, b.end, link));
                if !new_ids.contains(&id) {
                    new_ids.push(id);
                }
                out.moved += 1;
            }
        }
    }

    // Rebuild the source with replacements applied bottom-up.
    let mut src_lines: Vec<String> = lines.iter().map(|s| (*s).to_string()).collect();
    replacements.sort_by_key(|r| std::cmp::Reverse(r.0));
    for (start, end, replacement) in &replacements {
        src_lines.splice(*start..*end, [replacement.clone(), String::new()]);
    }
    let mut new_source = src_lines.join("\n");
    if text.ends_with('\n') && !new_source.ends_with('\n') {
        new_source.push('\n');
    }
    new_source = add_graduated_to(&new_source, &new_ids)?;

    if let (Some(who), Some(tree)) = (confirmed, &tree) {
        let source = tree
            .pages
            .iter()
            .find(|p| p.rel == source_rel && p.kind == crate::tree::Kind::Tracked)
            .ok_or_else(|| {
                format!("`{source_rel}` is no work file — graduation removes work files only")
            })?;
        let field = |k: &str| source.fm.as_ref().and_then(|f| f.fields.get(k)).cloned();
        if field("status").as_ref().and_then(fm::Value::as_str) == Some("graduated") {
            return Err(format!(
                "`{source_rel}` is already `graduated` and keeps its place (R-082)"
            ));
        }
        let leaving: Vec<(usize, usize)> = replacements.iter().map(|(a, b, _)| (*a, *b)).collect();
        if let Some(b) =
            staying_block(&lines, &bs, &leaving, &retained_headings(root)).and_then(|i| bs.get(i))
        {
            return Err(format!(
                "block {} · L{}-L{} · \"{}\" would leave with the file — move or link it, or graduate \
                 without --confirmed and keep the file (R-093)",
                b.index,
                b.start + 1,
                b.end,
                b.snippet
            ));
        }
        let mut dests: Vec<String> = field("graduated_to")
            .as_ref()
            .and_then(fm::Value::as_list)
            .map(<[String]>::to_vec)
            .unwrap_or_default();
        for id in &new_ids {
            if !dests.contains(id) {
                dests.push(id.clone());
            }
        }
        if dests.is_empty() {
            return Err(
                "no block of this file moves or links anywhere — graduation needs a destination; \
                 work that ends without one is `abandoned` (R-055)"
                    .to_string(),
            );
        }
        let id = field("id")
            .as_ref()
            .and_then(fm::Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                Path::new(&source_rel)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .unwrap_or_default();
        out.removed = Some(source_rel.clone());
        out.message = Some(format!(
            "docs: graduate {id} into {}\n\nConfirmed-by: {who}\n",
            dests.join(", ")
        ));
    }

    // Destinations first (R-092's ordering), then the source shrink.
    for (dest, body) in &dest_appends {
        let path = root.join(format!("{dest}.md"));
        let mut existing = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if !existing.ends_with('\n') {
            existing.push('\n');
        }
        existing.push('\n');
        existing.push_str(body);
        existing.push('\n');
        // a docsys/0.4 page's `updated:` is tool-maintained; a 0.5 page's
        // date is history's (D-122)
        if !crate::era::Era::at(root).derived_dates() {
            existing = crate::hook::bump_updated(&existing, &today()).unwrap_or(existing);
        }
        fs::write(&path, existing).map_err(|e| e.to_string())?;
        if !out.dest_files.contains(dest) {
            out.dest_files.push(dest.clone());
        }
    }
    if out.removed.is_some() {
        fs::remove_file(&source_path).map_err(|e| e.to_string())?;
    } else {
        fs::write(&source_path, new_source).map_err(|e| e.to_string())?;
    }
    Ok(out)
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

    #[test]
    fn fnv_is_stable_and_sensitive() {
        assert_eq!(fnv(b"abc"), fnv(b"abc"));
        assert_ne!(fnv(b"abc"), fnv(b"abd"));
        assert_ne!(fnv(b""), fnv(b"a"));
    }

    #[test]
    fn heading_levels() {
        assert_eq!(heading_level("# a"), Some(1));
        assert_eq!(heading_level("### a"), Some(3));
        assert_eq!(heading_level("#nospace"), None);
        assert_eq!(heading_level("text # a"), None);
        assert_eq!(heading_level(""), None);
    }

    #[test]
    fn graduated_to_is_added_or_merged_inside_the_frontmatter() {
        let ids = vec!["ref-a".to_string(), "ref-b".to_string()];
        assert_eq!(
            add_graduated_to("no frontmatter\n", &ids).unwrap(),
            "no frontmatter\n"
        );
        assert_eq!(
            add_graduated_to("---\nid: x\n---\nbody\n", &[]).unwrap(),
            "---\nid: x\n---\nbody\n"
        );
        let added = add_graduated_to("---\nid: x\n---\nbody\n", &ids).unwrap();
        let close = added.lines().skip(1).position(|l| l == "---").unwrap() + 1;
        let fm: Vec<&str> = added.lines().take(close).collect();
        let line = fm
            .iter()
            .find(|l| l.starts_with("graduated_to:"))
            .expect("added inside the block");
        assert!(line.contains("ref-a") && line.contains("ref-b"), "{line}");
        assert!(added.ends_with("body\n"));
        let merged = add_graduated_to(&added, &["ref-c".to_string()]).unwrap();
        let line = merged
            .lines()
            .find(|l| l.starts_with("graduated_to:"))
            .unwrap();
        assert!(line.contains("ref-a") && line.contains("ref-c"), "{line}");
        assert_eq!(merged.matches("graduated_to:").count(), 1);
    }

    #[test]
    fn graduated_to_refuses_an_unclosed_list_and_writes_nothing() {
        let open = "---\nid: x\ngraduated_to: [ref-a,\nstatus: done\n---\nbody\n";
        let err = add_graduated_to(open, &["ref-b".to_string()]).unwrap_err();
        assert!(err.contains("`graduated_to` on line 3"), "{err}");
    }

    #[test]
    fn graduated_to_merges_the_value_the_parser_reads() {
        let block = "---\nid: x\ngraduated_to:\n  - ref-a   # the first\nstatus: done\n---\nbody\n";
        let merged = add_graduated_to(block, &["ref-b".to_string()]).unwrap();
        assert_eq!(
            merged,
            "---\nid: x\ngraduated_to: [ref-a, ref-b]\nstatus: done\n---\nbody\n"
        );
        let commented = add_graduated_to(
            "---\nid: x\ngraduated_to: [ref-a]  # so far\n---\n",
            &["ref-a".to_string()],
        )
        .unwrap();
        assert_eq!(commented, "---\nid: x\ngraduated_to: [ref-a]\n---\n");
    }
}
