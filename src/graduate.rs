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

fn load_heading_map(root: &Path) -> Vec<String> {
    // Displayed forms of the template headings: canonical + any mapped names.
    let mut shown: Vec<String> = TEMPLATE_HEADINGS.iter().map(|s| (*s).to_string()).collect();
    if let Ok(text) = fs::read_to_string(root.join(".docmeta.yml")) {
        let framed = format!("---\n{text}---\n");
        if let Some(f) = fm::parse(&framed) {
            if let Some(list) = f.fields.get("headings").and_then(fm::Value::as_list) {
                for e in list {
                    if let Some((_, v)) = e.split_once('=') {
                        shown.push(format!("## {}", v.trim()));
                    }
                }
            }
        }
    }
    shown
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
    let mut cursor = i;
    let body_of = |a: usize, b: usize| -> String {
        lines.get(a..b).unwrap_or(&[]).join("\n")
    };
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
    cursor = first_heading;
    while cursor < lines.len() {
        let Some(line) = lines.get(cursor) else { break };
        let Some(level) = heading_level(line) else {
            cursor += 1;
            continue;
        };
        let mut end = cursor + 1;
        while end < lines.len() {
            if let Some(l) = lines.get(end) {
                if heading_level(l).is_some_and(|n| n <= level) {
                    break;
                }
            }
            end += 1;
        }
        let keep = template_headings.iter().any(|h| h == line.trim_end());
        let body_start = if keep { cursor + 1 } else { cursor };
        let body = body_of(body_start, end);
        out.push(Block {
            index: out.len(),
            start: cursor,
            body_start,
            end,
            keep_heading: keep,
            snippet: line.trim().to_string(),
            checksum: fnv(body.as_bytes()),
        });
        cursor = end;
    }
    out
}

pub fn blocks(text: &str) -> Vec<Block> {
    let canonical: Vec<String> = TEMPLATE_HEADINGS.iter().map(|s| (*s).to_string()).collect();
    blocks_with(text, &canonical)
}

pub fn plan(root: &Path, source_rel: &str) -> Result<String, String> {
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
        let kept = if b.keep_heading { " · heading stays (R-048)" } else { "" };
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
fn add_graduated_to(text: &str, ids: &[String]) -> String {
    if ids.is_empty() {
        return text.to_string();
    }
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let close = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| l.as_str() == "---")
        .map(|(i, _)| i);
    let Some(close) = close else { return text.to_string() };
    let existing = lines
        .iter()
        .take(close)
        .position(|l| l.starts_with("graduated_to:"));
    match existing {
        Some(i) => {
            let line = lines.get(i).cloned().unwrap_or_default();
            let inner = line
                .trim_start_matches("graduated_to:")
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim();
            let mut items: Vec<String> = if inner.is_empty() {
                Vec::new()
            } else {
                inner.split(',').map(|s| s.trim().to_string()).collect()
            };
            for id in ids {
                if !items.iter().any(|x| x == id) {
                    items.push(id.clone());
                }
            }
            if let Some(slot) = lines.get_mut(i) {
                *slot = format!("graduated_to: [{}]", items.join(", "));
            }
        }
        None => lines.insert(close, format!("graduated_to: [{}]", ids.join(", "))),
    }
    let mut out = lines.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub fn apply(root: &Path, plan_text: &str, force: bool) -> Result<Outcome, String> {
    // R-097: refuse a dirty tree unless forced (only when git is present).
    if !force {
        let dirty = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
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
            a if a.starts_with("link:") => Action::Link(a[5..].trim().to_string()),
            a if a.starts_with("move:") => Action::Move(a[5..].trim().to_string()),
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

    // Stage everything in memory first (R-097: all writes land or none do).
    let lines: Vec<&str> = text.lines().collect();
    let mut dest_appends: Vec<(String, String)> = Vec::new(); // dest rel → bytes
    let mut replacements: Vec<(usize, usize, String)> = Vec::new(); // line range → text
    let mut new_ids: Vec<String> = Vec::new();
    let mut out = Outcome {
        moved: 0,
        linked: 0,
        dest_files: Vec::new(),
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
                let body = lines
                    .get(b.body_start..b.end)
                    .unwrap_or(&[])
                    .join("\n");
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
    new_source = add_graduated_to(&new_source, &new_ids);

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
        // updated: is tool-maintained (R-052).
        let bumped = existing
            .lines()
            .map(|l| {
                if l.starts_with("updated:") {
                    format!("updated: {}", today())
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        fs::write(&path, bumped).map_err(|e| e.to_string())?;
        if !out.dest_files.contains(dest) {
            out.dest_files.push(dest.clone());
        }
    }
    fs::write(&source_path, new_source).map_err(|e| e.to_string())?;
    Ok(out)
}
