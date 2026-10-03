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

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::checks::Report;
use crate::era::Era;
use crate::fm::{Frontmatter, Value};
use crate::model::{is_iso_date, Finding, RuleId};
use crate::tree::{DocTree, Kind};

const R085: RuleId = RuleId("R-085");
const R106: RuleId = RuleId("R-106");
const R111: RuleId = RuleId("R-111");
const R113: RuleId = RuleId("R-113");
const R114: RuleId = RuleId("R-114");
const R212: RuleId = RuleId("R-212");

/// Above this many lines, `pin` notes that a whole-file pin goes stale on
/// every edit (D-106).
const WHOLE_FILE_LINES: usize = 300;

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

/// The text a pin covers in a 0.5 tree: the whole file, or the lines of the
/// declaration `symbol` names (D-106). A symbol that is absent, only used or
/// declared more than once is an error, never a guess (R-114).
pub fn declared_region(source: &str, path: &str, symbol: Option<&str>) -> Result<String, String> {
    let Some(sym) = symbol.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(source.to_string());
    };
    let (first, last) = crate::symbols::resolve(source, path, sym)?;
    let lines: Vec<&str> = source.lines().collect();
    Ok(lines
        .get(first.saturating_sub(1)..last)
        .unwrap_or(&[])
        .join("\n"))
}

// ---------------------------------------------------------------- pins

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    pub path: String,
    pub symbol: Option<String>,
    pub hash: String,
    /// the block of the body this pin backs (R-212, D-103)
    pub block: Option<String>,
}

impl Pin {
    pub fn label(&self) -> String {
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
                        block: m.get("block").map(|b| b.trim().to_string()),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The text a pin covers now, or why it cannot be read. A 0.4 tree resolves a
/// symbol by D-069, a later one by D-106 (D-118).
fn region_now(repo: &Path, pin: &Pin, era: Era) -> Result<String, (RuleId, String)> {
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
    let symbol = pin.symbol.as_deref();
    if era.declaration_pins() {
        declared_region(&source, &pin.path, symbol)
    } else {
        region(&source, &pin.path, symbol)
    }
    .map_err(|e| (R114, e))
}

/// The canonical hash a pin's `hash:` line carries (R-113's content hash).
fn current_hash(repo: &Path, pin: &Pin, era: Era) -> Result<String, (RuleId, String)> {
    region_now(repo, pin, era).map(|text| content_hash(&text))
}

/// The region hash an acknowledgement is named by (R-113's token form, D-119).
fn region_hash_now(repo: &Path, pin: &Pin, era: Era) -> Result<String, (RuleId, String)> {
    region_now(repo, pin, era).map(|text| crate::ack::region_hash(&text, &pin.path))
}

fn page_id(fm: &Frontmatter) -> Option<String> {
    fm.fields
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Why a pin is not fresh now — the finding's rule and message — or `None`.
/// In a 0.5 tree a pin is fresh while an acknowledgement of its region exists
/// (D-119); a pin that still carries `hash:` is a record from before 0.5 and
/// is checked against it, in R-113's canonical form, until a refresh or
/// `docsys upgrade` moves it into an acknowledgement.
fn pin_problem(
    root: &Path,
    repo: &Path,
    era: Era,
    id: Option<&str>,
    pin: &Pin,
    rel: &str,
) -> Option<(RuleId, String)> {
    if era.acknowledged_pins() && pin.hash.is_empty() {
        return match region_hash_now(repo, pin, era) {
            Err(e) => Some(e),
            Ok(now) if !id.is_some_and(|i| crate::ack::holds(root, i, &now)) => Some((
                R111,
                format!(
                    "stale: `{}` reads differently from every acknowledgement of this \
                     page — re-read the page against it, then `docsys pin --refresh {rel}`",
                    pin.label()
                ),
            )),
            Ok(_) => None,
        };
    }
    if !pin.hash.starts_with("sha256:") || pin.hash.len() != 71 {
        return Some((
            R113,
            format!(
                "hash `{}` is not `sha256:<64 hex>` — `docsys pin {rel} {}` writes it",
                pin.hash, pin.path
            ),
        ));
    }
    match current_hash(repo, pin, era) {
        Err(e) => Some(e),
        Ok(now) if now != pin.hash => Some((
            R111,
            format!(
                "stale: `{}` moved since the page was pinned — re-read the page, then \
                 `docsys pin --refresh {rel}`",
                pin.label()
            ),
        )),
        Ok(_) => None,
    }
}

/// R-110/R-111/R-113/R-114 over every pinned permanent page. In a 0.5 tree a
/// stale pin bound to a block names it (R-212).
pub fn check_pins(tree: &DocTree, repo: &Path, r: &mut Report) {
    let era = Era::of(tree);
    let mut inspected = 0usize;
    for page in &tree.pages {
        if page.kind != Kind::Permanent {
            continue;
        }
        let Some(fm) = &page.fm else { continue };
        let id = page_id(fm);
        let mut blocks: Option<Vec<String>> = None;
        for pin in pins_of(fm) {
            inspected += 1;
            let Some((rule, mut msg)) =
                pin_problem(&tree.root, repo, era, id.as_deref(), &pin, &page.rel)
            else {
                continue;
            };
            if let Some(b) = pin
                .block
                .as_ref()
                .filter(|_| rule == R111 && era.anchored_verification())
            {
                let current =
                    blocks.get_or_insert_with(|| crate::blocks::hashes(&body_text(&page.text)));
                if let Some(k) = current.iter().position(|h| h == b) {
                    msg.push_str(&format!(
                        " — it backs block [{}]: only that block stops reading as verified \
                         (`docsys verify --show {}`)",
                        k + 1,
                        page.rel
                    ));
                }
            }
            r.findings
                .push(Finding::err(rule, &page.rel, &pin.label(), msg));
        }
    }
    r.inspected.insert("verifies-pins", inspected);
}

/// The blocks whose bound pin is stale now (R-212, R-111): on a verified page
/// only they stop reading as verified. Empty before 0.5.
pub fn stale_blocks(root: &Path, repo: &Path, era: Era, fm: &Frontmatter) -> Vec<String> {
    if !era.anchored_verification() {
        return Vec::new();
    }
    let id = page_id(fm);
    pins_of(fm)
        .into_iter()
        .filter(|p| p.block.is_some())
        .filter(|p| {
            matches!(
                pin_problem(root, repo, era, id.as_deref(), p, ""),
                Some((rule, _)) if rule == R111
            )
        })
        .filter_map(|p| p.block)
        .collect()
}

/// R-212 (§21, D-103), without history: a pin bound to a block the body no
/// longer holds is reported — the block was rewritten, and which block the pin
/// backs now is the author's judgment.
pub fn check_block_bindings(tree: &DocTree, r: &mut Report) {
    let mut inspected = 0usize;
    for page in &tree.pages {
        if page.kind != Kind::Permanent {
            continue;
        }
        let Some(fm) = &page.fm else { continue };
        let bound: Vec<Pin> = pins_of(fm)
            .into_iter()
            .filter(|p| p.block.is_some())
            .collect();
        if bound.is_empty() {
            continue;
        }
        let current = crate::blocks::hashes(&body_text(&page.text));
        for pin in bound {
            inspected += 1;
            let Some(b) = pin.block.as_ref().filter(|b| !current.contains(b)) else {
                continue;
            };
            let symbol = pin
                .symbol
                .as_ref()
                .map(|s| format!(" --symbol {s}"))
                .unwrap_or_default();
            r.findings.push(Finding::warn(
                R212,
                &page.rel,
                &pin.label(),
                format!(
                    "bound to block `{b}`, which the body no longer holds — the block was \
                     rewritten: re-read it, then bind the pin again (`docsys pin {} {}{symbol} \
                     --block <n>`; `docsys verify --show {}` numbers the blocks) or drop its \
                     `block:`",
                    page.rel, pin.path, page.rel
                ),
            ));
        }
    }
    r.inspected.insert("block-bindings", inspected);
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
        // One walk: each commit's day, and for every path the blobs before and
        // after it. A page's last change is its newest CONTENT change (§2.4):
        // a commit that touched only bookkeeping — `updated:`, the
        // verification record, a pin's hash — is not one, so a tool's record
        // or a migration never makes `updated:` read as behind (R-106).
        let out = crate::git::cmd(repo)
            .args([
                "log",
                "--format=@@%cs",
                "--raw",
                "--no-abbrev",
                "--no-renames",
                "--",
                &scope,
            ])
            .output();
        let mut changes: BTreeMap<String, Vec<(String, String, String)>> = BTreeMap::new();
        if let Some(o) = out.ok().filter(|o| o.status.success()) {
            let text = String::from_utf8_lossy(&o.stdout);
            let mut date = String::new();
            for line in text.lines() {
                if let Some(d) = line.strip_prefix("@@") {
                    date = d.trim().to_string();
                } else if let Some((meta, path)) =
                    line.strip_prefix(':').and_then(|l| l.split_once('\t'))
                {
                    let blobs: Vec<&str> = meta.split(' ').collect();
                    if let (Some(old), Some(new)) = (blobs.get(2), blobs.get(3)) {
                        if is_iso_date(&date) {
                            changes.entry(path.to_string()).or_default().push((
                                date.clone(),
                                old.to_string(),
                                new.to_string(),
                            ));
                        }
                    }
                }
            }
        }
        // a docsys/0.4 tree counts every commit, as 0.15 did (D-118)
        let content_only = crate::era::Era::at(root).content_history();
        let mut blobs = if content_only {
            crate::git::Blobs::open(repo)
        } else {
            None
        };
        let mut last_change = BTreeMap::new();
        for (path, commits) in changes {
            let newest = commits.first().map(|(d, _, _)| d.clone());
            let content = if content_only && path.ends_with(".md") {
                commits.iter().find(|(_, old, new)| match blobs.as_mut() {
                    Some(b) => {
                        let (o, n) = (b.read(old), b.read(new));
                        match (o, n) {
                            (Some(o), Some(n)) => significant(&o) != significant(&n),
                            _ => true,
                        }
                    }
                    None => true,
                })
            } else {
                commits.first()
            };
            if let Some(day) = content.map(|(d, _, _)| d.clone()).or(newest) {
                last_change.insert(path, day);
            }
        }
        History {
            last_change,
            root_rel,
            today: crate::migrate::today(),
        }
    }
}

/// The date a reader is shown for a page of one tree.
pub struct Dates {
    derived: bool,
    history: History,
    prefix: String,
}

impl Dates {
    /// On a docsys/0.5 tree a page's date is its last content change in
    /// history (R-050, D-122); before, its `updated:` field.
    pub fn of(tree: &DocTree) -> Dates {
        let derived = Era::of(tree).derived_dates();
        let history = match crate::repo_of(&tree.root).filter(|_| derived) {
            Some(repo) => History::load(&repo, &tree.root),
            None => History::default(),
        };
        let prefix = if history.root_rel.is_empty() {
            String::new()
        } else {
            format!("{}/", history.root_rel)
        };
        Dates {
            derived,
            history,
            prefix,
        }
    }

    /// The page's date, or "unknown" where nothing records one.
    pub fn of_page(&self, rel: &str, fm: Option<&Frontmatter>) -> String {
        let found = if self.derived {
            self.history
                .last_change
                .get(&format!("{}{rel}", self.prefix))
                .cloned()
        } else {
            fm.and_then(|f| f.fields.get("updated"))
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        found.unwrap_or_else(|| "unknown".to_string())
    }
}

/// R-106 (`updated` behind history) and R-085 (untouched draft/active/done)
/// over every page history knows.
pub fn check_history(tree: &DocTree, repo: &Path, h: &History, r: &mut Report) {
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
        // a docsys/0.5 page keeps no date to fall behind (R-050, D-122)
        let dated = !crate::era::Era::of(tree).derived_dates();
        if let Some(u) = fm
            .fields
            .get("updated")
            .and_then(Value::as_str)
            .filter(|_| dated)
        {
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
    // every page that carries a verification answers to the same contract,
    // in either profile (§3.1 for the base, §3.2 for a project — D-092)
    let era = crate::era::Era::of(tree);
    // a docsys/0.5 page's verification is history's, checked where it is read;
    // a work file's `confirmed:` is still a maintainer's act (D-126)
    if era.verification_from_history() {
        check_confirmed_authors(tree, repo, &prefix, r);
        return;
    }
    check_verified_bodies(tree, repo, &prefix, r);
    check_record_authors_04(tree, repo, &prefix, r);
}

/// The frontmatter fields that are bookkeeping, not content (§2.4): the
/// freshness date, the verification record (R-028), and — inside `verifies:` — a pin's hash, which records a re-read, not a
/// claim.
const BOOKKEEPING: [&str; 6] = [
    "updated",
    "verification",
    "verified_by",
    "verified_rev",
    "verified_sources",
    "verified_blocks",
];

/// What of a page counts as its content (§2.4): the body's hash and every
/// frontmatter field but the bookkeeping ones. A text without readable
/// frontmatter is content whole.
fn significant(text: &str) -> (String, Vec<(String, crate::fm::Value)>) {
    match crate::fm::parse(text) {
        Some(fm) if fm.problems.is_empty() => {
            let fields = fm
                .fields
                .into_iter()
                .filter(|(k, _)| !BOOKKEEPING.contains(&k.as_str()))
                .map(|(k, v)| match v {
                    Value::Maps(maps) if k == "verifies" => (
                        k,
                        Value::Maps(
                            maps.into_iter()
                                .map(|mut m| {
                                    m.remove("hash");
                                    m
                                })
                                .collect(),
                        ),
                    ),
                    other => (k, other),
                })
                .collect();
            (content_hash(&body_text(text)), fields)
        }
        _ => (content_hash(text), Vec::new()),
    }
}

/// The body of a page text — everything after the frontmatter.
pub(crate) fn body_text(text: &str) -> String {
    match crate::fm::parse(text) {
        Some(fm) => {
            let mut body = text
                .lines()
                .skip(fm.body_start)
                .collect::<Vec<_>>()
                .join("\n");
            body.push('\n');
            body
        }
        None => text.to_string(),
    }
}

/// A record's author and the e-mails its trailers name — the people whose act
/// the commit carries (D-102). A host's squash merge writes the squashed
/// authors as `Co-authored-by:`; a review's word is `Reviewed-by:` or
/// `Approved-by:` (D-095).
struct Act {
    author: String,
    named: Vec<String>,
}

impl Act {
    fn carries(&self, email: &str) -> bool {
        self.author == email || self.named.iter().any(|e| e == email)
    }
}

/// `%(trailers:…)` placeholders for the three trailers, `\x1f`-separated.
const ACT_FORMAT: &str = "%ae%x1e%(trailers:key=Co-authored-by,valueonly,separator=%x1f)%x1f%(trailers:key=Reviewed-by,valueonly,separator=%x1f)%x1f%(trailers:key=Approved-by,valueonly,separator=%x1f)";

fn act_of(field: &str) -> Act {
    let (author, trailers) = field.split_once('\u{1e}').unwrap_or((field, ""));
    let named = trailers
        .split('\u{1f}')
        .filter_map(|t| {
            let t = t.trim();
            let e = t
                .rsplit_once('<')
                .map(|(_, e)| e.trim_end_matches('>').trim())
                .unwrap_or(t);
            (!e.is_empty()).then(|| e.to_lowercase())
        })
        .collect();
    Act {
        author: author.trim().to_lowercase(),
        named,
    }
}

/// R-208's history half for `confirmed:` (D-092): when a maintainer entry
/// carries an email, the commit that recorded the line is that maintainer's
/// act — authored by them, or naming them in a trailer (a host's squash keeps
/// the squashed authors as `Co-authored-by:`). A page's verification is
/// history's own on a docsys/0.5 tree (D-126); a line not yet committed is the
/// gate's business.
fn check_confirmed_authors(tree: &DocTree, repo: &Path, prefix: &str, r: &mut Report) {
    let maintainers = crate::checks::maintainer_handles(tree);
    if maintainers.iter().all(|m| m.email.is_none()) {
        return;
    }
    let mut inspected = 0usize;
    for page in tree.pages.iter().filter(|p| p.kind == Kind::Tracked) {
        let Some(fm) = &page.fm else { continue };
        let Some((who, email)) =
            fm.fields
                .get("confirmed")
                .and_then(Value::as_str)
                .and_then(|value| {
                    let who = crate::checks::record_handle(value);
                    maintainers
                        .iter()
                        .find(|m| m.handle == who)
                        .and_then(|m| m.email.clone())
                        .map(|e| (who, e))
                })
        else {
            continue; // an unknown handle is checks.rs's finding; no email, nothing to compare
        };
        let Some(line) = page.text.lines().find(|l| l.starts_with("confirmed:")) else {
            continue;
        };
        inspected += 1;
        let out = crate::git::cmd(repo)
            .args(["log", "--reverse", &format!("--format={ACT_FORMAT}"), "-S"])
            .arg(line)
            .arg("--")
            .arg(format!("{prefix}{}", page.rel))
            .output()
            .ok()
            .filter(|o| o.status.success());
        let Some(first) = out.as_ref().and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .map(str::to_string)
        }) else {
            continue; // not committed yet
        };
        let act = act_of(&first);
        if !act.carries(&email) {
            r.findings.push(Finding::err(
                RuleId("R-208"),
                &page.rel,
                "confirmed",
                format!(
                    "`confirmed:` names `{who}` but the commit that recorded it was authored by `{}`, not `{email}`, and names it in no trailer — the record must be the maintainer's own act (R-208)",
                    act.author
                ),
            ));
        }
    }
    r.inspected.insert("record-authors", inspected);
}

/// R-208's history half on a docsys/0.4 tree (D-118): the commit that introduced
/// the exact record line, its author — the check 0.15 ran.
fn check_record_authors_04(tree: &DocTree, repo: &Path, prefix: &str, r: &mut Report) {
    let maintainers = crate::checks::maintainer_handles(tree);
    if maintainers.iter().all(|m| m.email.is_none()) {
        return;
    }
    let mut inspected = 0usize;
    for page in &tree.pages {
        let Some(fm) = &page.fm else { continue };
        let field = match page.kind {
            Kind::Tracked => "confirmed",
            Kind::Permanent => "verified_by",
            _ => continue,
        };
        let Some(value) = fm.fields.get(field).and_then(Value::as_str) else {
            continue;
        };
        let who = crate::checks::record_handle(value);
        let Some(email) = maintainers
            .iter()
            .find(|m| m.handle == who)
            .and_then(|m| m.email.as_ref())
        else {
            continue; // an unknown handle is checks.rs's finding; no email, nothing to compare
        };
        // the line as the file carries it, so -S finds the commit that added it
        let Some(line) = page
            .text
            .lines()
            .find(|l| l.starts_with(&format!("{field}:")))
        else {
            continue;
        };
        inspected += 1;
        let out = crate::git::cmd(repo)
            .args(["log", "--reverse", "--format=%ae", "-S"])
            .arg(line)
            .arg("--")
            .arg(format!("{prefix}{}", page.rel))
            .output()
            .ok()
            .filter(|o| o.status.success());
        let Some(out) = out else { continue };
        let Some(author) = String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .map(|l| l.trim().to_lowercase())
        else {
            continue; // not committed yet
        };
        if author != *email {
            r.findings.push(Finding::err(
                RuleId("R-208"),
                &page.rel,
                field,
                format!(
                    "`{field}:` names `{who}` but the commit that recorded it was authored by `{author}`, not `{email}` — the record must be the maintainer's own commit (R-208)"
                ),
            ));
        }
    }
    r.inspected.insert("record-authors", inspected);
}

/// R-024 through history, for a record without blocks (written before 0.5):
/// the body at `verified_rev` against the body now (D-077), and each consumed
/// source's provenance then against now (D-082) — a docsys/0.4 tree's check;
/// a 0.5 tree's verification is read from history (D-126).
fn check_verified_bodies(tree: &DocTree, repo: &Path, prefix: &str, r: &mut Report) {
    let mut inspected = 0usize;
    for page in &tree.pages {
        if page.kind != Kind::Permanent {
            continue;
        }
        let Some(fm) = &page.fm else { continue };
        if fm.fields.get("verification").and_then(Value::as_str) != Some("verified") {
            continue;
        }
        let Some(rev) = fm.fields.get("verified_rev").and_then(Value::as_str) else {
            continue; // R-028 reports the missing record
        };
        inspected += 1;
        let spec = format!("{}:{prefix}{}", rev.trim(), page.rel);
        let shown = crate::git::cmd(repo)
            .args(["show", &spec])
            .output()
            .ok()
            .filter(|o| o.status.success());
        match shown {
            None => r.findings.push(Finding::err(
                RuleId("R-028"),
                &page.rel,
                "verified_rev",
                format!(
                    "`verified_rev: {rev}` does not hold this page — not a revision of this \
                     repository, or the page was not there: the record cannot be audited"
                ),
            )),
            Some(o) => {
                let then = String::from_utf8_lossy(&o.stdout);
                if content_hash(&body_text(&then)) != content_hash(&body_text(&page.text)) {
                    r.findings.push(Finding::err(
                        RuleId("R-024"),
                        &page.rel,
                        "verification",
                        format!(
                            "`verified` at {rev}, but the body changed since — verification \
                             describes content that no longer exists: set `verification: \
                             unverified` and audit again (R-025)"
                        ),
                    ));
                }
            }
        }
        // The other half of the claim: the consumed sources the page rests
        // on still hash to what they held when it was verified (D-082). The
        // provenance sidecar at verified_rev is the baseline; the current
        // sidecar is what `fetch` brought since.
        let sources = fm
            .fields
            .get("sources")
            .and_then(Value::as_list)
            .map(<[String]>::to_vec)
            .unwrap_or_default();
        for s in sources.iter().filter(|s| s.starts_with('@')) {
            let Some((ns, id)) = s.trim_start_matches('@').split_once('/') else {
                continue;
            };
            let side_rel = format!(".federation/{ns}/{id}.provenance.yml");
            let now_text = fs::read_to_string(tree.root.join(&side_rel)).ok();
            let Some(now) = now_text.as_deref().and_then(|t| sidecar_field(t, "hash")) else {
                continue; // R-059 reports an unmaterialized source
            };
            let then = crate::git::cmd(repo)
                .args(["show", &format!("{}:{prefix}{side_rel}", rev.trim())])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| sidecar_field(&String::from_utf8_lossy(&o.stdout), "hash"));
            match then {
                None => r.findings.push(Finding::err(
                    RuleId("R-028"),
                    &page.rel,
                    s,
                    format!(
                        "`{s}` had no committed provenance at verified_rev {rev} — the \
                         materialization is part of the base's history (D-082): commit \
                         `.federation/`, then audit again"
                    ),
                )),
                Some(then) if then != now => {
                    let fetched = now_text
                        .as_deref()
                        .and_then(|t| sidecar_field(t, "fetched"))
                        .unwrap_or_default();
                    r.findings.push(Finding::err(
                        RuleId("R-024"),
                        &page.rel,
                        s,
                        format!(
                            "`verified` at {rev}, but `{s}` moved since (fetched {fetched}) — \
                             re-read the page against the source as it now reads, then audit \
                             again, or set `verification: unverified`"
                        ),
                    ));
                }
                Some(_) => {}
            }
        }
    }
    r.inspected.insert("verified-bodies", inspected);
}

/// The consumed sources (`@namespace/id`) a page's `sources:` names.
pub(crate) fn consumed_sources(fm: &crate::fm::Frontmatter) -> Vec<String> {
    fm.fields
        .get("sources")
        .and_then(Value::as_list)
        .map(|l| l.iter().filter(|s| s.starts_with('@')).cloned().collect())
        .unwrap_or_default()
}

/// The provenance hash a consumed source's materialization carries now.
pub(crate) fn source_hash(root: &Path, source: &str) -> Option<String> {
    let (ns, id) = source.trim_start_matches('@').split_once('/')?;
    let text =
        fs::read_to_string(root.join(format!(".federation/{ns}/{id}.provenance.yml"))).ok()?;
    sidecar_field(&text, "hash")
}

/// One field of a provenance sidecar, read as every value is (D-002).
pub(crate) fn sidecar_field(text: &str, key: &str) -> Option<String> {
    crate::fm::parse_fields(text)
        .fields
        .get(key)
        .and_then(crate::fm::Value::as_str)
        .map(str::to_string)
        .filter(|v| !v.is_empty())
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

/// The page text with its `verifies:` block replaced — the author re-read the
/// page against the code it pins — and, on a docsys/0.4 tree, `updated:` set
/// to `today`. A pin without a hash (a 0.5 tree's, D-119) is written without
/// the line.
fn rewrite(text: &str, pins: &[Pin], today: Option<&str>) -> Result<String, String> {
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
        if let (true, Some(today)) = (l.starts_with("updated:"), today) {
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
            if let Some(b) = &p.block {
                out.push(format!("    block: {b}"));
            }
            if !p.hash.is_empty() {
                out.push(format!("    hash: \"{}\"", p.hash));
            }
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

/// The page text with every pin's `hash:` line removed and nothing else
/// touched — a record from before 0.5 leaving for an acknowledgement (D-119).
/// `None` when no pin carries one. A pin written `hash:` first hands its
/// list marker to the line after it.
fn strip_pin_hashes(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut frontmatter = false;
    let mut verifies = false;
    let mut promote = false;
    let mut changed = false;
    for (n, line) in text.split_inclusive('\n').enumerate() {
        let bare = line.trim_end_matches(['\n', '\r']);
        if n == 0 {
            if bare != "---" {
                return None;
            }
            frontmatter = true;
            out.push_str(line);
            continue;
        }
        if frontmatter && bare == "---" {
            frontmatter = false;
            verifies = false;
        } else if frontmatter && bare.starts_with("verifies:") {
            verifies = true;
        } else if frontmatter && verifies {
            if bare.starts_with("    hash:") {
                changed = true;
                continue;
            }
            if bare.starts_with("  - hash:") {
                changed = true;
                promote = true;
                continue;
            }
            if let Some(rest) = line.strip_prefix("    ").filter(|_| promote) {
                promote = false;
                out.push_str("  - ");
                out.push_str(rest);
                continue;
            }
            promote = false;
            verifies = bare.starts_with("  - ") || bare.starts_with("    ");
        }
        out.push_str(line);
    }
    changed.then_some(out)
}

fn short(hash: &str) -> &str {
    hash.get(..12).unwrap_or(hash)
}

/// `docsys pin <page> <path> [--symbol <s>]`: add or refresh one pin. In a
/// 0.5 tree the re-read is recorded as an acknowledgement (D-119): the page
/// changes only when the pin is new — a content change, so `updated:` moves —
/// or when a hash from before 0.5 leaves it.
pub fn pin(
    root: &Path,
    repo: &Path,
    page: &str,
    path: &str,
    symbol: Option<&str>,
) -> Result<String, String> {
    pin_block(root, repo, page, path, symbol, None)
}

/// `pin`, and with `--block <n>` (R-212, a docsys/0.5 tree) the entry bound to
/// the hash of the body's block n as `docsys verify --show` numbers them — the
/// hash, never the number, so the binding survives blocks added around it.
pub fn pin_block(
    root: &Path,
    repo: &Path,
    page: &str,
    path: &str,
    symbol: Option<&str>,
    block: Option<usize>,
) -> Result<String, String> {
    let rel = locate(root, page)?;
    let file = root.join(&rel);
    let text = fs::read_to_string(&file).map_err(|e| e.to_string())?;
    crate::fm::refuse_unclosed_in(&rel, &text)?;
    let fm = crate::fm::parse(&text).ok_or("the page has no frontmatter (R-050)")?;
    let era = Era::at(root);
    let bound = match block {
        None => None,
        Some(_) if !era.anchored_verification() => {
            return Err(
                "a pin bound to a block is a docsys/0.5 format — `docsys upgrade` moves the tree first (D-118)"
                    .into(),
            )
        }
        Some(n) => {
            let blocks = crate::blocks::hashes(&body_text(&text));
            let hash = n.checked_sub(1).and_then(|i| blocks.get(i)).cloned();
            Some(hash.ok_or_else(|| {
                format!(
                    "{rel} has {} block(s), and `--block` takes one of them by number — `docsys verify --show {rel}` numbers them",
                    blocks.len()
                )
            })?)
        }
    };
    let symbol = symbol
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let path = path.trim_start_matches("./").replace('\\', "/");
    let mut new = Pin {
        path: path.clone(),
        symbol: symbol.clone(),
        hash: String::new(),
        block: bound.clone(),
    };
    let today = (!era.derived_dates()).then(crate::migrate::today);
    let today = today.as_deref();
    let mut out = if era.acknowledged_pins() {
        let id = page_id(&fm).ok_or_else(|| {
            format!("{rel} has no `id` (R-050) — acknowledgements are kept by page id")
        })?;
        let hash = region_hash_now(repo, &new, era).map_err(|(_, m)| m)?;
        let mut pins = pins_of(&fm);
        let known = pins
            .iter()
            .any(|p| p.path == new.path && p.symbol == new.symbol);
        if known {
            // the other pins' records from before 0.5 move with this one
            convert_page(root, repo, &rel, true)?;
            if bound.is_some() {
                let text = fs::read_to_string(&file).map_err(|e| e.to_string())?;
                let fm = crate::fm::parse(&text).ok_or("the page has no frontmatter (R-050)")?;
                let mut pins = pins_of(&fm);
                for p in pins
                    .iter_mut()
                    .filter(|p| p.path == new.path && p.symbol == new.symbol)
                {
                    p.block = bound.clone();
                }
                fs::write(&file, rewrite(&text, &pins, today)?).map_err(|e| e.to_string())?;
            }
        } else {
            pins.push(new.clone());
            fs::write(&file, rewrite(&text, &pins, today)?).map_err(|e| e.to_string())?;
        }
        crate::ack::write(root, &id, &hash).map_err(|e| e.to_string())?;
        let binding = match (block, &bound) {
            (Some(n), Some(b)) => format!(", bound to block [{n}] ({b})"),
            _ => String::new(),
        };
        format!(
            "pinned {rel} → {}, acknowledged {}{binding}",
            new.label(),
            short(&hash)
        )
    } else {
        new.hash = current_hash(repo, &new, era).map_err(|(_, m)| m)?;
        let mut pins = pins_of(&fm);
        match pins
            .iter_mut()
            .find(|p| p.path == new.path && p.symbol == new.symbol)
        {
            Some(existing) => existing.hash = new.hash.clone(),
            None => pins.push(new.clone()),
        }
        fs::write(&file, rewrite(&text, &pins, today)?).map_err(|e| e.to_string())?;
        format!("pinned {rel} → {} {}", new.label(), new.hash)
    };
    if new.symbol.is_none() {
        let lines = fs::read_to_string(repo.join(&path)).map_or(0, |s| s.lines().count());
        if lines > WHOLE_FILE_LINES {
            out.push_str(&format!(
                "\nnote: `{path}` has {lines} lines — every edit to that file stales this page; \
                 pin a symbol (`--symbol <name>`)"
            ));
        }
    }
    Ok(out)
}

/// `docsys pin --refresh <page>`: every pin recomputed after the author
/// re-read the page; names what changed. In a 0.5 tree it writes the
/// acknowledgements of the regions that have none and removes this page's
/// acknowledgements no current region matches — never the page, except to
/// drop a hash from before 0.5 (D-119).
pub fn refresh(root: &Path, repo: &Path, page: &str) -> Result<String, String> {
    let rel = locate(root, page)?;
    let file = root.join(&rel);
    let text = fs::read_to_string(&file).map_err(|e| e.to_string())?;
    crate::fm::refuse_unclosed_in(&rel, &text)?;
    let fm = crate::fm::parse(&text).ok_or("the page has no frontmatter (R-050)")?;
    let mut pins = pins_of(&fm);
    if pins.is_empty() {
        return Err(format!("{rel} carries no `verifies:` pin"));
    }
    let era = Era::at(root);
    if era.acknowledged_pins() {
        let id = page_id(&fm).ok_or_else(|| {
            format!("{rel} has no `id` (R-050) — acknowledgements are kept by page id")
        })?;
        // every region first: one that cannot be read leaves everything as it was
        let mut now = Vec::with_capacity(pins.len());
        for p in &pins {
            now.push((
                p.label(),
                region_hash_now(repo, p, era).map_err(|(_, m)| m)?,
            ));
        }
        let mut written = Vec::new();
        for (label, hash) in &now {
            if crate::ack::write(root, &id, hash).map_err(|e| e.to_string())? {
                written.push(label.clone());
            }
        }
        let keep: BTreeSet<String> = now.iter().map(|(_, h)| h.clone()).collect();
        let removed = crate::ack::remove_except(root, &id, &keep).map_err(|e| e.to_string())?;
        if let Some(stripped) = strip_pin_hashes(&text) {
            fs::write(&file, stripped).map_err(|e| e.to_string())?;
        }
        let gone = if removed.is_empty() {
            String::new()
        } else {
            format!("; {} superseded acknowledgement(s) removed", removed.len())
        };
        return Ok(if written.is_empty() {
            format!("{rel}: {} pin(s), all acknowledged{gone}", pins.len())
        } else {
            format!("{rel}: acknowledged {}{gone}", written.join(", "))
        });
    }
    let mut changed = Vec::new();
    for p in &mut pins {
        let now = current_hash(repo, p, era).map_err(|(_, m)| m)?;
        if now != p.hash {
            changed.push(p.label());
            p.hash = now;
        }
    }
    let today = (!Era::at(root).derived_dates()).then(crate::migrate::today);
    fs::write(&file, rewrite(&text, &pins, today.as_deref())?).map_err(|e| e.to_string())?;
    Ok(if changed.is_empty() {
        format!("{rel}: {} pin(s), all current", pins.len())
    } else {
        format!("{rel}: refreshed {}", changed.join(", "))
    })
}

/// `docsys pin --gc`: remove the acknowledgements nothing needs any more — a
/// directory whose page id pins nothing (the page was retired, renamed or
/// unpinned), and inside a page's directory every acknowledgement no current
/// region of its pins matches. A page whose regions cannot all be read is
/// left as it is and named. Running it again removes nothing.
pub fn gc(root: &Path, repo: &Path) -> Result<Vec<String>, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    let era = Era::of(&tree);
    let mut by_id: BTreeMap<String, Vec<Pin>> = BTreeMap::new();
    for page in &tree.pages {
        if page.kind != Kind::Permanent {
            continue;
        }
        // what a page pins is read whole, or no acknowledgement is judged unneeded
        crate::fm::refuse_unclosed_in(&page.rel, &page.text)?;
        let Some(fm) = &page.fm else { continue };
        if let Some(id) = page_id(fm) {
            by_id.entry(id).or_default().extend(pins_of(fm));
        }
    }
    let mut out = Vec::new();
    for id in crate::ack::page_ids(root) {
        let Some(pins) = by_id.get(&id).filter(|p| !p.is_empty()) else {
            let gone = crate::ack::remove_page(root, &id).map_err(|e| e.to_string())?;
            out.push(format!(
                "removed {}/{id}/ ({} acknowledgement(s)): no page with that id pins anything",
                crate::ack::DIR,
                gone.len()
            ));
            continue;
        };
        let mut keep = BTreeSet::new();
        let mut unreadable = None;
        for p in pins {
            match region_hash_now(repo, p, era) {
                Ok(h) => {
                    keep.insert(h);
                }
                Err((_, m)) => {
                    unreadable = Some(m);
                    break;
                }
            }
        }
        if let Some(why) = unreadable {
            out.push(format!("kept {}/{id}/: {why}", crate::ack::DIR));
            continue;
        }
        for name in crate::ack::remove_except(root, &id, &keep).map_err(|e| e.to_string())? {
            out.push(format!("removed {}/{id}/{name}", crate::ack::DIR));
        }
    }
    Ok(out)
}

/// What the conversion of one pin's `hash:` found (D-119, for `docsys
/// upgrade`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conversion {
    /// fresh as recorded, and the declaration reads the same lines: the
    /// acknowledgement is written
    Acknowledged,
    /// fresh as recorded, but its D-106 declaration is another region — 0.15
    /// had bound the symbol to a use; re-read the page against these lines
    Reresolved { first: usize, last: usize },
    /// fresh as recorded, but the symbol no longer resolves (R-114)
    Unresolvable(String),
    /// stale before the conversion; stale after it
    StaleAsRecorded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinConversion {
    /// the page, root-relative
    pub page: String,
    /// `path#symbol`
    pub pin: String,
    pub outcome: Conversion,
}

/// One pin carrying a 0.4 `hash:`: is it fresh as recorded (its D-069 region,
/// canonical form), and does D-106 read the same region? The region hash is
/// returned for an acknowledgement.
fn convert_one(repo: &Path, pin: &Pin) -> (Conversion, Option<String>) {
    let Ok(source) = fs::read_to_string(repo.join(&pin.path)) else {
        return (Conversion::StaleAsRecorded, None);
    };
    let symbol = pin.symbol.as_deref();
    let Ok(old) = region(&source, &pin.path, symbol) else {
        return (Conversion::StaleAsRecorded, None);
    };
    if content_hash(&old) != pin.hash {
        return (Conversion::StaleAsRecorded, None);
    }
    match declared_region(&source, &pin.path, symbol) {
        Err(e) => (Conversion::Unresolvable(e), None),
        Ok(new) if new == old => (
            Conversion::Acknowledged,
            Some(crate::ack::region_hash(&new, &pin.path)),
        ),
        Ok(_) => {
            let (first, last) = symbol
                .and_then(|s| crate::symbols::resolve(&source, &pin.path, s).ok())
                .unwrap_or((1, source.lines().count()));
            (Conversion::Reresolved { first, last }, None)
        }
    }
}

fn convert_page(
    root: &Path,
    repo: &Path,
    rel: &str,
    apply: bool,
) -> Result<Vec<PinConversion>, String> {
    let file = root.join(rel);
    let text = fs::read_to_string(&file).map_err(|e| e.to_string())?;
    let Some(fm) = crate::fm::parse(&text) else {
        return Ok(Vec::new());
    };
    let id = page_id(&fm);
    let mut out = Vec::new();
    let mut acks = Vec::new();
    for pin in pins_of(&fm).into_iter().filter(|p| !p.hash.is_empty()) {
        let (mut outcome, hash) = convert_one(repo, &pin);
        match (&id, hash) {
            (Some(_), Some(h)) => acks.push(h),
            (None, Some(_)) => {
                outcome = Conversion::Unresolvable(format!(
                    "{rel} has no `id` (R-050) — acknowledgements are kept by page id"
                ))
            }
            _ => {}
        }
        out.push(PinConversion {
            page: rel.to_string(),
            pin: pin.label(),
            outcome,
        });
    }
    if apply && !out.is_empty() {
        if let Some(id) = &id {
            for h in &acks {
                crate::ack::write(root, id, h).map_err(|e| e.to_string())?;
            }
        }
        if let Some(stripped) = strip_pin_hashes(&text) {
            fs::write(&file, stripped).map_err(|e| e.to_string())?;
        }
    }
    Ok(out)
}

/// The migration of pins to acknowledgements (D-119), for `docsys upgrade`:
/// every pin that still carries a `hash:` is checked as 0.15 recorded it. One
/// that is fresh and whose D-106 declaration reads the same lines gets its
/// acknowledgement; every `hash:` line leaves the page, and nothing else in
/// it changes — `updated:` included, because a hash is bookkeeping (§2.4). No
/// pin is ever refreshed: a stale one, or one whose declaration is another
/// region, stays unacknowledged and is reported for a re-read (R-175). With
/// `apply` false nothing is written. A pin without `hash:` is not touched, so
/// a second run finds nothing.
pub fn convert_legacy(root: &Path, repo: &Path, apply: bool) -> Result<Vec<PinConversion>, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for page in &tree.pages {
        if page.kind != Kind::Permanent {
            continue;
        }
        let carries = page
            .fm
            .as_ref()
            .is_some_and(|fm| pins_of(fm).iter().any(|p| !p.hash.is_empty()));
        if carries {
            out.extend(convert_page(root, repo, &page.rel, apply)?);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn stripping_pin_hashes_touches_nothing_else() {
        let page = "---\nid: p\nverifies:\n  - path: a.rs\n    symbol: f\n    hash: \"sha256:x\"\n  - hash: \"sha256:y\"\n    path: b.rs\nupdated: 2026-09-01\n---\nbody\n    hash: stays\n";
        let want = "---\nid: p\nverifies:\n  - path: a.rs\n    symbol: f\n  - path: b.rs\nupdated: 2026-09-01\n---\nbody\n    hash: stays\n";
        assert_eq!(strip_pin_hashes(page).as_deref(), Some(want));
        assert_eq!(strip_pin_hashes(want), None, "nothing left to strip");
        assert_eq!(strip_pin_hashes("no frontmatter\n    hash: x\n"), None);
    }

    #[test]
    fn a_pin_without_a_hash_is_written_without_the_line() {
        let page = "---\nid: p\nupdated: 2026-09-01\n---\nbody\n";
        let pins = [
            Pin {
                path: "a.rs".into(),
                symbol: Some("f".into()),
                hash: String::new(),
                block: Some("941ba81fbfec".into()),
            },
            Pin {
                path: "b.rs".into(),
                symbol: None,
                hash: "sha256:x".into(),
                block: None,
            },
        ];
        let out = rewrite(page, &pins, Some("2026-10-02")).unwrap();
        assert_eq!(
            out,
            "---\nid: p\nupdated: 2026-10-02\nverifies:\n  - path: a.rs\n    symbol: f\n    block: 941ba81fbfec\n  - path: b.rs\n    hash: \"sha256:x\"\n---\nbody\n"
        );
        // read back as written: a refresh that rewrites the page keeps the binding
        let fm = crate::fm::parse(&out).unwrap();
        assert_eq!(pins_of(&fm), pins);
    }

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
            block: None,
        }];
        let out = rewrite(text, &pins, Some("2026-09-02")).unwrap();
        assert_eq!(
            out,
            "---\nid: x\ntype: reference\nupdated: 2026-09-02\ntags: [a]\nverifies:\n  - path: src/a.rs\n    symbol: f\n    hash: \"sha256:abc\"\n---\nBody.\n"
        );
        let fm = crate::fm::parse(&out).unwrap();
        assert_eq!(pins_of(&fm), pins);
    }
}
