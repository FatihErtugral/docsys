//! `docsys upgrade` — a tree moves to the next spec version in one commit
//! (D-117). The default is the plan (R-176); `--apply` writes it; the steps
//! are declared as data, each with its strategy (R-173, R-174), and what needs
//! judgment is listed, never applied (R-175). A second run changes nothing.
//!
//! A docsys/0.4 tree keeps working under this binary exactly as it did
//! (D-118); the upgrade is how a repository opts into 0.5, when it is ready.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::era::Era;
use crate::fm::Value;
use crate::tree::{DocTree, Kind, Profile};

/// The steps of the move, as data (R-173): `step <TAB> strategy <TAB> scope
/// <TAB> what`. Each step id is one block of `run` below.
pub const STEPS: &str = include_str!("../migrations/0.4-0.5.tsv");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// `auto` (applied), `manual` (listed for a person), `info`
    pub strategy: &'static str,
    pub step: &'static str,
    pub file: String,
    pub what: String,
}

#[derive(Debug, Default)]
pub struct Upgrade {
    pub from: u32,
    pub to: u32,
    pub items: Vec<Item>,
    /// how the findings change when the tree is judged by the next version,
    /// before the automatic steps
    pub preview: Vec<String>,
    /// diffs for files that are a person's to change
    pub diffs: Vec<(String, String)>,
    /// repository-relative paths the automatic steps wrote and the commit
    /// carries
    pub written: Vec<String>,
}

impl Upgrade {
    fn item(&mut self, strategy: &'static str, step: &'static str, file: &str, what: String) {
        self.items.push(Item {
            strategy,
            step,
            file: file.to_string(),
            what,
        });
    }
}

/// The spec minor this binary implements.
pub fn implemented() -> u32 {
    crate::rules::spec_version()
        .strip_prefix("0.")
        .and_then(|m| m.parse().ok())
        .unwrap_or(4)
}

fn git_out(repo: &Path, args: &[&str]) -> Option<String> {
    crate::git::cmd(repo)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
}

fn rel(repo: &Path, path: &Path) -> String {
    let repo_c = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let p = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    p.strip_prefix(&repo_c)
        .map(|r| r.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"))
}

/// A 0.15 relay: comments, the `command -v` guard, and one `exec docsys hook`
/// line — the shape every docsys template had before the 0.16 guard.
fn relay_shape(text: &str) -> bool {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    matches!(
        lines.as_slice(),
        [guard, exec] if *guard == "command -v docsys >/dev/null || exit 0"
            && exec.starts_with("exec docsys hook ")
            && exec.contains("--root \"${DOCS_ROOT:-")
    )
}

/// The frontmatter with the 0.5 hashes added right after `verified_rev:`.
fn add_hashes(text: &str, hash: &str, sources: &[(String, String)]) -> Option<String> {
    let mut out = Vec::new();
    let mut done = false;
    let mut in_fm = false;
    for (i, line) in text.lines().enumerate() {
        out.push(line.to_string());
        if i == 0 && line == "---" {
            in_fm = true;
            continue;
        }
        if in_fm && line == "---" {
            in_fm = false;
        }
        if in_fm && !done && line.starts_with("verified_rev:") {
            out.push(format!("verified_hash: \"{hash}\""));
            if !sources.is_empty() {
                out.push("verified_sources:".to_string());
                for (s, h) in sources {
                    out.push(format!("  - source: \"{s}\""));
                    out.push(format!("    hash: \"{h}\""));
                }
            }
            done = true;
        }
    }
    let mut s = out.join("\n");
    if text.ends_with('\n') {
        s.push('\n');
    }
    done.then_some(s)
}

/// The plan, or with `apply` the move itself. `claude` is the agent layer's
/// directory.
pub fn run(repo: &Path, root: &Path, claude: &Path, apply: bool) -> Result<Upgrade, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    let kb = tree.profile == Profile::KnowledgeBase;
    let mut u = Upgrade {
        from: Era::of(&tree).0,
        to: implemented(),
        ..Upgrade::default()
    };
    if u.from > u.to {
        return Err(format!(
            "this tree declares docsys/0.{}; this docsys implements docsys/0.{} — install a newer docsys",
            u.from, u.to
        ));
    }
    let moving = u.from < u.to;
    let root_rel = match crate::fresh::root_rel(repo, root) {
        r if r.is_empty() => ".".to_string(),
        r => r,
    };
    let prefix = if root_rel == "." {
        String::new()
    } else {
        format!("{root_rel}/")
    };
    let preamble = crate::migrate::generated_preamble(root);

    // what the move changes in the findings, before the automatic steps
    if moving {
        let key = |r: &crate::checks::Report| -> BTreeSet<String> {
            r.findings
                .iter()
                // the message too: two findings on one line under one rule are
                // two findings
                .map(|f| {
                    format!(
                        "{} {} {} [{}] {}",
                        f.severity.tag(),
                        f.rule,
                        f.file,
                        f.subject,
                        f.message
                    )
                })
                .collect()
        };
        let now = key(&crate::lint_in(root, Some(repo)).0);
        let next = key(&crate::era::preview(u.to, || crate::lint_in(root, Some(repo))).0);
        let added: Vec<&String> = next.difference(&now).collect();
        let removed: Vec<&String> = now.difference(&next).collect();
        u.preview.push(format!(
            "judged by docsys/0.{} as the tree is now: {} new finding(s), {} gone — the ledger and pin steps clear their part",
            u.to,
            added.len(),
            removed.len()
        ));
        u.preview
            .extend(added.iter().take(25).map(|f| format!("+ {f}")));
        if added.len() > 25 {
            u.preview.push(format!("+ … {} more", added.len() - 25));
        }
        u.preview
            .extend(removed.iter().take(25).map(|f| format!("- {f}")));
        if removed.len() > 25 {
            u.preview.push(format!("- … {} more", removed.len() - 25));
        }
    }

    // hook-wires: settings.json names each relay in the one form (D-099)
    let settings = claude.join("settings.json");
    if let Ok(text) = fs::read_to_string(&settings) {
        let file = rel(repo, &settings);
        match crate::hook::parse_json(&text) {
            Some(mut doc) => {
                if let Some(n) = crate::agents::canonicalize_wires(&mut doc).filter(|n| *n > 0) {
                    u.item(
                        "auto",
                        "hook-wires",
                        &file,
                        format!("{n} change(s): every docsys wire to `\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/<name>.sh`, duplicates folded"),
                    );
                    if apply {
                        fs::write(&settings, doc.render()).map_err(|e| e.to_string())?;
                        u.written.push(file);
                    }
                }
            }
            None => u.item(
                "manual",
                "hook-wires",
                &file,
                "not valid JSON — wire the relays by hand (`docsys agents` prints the snippet)"
                    .to_string(),
            ),
        }
    }

    // hook-scripts: a relay still in a docsys template's shape is refreshed
    for hook in crate::agents::HOOK_FILES {
        let path = claude.join(hook);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Some(fresh) = crate::agents::relay_for(hook, &root_rel) else {
            continue;
        };
        if text == fresh {
            continue;
        }
        let file = rel(repo, &path);
        if relay_shape(&text) {
            u.item(
                "auto",
                "hook-scripts",
                &file,
                "refreshed: starts in the project directory, names the tree's root, says when the tree needs a newer docsys".to_string(),
            );
            if apply {
                fs::write(&path, &fresh).map_err(|e| e.to_string())?;
                u.written.push(file);
            }
        } else {
            u.item(
                "manual",
                "hook-scripts",
                &file,
                "edited by its owner — the diff to this version's relay is below".to_string(),
            );
            u.diffs.push((
                file.clone(),
                crate::diff::unified(&text, &fresh, &file, &file, 3),
            ));
        }
    }

    // git-gate: the block in this clone's pre-commit hook, its mode kept
    match crate::adopt::gate_current(repo, &root_rel) {
        None => u.item(
            "info",
            "git-gate",
            "pre-commit",
            "no docsys gate in this clone — `docsys adopt` writes it".to_string(),
        ),
        Some(true) => {}
        Some(false) => {
            let hooks = crate::git::hooks_dir(repo).unwrap_or_else(|| repo.join(".git/hooks"));
            let file = rel(repo, &hooks.join("pre-commit"));
            let tracked = git_out(repo, &["ls-files", "--error-unmatch", "--", &file]).is_some();
            u.item(
                "auto",
                "git-gate",
                &file,
                format!(
                    "the docsys block rewritten for this version, its mode kept{}",
                    if tracked {
                        ""
                    } else {
                        " — this clone only: every clone runs `docsys upgrade --apply` once"
                    }
                ),
            );
            if apply {
                let done = crate::adopt::ensure_git_gate(repo, &root_rel, false);
                if done == "failed" {
                    return Err(format!("{file}: the gate block could not be written"));
                }
                if tracked {
                    u.written.push(file);
                }
            }
        }
    }

    // verified-hash: a record proven by history gains its own evidence (D-101)
    if moving {
        for page in tree.pages.iter().filter(|p| p.kind == Kind::Permanent) {
            let Some(fm) = &page.fm else { continue };
            let get = |k: &str| fm.fields.get(k).and_then(Value::as_str);
            if get("verification") != Some("verified") || fm.fields.contains_key("verified_hash") {
                continue;
            }
            let file = format!("{prefix}{}", page.rel);
            let Some(rev) = get("verified_rev").map(str::trim) else {
                u.item(
                    "manual",
                    "verified-hash",
                    &file,
                    "verified without a revision — a maintainer verifies it again: `docsys verify <page>`".to_string(),
                );
                continue;
            };
            let Some(then) = git_out(repo, &["show", &format!("{rev}:{file}")]) else {
                u.item(
                    "manual",
                    "verified-hash",
                    &file,
                    format!("`verified_rev: {rev}` does not hold the page in this history — never hashed blind; a maintainer verifies it again"),
                );
                continue;
            };
            let now_hash = crate::fresh::content_hash(&crate::fresh::body_text(&page.text));
            if crate::fresh::content_hash(&crate::fresh::body_text(&then)) != now_hash {
                u.item(
                    "manual",
                    "verified-hash",
                    &file,
                    format!("the body changed since {rev} — never hashed blind; a maintainer verifies it again"),
                );
                continue;
            }
            let mut sources = Vec::new();
            let mut moved = None;
            for s in crate::fresh::consumed_sources(fm) {
                let Some((ns, id)) = s.trim_start_matches('@').split_once('/') else {
                    continue;
                };
                let now = crate::fresh::source_hash(root, &s);
                let then = git_out(
                    repo,
                    &[
                        "show",
                        &format!("{rev}:{prefix}.federation/{ns}/{id}.provenance.yml"),
                    ],
                )
                .and_then(|t| crate::fresh::sidecar_field(&t, "hash"));
                match (now, then) {
                    (Some(n), Some(t)) if n == t => sources.push((s.clone(), n)),
                    _ => moved = Some(s.clone()),
                }
            }
            if let Some(s) = moved {
                u.item(
                    "manual",
                    "verified-hash",
                    &file,
                    format!("`{s}` moved since {rev}, or had no committed provenance — a maintainer verifies it again"),
                );
                continue;
            }
            u.item(
                "auto",
                "verified-hash",
                &file,
                format!(
                    "the body {rev} holds is the body now: `verified_hash` recorded{}",
                    if sources.is_empty() {
                        String::new()
                    } else {
                        format!(", with {} source hash(es)", sources.len())
                    }
                ),
            );
            if apply {
                let path = root.join(&page.rel);
                let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
                if let Some(new) = add_hashes(&text, &now_hash, &sources) {
                    fs::write(&path, new).map_err(|e| e.to_string())?;
                    u.written.push(file);
                }
            }
        }
    }

    // pins: a pin's hash becomes its acknowledgement where it still holds and
    // the declaration reads the same region (D-119); everything else is listed
    if moving {
        for c in crate::fresh::convert_legacy(root, repo, apply)? {
            let file = format!("{prefix}{}", c.page);
            // with --apply every converted page loses its pin hashes, whatever
            // the outcome: the commit carries each one
            if apply && !u.written.contains(&file) {
                u.written.push(file.clone());
            }
            match c.outcome {
                crate::fresh::Conversion::Acknowledged => {
                    u.item("auto", "pins", &file, format!("`{}`: acknowledged beside the page", c.pin));
                }
                crate::fresh::Conversion::Reresolved { first, last } => u.item(
                    "manual",
                    "pins",
                    &file,
                    format!("`{}`: its declaration is L{first}-L{last}, not the region 0.15 read — re-read the page against it, then `docsys pin --refresh {}`", c.pin, c.page),
                ),
                crate::fresh::Conversion::Unresolvable(why) => u.item(
                    "manual",
                    "pins",
                    &file,
                    format!("`{}`: {why} — pin a narrower symbol or the file", c.pin),
                ),
                crate::fresh::Conversion::StaleAsRecorded => u.item(
                    "manual",
                    "pins",
                    &file,
                    format!("`{}`: stale before the upgrade, still stale — re-read, then `docsys pin --refresh {}`", c.pin, c.page),
                ),
            }
        }
        if apply && root.join(crate::ack::DIR).is_dir() {
            u.written.push(format!("{prefix}{}", crate::ack::DIR));
        }
    }

    // ledger-separators: the em-dash markers to ASCII (D-108)
    let ledger = crate::capture::ledger_fix_with(root, apply)?;
    for line in ledger.lines().filter(|l| l.starts_with("fixed: ")) {
        let file_rel = line
            .trim_start_matches("fixed: ")
            .split(" line ")
            .next()
            .unwrap_or("");
        let file = format!("{prefix}{file_rel}");
        u.item(
            "auto",
            "ledger-separators",
            &file,
            line.trim_start_matches("fixed: ").to_string(),
        );
        if apply {
            u.written.push(file);
        }
    }

    // ci-workflow: regenerated when untouched, a diff when it is the owner's
    match crate::workflow::classify(repo, &root_rel) {
        None => {}
        Some(crate::workflow::Existing::Untouched { from, params }) => {
            let path = repo.join(crate::workflow::PATH);
            let fresh = crate::workflow::render(&params);
            let current = fs::read_to_string(&path).unwrap_or_default();
            if fresh != current {
                if matches!(params.ci.install, crate::workflow::Install::Release(_)) {
                    u.item("manual", "ci-workflow", crate::workflow::PATH, format!("a release install pins the archives of {from}: write this version's sha256 values by hand — the diff is below"));
                    u.diffs.push((
                        crate::workflow::PATH.to_string(),
                        crate::diff::unified(
                            &current,
                            &fresh,
                            crate::workflow::PATH,
                            crate::workflow::PATH,
                            3,
                        ),
                    ));
                } else {
                    u.item("auto", "ci-workflow", crate::workflow::PATH, format!("regenerated from {from} with its own parameters, pinned to this version"));
                    if apply {
                        fs::write(&path, fresh).map_err(|e| e.to_string())?;
                        u.written.push(crate::workflow::PATH.to_string());
                    }
                }
            }
        }
        Some(crate::workflow::Existing::Legacy { from, params }) => {
            u.item("auto", "ci-workflow", crate::workflow::PATH, format!("the {from} workflow, untouched: regenerated with its mode, pinned to this version"));
            if apply {
                fs::write(
                    repo.join(crate::workflow::PATH),
                    crate::workflow::render(&params),
                )
                .map_err(|e| e.to_string())?;
                u.written.push(crate::workflow::PATH.to_string());
            }
        }
        Some(crate::workflow::Existing::Owned { diff }) if !diff.trim().is_empty() => {
            u.item("manual", "ci-workflow", crate::workflow::PATH, "the workflow is yours: never rewritten — apply what you want of the diff below, the version pin first".to_string());
            u.diffs.push((crate::workflow::PATH.to_string(), diff));
        }
        Some(crate::workflow::Existing::Owned { .. }) => {}
    }

    // rules-block: refreshed where its markers are (D-110, D-114)
    if !kb {
        match crate::adopt::rules_target(repo, None) {
            Some(target) => {
                let file = rel(repo, &target);
                let text = fs::read_to_string(&target).unwrap_or_default();
                let want = crate::rules::agents_block_with(&preamble);
                let held = match (
                    text.find(crate::rules::BLOCK_BEGIN),
                    text.find(crate::rules::BLOCK_END),
                ) {
                    (Some(a), Some(b)) if b > a => text.get(a..b + crate::rules::BLOCK_END.len()),
                    _ => None,
                };
                match held {
                    Some(block) if want.trim_end() == block.trim_end() => {}
                    Some(_) => {
                        u.item(
                            "auto",
                            "rules-block",
                            &file,
                            "the generated block refreshed in place".to_string(),
                        );
                        if apply {
                            crate::rules::write_agents_block_with(&target, &preamble)?;
                            u.written.push(file);
                        }
                    }
                    None => u.item(
                        "info",
                        "rules-block",
                        &file,
                        "no docsys rules block — `docsys adopt` writes it".to_string(),
                    ),
                }
            }
            None => u.item(
                "info",
                "rules-block",
                "-",
                "no file to hold the rules block — `docsys adopt` names one".to_string(),
            ),
        }
    }

    // assets: the skills and commands docsys owns, unless their owner edited them
    for (asset, now, legacy) in crate::agents::owned_assets(kb) {
        let path = claude.join(asset);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let want = if kb {
            now.to_string()
        } else {
            crate::migrate::with_preamble(now, &preamble)
        };
        if text == want {
            continue;
        }
        let file = rel(repo, &path);
        let was = if kb {
            legacy.to_string()
        } else {
            crate::migrate::with_preamble(legacy, &preamble)
        };
        if text == was || text == legacy {
            u.item(
                "auto",
                "assets",
                &file,
                "refreshed: the 0.15 text, untouched".to_string(),
            );
            if apply {
                fs::write(&path, &want).map_err(|e| e.to_string())?;
                u.written.push(file);
            }
        } else {
            u.item(
                "manual",
                "assets",
                &file,
                "edited by its owner — the diff to this version's text is below".to_string(),
            );
            u.diffs.push((
                file.clone(),
                crate::diff::unified(&text, &want, &file, &file, 3),
            ));
        }
    }

    // code-citations: a `doc:` 0.15 read mid-comment that 0.5 reads as prose
    if moving && !kb {
        let idx = crate::checks::build_index(&tree);
        let mut listed = 0usize;
        let mut more = 0usize;
        for f in crate::migrate::repo_text_files(repo, root) {
            let Ok(text) = fs::read_to_string(&f) else {
                continue;
            };
            let file = rel(repo, &f);
            if tree
                .docmeta_list("scan_exclude")
                .iter()
                .filter_map(|e| crate::tree::scan_prefix(e).ok())
                .any(|p| crate::tree::under_prefix(&file, &p))
            {
                continue;
            }
            for (i, line) in text.lines().enumerate() {
                let now: Vec<String> = crate::checks::code_doc_tokens_on_line(line);
                for tok in crate::checks::doc_tokens_on_line(line) {
                    if now.contains(&tok) || crate::checks::resolve_doc_token(&idx, &tok).is_err() {
                        continue;
                    }
                    if listed < 50 {
                        u.item("manual", "code-citations", &format!("{file}:{}", i + 1), format!("`doc: {tok}` sits mid-comment: from 0.5 it is prose (R-072) — move it to the start of its comment, or leave it as prose"));
                        listed += 1;
                    } else {
                        more += 1;
                    }
                }
            }
        }
        if more > 0 {
            u.item(
                "manual",
                "code-citations",
                "-",
                format!("… and {more} more of the same"),
            );
        }
    }

    // raw-records: what a project's records become (D-112)
    if moving && !kb && root.join("raw").is_dir() {
        let n = tree
            .pages
            .iter()
            .filter(|p| p.rel.starts_with("raw/"))
            .count();
        u.item("info", "raw-records", &format!("{prefix}raw/"), format!("{n} record(s): from 0.5 a record layer — their dangling references are reported, an edited record stops the commit (R-023)"));
    }

    // spec-line: last, so a failed step leaves the tree at its version
    if moving {
        let path = root.join(".docmeta.yml");
        let file = rel(repo, &path);
        u.item(
            "auto",
            "spec-line",
            &file,
            format!("`spec: docsys/0.{}` → `spec: docsys/0.{}`", u.from, u.to),
        );
        if apply {
            let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let new: Vec<String> = text
                .lines()
                .map(|l| {
                    if l.trim_start().starts_with("spec:") {
                        format!("spec: docsys/0.{}", u.to)
                    } else {
                        l.to_string()
                    }
                })
                .collect();
            let mut out = new.join("\n");
            if text.ends_with('\n') {
                out.push('\n');
            }
            fs::write(&path, out).map_err(|e| e.to_string())?;
            u.written.push(file);
        }
    }
    u.written.sort();
    u.written.dedup();
    Ok(u)
}

/// Commit what the upgrade wrote as one commit (R-177), under the current
/// identity.
pub fn commit(repo: &Path, u: &Upgrade) -> Result<(), String> {
    if u.written.is_empty() {
        return Ok(());
    }
    let added = crate::git::cmd(repo)
        .args(["add", "--"])
        .args(&u.written)
        .status()
        .is_ok_and(|s| s.success());
    if !added {
        return Err("git add failed".into());
    }
    let ok = crate::git::cmd(repo)
        .args(["commit", "-q", "-m"])
        .arg(format!("docsys: upgrade the tree to docsys/0.{}", u.to))
        .status()
        .is_ok_and(|s| s.success());
    if ok {
        Ok(())
    } else {
        Err("the upgrade is written but the commit did not land — commit it yourself (the gate may have refused; `docsys lint` says why)".into())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_0_15_relay_has_the_template_shape() {
        let old = "#!/usr/bin/env bash\n# docsys-template: 0.15.1\n# a comment\ncommand -v docsys >/dev/null || exit 0\nexec docsys hook stop --root \"${DOCS_ROOT:-docs}\" --stdin\n";
        assert!(relay_shape(old));
        assert!(!relay_shape(
            &old.replace("exec docsys", "my-check && exec docsys")
        ));
        assert!(!relay_shape("#!/bin/sh\nexit 0\n"));
    }

    #[test]
    fn the_hashes_go_right_after_the_revision() {
        let text = "---\nid: a\nverification: verified\nverified_by: ayse\nverified_rev: abc\nsources: []\n---\nBody.\n";
        let out = add_hashes(text, "sha256:x", &[("@up/x".into(), "fnv:1".into())]).unwrap();
        assert!(out.contains("verified_rev: abc\nverified_hash: \"sha256:x\"\nverified_sources:\n  - source: \"@up/x\"\n    hash: \"fnv:1\"\nsources: []\n"), "{out}");
        assert!(add_hashes("---\nid: a\n---\nBody.\n", "h", &[]).is_none());
    }

    #[test]
    fn the_steps_are_data_with_a_strategy_each() {
        let rows: Vec<&str> = STEPS
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .collect();
        assert!(rows.len() >= 10);
        for r in rows {
            let cells: Vec<&str> = r.split('\t').collect();
            assert_eq!(cells.len(), 4, "{r}");
            assert!(
                cells
                    .get(1)
                    .is_some_and(|s| ["auto", "manual", "auto/manual", "info"].contains(s)),
                "{r}"
            );
        }
    }
}
