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
/// <TAB> what`. Each step id is one block below.
pub const STEPS: &str = include_str!("../migrations/0.4-0.5.tsv");

/// Where a release's upgrade note lives: its CHANGELOG section's
/// `### Upgrading` part, embedded so `upgrade` prints the note it ships with.
const CHANGELOG: &str = include_str!("../CHANGELOG.md");

/// One spec version's move.
pub struct Migration {
    pub from: u32,
    pub to: u32,
    /// the release that brings `to`; its CHANGELOG `### Upgrading` part is
    /// the note
    pub release: &'static str,
    /// the steps as data (R-173)
    pub steps: &'static str,
    pub apply: fn(&Ctx, &mut Upgrade, bool) -> Result<(), String>,
}

/// Every move this binary knows, in order.
pub const MIGRATIONS: [Migration; 1] = [Migration {
    from: 4,
    to: 5,
    release: "0.16.0",
    steps: STEPS,
    apply: move_0_4_to_0_5,
}];

/// What a step works on.
pub struct Ctx<'a> {
    pub repo: &'a Path,
    pub root: &'a Path,
    pub claude: &'a Path,
    pub tree: DocTree,
    pub kb: bool,
    pub root_rel: String,
    pub prefix: String,
    pub preamble: String,
}

/// A release's upgrade note: the lines under `### Upgrading` in its
/// CHANGELOG section, verbatim.
pub fn note(release: &str) -> Option<String> {
    let head = format!("## [{release}]");
    let section = CHANGELOG
        .lines()
        .skip_while(|l| !l.starts_with(&head))
        .skip(1)
        .take_while(|l| !l.starts_with("## "));
    let lines: Vec<&str> = section
        .skip_while(|l| !l.starts_with("### Upgrading"))
        .skip(1)
        .take_while(|l| !l.starts_with("### "))
        .collect();
    let text = lines.join("\n").trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// `auto` (applied), `manual` (listed for a person), `info`
    pub strategy: &'static str,
    pub step: &'static str,
    pub file: String,
    pub what: String,
    /// the command that completes the item, when one does
    pub command: Option<String>,
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
    /// the last move: the pin moves with it
    pub last: bool,
    /// the upgrade notes of the releases this move crosses
    pub notes: Vec<(String, String)>,
}

impl Upgrade {
    fn item(&mut self, strategy: &'static str, step: &'static str, file: &str, what: String) {
        self.items.push(Item {
            strategy,
            step,
            file: file.to_string(),
            what,
            command: None,
        });
    }

    /// The command that completes the item just listed.
    fn completed_by(&mut self, command: String) {
        if let Some(last) = self.items.last_mut() {
            last.command = Some(command);
        }
    }
}

impl Upgrade {
    /// The plan as data, for an agent that completes it (D-104): what the
    /// text form prints, field by field.
    pub fn to_json(&self) -> String {
        use crate::hook::Json;
        let s = |v: &str| Json::Str(v.to_string());
        let items = self
            .items
            .iter()
            .map(|i| {
                Json::Obj(vec![
                    ("strategy".into(), s(i.strategy)),
                    ("step".into(), s(i.step)),
                    ("file".into(), s(&i.file)),
                    ("what".into(), s(&i.what)),
                    ("command".into(), i.command.as_deref().map_or(Json::Null, s)),
                ])
            })
            .collect();
        let notes = self
            .notes
            .iter()
            .map(|(release, text)| {
                Json::Obj(vec![
                    ("release".into(), s(release)),
                    ("text".into(), s(text)),
                ])
            })
            .collect();
        let diffs = self
            .diffs
            .iter()
            .map(|(file, diff)| Json::Obj(vec![("file".into(), s(file)), ("diff".into(), s(diff))]))
            .collect();
        Json::Obj(vec![
            ("from".into(), s(&format!("docsys/0.{}", self.from))),
            ("to".into(), s(&format!("docsys/0.{}", self.to))),
            ("last".into(), Json::Bool(self.last)),
            ("notes".into(), Json::Arr(notes)),
            ("items".into(), Json::Arr(items)),
            (
                "preview".into(),
                Json::Arr(self.preview.iter().map(|p| s(p)).collect()),
            ),
            ("diffs".into(), Json::Arr(diffs)),
            (
                "written".into(),
                Json::Arr(self.written.iter().map(|w| s(w)).collect()),
            ),
        ])
        .render()
    }
}

/// The docsys version an owner's workflow installs, where a line that names
/// docsys carries one (`DOCSYS_VERSION: v0.15.1`, `cargo install docsys
/// --version 0.15.1`).
fn installed_version(workflow: &str) -> Option<String> {
    workflow
        .lines()
        .filter(|l| l.to_ascii_lowercase().contains("docsys"))
        .flat_map(|l| {
            l.split(|c: char| !(c.is_ascii_digit() || c == '.'))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .find(|t| crate::dispatch::parse(t).is_some())
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

/// The frontmatter with the 0.5 record added right after `verified_rev:`, in
/// the order `verify` writes it: the body's blocks, then the sources.
fn add_record(text: &str, blocks: &[String], sources: &[(String, String)]) -> Option<String> {
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
            out.push(format!("verified_blocks: [{}]", blocks.join(", ")));
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

/// The next move of the tree: a plan, or with `apply` the move itself.
/// `claude` is the agent layer's directory.
pub fn run(repo: &Path, root: &Path, claude: &Path, apply: bool) -> Result<Upgrade, String> {
    run_with(&MIGRATIONS, implemented(), repo, root, claude, apply)
}

/// One move towards `target` through `migrations`, one spec version at a
/// time (R-177): the next migration's steps, the steps every run takes, and
/// with the last move the pin. Run it again for the move after.
pub fn run_with(
    migrations: &[Migration],
    target: u32,
    repo: &Path,
    root: &Path,
    claude: &Path,
    apply: bool,
) -> Result<Upgrade, String> {
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    if !tree.docmeta_present {
        return Err(format!("`{}` has no .docmeta.yml", root.display()));
    }
    let from = Era::of(&tree).0;
    if from > target {
        return Err(format!(
            "this tree declares docsys/0.{from}; this docsys implements docsys/0.{target} — install a newer docsys"
        ));
    }
    let pin = crate::dispatch::read(root)?;
    let own = crate::dispatch::own();
    if let Some(p) = pin.as_deref() {
        if crate::dispatch::parse(p) > crate::dispatch::parse(own) {
            return Err(format!(
                "this tree pins docsys {p}, newer than this docsys {own} — any other docsys command here runs {p}, and its own `docsys upgrade` moves the tree"
            ));
        }
    }
    let next = if from < target {
        Some(
            migrations
                .iter()
                .find(|m| m.from == from)
                .ok_or_else(|| format!("no migration declared from docsys/0.{from}"))?,
        )
    } else {
        None
    };
    let root_rel = match crate::fresh::root_rel(repo, root) {
        r if r.is_empty() => ".".to_string(),
        r => r,
    };
    let prefix = if root_rel == "." {
        String::new()
    } else {
        format!("{root_rel}/")
    };
    let ctx = Ctx {
        repo,
        root,
        claude,
        kb: tree.profile == Profile::KnowledgeBase,
        tree,
        preamble: crate::migrate::generated_preamble(root),
        root_rel,
        prefix,
    };
    let mut u = Upgrade {
        from,
        to: next.map_or(from, |m| m.to),
        last: next.is_none_or(|m| m.to == target),
        ..Upgrade::default()
    };
    if let Some(m) = next {
        preview(&ctx, &mut u);
        common(&ctx, &mut u, apply)?;
        (m.apply)(&ctx, &mut u, apply)?;
        spec_line(&ctx, &mut u, apply)?;
        if let Some(text) = note(m.release) {
            u.notes.push((m.release.to_string(), text));
        }
    } else {
        common(&ctx, &mut u, apply)?;
    }
    if u.last {
        pin_step(&ctx, &mut u, pin.as_deref(), apply)?;
    }
    u.written.sort();
    u.written.dedup();
    Ok(u)
}

/// What the move changes in the findings, before any step writes.
fn preview(ctx: &Ctx, u: &mut Upgrade) {
    let (repo, root) = (ctx.repo, ctx.root);
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
    // lint and refs alike: a move changes what both report
    let findings = || {
        let mut all = key(&crate::lint_in(root, Some(repo)).0);
        if let Ok(tree) = DocTree::load(root) {
            all.extend(key(&crate::refs::run(repo, &tree)));
        }
        all
    };
    let now = findings();
    let next = crate::era::preview(u.to, findings);
    let added: Vec<&String> = next.difference(&now).collect();
    let removed: Vec<&String> = now.difference(&next).collect();
    u.preview.push(format!(
            "judged by docsys/0.{} as the tree is now: {} new finding(s), {} gone — the steps below clear their part",
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

/// The `spec:` line, last of a move, so a failed step leaves the tree at its version.
fn spec_line(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    let (repo, root) = (ctx.repo, ctx.root);
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
    Ok(())
}

/// The pin, with the last move or alone: every docsys call here runs this
/// version from now on (D-120).
fn pin_step(ctx: &Ctx, u: &mut Upgrade, pin: Option<&str>, apply: bool) -> Result<(), String> {
    let own = crate::dispatch::own();
    if pin == Some(own) {
        return Ok(());
    }
    let path = ctx.root.join(crate::dispatch::FILE);
    let file = rel(ctx.repo, &path);
    u.item(
        "auto",
        "pin",
        &file,
        format!(
            "{} → `{own}`: every docsys call here runs it",
            pin.map_or("no pin".to_string(), |p| format!("`{p}`"))
        ),
    );
    if apply {
        crate::dispatch::write(ctx.root)?;
        u.written.push(file);
    }
    Ok(())
}

/// A file holding exactly the text this version writes is docsys's own: when
/// nobody committed it yet — `docsys agents` wrote it before the upgrade — the
/// upgrade commit carries it.
fn carry_untracked(repo: &Path, u: &mut Upgrade, step: &'static str, file: &str, apply: bool) {
    if git_out(repo, &["ls-files", "--error-unmatch", "--", file]).is_some() {
        return;
    }
    u.item(
        "auto",
        step,
        file,
        "committed: this version's text, not tracked yet".to_string(),
    );
    if apply {
        u.written.push(file.to_string());
    }
}

/// The steps every run takes: this binary's relays, gate, workflow, rules
/// block and assets, each regenerated only where nobody edited it.
fn common(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    let (repo, root, claude, kb) = (ctx.repo, ctx.root, ctx.claude, ctx.kb);
    let (root_rel, preamble) = (ctx.root_rel.as_str(), ctx.preamble.as_str());
    // hook-wires: settings.json names each relay in the one form (D-099)
    let settings = claude.join("settings.json");
    if let Ok(text) = fs::read_to_string(&settings) {
        let file = rel(repo, &settings);
        match crate::hook::parse_json(&text) {
            Some(mut doc) => {
                // a docsys/0.5 tree runs no post-edit relay (D-126)
                if Era(u.to).verification_from_history() {
                    let n = crate::agents::remove_relay_wires(&mut doc, crate::agents::POST_EDIT);
                    if n > 0 {
                        u.item(
                            "auto",
                            "hook-wires",
                            &file,
                            format!("{n} post-edit wire(s) taken out: a page's date and its verification are history's, and the relay has nothing left to do (D-126)"),
                        );
                        if apply {
                            fs::write(&settings, doc.render()).map_err(|e| e.to_string())?;
                            u.written.push(file.clone());
                        }
                    }
                }
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
            None => {
                u.item(
                    "manual",
                    "hook-wires",
                    &file,
                    "not valid JSON — wire the relays by hand; this prints the snippet".to_string(),
                );
                u.completed_by("docsys agents".to_string());
            }
        }
    }

    // hook-scripts: a relay still in a docsys template's shape is refreshed
    for hook in crate::agents::HOOK_FILES {
        let path = claude.join(hook);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Some(fresh) = crate::agents::relay_for(hook, root_rel) else {
            continue;
        };
        let file = rel(repo, &path);
        // a docsys/0.5 tree runs no post-edit relay: the one docsys wrote
        // goes, an edited one is its owner's to retire (D-126)
        if hook == crate::agents::POST_EDIT && Era(u.to).verification_from_history() {
            if text == fresh || crate::agents::released(hook, &text, "").is_some() {
                u.item(
                    "auto",
                    "hook-scripts",
                    &file,
                    "removed: a page's date and its verification are history's, and the relay has nothing left to do (D-126)".to_string(),
                );
                if apply {
                    let tracked =
                        git_out(repo, &["ls-files", "--error-unmatch", "--", &file]).is_some();
                    fs::remove_file(&path).map_err(|e| e.to_string())?;
                    if tracked {
                        u.written.push(file);
                    }
                }
            } else {
                u.item(
                    "manual",
                    "hook-scripts",
                    &file,
                    "edited by its owner, and with no job on docsys/0.5 (D-126) — move what you added elsewhere, then remove it".to_string(),
                );
            }
            continue;
        }
        if text == fresh {
            carry_untracked(repo, u, "hook-scripts", &file, apply);
            continue;
        }
        // a relay byte for byte as a release wrote it; any touch, a comment
        // included, makes it its owner's (D-117)
        if let Some(release) = crate::agents::released(hook, &text, "") {
            u.item(
                "auto",
                "hook-scripts",
                &file,
                format!("refreshed: a text docsys {release} wrote, untouched — it now starts in the project directory, names the tree's root, and names the pinned docsys to a binary from before pins"),
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
    let message = Era(u.to).journal_from_history();
    match crate::adopt::gate_current(repo, root_rel, message) {
        None => {
            u.item(
                "info",
                "git-gate",
                "pre-commit",
                "no docsys gate in this clone".to_string(),
            );
            u.completed_by("docsys adopt".to_string());
        }
        // the repository's gate is current, but this clone's git does not run it
        Some(true) if crate::adopt::tracked_hooks_unset(repo) => {
            u.item(
                "auto",
                "git-gate",
                ".git/config",
                "core.hooksPath = .githooks: the gate the repository tracks fires in this clone"
                    .to_string(),
            );
            if apply {
                let set = crate::git::cmd(repo)
                    .args(["config", "core.hooksPath", ".githooks"])
                    .status()
                    .is_ok_and(|s| s.success());
                if !set {
                    return Err("core.hooksPath could not be set".to_string());
                }
            }
        }
        Some(true) => {}
        Some(false) => {
            let hooks =
                crate::adopt::gate_hooks_dir(repo).unwrap_or_else(|| repo.join(".git/hooks"));
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
                let done = crate::adopt::ensure_git_gate(repo, root_rel, false, message);
                if done == "failed" {
                    return Err(format!("{file}: the gate block could not be written"));
                }
                if tracked {
                    u.written.push(file);
                    let msg = rel(repo, &hooks.join("commit-msg"));
                    if hooks.join("commit-msg").is_file() {
                        u.written.push(msg);
                    }
                }
            }
        }
    }

    // ci-workflow: regenerated when untouched, a diff when it is the owner's
    match crate::workflow::classify(repo, root_rel) {
        None => {}
        Some(crate::workflow::Existing::Untouched { from, mut params }) => {
            on_description(&mut params, u.to);
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
        Some(crate::workflow::Existing::Legacy { from, mut params }) => {
            on_description(&mut params, u.to);
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
            // the one line of an owner's workflow that must move with the tree:
            // the docsys it installs
            let text = fs::read_to_string(repo.join(crate::workflow::PATH)).unwrap_or_default();
            let own = crate::dispatch::own();
            let what = match installed_version(&text).filter(|v| v != own) {
                Some(v) => format!("the workflow is yours: never rewritten — it installs docsys {v} and the tree will pin {own}: CI installs the same version in this change (the diff below reads the pin)"),
                None => "the workflow is yours: never rewritten — apply what you want of the diff below, the version pin first".to_string(),
            };
            u.item("manual", "ci-workflow", crate::workflow::PATH, what);
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
                let want = crate::rules::agents_block_with(preamble);
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
                            crate::rules::write_agents_block_with(&target, preamble)?;
                            u.written.push(file);
                        }
                    }
                    None => {
                        u.item(
                            "info",
                            "rules-block",
                            &file,
                            "no docsys rules block".to_string(),
                        );
                        u.completed_by("docsys adopt".to_string());
                    }
                }
            }
            None => {
                u.item(
                    "info",
                    "rules-block",
                    "-",
                    "no file to hold the rules block".to_string(),
                );
                u.completed_by("docsys adopt".to_string());
            }
        }
    }

    // assets: the skills and commands docsys owns, unless their owner edited them
    for (asset, now, new) in crate::agents::owned_assets(kb) {
        let path = claude.join(asset);
        let want = if kb {
            now.to_string()
        } else {
            crate::migrate::with_preamble(&crate::agents::render_root(now, root_rel), preamble)
        };
        let file = rel(repo, &path);
        let Ok(text) = fs::read_to_string(&path) else {
            // new since 0.15: written where the agent layer is
            if new && claude.is_dir() {
                u.item(
                    "auto",
                    "assets",
                    &file,
                    "written: new in this version".to_string(),
                );
                if apply {
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    fs::write(&path, &want).map_err(|e| e.to_string())?;
                    u.written.push(file);
                }
            }
            continue;
        };
        if text == want {
            carry_untracked(repo, u, "assets", &file, apply);
            continue;
        }
        if let Some(release) = crate::agents::released(asset, &text, preamble) {
            u.item(
                "auto",
                "assets",
                &file,
                format!("refreshed: a text docsys {release} wrote, untouched"),
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
    // kb-contract: the knowledge base's contract, while it is a text a release wrote
    if kb {
        let path = root.join("AGENTS.md");
        if let Ok(text) = fs::read_to_string(&path) {
            let want = crate::agents::kb_contract();
            let file = rel(repo, &path);
            if text == want {
                carry_untracked(repo, u, "kb-contract", &file, apply);
            } else if let Some(release) = crate::agents::released("AGENTS.md", &text, "") {
                u.item(
                    "auto",
                    "kb-contract",
                    &file,
                    format!("refreshed: a text docsys {release} wrote, untouched"),
                );
                if apply {
                    fs::write(&path, &want).map_err(|e| e.to_string())?;
                    u.written.push(file);
                }
            } else {
                u.item(
                    "manual",
                    "kb-contract",
                    &file,
                    "the contract is its owner's — the diff to this version's text is below"
                        .to_string(),
                );
                u.diffs.push((
                    file.clone(),
                    crate::diff::unified(&text, &want, &file, &file, 3),
                ));
            }
        }
    }
    if Era(u.to).derived_dates() {
        dates(ctx, u, apply)?;
    }
    if Era(u.to).directory_routes() && !kb {
        routes(ctx, u, apply)?;
    }
    // a re-run on a moved tree absorbs what a branch from before the move
    // still wrote; the move itself runs this after the separators
    if Era::of(&ctx.tree).item_files() {
        ledgers(ctx, u, apply)?;
    }
    if Era::of(&ctx.tree).journal_from_history() {
        journal(ctx, u, apply)?;
    }
    if Era::of(&ctx.tree).verification_from_history() {
        records(ctx, u, apply)?;
    }
    Ok(())
}

/// The fields a verification record took in a page.
const RECORD_FIELDS: [&str; 5] = [
    "verification",
    "verified_by",
    "verified_rev",
    "verified_blocks",
    "verified_sources",
];

/// records: a docsys/0.5 page's verification is read from history (D-126).
/// A record that still holds — its blocks the body's, its sources as
/// recorded — stays valid evidence until the body moves; every other one
/// leaves the page. A page that took part in verification keeps `sources:`.
fn records(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    // the pages as the steps before left them
    let tree = DocTree::load(ctx.root).map_err(|e| e.to_string())?;
    for page in tree.pages.iter().filter(|p| p.kind == Kind::Permanent) {
        let Some(fm) = &page.fm else { continue };
        let file = format!("{}{}", ctx.prefix, page.rel);
        // in a plan, a record the verified-record step converts is one that holds
        let converted = u
            .items
            .iter()
            .any(|i| i.step == "verified-record" && i.strategy == "auto" && i.file == file);
        if !RECORD_FIELDS.iter().any(|k| fm.fields.contains_key(*k))
            || converted
            || crate::approval::holding_record(&tree, page)
        {
            continue;
        }
        u.item(
            "auto",
            "records",
            &file,
            "the verification record taken out: it no longer holds, and the state is read from history — an `Approved-by:` after the body's last change (D-126)".to_string(),
        );
        if !apply {
            continue;
        }
        let Some(mut text) = crate::fm::without_fields(&page.text, &RECORD_FIELDS) else {
            continue;
        };
        if !fm.fields.contains_key("sources") {
            if let Some(at) = text.find("\n---\n") {
                text.insert_str(at + 1, "sources: []\n");
            }
        }
        fs::write(ctx.root.join(&page.rel), text).map_err(|e| e.to_string())?;
        u.written.push(file);
    }
    Ok(())
}

/// journal: the journal is history from the move on (D-125). The journal
/// file and its slices move under `_archive/journal/` byte for byte, keeping
/// their names, so a branch that runs the same upgrade makes the same move;
/// every link to them gets its new target (R-172).
fn journal(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    let mut files: Vec<String> = Vec::new();
    if ctx.root.join("work/journal.md").is_file() {
        files.push("work/journal.md".to_string());
    }
    if let Ok(entries) = fs::read_dir(ctx.root.join("work/journal")) {
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|n| n.ends_with(".md"))
            .collect();
        names.sort();
        files.extend(names.into_iter().map(|n| format!("work/journal/{n}")));
    }
    let mut moves: Vec<(String, String)> = Vec::new();
    for rel in files {
        let name = rel.rsplit('/').next().unwrap_or("journal.md");
        let to = free_name(ctx.root, "_archive/journal", name, &moves);
        u.item(
            "auto",
            "journal",
            &format!("{}{rel}", ctx.prefix),
            format!(
                "→ {}{to}, as written; the journal is history from here on (D-125)",
                ctx.prefix
            ),
        );
        moves.push((rel, to));
    }
    if !apply || moves.is_empty() {
        return Ok(());
    }
    fs::create_dir_all(ctx.root.join("_archive/journal")).map_err(|e| e.to_string())?;
    for (from, to) in &moves {
        fs::rename(ctx.root.join(from), ctx.root.join(to)).map_err(|e| e.to_string())?;
        u.written.push(format!("{}{from}", ctx.prefix));
        u.written.push(format!("{}{to}", ctx.prefix));
    }
    let _ = fs::remove_dir(ctx.root.join("work/journal"));
    // the links that named the moved files name them where they are now
    for page in &ctx.tree.pages {
        let path = ctx.root.join(&page.rel);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let mut new = text.clone();
        for (from, to) in &moves {
            let (from, to) = (from.trim_end_matches(".md"), to.trim_end_matches(".md"));
            for end in ["]]", "|", "#"] {
                new = new.replace(&format!("[[{from}{end}"), &format!("[[{to}{end}"));
            }
        }
        if new != text {
            fs::write(&path, new).map_err(|e| e.to_string())?;
            u.written.push(format!("{}{}", ctx.prefix, page.rel));
        }
    }
    Ok(())
}

/// `<dir>/<name>` when nothing holds that name yet, else the name with a date
/// and a number beside it.
fn free_name(root: &Path, dir: &str, name: &str, taken: &[(String, String)]) -> String {
    let free = |rel: &str| !root.join(rel).exists() && !taken.iter().any(|(_, t)| t == rel);
    let first = format!("{dir}/{name}");
    if free(&first) {
        return first;
    }
    let stem = name.trim_end_matches(".md");
    let today = crate::migrate::today();
    let mut n = 1;
    loop {
        let rel = if n == 1 {
            format!("{dir}/{stem}-{today}.md")
        } else {
            format!("{dir}/{stem}-{today}-{n}.md")
        };
        if free(&rel) {
            return rel;
        }
        n += 1;
    }
}

/// ledgers: each open item of a docsys/0.4 ledger into its topic's file, its
/// line verbatim; what else the ledger held — closed items, prose — into a
/// frozen slice under `_archive/`, every line as written; the ledger goes
/// (D-124). An open item a branch from before the move appended to a slice
/// leaves it the same way.
fn ledgers(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    use crate::items::List;
    let lists: &[List] = if ctx.kb {
        &[List::Questions]
    } else {
        &[List::Debt, List::Questions]
    };
    for &list in lists {
        let ledger = list.ledger(ctx.kb);
        let mut sources = vec![ledger.to_string()];
        sources.extend(crate::checks::archive_slices(ctx.root, ledger));
        for rel in sources {
            let path = ctx.root.join(&rel);
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let is_ledger = rel == ledger;
            let open: Vec<&str> = text.lines().filter(|l| l.starts_with("- [ ] ")).collect();
            if open.is_empty() && !is_ledger {
                continue;
            }
            let rest: String = text
                .split_inclusive('\n')
                .filter(|l| !l.starts_with("- [ ] "))
                .collect();
            // a title and blank lines are no record; anything else is kept
            let kept = rest
                .lines()
                .any(|l| !l.trim().is_empty() && !l.starts_with("# "));
            let file = format!("{}{rel}", ctx.prefix);
            let slice = if is_ledger && kept {
                Some(free_slice(ctx.root, ledger))
            } else {
                None
            };
            let mut what = if open.is_empty() {
                "no open item".to_string()
            } else {
                format!("{} open item(s) into {}/", open.len(), list.dir(ctx.kb))
            };
            match &slice {
                Some(s) => what.push_str(&format!(", the rest frozen in {s}")),
                None if is_ledger => what.push_str("; the ledger goes"),
                None => {}
            }
            if !is_ledger {
                what.push_str(" — a branch from before the move appended them");
            }
            u.item("auto", "ledgers", &file, format!("{what} (D-124)"));
            if !apply {
                continue;
            }
            // an item already in its file — a merge that kept both sides —
            // is not written twice
            let have = crate::items::open(ctx.root, list, ctx.kb);
            for line in &open {
                if have.iter().any(|i| i.line == *line) {
                    continue;
                }
                let item = crate::items::add(ctx.root, list, ctx.kb, line)?;
                u.written.push(format!("{}{item}", ctx.prefix));
            }
            if let Some(s) = &slice {
                let target = ctx.root.join(s);
                if let Some(dir) = target.parent() {
                    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                }
                fs::write(&target, &rest).map_err(|e| e.to_string())?;
                u.written.push(format!("{}{s}", ctx.prefix));
            }
            if is_ledger {
                fs::remove_file(&path).map_err(|e| e.to_string())?;
            } else {
                fs::write(&path, &rest).map_err(|e| e.to_string())?;
            }
            u.written.push(file);
        }
        misfiled(ctx, u, list, apply)?;
    }
    Ok(())
}

/// A tagged item in another topic's file into its own, its line verbatim:
/// a ledger that became `general.md` reads to git as a rename, and a branch
/// from before the move that appended to the ledger lands there (D-124).
fn misfiled(
    ctx: &Ctx,
    u: &mut Upgrade,
    list: crate::items::List,
    apply: bool,
) -> Result<(), String> {
    use crate::items::{topic_of, GENERAL};
    let open = crate::items::open(ctx.root, list, ctx.kb);
    let mut by_file: Vec<(String, Vec<String>)> = Vec::new();
    for item in &open {
        let tag = topic_of(&item.line);
        if tag == GENERAL || tag == item.topic {
            continue;
        }
        match by_file.iter_mut().find(|(rel, _)| *rel == item.rel) {
            Some((_, lines)) => lines.push(item.line.clone()),
            None => by_file.push((item.rel.clone(), vec![item.line.clone()])),
        }
    }
    for (rel, lines) in by_file {
        let file = format!("{}{rel}", ctx.prefix);
        u.item(
            "auto",
            "ledgers",
            &file,
            format!(
                "{} item(s) tagged for another topic into their own files, lines verbatim — git's rename of the ledger carried a branch's line here (D-124)",
                lines.len()
            ),
        );
        if !apply {
            continue;
        }
        let path = ctx.root.join(&rel);
        let mut text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        for line in &lines {
            let mut removed = false;
            text = text
                .split_inclusive('\n')
                .filter(|l| {
                    if !removed && l.trim_end_matches('\n') == line {
                        removed = true;
                        return false;
                    }
                    true
                })
                .collect();
            let held = crate::items::open(ctx.root, list, ctx.kb)
                .iter()
                .any(|i| i.line == *line && i.topic == topic_of(line));
            if !held {
                let to = crate::items::add(ctx.root, list, ctx.kb, line)?;
                u.written.push(format!("{}{to}", ctx.prefix));
            }
        }
        if text.trim().is_empty() {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        } else {
            fs::write(&path, &text).map_err(|e| e.to_string())?;
        }
        u.written.push(file);
    }
    Ok(())
}

/// The archive slice a moved ledger's remainder goes to: `_archive/<ledger>`,
/// or a dated name beside it when a slice already holds that name.
fn free_slice(root: &Path, ledger: &str) -> String {
    let first = format!("_archive/{ledger}");
    if !root.join(&first).exists() {
        return first;
    }
    let stem = first.trim_end_matches(".md");
    let today = crate::migrate::today();
    let mut name = format!("{stem}-{today}.md");
    let mut n = 2;
    while root.join(&name).exists() {
        name = format!("{stem}-{today}-{n}.md");
        n += 1;
    }
    name
}

/// routes: the index routes the type directories, so a new page adds no line
/// to it (D-123); every existing line stays where it is (R-172).
fn routes(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    let path = ctx.root.join("index.md");
    let Ok(text) = fs::read_to_string(&path) else {
        return Ok(());
    };
    let missing: Vec<&str> = crate::migrate::DIRECTORY_ROUTES
        .iter()
        .copied()
        .filter(|line| {
            // the link itself, `[[reference/|`, never a page under it
            let link = line
                .trim_start_matches("- ")
                .split('|')
                .next()
                .unwrap_or("");
            !text.contains(&format!("{link}|"))
        })
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let file = format!("{}index.md", ctx.prefix);
    u.item(
        "auto",
        "routes",
        &file,
        format!(
            "{} directory route(s) appended: a new page under a routed directory needs no line (D-123)",
            missing.len()
        ),
    );
    if apply {
        let mut new = text;
        if !new.ends_with('\n') {
            new.push('\n');
        }
        for line in missing {
            new.push_str(line);
            new.push('\n');
        }
        fs::write(&path, new).map_err(|e| e.to_string())?;
        u.written.push(file);
    }
    Ok(())
}

/// dates: a docsys/0.5 page's date is its last content change in history
/// (D-122), so the `updated:` lines leave pages, tracked work and templates —
/// a structural change (R-172). A re-run absorbs a line a branch from before
/// the move still wrote.
fn dates(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    let mut files: Vec<String> = ctx
        .tree
        .pages
        .iter()
        .filter(|p| matches!(p.kind, Kind::Permanent | Kind::Tracked))
        .map(|p| p.rel.clone())
        .collect();
    if let Ok(entries) = fs::read_dir(ctx.root.join("_templates")) {
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|n| n.ends_with(".md"))
            .collect();
        names.sort();
        files.extend(names.into_iter().map(|n| format!("_templates/{n}")));
    }
    let mut n = 0usize;
    for rel in files {
        let path = ctx.root.join(&rel);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Some(new) = crate::fm::without_scalar(&text, "updated") else {
            continue;
        };
        n += 1;
        if apply {
            fs::write(&path, new).map_err(|e| e.to_string())?;
            u.written.push(format!("{}{rel}", ctx.prefix));
        }
    }
    if n > 0 {
        u.item(
            "auto",
            "dates",
            &ctx.root_rel,
            format!("`updated:` removed from {n} page(s) and template(s): a page's date is its last content change in history (D-122)"),
        );
    }
    Ok(())
}

/// On docsys/0.5 a review's approval rides the pull request's description
/// (D-126): a workflow that recorded it in a follow-up pull request or a push
/// takes the approval job instead.
fn on_description(params: &mut crate::workflow::Workflow, to: u32) {
    use crate::workflow::Verify;
    if Era(to).verification_from_history()
        && matches!(params.ci.verify, Verify::PullRequest | Verify::Direct)
    {
        params.ci.verify = Verify::Description;
    }
}

/// The steps of the move from docsys/0.4 to docsys/0.5 (`migrations/0.4-0.5.tsv`).
fn move_0_4_to_0_5(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    let (repo, root, tree, kb) = (ctx.repo, ctx.root, &ctx.tree, ctx.kb);
    let prefix = ctx.prefix.as_str();
    // verified-record: a record proven by history gains its own evidence (D-101)
    {
        for page in tree.pages.iter().filter(|p| p.kind == Kind::Permanent) {
            let Some(fm) = &page.fm else { continue };
            let get = |k: &str| fm.fields.get(k).and_then(Value::as_str);
            if get("verification") != Some("verified") || fm.fields.contains_key("verified_blocks")
            {
                continue;
            }
            let file = format!("{prefix}{}", page.rel);
            let Some(rev) = get("verified_rev").map(str::trim) else {
                u.item(
                    "manual",
                    "verified-record",
                    &file,
                    "verified without a revision — a maintainer reads it and verifies it again"
                        .to_string(),
                );
                u.completed_by(format!("docsys verify {}", page.rel));
                continue;
            };
            let Some(then) = git_out(repo, &["show", &format!("{rev}:{file}")]) else {
                u.item(
                    "manual",
                    "verified-record",
                    &file,
                    format!("`verified_rev: {rev}` does not hold the page in this history — never recorded blind; a maintainer reads it and verifies it again"),
                );
                u.completed_by(format!("docsys verify {}", page.rel));
                continue;
            };
            let body = crate::fresh::body_text(&page.text);
            // the blocks the maintainer read are the blocks now, as R-024 reads it
            let blocks = crate::blocks::hashes(&body);
            if crate::blocks::hashes(&crate::fresh::body_text(&then)) != blocks {
                u.item(
                    "manual",
                    "verified-record",
                    &file,
                    format!("the body changed since {rev} — never recorded blind; a maintainer reads it and verifies it again"),
                );
                u.completed_by(format!("docsys verify {}", page.rel));
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
                    "verified-record",
                    &file,
                    format!("`{s}` moved since {rev}, or had no committed provenance — a maintainer reads it and verifies it again"),
                );
                u.completed_by(format!("docsys verify {}", page.rel));
                continue;
            }
            u.item(
                "auto",
                "verified-record",
                &file,
                format!(
                    "the body {rev} holds is the body now: `verified_blocks` recorded{}",
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
                if let Some(new) = add_record(&text, &blocks, &sources) {
                    fs::write(&path, new).map_err(|e| e.to_string())?;
                    u.written.push(file);
                }
            }
        }
    }

    // pins: a pin's hash becomes its acknowledgement where it still holds and
    // the declaration reads the same region (D-119); everything else is listed
    {
        for c in crate::fresh::convert_legacy(root, repo, apply)? {
            let file = format!("{prefix}{}", c.page);
            // with --apply every converted page loses its pin hashes, whatever
            // the outcome: the commit carries each one
            if apply && !u.written.contains(&file) {
                u.written.push(file.clone());
            }
            match c.outcome {
                crate::fresh::Conversion::Acknowledged => {
                    u.item(
                        "auto",
                        "pins",
                        &file,
                        format!("`{}`: acknowledged beside the page", c.pin),
                    );
                }
                crate::fresh::Conversion::Reresolved { first, last } => {
                    u.item(
                        "manual",
                        "pins",
                        &file,
                        format!("`{}`: its declaration is L{first}-L{last}, not the region 0.15 read — re-read the page against it", c.pin),
                    );
                    u.completed_by(format!("docsys pin --refresh {}", c.page));
                }
                crate::fresh::Conversion::Unresolvable(why) => u.item(
                    "manual",
                    "pins",
                    &file,
                    format!("`{}`: {why} — pin a narrower symbol or the file", c.pin),
                ),
                crate::fresh::Conversion::StaleAsRecorded => {
                    u.item(
                        "manual",
                        "pins",
                        &file,
                        format!("`{}`: stale before the upgrade, still stale — re-read the page against it", c.pin),
                    );
                    u.completed_by(format!("docsys pin --refresh {}", c.page));
                }
            }
        }
        if apply && root.join(crate::ack::DIR).is_dir() {
            u.written.push(format!("{prefix}{}", crate::ack::DIR));
        }
    }

    // ledger-separators: the em-dash markers to ASCII (D-108); after the move
    // R-108's message names `docsys ledger fix`
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

    // ledgers: open items into their topics' files, after the separators are
    // ASCII (D-124)
    ledgers(ctx, u, apply)?;
    // journal: history from here on, the files frozen under _archive/ (D-125)
    journal(ctx, u, apply)?;
    // records: after verified-record, so a record that holds stays (D-126)
    records(ctx, u, apply)?;

    // code-citations: a `doc:` 0.15 read mid-comment that 0.5 reads as prose
    if !kb {
        let idx = crate::checks::build_index(tree);
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
    if !kb && root.join("raw").is_dir() {
        let n = tree
            .pages
            .iter()
            .filter(|p| p.rel.starts_with("raw/"))
            .count();
        u.item("info", "raw-records", &format!("{prefix}raw/"), format!("{n} record(s): from 0.5 a record layer — their dangling references are reported, an edited record stops the commit (R-023)"));
    }

    Ok(())
}

/// What a person pulling the upgrade needs in this clone, whatever the release.
pub const TEAMMATES: &str = "Teammates: after pulling, run `docsys upgrade --apply` once in your clone — the git gate is per clone.";

/// The upgrade commit's message: the move, the notes it crosses, and on the
/// last move the step every other clone takes. It also describes a pull
/// request.
pub fn message(u: &Upgrade) -> String {
    let mut out = if u.last {
        format!(
            "docsys: upgrade the tree to docsys {}",
            crate::dispatch::own()
        )
    } else {
        format!("docsys: upgrade the tree to docsys/0.{}", u.to)
    };
    for (release, text) in &u.notes {
        out.push_str(&format!("\n\nUpgrading to docsys {release}:\n{text}"));
    }
    if u.last {
        out.push_str("\n\n");
        out.push_str(TEAMMATES);
    }
    out
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
        .arg(message(u))
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
    fn the_record_goes_right_after_the_revision() {
        let text = "---\nid: a\nverification: verified\nverified_by: ayse\nverified_rev: abc\nsources: []\n---\nBody.\n";
        let out = add_record(
            text,
            &["941ba81fbfec".into(), "28949667d156".into()],
            &[("@up/x".into(), "fnv:1".into())],
        )
        .unwrap();
        assert!(out.contains("verified_rev: abc\nverified_blocks: [941ba81fbfec, 28949667d156]\nverified_sources:\n  - source: \"@up/x\"\n    hash: \"fnv:1\"\nsources: []\n"), "{out}");
        assert!(add_record("---\nid: a\n---\nBody.\n", &[], &[]).is_none());
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

    /// A release cannot ship a spec without the steps that move a tree to it,
    /// nor without the note that tells a person what changes.
    #[test]
    fn every_spec_has_its_migration_and_its_note() {
        assert_eq!(MIGRATIONS.last().map(|m| m.to), Some(implemented()));
        for (a, b) in MIGRATIONS.iter().zip(MIGRATIONS.iter().skip(1)) {
            assert_eq!(
                a.to, b.from,
                "a gap between docsys/0.{} and docsys/0.{}",
                a.to, b.from
            );
        }
        for m in &MIGRATIONS {
            assert!(
                m.steps.lines().any(|l| l.starts_with("spec-line\t")),
                "{}",
                m.release
            );
            assert!(
                note(m.release).is_some(),
                "CHANGELOG [{}] has no Upgrading section",
                m.release
            );
        }
    }
}
