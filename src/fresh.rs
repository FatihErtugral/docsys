//! §11 freshness — the only mechanics in the specification that catch
//! code–documentation drift without a person noticing it first.
//!
//! - `verifies:` pins (R-110–R-114): a permanent page names a code region
//!   and its content hash; lint recomputes the hash and the page is stale
//!   when the region moved. SHA-256 is implemented here, zero-dep (D-001).
//! - history (R-085, R-106): one `git log` walk gives every page its last
//!   content change. `updated:` older than that is a hand edit that skipped
//!   the tooling; a draft nobody touched for `stale_active_days` is
//!   undeclared abandonment.
//!
//! Every finding here is an error (D-070): a pin is a promise the author
//! made, the freshness field is the one thing a reader cannot check by hand,
//! and the fix is always one field or one command.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::checks::Report;
use crate::fm::{Frontmatter, Value};
use crate::model::{is_iso_date, Finding, RuleId};
use crate::tree::{DocTree, Kind};

const R085: RuleId = RuleId("R-085");
const R106: RuleId = RuleId("R-106");
const R111: RuleId = RuleId("R-111");
const R113: RuleId = RuleId("R-113");
const R114: RuleId = RuleId("R-114");

// ---------------------------------------------------------------- SHA-256

const K: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

const H0: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

fn at(w: &[u32; 64], i: usize) -> u32 {
    w.get(i).copied().unwrap_or(0)
}

fn set(w: &mut [u32; 64], i: usize, v: u32) {
    if let Some(slot) = w.get_mut(i) {
        *slot = v;
    }
}

/// SHA-256 (FIPS 180-4) over `data`.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = H0;
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    let (blocks, _) = msg.as_chunks::<64>();
    for chunk in blocks {
        let mut w = [0u32; 64];
        let (words, _) = chunk.as_chunks::<4>();
        for (i, word) in words.iter().enumerate() {
            set(&mut w, i, u32::from_be_bytes(*word));
        }
        for i in 16..64 {
            let w15 = at(&w, i - 15);
            let w2 = at(&w, i - 2);
            let s0 = w15.rotate_right(7) ^ w15.rotate_right(18) ^ (w15 >> 3);
            let s1 = w2.rotate_right(17) ^ w2.rotate_right(19) ^ (w2 >> 10);
            let v = at(&w, i - 16)
                .wrapping_add(s0)
                .wrapping_add(at(&w, i - 7))
                .wrapping_add(s1);
            set(&mut w, i, v);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for (i, k) in K.iter().enumerate() {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(*k)
                .wrapping_add(at(&w, i));
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(v);
        }
    }
    let mut out = [0u8; 32];
    let bytes: Vec<u8> = h.iter().flat_map(|v| v.to_be_bytes()).collect();
    for (slot, b) in out.iter_mut().zip(bytes) {
        *slot = b;
    }
    out
}

pub fn sha256_hex(data: &[u8]) -> String {
    sha256(data).iter().map(|b| format!("{b:02x}")).collect()
}

/// R-113's canonical form: LF line endings, trailing whitespace removed from
/// every line, exactly one trailing LF. NFC normalization is not applied
/// (D-068): the bytes are hashed as written.
pub fn canonical(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 1);
    for line in text.replace("\r\n", "\n").split('\n') {
        out.push_str(line.trim_end());
        out.push('\n');
    }
    let body = out.trim_end_matches('\n');
    format!("{body}\n")
}

/// `sha256:<hex>` over the canonical form (R-113).
pub fn content_hash(text: &str) -> String {
    format!("sha256:{}", sha256_hex(canonical(text).as_bytes()))
}

// ---------------------------------------------------------------- regions

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Byte position of `sym` as a whole word in `line`, if any.
fn whole_word(line: &str, sym: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(rel) = line.get(from..)?.find(sym) {
        let pos = from + rel;
        let before = line.get(..pos).and_then(|s| s.chars().next_back());
        let after = line.get(pos + sym.len()..).and_then(|s| s.chars().next());
        if !before.is_some_and(is_ident) && !after.is_some_and(is_ident) {
            return Some(pos);
        }
        from = pos + sym.len();
    }
    None
}

/// Does this line open the definition of `sym`? (D-069: `def`/`class` for
/// Python; for brace languages a whole-word occurrence on a line that opens a
/// block — `{` on the line or the next — and is not a statement.)
fn is_header(line: &str, sym: &str, py: bool, next: Option<&str>) -> bool {
    let t = line.trim_start();
    if t.starts_with("//")
        || t.starts_with('*')
        || t.starts_with("use ")
        || t.starts_with("import ")
        || t.starts_with("from ")
        || (!py && t.starts_with('#'))
    {
        return false;
    }
    if py {
        let head = t.strip_prefix("async ").unwrap_or(t).trim_start();
        let rest = head
            .strip_prefix("def ")
            .or_else(|| head.strip_prefix("class "))
            .map(str::trim_start);
        return rest.is_some_and(|r| {
            r.starts_with(sym)
                && !r
                    .get(sym.len()..)
                    .and_then(|s| s.chars().next())
                    .is_some_and(is_ident)
        });
    }
    let Some(pos) = whole_word(line, sym) else {
        return false;
    };
    let after = line.get(pos + sym.len()..).unwrap_or("");
    let ends_statement = line.trim_end().ends_with(';');
    let opens_here = after.contains('{');
    let opens_next = next.is_some_and(|n| n.trim_start().starts_with('{'));
    !ends_statement && (opens_here || opens_next)
}

fn indent_of(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').count()
}

/// The region a symbol covers, from its header line: a brace block to the
/// matching `}`, or an indentation block for Python.
fn extract(lines: &[&str], start: usize, py: bool) -> String {
    let mut end = start;
    if py {
        let base = lines.get(start).map_or(0, |l| indent_of(l));
        let mut last_content = start;
        for (i, l) in lines.iter().enumerate().skip(start + 1) {
            if l.trim().is_empty() {
                continue;
            }
            if indent_of(l) <= base {
                break;
            }
            last_content = i;
        }
        end = last_content;
    } else {
        let mut depth: i64 = 0;
        let mut opened = false;
        'outer: for (i, l) in lines.iter().enumerate().skip(start) {
            for c in l.chars() {
                match c {
                    '{' => {
                        depth += 1;
                        opened = true;
                    }
                    '}' => {
                        depth -= 1;
                        if opened && depth <= 0 {
                            end = i;
                            break 'outer;
                        }
                    }
                    _ => {}
                }
            }
            end = i;
        }
    }
    lines.get(start..=end).unwrap_or(&[]).join("\n")
}

/// The text a pin covers: the whole file, or the block that defines `symbol`.
/// A symbol that is absent or ambiguous is an error, never a guess (R-114).
pub fn region(source: &str, path: &str, symbol: Option<&str>) -> Result<String, String> {
    let Some(sym) = symbol.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(source.to_string());
    };
    let py = path.ends_with(".py");
    let lines: Vec<&str> = source.lines().collect();
    let candidates: Vec<usize> = (0..lines.len())
        .filter(|&i| {
            lines
                .get(i)
                .is_some_and(|l| is_header(l, sym, py, lines.get(i + 1).copied()))
        })
        .collect();
    match candidates.as_slice() {
        [] => Err(format!("symbol `{sym}` not found in `{path}`")),
        [one] => Ok(extract(&lines, *one, py)),
        many => Err(format!(
            "symbol `{sym}` is ambiguous in `{path}`: lines {}",
            many.iter()
                .map(|i| (i + 1).to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

// ---------------------------------------------------------------- pins

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    pub path: String,
    pub symbol: Option<String>,
    pub hash: String,
}

impl Pin {
    fn label(&self) -> String {
        match &self.symbol {
            Some(s) => format!("{}#{s}", self.path),
            None => self.path.clone(),
        }
    }
}

/// The `verifies:` entries of a page (the §11 grammar: a list of maps).
pub fn pins_of(fm: &Frontmatter) -> Vec<Pin> {
    fm.fields
        .get("verifies")
        .and_then(Value::as_maps)
        .map(|maps| {
            maps.iter()
                .filter_map(|m| {
                    let path = m.get("path")?.trim().to_string();
                    Some(Pin {
                        path,
                        symbol: m.get("symbol").map(|s| s.trim().to_string()),
                        hash: m
                            .get("hash")
                            .map(|h| h.trim().to_string())
                            .unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The hash a pin should carry now, or why it cannot be computed.
fn current_hash(repo: &Path, pin: &Pin) -> Result<String, (RuleId, String)> {
    let file = repo.join(&pin.path);
    let source = fs::read_to_string(&file).map_err(|_| {
        (
            R111,
            format!(
                "pins `{}`, which no longer exists — the region is gone; re-read the page, then \
                 `docsys pin --refresh` or drop the pin",
                pin.label()
            ),
        )
    })?;
    let text = region(&source, &pin.path, pin.symbol.as_deref()).map_err(|e| (R114, e))?;
    Ok(content_hash(&text))
}

/// R-110/R-111/R-113/R-114 over every pinned permanent page.
pub fn check_pins(tree: &DocTree, repo: &Path, r: &mut Report) {
    let mut inspected = 0usize;
    for page in &tree.pages {
        if page.kind != Kind::Permanent {
            continue;
        }
        let Some(fm) = &page.fm else { continue };
        for pin in pins_of(fm) {
            inspected += 1;
            if !pin.hash.starts_with("sha256:") || pin.hash.len() != 71 {
                r.findings.push(Finding::err(
                    R113,
                    &page.rel,
                    &pin.label(),
                    format!(
                        "hash `{}` is not `sha256:<64 hex>` — `docsys pin {} {}` writes it",
                        pin.hash, page.rel, pin.path
                    ),
                ));
                continue;
            }
            match current_hash(repo, &pin) {
                Err((rule, msg)) => {
                    r.findings
                        .push(Finding::err(rule, &page.rel, &pin.label(), msg))
                }
                Ok(now) if now != pin.hash => r.findings.push(Finding::err(
                    R111,
                    &page.rel,
                    &pin.label(),
                    format!(
                        "stale: `{}` moved since the page was pinned — re-read the page, then \
                         `docsys pin --refresh {}`",
                        pin.label(),
                        page.rel
                    ),
                )),
                Ok(_) => {}
            }
        }
    }
    r.inspected.insert("verifies-pins", inspected);
}

// ---------------------------------------------------------------- history

/// Days since 1970-01-01 for a civil date (Howard Hinnant's days_from_civil).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn days_of(iso: &str) -> Option<i64> {
    let mut it = iso.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    Some(days_from_civil(y, m, d))
}

/// What version control knows about the tree: the last content change of
/// every path under the docs root (D-071: one `git log` walk, committer
/// dates, renames not followed).
#[derive(Debug, Default)]
pub struct History {
    /// repo-relative path → ISO date of its latest commit
    pub last_change: BTreeMap<String, String>,
    /// the docs root as git names it (`docs`, or empty for a root-level base)
    pub root_rel: String,
    pub today: String,
}

/// The docs root relative to the repository, `/`-separated; empty when they
/// coincide.
pub fn root_rel(repo: &Path, root: &Path) -> String {
    let repo_c = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let root_c = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    root_c
        .strip_prefix(&repo_c)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| root.to_string_lossy().replace('\\', "/"))
        .trim_matches('/')
        .to_string()
}

impl History {
    pub fn load(repo: &Path, root: &Path) -> History {
        let root_rel = root_rel(repo, root);
        let scope = if root_rel.is_empty() {
            ".".to_string()
        } else {
            root_rel.clone()
        };
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args([
                "-c",
                "core.quotePath=false",
                "log",
                "--format=%cs",
                "--name-only",
                "--no-renames",
                "--",
                &scope,
            ])
            .output();
        let mut last_change = BTreeMap::new();
        if let Some(o) = out.ok().filter(|o| o.status.success()) {
            let text = String::from_utf8_lossy(&o.stdout);
            let mut date = String::new();
            for line in text.lines() {
                let line = line.trim_end();
                if line.is_empty() {
                    continue;
                }
                if is_iso_date(line) {
                    date = line.to_string();
                } else if !date.is_empty() {
                    last_change
                        .entry(line.to_string())
                        .or_insert_with(|| date.clone());
                }
            }
        }
        History {
            last_change,
            root_rel,
            today: crate::migrate::today(),
        }
    }
}

/// R-106 (`updated` behind history) and R-085 (untouched draft/active/done)
/// over every page history knows.
pub fn check_history(tree: &DocTree, h: &History, r: &mut Report) {
    let prefix = if h.root_rel.is_empty() {
        String::new()
    } else {
        format!("{}/", h.root_rel)
    };
    let stale_days: i64 = tree
        .docmeta_str("stale_active_days")
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(90);
    let today = days_of(&h.today).unwrap_or(0);
    let mut inspected = 0usize;
    for page in &tree.pages {
        if !matches!(page.kind, Kind::Permanent | Kind::Tracked) {
            continue;
        }
        let Some(fm) = &page.fm else { continue };
        let Some(last) = h.last_change.get(&format!("{prefix}{}", page.rel)) else {
            continue;
        };
        inspected += 1;
        if let Some(u) = fm.fields.get("updated").and_then(Value::as_str) {
            if is_iso_date(u) && u < last.as_str() {
                r.findings.push(Finding::err(
                    R106,
                    &page.rel,
                    "updated",
                    format!(
                        "`updated: {u}` is older than the page's last change in history ({last}) — \
                         an edit skipped the tooling; set it to that date or later"
                    ),
                ));
            }
        }
        if page.kind == Kind::Tracked {
            let status = fm
                .fields
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("");
            if matches!(status, "draft" | "active" | "done") {
                let age = today - days_of(last).unwrap_or(today);
                if age > stale_days {
                    r.findings.push(Finding::err(
                        R085,
                        &page.rel,
                        "status",
                        format!(
                            "`status: {status}` and untouched since {last} ({age} days; limit \
                             `stale_active_days: {stale_days}`) — undeclared abandonment: set \
                             `abandoned` with its reason, graduate it, or work on it"
                        ),
                    ));
                }
            }
        }
    }
    r.inspected.insert("history-freshness", inspected);
}

// ---------------------------------------------------------------- pin command

/// A page by root-relative path (with or without `.md`) or by identifier.
fn locate(root: &Path, page: &str) -> Result<String, String> {
    let rel = page.trim_start_matches("./").trim_end_matches(".md");
    let candidate = format!("{rel}.md");
    if root.join(&candidate).is_file() {
        return Ok(candidate);
    }
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    tree.pages
        .iter()
        .find(|p| {
            p.fm.as_ref()
                .and_then(|f| f.fields.get("id"))
                .and_then(Value::as_str)
                == Some(page)
        })
        .map(|p| p.rel.clone())
        .ok_or_else(|| format!("no page at `{page}` and no page with that id"))
}

/// The page text with its `verifies:` block replaced and `updated:` set to
/// `today` — the author re-read the page against the code it pins.
fn rewrite(text: &str, pins: &[Pin], today: &str) -> Result<String, String> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.first() != Some(&"---") {
        return Err("the page has no frontmatter (R-050) — `docsys page new` writes one".into());
    }
    let close = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| **l == "---")
        .map(|(i, _)| i)
        .ok_or("unterminated frontmatter")?;
    let mut out: Vec<String> = vec!["---".to_string()];
    let mut skipping = false;
    for l in lines.iter().take(close).skip(1) {
        if l.starts_with("verifies:") {
            skipping = true;
            continue;
        }
        if skipping && (l.starts_with("  - ") || l.starts_with("    ")) {
            continue;
        }
        skipping = false;
        if l.starts_with("updated:") {
            out.push(format!("updated: {today}"));
        } else {
            out.push((*l).to_string());
        }
    }
    if !pins.is_empty() {
        out.push("verifies:".to_string());
        for p in pins {
            out.push(format!("  - path: {}", p.path));
            if let Some(s) = &p.symbol {
                out.push(format!("    symbol: {s}"));
            }
            out.push(format!("    hash: \"{}\"", p.hash));
        }
    }
    out.push("---".to_string());
    for l in lines.iter().skip(close + 1) {
        out.push((*l).to_string());
    }
    let mut joined = out.join("\n");
    if text.ends_with('\n') {
        joined.push('\n');
    }
    Ok(joined)
}

/// `docsys pin <page> <path> [--symbol <s>]`: add or refresh one pin.
pub fn pin(
    root: &Path,
    repo: &Path,
    page: &str,
    path: &str,
    symbol: Option<&str>,
) -> Result<String, String> {
    let rel = locate(root, page)?;
    let file = root.join(&rel);
    let text = fs::read_to_string(&file).map_err(|e| e.to_string())?;
    let fm = crate::fm::parse(&text).ok_or("the page has no frontmatter (R-050)")?;
    let symbol = symbol
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let path = path.trim_start_matches("./").replace('\\', "/");
    let mut new = Pin {
        path: path.clone(),
        symbol: symbol.clone(),
        hash: String::new(),
    };
    new.hash = current_hash(repo, &new).map_err(|(_, m)| m)?;
    let mut pins = pins_of(&fm);
    match pins
        .iter_mut()
        .find(|p| p.path == new.path && p.symbol == new.symbol)
    {
        Some(existing) => existing.hash = new.hash.clone(),
        None => pins.push(new.clone()),
    }
    let today = crate::migrate::today();
    fs::write(&file, rewrite(&text, &pins, &today)?).map_err(|e| e.to_string())?;
    Ok(format!("pinned {rel} → {} {}", new.label(), new.hash))
}

/// `docsys pin --refresh <page>`: every pin recomputed after the author
/// re-read the page; names what changed.
pub fn refresh(root: &Path, repo: &Path, page: &str) -> Result<String, String> {
    let rel = locate(root, page)?;
    let file = root.join(&rel);
    let text = fs::read_to_string(&file).map_err(|e| e.to_string())?;
    let fm = crate::fm::parse(&text).ok_or("the page has no frontmatter (R-050)")?;
    let mut pins = pins_of(&fm);
    if pins.is_empty() {
        return Err(format!("{rel} carries no `verifies:` pin"));
    }
    let mut changed = Vec::new();
    for p in &mut pins {
        let now = current_hash(repo, p).map_err(|(_, m)| m)?;
        if now != p.hash {
            changed.push(p.label());
            p.hash = now;
        }
    }
    let today = crate::migrate::today();
    fs::write(&file, rewrite(&text, &pins, &today)?).map_err(|e| e.to_string())?;
    Ok(if changed.is_empty() {
        format!("{rel}: {} pin(s), all current", pins.len())
    } else {
        format!("{rel}: refreshed {}", changed.join(", "))
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_the_published_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn canonical_form_is_lf_trimmed_single_trailing_newline() {
        assert_eq!(canonical("a  \r\nb\t\n\n\n"), "a\nb\n");
        assert_eq!(canonical("x"), "x\n");
        assert_eq!(content_hash("x\n"), content_hash("x   \r\n\n"));
    }

    #[test]
    fn brace_symbol_resolves_to_its_block_and_ambiguity_is_an_error() {
        let src = "use x::refresh;\nfn other() {\n    refresh(1);\n}\n\npub fn refresh(t: u32) -> u32 {\n    if t > 0 {\n        t\n    } else {\n        0\n    }\n}\nfn tail() {}\n";
        let r = region(src, "a.rs", Some("refresh")).unwrap();
        assert!(r.starts_with("pub fn refresh"), "{r}");
        assert!(r.ends_with("}\n}"), "{r}");
        assert!(!r.contains("tail"));
        assert!(region(src, "a.rs", Some("nope"))
            .unwrap_err()
            .contains("not found"));
        let dup = "fn f() {\n}\nfn f() {\n}\n";
        assert!(region(dup, "a.rs", Some("f"))
            .unwrap_err()
            .contains("ambiguous"));
        assert_eq!(region(src, "a.rs", None).unwrap(), src);
    }

    #[test]
    fn python_symbol_is_an_indentation_block() {
        let src = "import os\n\ndef a():\n    return 1\n\n\nclass Ledger:\n    def transfer(self):\n        pass\n\n    def other(self):\n        pass\n\ndef transfer():\n    pass\n";
        let r = region(src, "core.py", Some("Ledger")).unwrap();
        assert!(r.starts_with("class Ledger:"), "{r}");
        assert!(r.contains("def other"), "{r}");
        assert!(!r.contains("def transfer():\n    pass"), "{r}");
        assert!(region(src, "core.py", Some("transfer"))
            .unwrap_err()
            .contains("ambiguous"));
    }

    #[test]
    fn days_from_civil_round_trips_known_dates() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(
            days_of("2026-09-02").unwrap() - days_of("2026-06-04").unwrap(),
            90
        );
    }

    #[test]
    fn rewrite_replaces_the_verifies_block_and_bumps_updated() {
        let text = "---\nid: x\ntype: reference\nupdated: 2026-01-01\nverifies:\n  - path: old.rs\n    hash: \"sha256:0\"\ntags: [a]\n---\nBody.\n";
        let pins = vec![Pin {
            path: "src/a.rs".into(),
            symbol: Some("f".into()),
            hash: "sha256:abc".into(),
        }];
        let out = rewrite(text, &pins, "2026-09-02").unwrap();
        assert_eq!(
            out,
            "---\nid: x\ntype: reference\nupdated: 2026-09-02\ntags: [a]\nverifies:\n  - path: src/a.rs\n    symbol: f\n    hash: \"sha256:abc\"\n---\nBody.\n"
        );
        let fm = crate::fm::parse(&out).unwrap();
        assert_eq!(pins_of(&fm), pins);
    }
}
