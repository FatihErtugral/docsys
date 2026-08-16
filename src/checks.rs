//! The v0 lint checks. Severity follows SPEC §2.2 literally: a rule that says
//! "is an error" blocks, everything else warns (R-017). Every check counts the
//! units it inspected (R-011).

use crate::fm::Value;
use crate::model::{is_iso_date, is_local_id, Finding, RuleId, VALID_STATUS, VALID_TYPES};
use crate::tree::{DocTree, Kind, Page, Profile};
use std::collections::{BTreeMap, BTreeSet};

pub struct Report {
    pub findings: Vec<Finding>,
    /// check name → units inspected (R-011: zero on an empty population passes;
    /// the caller decides nothing here beyond honest counting).
    pub inspected: BTreeMap<&'static str, usize>,
}

const R011: RuleId = RuleId("R-011");
const R013: RuleId = RuleId("R-013");
const R020: RuleId = RuleId("R-020");
const R023: RuleId = RuleId("R-023");
const R024: RuleId = RuleId("R-024");
const R026: RuleId = RuleId("R-026");
const R028: RuleId = RuleId("R-028");
const R029: RuleId = RuleId("R-029");
const R030: RuleId = RuleId("R-030");
const R034: RuleId = RuleId("R-034");
const R035: RuleId = RuleId("R-035");
const R041: RuleId = RuleId("R-041");
const R048: RuleId = RuleId("R-048");
const R050: RuleId = RuleId("R-050");
const R054: RuleId = RuleId("R-054");
const R055: RuleId = RuleId("R-055");
const R056: RuleId = RuleId("R-056");
const R058: RuleId = RuleId("R-058");
const R059: RuleId = RuleId("R-059");
const R060: RuleId = RuleId("R-060");
const R061: RuleId = RuleId("R-061");
const R070: RuleId = RuleId("R-070");
const R071: RuleId = RuleId("R-071");
const R073: RuleId = RuleId("R-073");
const R076: RuleId = RuleId("R-076");
const R080: RuleId = RuleId("R-080");
const R081: RuleId = RuleId("R-081");
const R075: RuleId = RuleId("R-075");
const R100: RuleId = RuleId("R-100");
const R101: RuleId = RuleId("R-101");
const R103: RuleId = RuleId("R-103");
const R108: RuleId = RuleId("R-108");
const R160: RuleId = RuleId("R-160");
const R161: RuleId = RuleId("R-161");
const R194: RuleId = RuleId("R-194");

/// Fenced-code and blockquote lines are quoted material (R-074/R-075): path
/// and reference scanning skips them. Returns the lines that participate.
fn scannable_lines(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for (i, line) in text.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence || t.starts_with('>') {
            continue;
        }
        out.push((i, line));
    }
    out
}

fn check_docmeta(tree: &DocTree, r: &mut Report) {
    r.inspected
        .insert("docmeta", usize::from(tree.docmeta_present));
    if !tree.docmeta_present {
        // Without .docmeta.yml nothing else is decidable; main turns this into
        // a configuration error (exit 2, D-005).
        r.findings.push(Finding::err(
            R160,
            "-",
            ".docmeta.yml",
            "missing .docmeta.yml at the documentation root".to_string(),
        ));
        return;
    }
    for p in &tree.docmeta_problems {
        r.findings.push(Finding::warn(
            R161,
            "-",
            ".docmeta.yml",
            format!("parse: {p}"),
        ));
    }
    match tree.docmeta_str("spec") {
        Some(v)
            if v.strip_prefix("docsys/0.")
                .is_some_and(|m| m.chars().all(|c| c.is_ascii_digit()) && !m.is_empty()) => {}
        Some(v) => r.findings.push(Finding::err(
            R013,
            "-",
            "spec",
            format!("`{v}` is not an implemented `docsys/0.<minor>` version"),
        )),
        None => r.findings.push(Finding::warn(
            R160,
            "-",
            "spec",
            "missing `spec:`".to_string(),
        )),
    }
    match tree.docmeta_str("profile") {
        Some("project") | Some("knowledge-base") => {}
        Some(v) => r.findings.push(Finding::warn(
            R020,
            "-",
            "profile",
            format!("`{v}` is not a profile"),
        )),
        None => r.findings.push(Finding::warn(
            R160,
            "-",
            "profile",
            "missing `profile:`".to_string(),
        )),
    }
    if tree.docmeta_str("default_content_language").is_none() {
        r.findings.push(Finding::warn(
            R160,
            "-",
            "default_content_language",
            "missing `default_content_language:`".to_string(),
        ));
    }
    // R-161: unknown keys are reported, never rejected.
    const KNOWN: [&str; 17] = [
        "spec",
        "profile",
        "default_content_language",
        "domains",
        "journal_entry_max_lines",
        "list_labels",
        "stale_active_days",
        "created",
        "namespace",
        "federation_role",
        "manifest_url",
        "content_url",
        "work_categories",
        "epics",
        "scan_exclude",
        "deprecation_window",
        "headings",
    ];
    for key in tree.docmeta.keys() {
        if !KNOWN.contains(&key.as_str()) {
            r.findings.push(Finding::warn(
                R161,
                "-",
                key,
                format!("unknown key `{key}` — a misspelling would silently disable a check"),
            ));
        }
    }
}

fn check_permanent_frontmatter(tree: &DocTree, r: &mut Report) {
    let mut inspected = 0usize;
    let mut ids: BTreeMap<String, String> = BTreeMap::new(); // id → first file
    for ts in &tree.tombstones {
        ids.insert(ts.clone(), "(tombstone ledger)".to_string());
    }
    for page in &tree.pages {
        if page.kind != Kind::Permanent {
            continue;
        }
        inspected += 1;
        let Some(fm) = &page.fm else {
            r.findings.push(Finding::warn(
                R050,
                &page.rel,
                "frontmatter",
                "permanent page has no frontmatter".to_string(),
            ));
            continue;
        };
        for p in &fm.problems {
            r.findings.push(Finding::warn(
                R050,
                &page.rel,
                "frontmatter",
                format!("parse: {p}"),
            ));
        }
        let missing: Vec<&str> = ["id", "type", "updated"]
            .into_iter()
            .filter(|k| fm.fields.get(*k).and_then(Value::as_str).is_none())
            .collect();
        if !missing.is_empty() {
            r.findings.push(Finding::warn(
                R050,
                &page.rel,
                &missing.join(","),
                format!("missing frontmatter field(s): {}", missing.join(", ")),
            ));
        }
        if let Some(t) = fm.fields.get("type").and_then(Value::as_str) {
            if !VALID_TYPES.contains(&t) {
                r.findings.push(Finding::warn(
                    R030,
                    &page.rel,
                    "type",
                    format!("`{t}` is not one of {}", VALID_TYPES.join("/")),
                ));
            }
        }
        if let Some(u) = fm.fields.get("updated").and_then(Value::as_str) {
            if !is_iso_date(u) {
                r.findings.push(Finding::warn(
                    R050,
                    &page.rel,
                    "updated",
                    format!("`{u}` is not an ISO date (D-004)"),
                ));
            }
        }
        if let Some(id) = fm.fields.get("id").and_then(Value::as_str) {
            if !is_local_id(id) {
                r.findings.push(Finding::warn(
                    R060,
                    &page.rel,
                    "id",
                    format!("`{id}` does not match local-id grammar"),
                ));
            } else if let Some(first) = ids.get(id) {
                r.findings.push(Finding::err(
                    R061,
                    &page.rel,
                    id,
                    format!("id `{id}` already claimed by {first}"),
                ));
            } else {
                ids.insert(id.to_string(), page.rel.clone());
            }
            // Aliases occupy the uniqueness domain too (R-061).
            if let Some(aliases) = fm.fields.get("aliases").and_then(Value::as_list) {
                for a in aliases {
                    if let Some(first) = ids.get(a) {
                        if first != &page.rel {
                            r.findings.push(Finding::err(
                                R061,
                                &page.rel,
                                a,
                                format!("alias `{a}` already claimed by {first}"),
                            ));
                        }
                    } else {
                        ids.insert(a.clone(), page.rel.clone());
                    }
                }
            }
        }
        if tree.profile == Profile::KnowledgeBase {
            kb_page_checks(tree, page, fm, r);
        }
    }
    r.inspected.insert("permanent-frontmatter", inspected);
}

/// Knowledge-base page fields (§3.1). The shared trio (id/type/updated) is the
/// caller's; this adds the profile's own contract: `domain`, `verification`
/// with its R-028 record, `sources` presence, and the R-029 directory type.
fn kb_page_checks(tree: &DocTree, page: &Page, fm: &crate::fm::Frontmatter, r: &mut Report) {
    let get = |k: &str| fm.fields.get(k).and_then(Value::as_str);
    let mut missing: Vec<&str> = Vec::new();
    if get("domain").is_none() {
        missing.push("domain");
    }
    if get("verification").is_none() {
        missing.push("verification");
    }
    if !fm.fields.contains_key("sources") {
        missing.push("sources");
    }
    if !missing.is_empty() {
        r.findings.push(Finding::warn(
            R024,
            &page.rel,
            &missing.join(","),
            format!("missing knowledge-base field(s): {}", missing.join(", ")),
        ));
    }
    if let Some(d) = get("domain") {
        if !tree.docmeta_list("domains").iter().any(|x| x == d) {
            r.findings.push(Finding::warn(
                R026,
                &page.rel,
                "domain",
                format!("domain `{d}` is not declared in .docmeta.yml `domains:`"),
            ));
        }
    }
    match get("verification") {
        Some("unverified") | None => {}
        Some("verified") => {
            // R-028: a verification nobody can audit is a claim, not a record.
            // D-030 names the fields.
            let rec: Vec<&str> = ["verified_by", "verified_rev"]
                .into_iter()
                .filter(|k| get(k).is_none())
                .collect();
            if !rec.is_empty() {
                r.findings.push(Finding::warn(
                    R028,
                    &page.rel,
                    &rec.join(","),
                    "verified without recording who verified and which source revision".to_string(),
                ));
            }
        }
        Some(v) => r.findings.push(Finding::warn(
            R024,
            &page.rel,
            "verification",
            format!("`{v}` is not unverified/verified"),
        )),
    }
    // R-029: readers navigate this profile by directory; the segment must not
    // contradict the page.
    if let Some(t) = get("type") {
        let dir_type = page.rel.split('/').nth(2).unwrap_or("");
        if VALID_TYPES.contains(&t) && dir_type != t {
            r.findings.push(Finding::warn(
                R029,
                &page.rel,
                "type",
                format!("page sits under `{dir_type}/` but declares `type: {t}`"),
            ));
        }
    }
}

/// R-059: every `sources:` entry resolves. A severed evidence trail is the
/// silent failure R-027 names, so a missing file blocks (§2.2).
fn check_sources(tree: &DocTree, r: &mut Report) {
    let mut inspected = 0usize;
    for page in &tree.pages {
        if page.kind != Kind::Permanent {
            continue;
        }
        let Some(fm) = &page.fm else { continue };
        let Some(srcs) = fm.fields.get("sources").and_then(Value::as_list) else {
            continue;
        };
        for s in srcs {
            if s.contains("://") {
                continue; // URLs are out of scope (D-030)
            }
            inspected += 1;
            if !tree.root.join(s).is_file() {
                r.findings.push(Finding::err(
                    R059,
                    &page.rel,
                    s,
                    format!("sources entry `{s}` does not resolve — the evidence trail is severed"),
                ));
            }
        }
    }
    r.inspected.insert("sources", inspected);
}

/// R-023: `raw/` is content-immutable. The git working tree is the only
/// observable baseline (D-031): uncommitted modification or deletion of a
/// tracked raw file is reported here — at the gate, before it becomes
/// history. Relocation (the basename reappearing under `raw/`) is permitted
/// and expected. Outside a git repository the hook layer owns the promise.
fn check_raw_immutability(tree: &DocTree, r: &mut Report) {
    use std::process::Command;
    let raw_count = tree.pages.iter().filter(|p| p.kind == Kind::Raw).count();
    r.inspected.insert("raw-immutable", raw_count);
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(&tree.root)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let Some(prefix) = git(&["rev-parse", "--show-prefix"]) else {
        return;
    };
    let prefix = prefix.trim().to_string();
    let Some(status) = git(&["status", "--porcelain=v1", "--no-renames", "--", "raw/"]) else {
        return;
    };
    for line in status.lines() {
        let (Some(xy), Some(path)) = (line.get(..2), line.get(3..)) else {
            continue;
        };
        let path = path.trim().trim_matches('"');
        let rel = path.strip_prefix(prefix.as_str()).unwrap_or(path);
        // An added file is not yet a record; it stays mutable until it lands.
        if !rel.starts_with("raw/") || xy == "??" || xy.contains('A') {
            continue;
        }
        if xy.contains('M') || xy.contains('T') {
            r.findings.push(Finding::err(
                R023,
                rel,
                "content",
                "bytes of an existing raw record changed — raw/ is content-immutable".to_string(),
            ));
        } else if xy.contains('D') {
            let base = rel.rsplit('/').next().unwrap_or(rel);
            if !basename_exists(&tree.root.join("raw"), base) {
                r.findings.push(Finding::err(
                    R023,
                    rel,
                    "deleted",
                    "raw record deleted — relocation keeps the file; deletion loses the record"
                        .to_string(),
                ));
            }
        }
    }
}

fn basename_exists(dir: &std::path::Path, name: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for e in entries.filter_map(Result::ok) {
        let p = e.path();
        if p.is_dir() {
            if basename_exists(&p, name) {
                return true;
            }
        } else if p.file_name().and_then(|n| n.to_str()) == Some(name) {
            return true;
        }
    }
    false
}

fn check_work(tree: &DocTree, r: &mut Report) {
    let mut inspected = 0usize;
    let epics = tree.docmeta_list("epics").to_vec();
    for page in &tree.pages {
        match page.kind {
            Kind::Tracked => {
                inspected += 1;
                let Some(fm) = &page.fm else {
                    r.findings.push(Finding::warn(
                        R054,
                        &page.rel,
                        "frontmatter",
                        "tracked-work file has no frontmatter".to_string(),
                    ));
                    continue;
                };
                let status = fm.fields.get("status").and_then(Value::as_str);
                match status {
                    None => r.findings.push(Finding::warn(
                        R054,
                        &page.rel,
                        "status",
                        "missing `status:`".to_string(),
                    )),
                    Some(s) if !VALID_STATUS.contains(&s) => r.findings.push(Finding::warn(
                        R080,
                        &page.rel,
                        "status",
                        format!("`{s}` is not one of {}", VALID_STATUS.join("/")),
                    )),
                    Some(_) => {}
                }
                if fm.fields.get("updated").and_then(Value::as_str).is_none() {
                    r.findings.push(Finding::warn(
                        R054,
                        &page.rel,
                        "updated",
                        "missing `updated:`".to_string(),
                    ));
                }
                if status == Some("abandoned")
                    && fm
                        .fields
                        .get("abandoned_reason")
                        .and_then(Value::as_str)
                        .is_none_or(str::is_empty)
                {
                    r.findings.push(Finding::warn(
                        R055,
                        &page.rel,
                        "abandoned_reason",
                        "abandoned without a reason".to_string(),
                    ));
                }
                if status == Some("graduated") {
                    let grads = fm.fields.get("graduated_to").and_then(Value::as_list);
                    match grads {
                        None | Some([]) => r.findings.push(Finding::warn(
                            R056,
                            &page.rel,
                            "graduated_to",
                            "graduated without `graduated_to`".to_string(),
                        )),
                        Some(_) => {}
                    }
                }
                if matches!(status, Some("done") | Some("graduated"))
                    && fm.fields.get("confirmed").and_then(Value::as_str).is_none()
                {
                    r.findings.push(Finding::warn(
                        R081,
                        &page.rel,
                        "confirmed",
                        "done/graduated without a recorded human confirmation".to_string(),
                    ));
                }
                if let Some(e) = fm.fields.get("epic").and_then(Value::as_str) {
                    if e.starts_with('@') {
                        r.findings.push(Finding::warn(
                            R058,
                            &page.rel,
                            "epic",
                            "foreign epics are unsupported (federation is experimental)"
                                .to_string(),
                        ));
                    } else if !epics.iter().any(|d| d == e) {
                        r.findings.push(Finding::warn(
                            R058,
                            &page.rel,
                            "epic",
                            format!("epic `{e}` is not declared in .docmeta.yml `epics:`"),
                        ));
                    }
                }
            }
            Kind::ListFile => {
                inspected += 1;
                if page
                    .fm
                    .as_ref()
                    .is_some_and(|f| f.fields.contains_key("status"))
                {
                    r.findings.push(Finding::warn(
                        R041,
                        &page.rel,
                        "status",
                        "a list file must not carry `status`".to_string(),
                    ));
                }
            }
            _ => {}
        }
    }
    r.inspected.insert("work", inspected);
}

/// `graduated_to` entries must resolve against permanent ids (R-056). The
/// resolution is the shared one (D-021): a destination named through a
/// `defines:` family is as real as a page id, and two resolvers over one
/// identifier space would eventually disagree.
fn check_graduated_targets(tree: &DocTree, r: &mut Report) {
    let idx = build_index(tree);
    let mut inspected = 0usize;
    for page in &tree.pages {
        if page.kind != Kind::Tracked {
            continue;
        }
        let Some(fm) = &page.fm else { continue };
        let Some(grads) = fm.fields.get("graduated_to").and_then(Value::as_list) else {
            continue;
        };
        for g in grads {
            inspected += 1;
            if !matches!(resolve_doc_token(&idx, g), Ok(Resolved::Permanent)) {
                r.findings.push(Finding::warn(
                    R056,
                    &page.rel,
                    g,
                    format!("graduated_to `{g}` resolves to no permanent id"),
                ));
            }
        }
    }
    r.inspected.insert("graduated-targets", inspected);
}

/// Everything a `doc:` token can resolve against (shared with `refs`).
pub struct ResolutionIndex {
    pub ids: BTreeSet<String>,
    /// Identifiers of the flowing layer — real pages, but temporary ones, so a
    /// citation resolves and is reported rather than blocking (R-194).
    pub flowing: BTreeSet<String>,
    /// Graduated pages with no resolvable destination — citing one dead-ends.
    pub graduated: BTreeSet<String>,
    /// Graduated pages that name where their value went: signposts, not husks.
    pub graduated_signposted: BTreeSet<String>,
    /// Identifiers of archived pages — records, resolvable, never live claims.
    pub archived: BTreeSet<String>,
    /// (prefix, defining-page body) pairs for `defines:` families (D-008).
    pub families: Vec<(String, String)>,
    pub tombstones: Vec<String>,
}

pub fn build_index(tree: &DocTree) -> ResolutionIndex {
    let mut families = Vec::new();
    for page in &tree.pages {
        if page.kind != Kind::Permanent {
            continue;
        }
        if let Some(fm) = &page.fm {
            // `provides:` is the pre-0.2 spelling of the same page-level
            // declaration (renamed to avoid colliding with the manifest field);
            // the founding field trees still carry it, so both are read.
            let fam = fm
                .fields
                .get("defines")
                .or_else(|| fm.fields.get("provides"))
                .and_then(Value::as_str);
            if let Some(fam) = fam {
                if let Some(prefix) = fam.strip_suffix('*') {
                    families.push((prefix.trim().to_string(), page.text.clone()));
                }
            }
        }
    }
    let permanent: BTreeSet<String> = permanent_ids(tree)
        .into_iter()
        .map(str::to_string)
        .collect();
    let (mut flowing, mut graduated) = (BTreeSet::new(), BTreeSet::new());
    let mut graduated_signposted = BTreeSet::new();
    for page in &tree.pages {
        if page.kind == Kind::Permanent {
            continue;
        }
        let Some(fm) = &page.fm else { continue };
        let Some(id) = fm.fields.get("id").and_then(Value::as_str) else {
            continue;
        };
        if permanent.contains(id) {
            continue;
        }
        let status = fm.fields.get("status").and_then(Value::as_str);
        if status == Some("graduated") {
            // A signposted husk redirects the reader; a dead end does not.
            let signposted = fm
                .fields
                .get("graduated_to")
                .and_then(Value::as_list)
                .is_some_and(|d| d.iter().any(|x| !x.as_str().trim().is_empty()));
            if signposted {
                graduated_signposted.insert(id.to_string());
            } else {
                graduated.insert(id.to_string());
            }
        } else {
            flowing.insert(id.to_string());
        }
    }
    ResolutionIndex {
        ids: permanent,
        flowing,
        graduated,
        graduated_signposted,
        archived: archived_ids(&tree.root),
        families,
        tombstones: tree.tombstones.clone(),
    }
}

/// R-073's trailing-punctuation strip set (spec list + backtick, D-015).
pub const DOC_TOKEN_PUNCT: [char; 11] = ['.', ',', ';', ':', ')', ']', '"', '\'', '?', '!', '`'];

/// Extract `doc:` tokens on a line: (token, before_ok) with punctuation
/// stripped per R-073. `before_ok` guards against `htmldoc:` lookalikes.
pub fn doc_tokens_on_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    // Absolute offset of `rest` in `line`: inline-code parity must be counted
    // from the start of the line, not from the last match (a second reference
    // on the same line would otherwise read the wrong span).
    let mut base = 0usize;
    while let Some(pos) = rest.find("doc: ") {
        let before_ok = pos == 0
            || rest
                .get(..pos)
                .and_then(|s| s.chars().last())
                .is_none_or(|c| !c.is_ascii_alphanumeric());
        let after = rest.get(pos + 5..).unwrap_or("");
        // R-073: a reference opened inside an inline-code span ends at the
        // closing backtick — prose glues suffixes to it (a case ending, a
        // possessive), and those belong to the sentence, not the identifier.
        let opened_in_code = line
            .get(..base + pos)
            .is_some_and(|s| s.matches('`').count() % 2 == 1);
        let raw = if opened_in_code {
            after
                .split('`')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("")
        } else {
            after.split_whitespace().next().unwrap_or("")
        };
        let token = raw.trim_end_matches(DOC_TOKEN_PUNCT);
        // R-073: `doc: <id>` in prose documents the form; it cites nothing.
        let token = if token.contains(['<', '>', '{', '}']) {
            ""
        } else {
            token
        };
        base += pos + 5;
        rest = after;
        if before_ok && !token.is_empty() {
            out.push(token.to_string());
        }
    }
    out
}

/// Resolve one token against the index. `Ok(())` when it resolves; `Err`
/// carries the finding class.
pub enum DocRefFail {
    Foreign,
    BadGrammar,
    Dangling,
}

/// What a resolved `doc:` reference points at (R-194). The layer decides the
/// severity: the permanent layer is the contract, the flowing layer is a
/// promise not yet kept, and a graduated page is a husk.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Resolved {
    Permanent,
    Flowing,
    /// Graduated with no destination recorded: following it dead-ends.
    Graduated,
    /// Graduated and signposted: following it redirects.
    GraduatedSignposted,
    /// An archived record: resolvable, but not a live claim.
    Archived,
}

pub fn resolve_doc_token(idx: &ResolutionIndex, token: &str) -> Result<Resolved, DocRefFail> {
    if token.starts_with('@') {
        return Err(DocRefFail::Foreign);
    }
    // A declared family carries its own case (R-063): match the pattern first,
    // then require the member to occur on the defining page (R-079). A prefix
    // hit with a missing member is a dangling reference, not bad grammar.
    let mut family_prefix_hit = false;
    for (prefix, body) in &idx.families {
        if let Some(short) = token.strip_prefix(prefix.as_str()) {
            family_prefix_hit = true;
            // R-079: the member may be written in full or in the page's own
            // short form — the register already says which family it lists.
            if occurs_as_token(body, token) || (!short.is_empty() && occurs_as_token(body, short)) {
                return Ok(Resolved::Permanent);
            }
        }
    }
    if family_prefix_hit {
        return Err(DocRefFail::Dangling);
    }
    if !is_local_id(token) {
        return Err(DocRefFail::BadGrammar);
    }
    if idx.ids.contains(token) || idx.tombstones.iter().any(|t| t == token) {
        Ok(Resolved::Permanent)
    } else if idx.graduated.contains(token) {
        Ok(Resolved::Graduated)
    } else if idx.graduated_signposted.contains(token) {
        Ok(Resolved::GraduatedSignposted)
    } else if idx.flowing.contains(token) {
        Ok(Resolved::Flowing)
    } else if idx.archived.contains(token) {
        Ok(Resolved::Archived)
    } else {
        Err(DocRefFail::Dangling)
    }
}

/// Identifiers declared by pages under `_archive/`. The walk skips that
/// directory for every check (D-007) — a record joins no population — but a
/// reference to a record still has to resolve, so its ids are read here.
fn archived_ids(root: &std::path::Path) -> BTreeSet<String> {
    fn walk(dir: &std::path::Path, out: &mut BTreeSet<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.filter_map(Result::ok) {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "md") {
                let Ok(text) = std::fs::read_to_string(&p) else {
                    continue;
                };
                if let Some(fm) = crate::fm::parse(&text) {
                    if let Some(id) = fm.fields.get("id").and_then(Value::as_str) {
                        out.insert(id.to_string());
                    }
                }
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(&root.join("_archive"), &mut out);
    out
}

fn permanent_ids(tree: &DocTree) -> BTreeSet<&str> {
    let mut ids = BTreeSet::new();
    for page in &tree.pages {
        if page.kind != Kind::Permanent {
            continue;
        }
        if let Some(fm) = &page.fm {
            if let Some(id) = fm.fields.get("id").and_then(Value::as_str) {
                ids.insert(id);
            }
            if let Some(aliases) = fm.fields.get("aliases").and_then(Value::as_list) {
                for a in aliases {
                    ids.insert(a.as_str());
                }
            }
            if let Some(fam) = fm.fields.get("defines").and_then(Value::as_str) {
                // R-063/R-079: a family resolves a citation only when the cited
                // identifier occurs on the defining page as a token.
                let _ = fam; // handled in resolve_doc_ref
            }
        }
    }
    ids
}

/// Collect wiki-links `[[path]]` / `[[path|alias]]` from scannable lines.
pub(crate) fn wiki_links(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (i, line) in scannable_lines(text) {
        let mut rest = line;
        while let Some(start) = rest.find("[[") {
            let Some(after) = rest.get(start + 2..) else {
                break;
            };
            let Some(end) = after.find("]]") else { break };
            let inner = after.get(..end).unwrap_or("");
            let target = inner.split('|').next().unwrap_or("").trim();
            if !target.is_empty() {
                out.push((i, target.to_string()));
            }
            rest = after.get(end + 2..).unwrap_or("");
        }
    }
    out
}

/// The path a wiki-link resolves against, per profile. In the knowledge-base
/// profile links are written wiki/-relative — `[[<domain>/<type>/<page>]]` is
/// the field convention (D-030) — so the permanent layer root is stripped;
/// `raw/` files are evidence, not link targets, and resolve to nothing.
pub(crate) fn link_path_of(tree: &DocTree, rel: &str) -> Option<String> {
    match tree.profile {
        Profile::Project => Some(rel.trim_end_matches(".md").to_string()),
        Profile::KnowledgeBase => rel
            .strip_prefix("wiki/")
            .map(|s| s.trim_end_matches(".md").to_string()),
    }
}

fn check_links(tree: &DocTree, r: &mut Report) {
    // Resolution set: relative page paths without extension (R-070 full paths).
    let paths: BTreeSet<String> = tree
        .pages
        .iter()
        .filter_map(|p| link_path_of(tree, &p.rel))
        .collect();
    let mut inspected = 0usize;
    for page in &tree.pages {
        for (line, target) in wiki_links(&page.text) {
            inspected += 1;
            // A root-level page's full path is its bare name (R-070).
            let root_level = paths.contains(target.as_str());
            if !target.contains('/') && !root_level && page.kind != Kind::Router {
                // Short-name links are invalid (R-070) — except the router,
                // where entries like [[reference/x|name]] always carry a path
                // anyway, so this branch simply never fires there.
                r.findings.push(Finding::warn(
                    R070,
                    &page.rel,
                    &target,
                    format!(
                        "line {}: short-name link — full path from the docs root required",
                        line + 1
                    ),
                ));
                continue;
            }
            // An explicit [[_archive/...]] link is a deliberate citation of a
            // record — it resolves silently (field convention from a pilot).
            let explicit_archive =
                target.starts_with("_archive/") && tree.root.join(format!("{target}.md")).exists();
            let archived = tree
                .root
                .join("_archive")
                .join(format!("{target}.md"))
                .exists();
            if paths.contains(&target) || explicit_archive {
                // resolves
            } else if archived {
                r.findings.push(Finding::warn(
                    R071,
                    &page.rel,
                    &target,
                    "target moved to _archive/ — still resolves, reported as remaining interest"
                        .to_string(),
                ));
            } else if page.kind == Kind::Raw {
                // A raw note is a record (R-023): its dangling link is
                // reported, never blocked — the record cannot be edited.
                r.findings.push(Finding::warn(
                    R071,
                    &page.rel,
                    &target,
                    format!(
                        "line {}: dangling wiki-link in a raw record — the target moved on",
                        line + 1
                    ),
                ));
            } else {
                r.findings.push(Finding::err(
                    R071,
                    &page.rel,
                    &target,
                    format!("line {}: dangling wiki-link", line + 1),
                ));
            }
        }
    }
    r.inspected.insert("wiki-links", inspected);
}

/// `doc:` references inside the docs tree (code scanning lives in `refs`).
fn check_doc_refs(tree: &DocTree, r: &mut Report) {
    let idx = build_index(tree);
    let mut inspected = 0usize;
    for page in &tree.pages {
        for (line, text_line) in scannable_lines(&page.text) {
            for token in doc_tokens_on_line(text_line) {
                inspected += 1;
                // R-194: the journal records moments; a dated entry citing what
                // was true then is history, not a broken reference. In the
                // knowledge-base profile the record layer is `raw/` (R-023).
                let historical = match tree.profile {
                    Profile::Project => {
                        page.rel == "work/journal.md"
                            || page.rel.starts_with("work/journal/")
                            || page.rel.starts_with("_archive/")
                    }
                    Profile::KnowledgeBase => page.rel.starts_with("raw/"),
                };
                match resolve_doc_token(&idx, &token) {
                    Ok(Resolved::Permanent) => {}
                    Ok(Resolved::Graduated) if historical => {}
                    Ok(Resolved::Flowing) if historical => {}
                    Ok(Resolved::GraduatedSignposted) if historical => {}
                    Ok(Resolved::Archived) if historical => {}
                    Ok(Resolved::Archived) => r.findings.push(Finding::warn(
                        R194,
                        &page.rel,
                        &token,
                        format!(
                            "line {}: `doc: {token}` cites an archived record — dated content, \
                             not a live claim",
                            line + 1
                        ),
                    )),
                    Ok(Resolved::GraduatedSignposted) => r.findings.push(Finding::warn(
                        R194,
                        &page.rel,
                        &token,
                        format!(
                            "line {}: `doc: {token}` cites a graduated page — it signposts its \
                             destination; cite that unless you mean the provenance",
                            line + 1
                        ),
                    )),
                    Ok(Resolved::Graduated) => r.findings.push(Finding::err(
                        R194,
                        &page.rel,
                        &token,
                        format!(
                            "line {}: `doc: {token}` points at a graduated page — its permanent \
                             value moved on; cite the destination",
                            line + 1
                        ),
                    )),
                    Ok(Resolved::Flowing) => r.findings.push(Finding::warn(
                        R194,
                        &page.rel,
                        &token,
                        format!(
                            "line {}: `doc: {token}` cites the flowing layer — distil it into a \
                             permanent page",
                            line + 1
                        ),
                    )),
                    Err(DocRefFail::Foreign) => r.findings.push(Finding::warn(
                        R076,
                        &page.rel,
                        &token,
                        "foreign reference — unresolvable here (federation is experimental)"
                            .to_string(),
                    )),
                    Err(DocRefFail::BadGrammar) => r.findings.push(Finding::warn(
                        R073,
                        &page.rel,
                        &token,
                        format!("line {}: `{token}` is not a local-id", line + 1),
                    )),
                    Err(DocRefFail::Dangling) => {
                        let msg = format!(
                            "line {}: `doc: {token}` resolves to no id, alias, or family member",
                            line + 1
                        );
                        r.findings.push(if historical {
                            Finding::warn(
                                R076,
                                &page.rel,
                                &token,
                                format!(
                                    "{msg} — a record cannot be edited; tombstone the rename or \
                                     distil the page"
                                ),
                            )
                        } else {
                            Finding::err(R076, &page.rel, &token, msg)
                        });
                    }
                }
            }
        }
    }
    r.inspected.insert("doc-refs", inspected);
}

/// R-079: family membership needs the cited identifier to occur as a token on
/// the defining page — substring hits like `adr-1` inside `adr-12` don't count.
fn occurs_as_token(body: &str, token: &str) -> bool {
    let mut start = 0usize;
    while let Some(pos) = body.get(start..).and_then(|s| s.find(token)) {
        let abs = start + pos;
        let before_ok = abs == 0
            || body
                .get(..abs)
                .and_then(|s| s.chars().last())
                .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-');
        // R-079: an occurrence that is itself a citation proves nothing.
        let is_citation = body
            .get(..abs)
            .is_some_and(|s| s.trim_end_matches('`').ends_with("doc: "));
        let after_ok = body
            .get(abs + token.len()..)
            .and_then(|s| s.chars().next())
            .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-');
        if before_ok && after_ok && !is_citation {
            return true;
        }
        start = abs + token.len().max(1);
    }
    false
}

fn check_paths(tree: &DocTree, r: &mut Report) {
    let mut inspected = 0usize;
    for page in &tree.pages {
        if page.kind == Kind::Raw {
            // A raw note is quoted source material wholesale (D-030): the
            // path scan skips it as it skips fenced and blockquoted lines.
            continue;
        }
        for (line, text_line) in scannable_lines(&page.text) {
            inspected += 1;
            let absolute = text_line.contains("](/home/")
                || text_line.contains("](/Users/")
                || text_line.contains("](C:\\")
                || text_line.split_whitespace().any(|w| {
                    w.starts_with("/home/") || w.starts_with("/Users/") || w.starts_with("C:\\")
                });
            if absolute {
                r.findings.push(Finding::err(
                    R075,
                    &page.rel,
                    "absolute-path",
                    format!(
                        "line {}: absolute filesystem path outside quoted material",
                        line + 1
                    ),
                ));
            }
            // A `..` segment is fine while it stays inside the tree; only a
            // resolution that pops past the root escapes (first migration
            // pilot: `../howto/x.md` from `reference/` was falsely flagged).
            let mut escapes = false;
            let mut all_targets_exist = true;
            for target in crate::migrate::md_link_targets_line(text_line) {
                if target.starts_with("..") && crate::migrate::resolve(&page.rel, &target).is_none()
                {
                    escapes = true;
                    // R-075: a link that actually points at a file is working
                    // documentation (a catalog README next to the code), not
                    // the silent breakage §2.2 reserves blocking for.
                    let from_dir = tree.root.join(&page.rel);
                    let exists = from_dir
                        .parent()
                        .map(|d| d.join(&target))
                        .is_some_and(|p| p.exists());
                    if !exists {
                        all_targets_exist = false;
                    }
                }
            }
            if text_line.contains("[[../") {
                escapes = true; // wiki-links are root-relative; `..` never valid
                all_targets_exist = false;
            }
            if escapes {
                let msg = format!(
                    "line {}: relative link traverses outside the tree",
                    line + 1
                );
                r.findings.push(if all_targets_exist {
                    Finding::warn(
                        R075,
                        &page.rel,
                        "escaping-link",
                        format!("{msg} — target exists; the link breaks if the tree moves"),
                    )
                } else {
                    Finding::err(R075, &page.rel, "escaping-link", msg)
                });
            }
        }
    }
    r.inspected.insert("path-scan", inspected);
}

fn check_router_and_orphans(tree: &DocTree, r: &mut Report) {
    let mut permanent = 0usize;
    for p in &tree.pages {
        if p.kind == Kind::Permanent {
            permanent += 1;
        }
    }
    r.inspected.insert("router", permanent);
    if permanent == 0 {
        return; // empty population passes (R-011)
    }
    // The root router per profile (D-030): navigation in the knowledge-base
    // profile starts at the permanent layer's own index.
    let root_router = match tree.profile {
        Profile::Project => "index.md",
        Profile::KnowledgeBase => "wiki/index.md",
    };
    // R-035: router entry grammar, on every router — in the knowledge-base
    // profile the domain indexes route too. Only lines that carry a wiki-link
    // are entries (D-014); plain bullets are prose, and whether prose belongs
    // on a router is judgment, not grammar.
    for router in tree.pages.iter().filter(|p| p.kind == Kind::Router) {
        for (i, line) in router.text.lines().enumerate() {
            if !line.starts_with("- ") || !line.contains("[[") {
                continue;
            }
            let ok = line.starts_with("- [[")
                && line.contains("]]")
                && line.contains('|')
                && (line.contains(" -- ") || line.contains(" — "));
            if !ok {
                r.findings.push(Finding::warn(
                    R035,
                    &router.rel,
                    &format!("line-{}", i + 1),
                    "router line is not `- [[<path>|<title>]] -- <one sentence>`".to_string(),
                ));
            }
        }
    }
    let Some(router) = tree.pages.iter().find(|p| p.rel == root_router) else {
        r.findings.push(Finding::warn(
            R034,
            "-",
            root_router,
            "permanent pages exist but there is no root router".to_string(),
        ));
        return;
    };
    // R-034: reachability = wiki-link edges from the router, transitively
    // through any non-archived page (registered decision D-009); link paths
    // resolve per profile (D-030).
    let mut reachable: BTreeSet<String> = BTreeSet::new();
    let mut queue: Vec<String> = wiki_links(&router.text)
        .into_iter()
        .map(|(_, t)| t)
        .collect();
    while let Some(t) = queue.pop() {
        if !reachable.insert(t.clone()) {
            continue;
        }
        if let Some(page) = tree
            .pages
            .iter()
            .find(|p| link_path_of(tree, &p.rel).as_deref() == Some(t.as_str()))
        {
            queue.extend(wiki_links(&page.text).into_iter().map(|(_, x)| x));
        }
    }
    for page in &tree.pages {
        if page.kind != Kind::Permanent {
            continue;
        }
        if link_path_of(tree, &page.rel).is_some_and(|p| !reachable.contains(&p)) {
            r.findings.push(Finding::warn(
                R034,
                &page.rel,
                "orphan",
                "not reachable from the root router".to_string(),
            ));
        }
    }
}

fn check_journal(tree: &DocTree, r: &mut Report) {
    let mut inspected = 0usize;
    for page in &tree.pages {
        if !(page.kind == Kind::ListFile && page.rel.ends_with("journal.md")) {
            continue;
        }
        let total_lines = page.text.lines().count();
        if total_lines > 500 {
            r.findings.push(Finding::warn(
                R103,
                &page.rel,
                "length",
                format!("{total_lines} lines — rotation (journal slice) resolves this"),
            ));
        }
        // R-101: 5 lines unless the tree declares a discipline of its own.
        let budget = tree
            .docmeta_str("journal_entry_max_lines")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(5);
        let mut entry_start: Option<(usize, usize)> = None; // (line, body_len)
        let flush = |r: &mut Report, e: Option<(usize, usize)>| {
            if let Some((line, body)) = e {
                if body > budget {
                    r.findings.push(Finding::warn(
                        R101,
                        &page.rel,
                        &format!("entry-{}", line + 1),
                        format!("entry body is {body} lines (max {budget})"),
                    ));
                }
            }
        };
        for (i, line) in page.text.lines().enumerate() {
            if let Some(rest) = line.strip_prefix("## ") {
                inspected += 1;
                flush(r, entry_start.take());
                let date = rest.get(..10).unwrap_or("");
                let sep_ok = rest.get(10..).is_some_and(|s| {
                    let s = s.trim_start();
                    // R-100: one bracketed annotation may sit between the date
                    // and the separator (an entry counter, a channel tag).
                    let s = match s.chars().next() {
                        Some('(') => s.split_once(')').map_or(s, |(_, rest)| rest).trim_start(),
                        Some('[') => s.split_once(']').map_or(s, |(_, rest)| rest).trim_start(),
                        _ => s,
                    };
                    s.starts_with('-') || s.starts_with('—')
                });
                if !is_iso_date(date) || !sep_ok {
                    r.findings.push(Finding::warn(
                        R100,
                        &page.rel,
                        &format!("entry-{}", i + 1),
                        "entry heading is not `## YYYY-MM-DD - title`".to_string(),
                    ));
                }
                entry_start = Some((i, 0));
            } else if let Some((_, ref mut body)) = entry_start {
                if !line.trim().is_empty() {
                    *body += 1;
                }
            }
        }
        flush(r, entry_start.take());
    }
    r.inspected.insert("journal", inspected);
}

fn check_list_grammars(tree: &DocTree, r: &mut Report) {
    let mut inspected = 0usize;
    for page in &tree.pages {
        let is_debt = page.rel.ends_with("debt.md");
        let is_q = page.rel.ends_with("questions.md");
        if !(page.kind == Kind::ListFile && (is_debt || is_q)) {
            continue;
        }
        for (i, line) in page.text.lines().enumerate() {
            if !line.starts_with("- [") {
                continue;
            }
            inspected += 1;
            let closed = line.starts_with("- [x] ");
            let open = line.starts_with("- [ ] ");
            if !closed && !open {
                r.findings.push(Finding::warn(
                    R108,
                    &page.rel,
                    &format!("line-{}", i + 1),
                    "entry must open `- [ ] ` or `- [x] `".to_string(),
                ));
                continue;
            }
            let norm = line.replace(" — ", " -- ");
            let ok = if is_debt {
                let base = norm.contains(&format!(" -- {}: ", label_of(tree, "deferred")))
                    && norm.contains(&format!(" -- {}: ", label_of(tree, "repay when")));
                if closed {
                    base && norm.contains(&format!(" -- {}: ", label_of(tree, "resolved")))
                } else {
                    base
                }
            } else {
                let after = norm.get(6..).unwrap_or("");
                let date_ok = is_iso_date(after.get(..10).unwrap_or(""));
                if closed {
                    date_ok && norm.contains(&format!(" -- {}: ", label_of(tree, "answered")))
                } else {
                    date_ok
                }
            };
            if !ok {
                r.findings.push(Finding::warn(
                    R108,
                    &page.rel,
                    &format!("line-{}", i + 1),
                    "entry does not match its item grammar".to_string(),
                ));
            }
        }
    }
    r.inspected.insert("list-grammar", inspected);
}

/// `headings: [Context=Bağlam, ...]` — canonical key → displayed heading.
/// The tool translates nothing; it only matches (D-025).
/// Local forms of the list-item field labels (R-108), same shape as the
/// heading map: canonical name → the form this tree actually writes.
fn label_of(tree: &DocTree, canonical: &str) -> String {
    tree.docmeta_list("list_labels")
        .iter()
        .filter_map(|e| e.split_once('='))
        .find(|(k, _)| k.trim() == canonical)
        .map_or_else(|| canonical.to_string(), |(_, v)| v.trim().to_string())
}

pub fn heading_map(tree: &DocTree) -> std::collections::BTreeMap<String, String> {
    tree.docmeta_list("headings")
        .iter()
        .filter_map(|e| e.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

fn check_templates(tree: &DocTree, r: &mut Report) {
    let map = heading_map(tree);
    const SECTIONS: [(&str, [&str; 4]); 3] = [
        (
            "features",
            [
                "Context",
                "Decision",
                "Contract surface",
                "Rejected alternatives",
            ],
        ),
        (
            "postmortems",
            ["What happened", "Root cause", "Recurrence", "Lesson"],
        ),
        (
            "research",
            ["Question", "Tried", "Learned", "Why no decision"],
        ),
    ];
    let mut inspected = 0usize;
    for page in &tree.pages {
        if page.kind != Kind::Tracked {
            continue;
        }
        let Some(category) = page
            .rel
            .strip_prefix("work/")
            .and_then(|s| s.split('/').next())
        else {
            continue;
        };
        let Some((_, required)) = SECTIONS.iter().find(|(c, _)| *c == category) else {
            continue; // R-042 categories have no template
        };
        // R-048: closed work is exempt — the template guides open work and
        // makes graduation mechanical; a finished page cannot use it.
        let closed = page.fm.as_ref().is_some_and(|fm| {
            matches!(
                fm.fields.get("status").and_then(Value::as_str),
                Some("graduated") | Some("abandoned")
            )
        });
        if closed {
            continue;
        }
        inspected += 1;
        let missing: Vec<&str> = required
            .iter()
            .filter(|s| {
                let shown = map.get(**s).map(String::as_str).unwrap_or(s);
                let heading = format!("## {shown}");
                !page.text.lines().any(|l| l.trim_end() == heading)
            })
            .copied()
            .collect();
        if !missing.is_empty() {
            r.findings.push(Finding::warn(
                R048,
                &page.rel,
                &missing.join(","),
                format!("missing template section(s): {}", missing.join(", ")),
            ));
        }
    }
    r.inspected.insert("templates", inspected);
}

/// Run every check. `dead_scan` is R-011's second half: the caller flags a
/// tree whose configured root matched nothing at all.
pub fn run(tree: &DocTree) -> Report {
    let mut r = Report {
        findings: Vec::new(),
        inspected: BTreeMap::new(),
    };
    check_docmeta(tree, &mut r);
    if !tree.docmeta_present {
        return r; // configuration error; nothing else is decidable
    }
    check_permanent_frontmatter(tree, &mut r);
    check_work(tree, &mut r);
    check_graduated_targets(tree, &mut r);
    check_links(tree, &mut r);
    check_doc_refs(tree, &mut r);
    check_paths(tree, &mut r);
    check_router_and_orphans(tree, &mut r);
    check_journal(tree, &mut r);
    check_list_grammars(tree, &mut r);
    check_templates(tree, &mut r);
    if tree.profile == Profile::KnowledgeBase {
        check_sources(tree, &mut r);
        check_raw_immutability(tree, &mut r);
    }
    if tree.pages.is_empty() {
        r.findings.push(Finding::warn(
            R011,
            "-",
            "tree",
            "the documentation root contains no markdown files".to_string(),
        ));
    } else {
        // D-016: files exist but none live inside the layout — that is not a
        // clean tree, it is an unmigrated one, and silence would be the lie.
        let in_layout = tree.pages.iter().any(|p| {
            matches!(
                p.kind,
                Kind::Permanent | Kind::Tracked | Kind::ListFile | Kind::Router | Kind::Raw
            )
        });
        if !in_layout {
            let n = tree.pages.len();
            let layout = match tree.profile {
                Profile::Project => "project",
                Profile::KnowledgeBase => "knowledge-base",
            };
            r.findings.push(Finding::warn(
                RuleId("R-020"),
                "-",
                "layout",
                format!(
                    "{n} markdown file(s), none inside the {layout} layout — \
                     a brownfield tree; migration, not linting, is the next step"
                ),
            ));
        }
    }
    r.findings.sort();
    r.findings.dedup();
    r
}
