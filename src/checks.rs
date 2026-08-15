//! The v0 lint checks. Severity follows SPEC §2.2 literally: a rule that says
//! "is an error" blocks, everything else warns (R-017). Every check counts the
//! units it inspected (R-011).

use crate::fm::Value;
use crate::model::{is_iso_date, is_local_id, Finding, RuleId, VALID_STATUS, VALID_TYPES};
use crate::tree::{DocTree, Kind};
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
    r.inspected.insert("docmeta", usize::from(tree.docmeta_present));
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
        r.findings
            .push(Finding::warn(R161, "-", ".docmeta.yml", format!("parse: {p}")));
    }
    match tree.docmeta_str("spec") {
        Some(v) if v.strip_prefix("docsys/0.").is_some_and(|m| {
            m.chars().all(|c| c.is_ascii_digit()) && !m.is_empty()
        }) => {}
        Some(v) => r.findings.push(Finding::err(
            R013,
            "-",
            "spec",
            format!("`{v}` is not an implemented `docsys/0.<minor>` version"),
        )),
        None => r
            .findings
            .push(Finding::warn(R160, "-", "spec", "missing `spec:`".to_string())),
    }
    match tree.docmeta_str("profile") {
        Some("project") => {}
        Some("knowledge-base") => r.findings.push(Finding::warn(
            R020,
            "-",
            "profile",
            "knowledge-base profile is not implemented in v0 (D-006)".to_string(),
        )),
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
    const KNOWN: [&str; 12] = [
        "spec",
        "profile",
        "default_content_language",
        "created",
        "namespace",
        "federation_role",
        "manifest_url",
        "content_url",
        "work_categories",
        "epics",
        "scan_exclude",
        "deprecation_window",
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
            r.findings
                .push(Finding::warn(R050, &page.rel, "frontmatter", format!("parse: {p}")));
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
    }
    r.inspected.insert("permanent-frontmatter", inspected);
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

/// `graduated_to` entries must resolve against permanent ids (R-056).
fn check_graduated_targets(tree: &DocTree, r: &mut Report) {
    let ids: BTreeSet<&str> = permanent_ids(tree);
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
            if !ids.contains(g.as_str()) && !tree.tombstones.iter().any(|t| t == g) {
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
            if let Some(fam) = fm.fields.get("defines").and_then(Value::as_str) {
                if let Some(prefix) = fam.strip_suffix('*') {
                    families.push((prefix.to_string(), page.text.clone()));
                }
            }
        }
    }
    ResolutionIndex {
        ids: permanent_ids(tree).into_iter().map(str::to_string).collect(),
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
    while let Some(pos) = rest.find("doc: ") {
        let before_ok = pos == 0
            || rest
                .get(..pos)
                .and_then(|s| s.chars().last())
                .is_none_or(|c| !c.is_ascii_alphanumeric());
        let after = rest.get(pos + 5..).unwrap_or("");
        let raw = after.split_whitespace().next().unwrap_or("");
        let token = raw.trim_end_matches(DOC_TOKEN_PUNCT);
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

pub fn resolve_doc_token(idx: &ResolutionIndex, token: &str) -> Result<(), DocRefFail> {
    if token.starts_with('@') {
        return Err(DocRefFail::Foreign);
    }
    if !is_local_id(token) {
        return Err(DocRefFail::BadGrammar);
    }
    let family_hit = idx
        .families
        .iter()
        .any(|(prefix, body)| token.starts_with(prefix.as_str()) && occurs_as_token(body, token));
    if idx.ids.contains(token) || family_hit || idx.tombstones.iter().any(|t| t == token) {
        Ok(())
    } else {
        Err(DocRefFail::Dangling)
    }
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
fn wiki_links(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (i, line) in scannable_lines(text) {
        let mut rest = line;
        while let Some(start) = rest.find("[[") {
            let Some(after) = rest.get(start + 2..) else { break };
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

fn check_links(tree: &DocTree, r: &mut Report) {
    // Resolution set: relative page paths without extension (R-070 full paths).
    let paths: BTreeSet<String> = tree
        .pages
        .iter()
        .map(|p| p.rel.trim_end_matches(".md").to_string())
        .collect();
    let mut inspected = 0usize;
    for page in &tree.pages {
        for (line, target) in wiki_links(&page.text) {
            inspected += 1;
            if !target.contains('/') && page.kind != Kind::Router {
                // Short-name links are invalid (R-070) — except the router,
                // where entries like [[reference/x|name]] always carry a path
                // anyway, so this branch simply never fires there.
                r.findings.push(Finding::warn(
                    R070,
                    &page.rel,
                    &target,
                    format!("line {}: short-name link — full path from the docs root required",
                        line + 1),
                ));
                continue;
            }
            let archived = tree
                .root
                .join("_archive")
                .join(format!("{target}.md"))
                .exists();
            if paths.contains(&target) {
                // resolves
            } else if archived {
                r.findings.push(Finding::warn(
                    R071,
                    &page.rel,
                    &target,
                    "target moved to _archive/ — still resolves, reported as remaining interest"
                        .to_string(),
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
                match resolve_doc_token(&idx, &token) {
                    Ok(()) => {}
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
                    Err(DocRefFail::Dangling) => r.findings.push(Finding::err(
                        R076,
                        &page.rel,
                        &token,
                        format!(
                            "line {}: `doc: {token}` resolves to no id, alias, or family member",
                            line + 1
                        ),
                    )),
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
        let after_ok = body
            .get(abs + token.len()..)
            .and_then(|s| s.chars().next())
            .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-');
        if before_ok && after_ok {
            return true;
        }
        start = abs + token.len().max(1);
    }
    false
}

fn check_paths(tree: &DocTree, r: &mut Report) {
    let mut inspected = 0usize;
    for page in &tree.pages {
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
                    format!("line {}: absolute filesystem path outside quoted material", line + 1),
                ));
            }
            // A `..` segment is fine while it stays inside the tree; only a
            // resolution that pops past the root escapes (first migration
            // pilot: `../howto/x.md` from `reference/` was falsely flagged).
            let mut escapes = false;
            for target in crate::migrate::md_link_targets_line(text_line) {
                if target.starts_with("..")
                    && crate::migrate::resolve(&page.rel, &target).is_none()
                {
                    escapes = true;
                }
            }
            if text_line.contains("[[../") {
                escapes = true; // wiki-links are root-relative; `..` never valid
            }
            if escapes {
                r.findings.push(Finding::err(
                    R075,
                    &page.rel,
                    "escaping-link",
                    format!("line {}: relative link traverses outside the tree", line + 1),
                ));
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
    let Some(router) = tree.pages.iter().find(|p| p.kind == Kind::Router) else {
        r.findings.push(Finding::warn(
            R034,
            "-",
            "index.md",
            "permanent pages exist but there is no root router".to_string(),
        ));
        return;
    };
    // R-035: router entry grammar. Only lines that carry a wiki-link are
    // entries (D-014); plain bullets are prose, and whether prose belongs on
    // a router is judgment, not grammar.
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
    // R-034: reachability = wiki-link edges from the router, transitively
    // through any non-archived page (registered decision D-009).
    let mut reachable: BTreeSet<String> = BTreeSet::new();
    let mut queue: Vec<String> = wiki_links(&router.text)
        .into_iter()
        .map(|(_, t)| t)
        .collect();
    while let Some(t) = queue.pop() {
        if !reachable.insert(t.clone()) {
            continue;
        }
        if let Some(page) = tree.pages.iter().find(|p| p.rel.trim_end_matches(".md") == t) {
            queue.extend(wiki_links(&page.text).into_iter().map(|(_, x)| x));
        }
    }
    for page in &tree.pages {
        if page.kind != Kind::Permanent {
            continue;
        }
        if !reachable.contains(page.rel.trim_end_matches(".md")) {
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
        let mut entry_start: Option<(usize, usize)> = None; // (line, body_len)
        let flush = |r: &mut Report, e: Option<(usize, usize)>| {
            if let Some((line, body)) = e {
                if body > 5 {
                    r.findings.push(Finding::warn(
                        R101,
                        &page.rel,
                        &format!("entry-{}", line + 1),
                        format!("entry body is {body} lines (max 5)"),
                    ));
                }
            }
        };
        for (i, line) in page.text.lines().enumerate() {
            if let Some(rest) = line.strip_prefix("## ") {
                inspected += 1;
                flush(r, entry_start.take());
                let date = rest.get(..10).unwrap_or("");
                let sep_ok = rest
                    .get(10..)
                    .is_some_and(|s| {
                        let s = s.trim_start();
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
                let base = norm.contains(" -- deferred: ") && norm.contains(" -- repay when: ");
                if closed {
                    base && norm.contains(" -- resolved: ")
                } else {
                    base
                }
            } else {
                let after = norm.get(6..).unwrap_or("");
                let date_ok = is_iso_date(after.get(..10).unwrap_or(""));
                if closed {
                    date_ok && norm.contains(" -- answered: ")
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

fn check_templates(tree: &DocTree, r: &mut Report) {
    const SECTIONS: [(&str, [&str; 4]); 3] = [
        ("features", ["Context", "Decision", "Contract surface", "Rejected alternatives"]),
        ("postmortems", ["What happened", "Root cause", "Recurrence", "Lesson"]),
        ("research", ["Question", "Tried", "Learned", "Why no decision"]),
    ];
    let mut inspected = 0usize;
    for page in &tree.pages {
        if page.kind != Kind::Tracked {
            continue;
        }
        let Some(category) = page.rel.strip_prefix("work/").and_then(|s| s.split('/').next())
        else {
            continue;
        };
        let Some((_, required)) = SECTIONS.iter().find(|(c, _)| *c == category) else {
            continue; // R-042 categories have no template
        };
        inspected += 1;
        let missing: Vec<&str> = required
            .iter()
            .filter(|s| {
                let heading = format!("## {s}");
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
                Kind::Permanent | Kind::Tracked | Kind::ListFile | Kind::Router
            )
        });
        if !in_layout {
            let n = tree.pages.len();
            r.findings.push(Finding::warn(
                RuleId("R-020"),
                "-",
                "layout",
                format!(
                    "{n} markdown file(s), none inside the project layout — \
                     a brownfield tree; migration, not linting, is the next step"
                ),
            ));
        }
    }
    r.findings.sort();
    r.findings.dedup();
    r
}
