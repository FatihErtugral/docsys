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

/// The concepts the move retires, as agent-facing text names them:
/// `concept <TAB> literal <TAB> what replaces it`.
pub const RETIRED: &str = include_str!("../migrations/0.4-0.5-retired.tsv");

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
    /// the concepts it retires, as data
    pub retired: &'static str,
    pub apply: fn(&Ctx, &mut Upgrade, bool) -> Result<(), String>,
}

/// Every move this binary knows, in order.
pub const MIGRATIONS: [Migration; 1] = [Migration {
    from: 4,
    to: 5,
    release: "0.16.0",
    steps: STEPS,
    retired: RETIRED,
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
    /// journal lines a branch from before the move wrote after it: the
    /// commit's message carries them into history (D-125)
    pub carried: Vec<String>,
    /// whether the steps wrote; a plan lists in `written` what they would
    pub applied: bool,
    /// the findings the next version adds and removes, before the steps
    added: Vec<String>,
    removed: Vec<String>,
}

/// Why a run stops.
#[derive(Debug)]
pub enum Stop {
    /// refused before any step wrote: the tree is as it was
    Refused(String),
    /// a step failed: what the steps before it wrote is in the working tree
    Failed(String),
}

impl From<String> for Stop {
    fn from(e: String) -> Stop {
        Stop::Failed(e)
    }
}

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stop::Refused(e) | Stop::Failed(e) => f.write_str(e),
        }
    }
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
                Json::Arr(
                    self.written
                        .iter()
                        .filter(|_| self.applied)
                        .map(|w| s(w))
                        .collect(),
                ),
            ),
        ])
        .render()
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
    // a file the move has yet to write is named from its directory
    let p = path
        .canonicalize()
        .ok()
        .or_else(|| {
            let dir = path.parent().filter(|d| !d.as_os_str().is_empty());
            let dir = dir.unwrap_or(Path::new(".")).canonicalize().ok()?;
            Some(dir.join(path.file_name()?))
        })
        .unwrap_or_else(|| path.to_path_buf());
    p.strip_prefix(&repo_c)
        .map(|r| r.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"))
}

/// The next move of the tree: a plan, or with `apply` the move itself.
/// `claude` is the agent layer's directory.
pub fn run(repo: &Path, root: &Path, claude: &Path, apply: bool) -> Result<Upgrade, Stop> {
    run_with(&MIGRATIONS, implemented(), repo, root, claude, apply)
}

/// Each inline list that never closes with `]`, in `.docmeta.yml`, a page or
/// a template: `file: \`field\` on line n`. A step that rewrites such a file
/// cannot know which lines are the list's (D-002).
fn unclosed_lists(repo: &Path, tree: &DocTree) -> Vec<String> {
    let root = tree.root.as_path();
    let named = |path: &Path, (field, line): &(String, usize)| {
        format!("{}: `{field}` on line {line}", rel(repo, path))
    };
    let mut out: Vec<String> = fs::read_to_string(root.join(".docmeta.yml"))
        .ok()
        .and_then(|t| crate::fm::parse_fields(&t).unclosed)
        .map(|u| named(&root.join(".docmeta.yml"), &u))
        .into_iter()
        .collect();
    for page in &tree.pages {
        if let Some(u) = page.fm.as_ref().and_then(|f| f.unclosed.as_ref()) {
            out.push(named(&root.join(&page.rel), u));
        }
    }
    let mut templates: Vec<std::path::PathBuf> = fs::read_dir(root.join("_templates"))
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    templates.sort();
    for path in templates
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
    {
        let unclosed = fs::read_to_string(path)
            .ok()
            .and_then(|t| crate::fm::parse(&t))
            .and_then(|f| f.unclosed);
        if let Some(u) = unclosed {
            out.push(named(path, &u));
        }
    }
    out
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
) -> Result<Upgrade, Stop> {
    let tree = DocTree::load(root).map_err(|e| Stop::Refused(e.to_string()))?;
    if !tree.docmeta_present {
        return Err(Stop::Refused(format!(
            "`{}` has no .docmeta.yml",
            root.display()
        )));
    }
    let from = Era::of(&tree).0;
    if from > target {
        return Err(Stop::Refused(format!(
            "this tree declares docsys/0.{from}; this docsys implements docsys/0.{target} — install a newer docsys"
        )));
    }
    let pin = crate::dispatch::read(root).map_err(Stop::Refused)?;
    let own = crate::dispatch::own();
    if let Some(p) = pin.as_deref() {
        if crate::dispatch::parse(p) > crate::dispatch::parse(own) {
            return Err(Stop::Refused(format!(
                "this tree pins docsys {p}, newer than this docsys {own} — any other docsys command here runs {p}, and its own `docsys upgrade` moves the tree"
            )));
        }
    }
    let unclosed = unclosed_lists(repo, &tree);
    if !unclosed.is_empty() {
        return Err(Stop::Refused(format!(
            "an inline list that never closes with `]` — the lines that are the list's are not known, so nothing is written; close each first (D-002):\n  {}",
            unclosed.join("\n  ")
        )));
    }
    let next =
        if from < target {
            Some(migrations.iter().find(|m| m.from == from).ok_or_else(|| {
                Stop::Refused(format!("no migration declared from docsys/0.{from}"))
            })?)
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
        applied: apply,
        ..Upgrade::default()
    };
    if let Some(m) = next {
        preview(&ctx, &mut u);
        common(&ctx, &mut u, apply)?;
        (m.apply)(&ctx, &mut u, apply)?;
        spec_line(&ctx, &mut u, apply)?;
        render_preview(&mut u);
        if let Some(text) = note(m.release) {
            u.notes.push((m.release.to_string(), text));
        }
    } else {
        common(&ctx, &mut u, apply)?;
    }
    let retired: Vec<&str> = migrations
        .iter()
        .filter(|m| m.to <= u.to)
        .map(|m| m.retired)
        .collect();
    retired_concepts(&ctx, &mut u, &retired);
    stray_layers(&ctx, &mut u);
    if u.last {
        pin_step(&ctx, &mut u, pin.as_deref(), apply)?;
    }
    u.written.sort();
    u.written.dedup();
    Ok(u)
}

/// agent-layer: the agent layer lives at the repository's top (D-098). A
/// `.claude/` directory below it that holds a file docsys writes into a
/// layer — one of its relays, skills or commands, by its path in the layer —
/// was written by an earlier docsys run from a subdirectory, or copied there.
/// A knowledge base's own layer, beside its `.docmeta.yml`, is not stray.
/// What git tracks or would track is read; an ignored directory is not.
fn stray_layers(ctx: &Ctx, u: &mut Upgrade) {
    let own: BTreeSet<&str> = [false, true]
        .into_iter()
        .flat_map(crate::agents::owned_assets)
        .map(|(asset, _, _)| asset)
        .chain(crate::agents::HOOK_FILES)
        .collect();
    let Some(listed) = git_out(
        ctx.repo,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    ) else {
        return;
    };
    let layer_here = rel(ctx.repo, ctx.claude);
    let kb_root = |dir: &str| {
        fs::read_to_string(ctx.repo.join(dir).join(".docmeta.yml")).is_ok_and(|t| {
            crate::fm::parse_fields(&t)
                .fields
                .get("profile")
                .and_then(Value::as_str)
                .is_some_and(|p| p.trim() == "knowledge-base")
        })
    };
    let mut layers: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for path in listed.split('\0') {
        let Some((dir, inside)) = path.split_once("/.claude/") else {
            continue;
        };
        let layer = format!("{dir}/.claude");
        if !own.contains(inside) || layer == layer_here || kb_root(dir) {
            continue;
        }
        *layers.entry(format!("{layer}/")).or_default() += 1;
    }
    for (layer, n) in layers {
        u.item(
            "manual",
            "agent-layer",
            &layer,
            format!("an agent layer below the repository's top, holding {n} file(s) docsys writes — the layer lives at the top (D-098): keep what is yours in `.claude/` there, then remove this one"),
        );
    }
}

/// The files agent-facing text lives in that docsys does not own: the
/// instructions files outside the rules block, the repository's own rules,
/// skills and commands, the README.
fn instruction_files(ctx: &Ctx) -> Vec<std::path::PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(&p, out);
            } else {
                out.push(p);
            }
        }
    }
    let repo = ctx.repo;
    let mut files: Vec<std::path::PathBuf> = [
        "CLAUDE.md",
        "AGENTS.md",
        ".claude/CLAUDE.md",
        "README.md",
        ".cursorrules",
    ]
    .iter()
    .map(|f| repo.join(f))
    .filter(|p| p.is_file())
    .collect();
    for dir in ["rules", "skills", "commands"] {
        walk(&ctx.claude.join(dir), &mut files);
    }
    walk(&repo.join(".cursor/rules"), &mut files);
    let owned: BTreeSet<String> = [false, true]
        .into_iter()
        .flat_map(crate::agents::owned_assets)
        .map(|(asset, _, _)| rel(repo, &ctx.claude.join(asset)))
        .collect();
    let contract = ctx.kb.then(|| rel(repo, &ctx.root.join("AGENTS.md")));
    files.dedup();
    files
        .into_iter()
        .filter(|p| {
            let file = rel(repo, p);
            !owned.contains(&file) && contract.as_ref() != Some(&file)
        })
        .collect()
}

/// retired-concepts: agent-facing text docsys did not write that still names
/// a concept a move retired — each line listed with what replaces it, for a
/// person and `/docsys-upgrade`; never edited.
fn retired_concepts(ctx: &Ctx, u: &mut Upgrade, tables: &[&str]) {
    let rows: Vec<(String, &str)> = tables
        .iter()
        .flat_map(|t| t.lines())
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let mut f = l.split('\t');
            let (_concept, literal, replacement) = (f.next()?, f.next()?, f.next()?);
            Some((literal.to_lowercase(), replacement))
        })
        .collect();
    if rows.is_empty() {
        return;
    }
    let (mut listed, mut more) = (0usize, 0usize);
    for path in instruction_files(ctx) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let file = rel(ctx.repo, &path);
        let mut in_block = false;
        for (i, line) in text.lines().enumerate() {
            if line.contains(crate::rules::BLOCK_BEGIN) {
                in_block = true;
            }
            if in_block {
                in_block = !line.contains(crate::rules::BLOCK_END);
                continue;
            }
            let lower = line.to_lowercase();
            // one item a line, naming each concept it holds once
            let mut named: Vec<String> = Vec::new();
            let mut replaced: Vec<&str> = Vec::new();
            for (literal, replacement) in rows.iter().filter(|(l, _)| lower.contains(l.as_str())) {
                let shown = if literal.contains('`') {
                    literal.clone()
                } else {
                    format!("`{literal}`")
                };
                if !named.contains(&shown) {
                    named.push(shown);
                }
                if !replaced.contains(replacement) {
                    replaced.push(replacement);
                }
            }
            if named.is_empty() {
                continue;
            }
            if listed < 50 {
                u.item(
                    "manual",
                    "retired-concepts",
                    &format!("{file}:{}", i + 1),
                    format!(
                        "names {}, which docsys/0.5 retired — {}",
                        named.join(" and "),
                        replaced.join("; ")
                    ),
                );
                listed += 1;
            } else {
                more += 1;
            }
        }
    }
    if more > 0 {
        u.item(
            "manual",
            "retired-concepts",
            "-",
            format!("… and {more} more of the same"),
        );
    }
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
    u.added = next.difference(&now).cloned().collect();
    u.removed = now.difference(&next).cloned().collect();
}

/// The preview's lines, once the steps are known.
fn render_preview(u: &mut Upgrade) {
    let added = u.added.clone();
    let removed = u.removed.clone();
    u.preview.push(crate::say::upgrade_preview(
        u.to,
        added.len(),
        removed.len(),
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
        let spec = format!("spec: docsys/0.{}", u.to);
        let mut new: Vec<String> = text
            .lines()
            .map(|l| {
                if l.trim_start().starts_with("spec:") {
                    spec.clone()
                } else {
                    l.to_string()
                }
            })
            .collect();
        // a file that declares no spec was read as docsys/0.4 (D-118): the
        // line goes first, where `adopt` writes it
        if !new.contains(&spec) {
            new.insert(0, spec);
        }
        let mut out = new.join("\n");
        if text.ends_with('\n') || text.is_empty() {
            out.push('\n');
        }
        fs::write(&path, out).map_err(|e| e.to_string())?;
    }
    u.written.push(file);
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
    }
    u.written.push(file);
    Ok(())
}

/// A file holding exactly the text this version writes is docsys's own: when
/// nobody committed it yet — `docsys agents` wrote it before the upgrade — the
/// upgrade commit carries it.
fn carry_untracked(repo: &Path, u: &mut Upgrade, step: &'static str, file: &str) {
    if git_out(repo, &["ls-files", "--error-unmatch", "--", file]).is_some() {
        return;
    }
    u.item(
        "auto",
        step,
        file,
        "committed: this version's text, not tracked yet".to_string(),
    );
    u.written.push(file.to_string());
}

/// The steps every run takes: this binary's relays, gate, workflow, rules
/// block and assets, each regenerated only where nobody edited it.
fn common(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    let (repo, root, claude, kb) = (ctx.repo, ctx.root, ctx.claude, ctx.kb);
    let (root_rel, preamble) = (ctx.root_rel.as_str(), ctx.preamble.as_str());
    // why the post-edit relay goes, said on the first of its rows
    let mut post_edit_said = false;
    let post_edit_gone = "a page's date is history's and a page carries no verification, so the relay has nothing left to do (D-122, D-130)";
    // hook-wires: settings.json names each relay in the one form (D-099)
    let settings = claude.join("settings.json");
    if let Ok(text) = fs::read_to_string(&settings) {
        let file = rel(repo, &settings);
        match crate::hook::parse_json(&text) {
            Some(mut doc) => {
                // a docsys/0.5 tree runs no post-edit relay (D-130)
                if !Era(u.to).page_verification() {
                    let n = crate::agents::remove_relay_wires(&mut doc, crate::agents::POST_EDIT);
                    if n > 0 {
                        u.item(
                            "auto",
                            "hook-wires",
                            &file,
                            format!("{n} post-edit wire(s) taken out: {post_edit_gone}"),
                        );
                        post_edit_said = true;
                        if apply {
                            fs::write(&settings, doc.render()).map_err(|e| e.to_string())?;
                        }
                        u.written.push(file.clone());
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
                    }
                    u.written.push(file);
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
        // goes, an edited one is its owner's to retire (D-130)
        if hook == crate::agents::POST_EDIT && !Era(u.to).page_verification() {
            if text == fresh || crate::agents::released(hook, &text, "").is_some() {
                let what = if post_edit_said {
                    "removed, with its wire above".to_string()
                } else {
                    format!("removed: {post_edit_gone}")
                };
                u.item("auto", "hook-scripts", &file, what);
                post_edit_said = true;
                let tracked =
                    git_out(repo, &["ls-files", "--error-unmatch", "--", &file]).is_some();
                if apply {
                    fs::remove_file(&path).map_err(|e| e.to_string())?;
                }
                if tracked {
                    u.written.push(file);
                }
            } else {
                u.item(
                    "manual",
                    "hook-scripts",
                    &file,
                    "edited by its owner, and with no job on docsys/0.5 (D-130) — move what you added elsewhere, then remove it".to_string(),
                );
            }
            continue;
        }
        if text == fresh {
            carry_untracked(repo, u, "hook-scripts", &file);
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
            }
            u.written.push(file);
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
        // a clone with no gate gets the one adopt writes: `docsys upgrade
        // --apply` is the step every clone takes after pulling a move
        None => {
            let hard = crate::adopt::gate_clean(root, repo);
            let hooks =
                crate::adopt::gate_hooks_dir(repo).unwrap_or_else(|| repo.join(".git/hooks"));
            let file = rel(repo, &hooks.join("pre-commit"));
            let mode = if hard {
                "hard, the tree linting clean"
            } else {
                "in warn mode while lint or refs report an error (D-072)"
            };
            let half = if message {
                ", with its commit-msg half"
            } else {
                ""
            };
            u.item(
                "auto",
                "git-gate",
                &file,
                format!("the docsys gate written in this clone{half}, {mode}"),
            );
            // a gate in a hooks directory the repository tracks goes into the commit
            let tracked = git_out(repo, &["ls-files", "--", &rel(repo, &hooks)])
                .is_some_and(|l| !l.trim().is_empty());
            if tracked {
                u.written.push(file);
                if message {
                    u.written.push(rel(repo, &hooks.join("commit-msg")));
                }
            }
            if apply && crate::adopt::ensure_git_gate(repo, root_rel, hard, message) == "failed" {
                return Err("the git gate could not be written in this clone".to_string());
            }
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
                "the docsys block rewritten for this version, its mode kept".to_string(),
            );
            // the half that reads the message, which a docsys/0.5 gate adds (D-125)
            let msg = hooks.join("commit-msg");
            if message {
                let verb = if msg.is_file() {
                    "rewritten"
                } else {
                    "written"
                };
                u.item(
                    "auto",
                    "git-gate",
                    &rel(repo, &msg),
                    format!("the docsys block {verb}, its mode kept: it reads the commit message, and under `commit_policy: require` code with no documentation needs `Docs: <why>` (D-125)"),
                );
            }
            if tracked {
                u.written.push(file.clone());
                if message || msg.is_file() {
                    u.written.push(rel(repo, &msg));
                }
            }
            if apply {
                let done = crate::adopt::ensure_git_gate(repo, root_rel, false, message);
                if done == "failed" {
                    return Err(format!("{file}: the gate block could not be written"));
                }
            }
        }
    }

    // ci-workflow: regenerated when untouched; of the owner's, only what
    // installs docsys is replaced, so it reads the pin — never a template's
    // default, never a sha256 value docsys cannot know
    let own = crate::dispatch::own();
    let copied = "copied from the release page — docsys cannot know them";
    let pinned = Era(u.to).pinned_ci_install();
    match crate::workflow::classify(repo, root_rel) {
        None => {}
        Some(crate::workflow::Existing::Untouched { from, mut params }) => {
            let mut moved = without_approval_job(&mut params, u.to);
            if pinned && matches!(params.ci.install, crate::workflow::Install::Release(_)) {
                params.ci.install = crate::workflow::Install::ReleaseSums;
                moved.push_str("; its release install reads the version the tree pins and checks the archive against its release's SHA256SUMS, so it holds no sha256 value");
            }
            let path = repo.join(crate::workflow::PATH);
            let fresh = crate::workflow::render(&params);
            let current = fs::read_to_string(&path).unwrap_or_default();
            if fresh != current {
                if matches!(params.ci.install, crate::workflow::Install::Release(_)) {
                    u.item(
                        "manual",
                        "ci-workflow",
                        crate::workflow::PATH,
                        format!(
                            "a release install of docsys {from}, untouched, and the tree pins {own}: write {own} and its archives' sha256 values, {copied}"
                        ),
                    );
                } else {
                    u.item("auto", "ci-workflow", crate::workflow::PATH, format!("regenerated from {from} with its own parameters, pinned to this version{moved}"));
                    if apply {
                        fs::write(&path, fresh).map_err(|e| e.to_string())?;
                    }
                    u.written.push(crate::workflow::PATH.to_string());
                }
            }
        }
        Some(crate::workflow::Existing::Legacy { from, mut params }) => {
            let moved = without_approval_job(&mut params, u.to);
            let kept = if moved.is_empty() {
                " with its mode"
            } else {
                ""
            };
            u.item("auto", "ci-workflow", crate::workflow::PATH, format!("the {from} workflow, untouched: regenerated{kept}, pinned to this version{moved}"));
            if apply {
                fs::write(
                    repo.join(crate::workflow::PATH),
                    crate::workflow::render(&params),
                )
                .map_err(|e| e.to_string())?;
            }
            u.written.push(crate::workflow::PATH.to_string());
        }
        Some(crate::workflow::Existing::Owned { .. }) if pinned => {
            let path = repo.join(crate::workflow::PATH);
            let text = fs::read_to_string(&path).unwrap_or_default();
            match crate::workflow::reinstall(&text, root_rel, own) {
                crate::workflow::Reinstall::Pinned => {}
                crate::workflow::Reinstall::Rewritten { text: new, what } => {
                    u.item(
                        "auto",
                        "ci-workflow",
                        crate::workflow::PATH,
                        format!("the workflow is yours, and only what installs docsys changes: {what}"),
                    );
                    u.diffs.push((
                        crate::workflow::PATH.to_string(),
                        crate::diff::unified(
                            &text,
                            &new,
                            crate::workflow::PATH,
                            crate::workflow::PATH,
                            3,
                        ),
                    ));
                    if apply {
                        fs::write(&path, new).map_err(|e| e.to_string())?;
                    }
                    u.written.push(crate::workflow::PATH.to_string());
                }
                crate::workflow::Reinstall::Unclear { line, what } => u.item(
                    "manual",
                    "ci-workflow",
                    &format!("{}:{line}", crate::workflow::PATH),
                    format!("the workflow is yours, and what installs docsys is not clear here, so nothing in it changed: {what}"),
                ),
            }
        }
        Some(crate::workflow::Existing::Owned { .. }) => {}
    }

    // rules-block: refreshed where its markers are (D-110, D-114)
    if !kb {
        match crate::adopt::rules_target(repo, None) {
            Some(target) => {
                let file = rel(repo, &target);
                let text = fs::read_to_string(&target).unwrap_or_default();
                let want =
                    crate::rules::agents_block_with(preamble, Era(u.to).journal_from_history());
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
                            crate::rules::write_agents_block_with(
                                &target,
                                preamble,
                                Era(u.to).journal_from_history(),
                            )?;
                        }
                        u.written.push(file);
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
        // the audit organ leaves with page verification (D-130): the text a
        // release wrote goes, an edited one is its owner's to retire
        if asset == crate::agents::KB_AUDIT_SKILL && !Era(u.to).page_verification() {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            if text == want || crate::agents::released(asset, &text, "").is_some() {
                u.item(
                    "auto",
                    "assets",
                    &file,
                    "removed: a docsys/0.5 page carries no verification (D-130)".to_string(),
                );
                if apply {
                    fs::remove_file(&path).map_err(|e| e.to_string())?;
                    if let Some(dir) = path.parent() {
                        let _ = fs::remove_dir(dir);
                    }
                }
                u.written.push(file);
            } else {
                u.item(
                    "manual",
                    "assets",
                    &file,
                    "edited by its owner, and with no job on docsys/0.5 (D-130) — move what you added elsewhere, then remove it".to_string(),
                );
            }
            continue;
        }
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
                }
                u.written.push(file);
            }
            continue;
        };
        if text == want {
            carry_untracked(repo, u, "assets", &file);
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
            }
            u.written.push(file);
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
                carry_untracked(repo, u, "kb-contract", &file);
            } else if let Some(release) = crate::agents::released("AGENTS.md", &text, "") {
                u.item(
                    "auto",
                    "kb-contract",
                    &file,
                    format!("refreshed: a text docsys {release} wrote, untouched"),
                );
                if apply {
                    fs::write(&path, &want).map_err(|e| e.to_string())?;
                }
                u.written.push(file);
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
    if !Era::of(&ctx.tree).page_verification() {
        no_verification(ctx, u, apply)?;
    }
    Ok(())
}

/// What docsys/0.5 keeps of page verification: nothing (D-130). Every record
/// field leaves every page, a page's pins live in `pins:` and their
/// acknowledgements under `.pins/`, and `.docmeta.yml` names no maintainer.
fn no_verification(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    records(ctx, u, apply)?;
    pin_names(ctx, u, apply)?;
    maintainers(ctx, u, apply)
}

/// The comments docsys wrote above `maintainers:` in a new `.docmeta.yml`.
const MAINTAINERS_COMMENTS: [&str; 2] = [
    "# Who may confirm work and verify pages (R-208); empty = anyone, as before.",
    "# Who may verify pages (R-208); empty = anyone, as before.",
];

/// maintainers: no one vouches on docsys/0.5 (D-130), so the list leaves
/// `.docmeta.yml`, with the comment docsys wrote above it.
fn maintainers(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    let path = ctx.root.join(".docmeta.yml");
    let Ok(text) = fs::read_to_string(&path) else {
        return Ok(());
    };
    let fields = crate::fm::parse_fields(&text);
    let Some(span) = fields.spans.get("maintainers").cloned() else {
        return Ok(());
    };
    let file = rel(ctx.repo, &path);
    u.item(
        "auto",
        "maintainers",
        &file,
        "`maintainers:` taken out: a docsys/0.5 page carries no verification, and a confirmation is the word of whoever gives it (D-130)".to_string(),
    );
    u.written.push(file);
    if !apply {
        return Ok(());
    }
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let comment = span.start.checked_sub(1).filter(|&i| {
        lines
            .get(i)
            .is_some_and(|l| MAINTAINERS_COMMENTS.contains(&l.trim_end_matches(['\n', '\r'])))
    });
    // the blank line that set the block apart goes with it, so no two blank
    // lines are left in a row and none ends the file
    let first = comment.unwrap_or(span.start);
    let blank = |i: usize| lines.get(i).is_some_and(|l| l.trim().is_empty());
    let separator = first
        .checked_sub(1)
        .filter(|&i| blank(i) && (span.end >= lines.len() || blank(span.end)));
    let kept: String = lines
        .iter()
        .enumerate()
        .filter(|(i, _)| !span.contains(i) && Some(*i) != comment && Some(*i) != separator)
        .map(|(_, l)| *l)
        .collect();
    fs::write(&path, kept).map_err(|e| e.to_string())
}

/// pin-names: a docsys/0.5 page's pins live in `pins:`, and their
/// acknowledgements under `.pins/` (D-130): no 0.5 name says verify.
fn pin_names(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    let tree = DocTree::load(ctx.root).map_err(|e| e.to_string())?;
    for page in tree.pages.iter().filter(|p| p.kind != Kind::Raw) {
        let Some(fm) = &page.fm else { continue };
        if !fm.fields.contains_key("verifies") {
            continue;
        }
        let file = format!("{}{}", ctx.prefix, page.rel);
        if fm.fields.contains_key("pins") {
            u.item(
                "manual",
                "pin-names",
                &file,
                "both `verifies:` and `pins:`: join the two lists under `pins:` (D-130)"
                    .to_string(),
            );
            continue;
        }
        u.item(
            "auto",
            "pin-names",
            &file,
            "`verifies:` renamed `pins:`: a pin says when the code moved, and vouches for nothing (D-130)".to_string(),
        );
        u.written.push(file);
        if !apply {
            continue;
        }
        let mut frontmatter = false;
        let mut renamed = String::with_capacity(page.text.len());
        for (n, line) in page.text.split_inclusive('\n').enumerate() {
            let bare = line.trim_end_matches(['\n', '\r']);
            if n == 0 {
                frontmatter = bare == "---";
            } else if frontmatter && bare == "---" {
                frontmatter = false;
            } else if frontmatter && bare.starts_with("verifies:") {
                renamed.push_str("pins:");
                renamed.push_str(&line["verifies:".len()..]);
                continue;
            }
            renamed.push_str(line);
        }
        fs::write(ctx.root.join(&page.rel), renamed).map_err(|e| e.to_string())?;
    }
    let old = ctx.root.join(".verifies");
    if old.is_dir() {
        let from = format!("{}.verifies", ctx.prefix);
        let to = format!("{}{}", ctx.prefix, crate::ack::DIR);
        u.item(
            "auto",
            "pin-names",
            &from,
            format!("→ {to}, every acknowledgement kept (D-130)"),
        );
        u.written.push(from);
        u.written.push(to);
        if apply {
            move_tree(&old, &ctx.root.join(crate::ack::DIR))?;
        }
    }
    Ok(())
}

/// Every file under `from` to the same place under `to`, then `from` gone.
fn move_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if src.is_dir() {
            move_tree(&src, &dst)?;
        } else if !dst.exists() {
            fs::rename(&src, &dst).map_err(|e| e.to_string())?;
        } else {
            fs::remove_file(&src).map_err(|e| e.to_string())?;
        }
    }
    fs::remove_dir(from).map_err(|e| e.to_string())
}

/// The fields a verification record took in a page.
const RECORD_FIELDS: [&str; 5] = [
    "verification",
    "verified_by",
    "verified_rev",
    "verified_blocks",
    "verified_sources",
];

/// records: a docsys/0.5 page carries no verification (D-130), so every
/// field a record took leaves every page, and nothing is carried.
fn records(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    // the pages as the steps before left them: every file with a frontmatter
    // but the records, which are never edited (R-023)
    let tree = DocTree::load(ctx.root).map_err(|e| e.to_string())?;
    for page in tree.pages.iter().filter(|p| p.kind != Kind::Raw) {
        let Some(fm) = &page.fm else { continue };
        if !RECORD_FIELDS.iter().any(|k| fm.fields.contains_key(*k)) {
            continue;
        }
        let file = format!("{}{}", ctx.prefix, page.rel);
        u.item(
            "auto",
            "records",
            &file,
            "the verification fields taken out: docsys/0.5 keeps no page verification — `/docsys-crosscheck` checks a page against its sources and code when a person asks (D-130)".to_string(),
        );
        u.written.push(file.clone());
        if !apply {
            continue;
        }
        match crate::fm::without_fields(&page.text, &RECORD_FIELDS) {
            Some(Ok(text)) => {
                fs::write(ctx.root.join(&page.rel), text).map_err(|e| e.to_string())?
            }
            Some(Err(e)) => return Err(format!("{file}: {e}")),
            None => {}
        }
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
    // a re-run after the move: what a branch from before it wrote since —
    // into a frozen file, or into a journal file it brought back — goes into
    // this commit's message, and the frozen files keep their bytes
    let frozen = crate::journal::frozen(ctx.root);
    if !frozen.is_empty() {
        let mut known = String::new();
        for (rel, text) in &frozen {
            let file = format!("{}{rel}", ctx.prefix);
            let then = crate::journal::frozen_at_move(ctx.repo, &ctx.prefix, rel);
            if let Some(then) = &then {
                known.push_str(then);
                let late = crate::journal::late_lines(then, text);
                if !late.is_empty() {
                    u.item(
                        "auto",
                        "journal",
                        &file,
                        format!("{} line(s) a branch from before the move wrote go into this commit's message, and the file gets its bytes back (D-125)", late.len()),
                    );
                    u.carried.extend(late);
                    if apply {
                        fs::write(ctx.root.join(rel), then).map_err(|e| e.to_string())?;
                    }
                    u.written.push(file);
                }
            } else {
                known.push_str(text);
            }
        }
        for rel in files {
            let file = format!("{}{rel}", ctx.prefix);
            let text = fs::read_to_string(ctx.root.join(&rel)).unwrap_or_default();
            let late = crate::journal::late_lines(&known, &text);
            u.item(
                "auto",
                "journal",
                &file,
                format!("a journal file a branch from before the move brought back: {} line(s) not frozen yet go into this commit's message, and the file goes (D-125)", late.len()),
            );
            u.carried.extend(late);
            if apply {
                fs::remove_file(ctx.root.join(&rel)).map_err(|e| e.to_string())?;
            }
            u.written.push(file);
        }
        if apply {
            let _ = fs::remove_dir(ctx.root.join("work/journal"));
        }
        return Ok(());
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
    if moves.is_empty() {
        return Ok(());
    }
    if apply {
        fs::create_dir_all(ctx.root.join("_archive/journal")).map_err(|e| e.to_string())?;
    }
    for (from, to) in &moves {
        if apply {
            fs::rename(ctx.root.join(from), ctx.root.join(to)).map_err(|e| e.to_string())?;
        }
        u.written.push(format!("{}{from}", ctx.prefix));
        u.written.push(format!("{}{to}", ctx.prefix));
    }
    if apply {
        let _ = fs::remove_dir(ctx.root.join("work/journal"));
    }
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
            if apply {
                fs::write(&path, new).map_err(|e| e.to_string())?;
            }
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
            // a ledger a branch from before the move brought back: what a
            // slice already holds is frozen once
            let frozen: Vec<String> = if is_ledger {
                crate::checks::archive_slices(ctx.root, ledger)
                    .iter()
                    .filter_map(|s| fs::read_to_string(ctx.root.join(s)).ok())
                    .flat_map(|t| t.lines().map(str::to_string).collect::<Vec<_>>())
                    .collect()
            } else {
                Vec::new()
            };
            let rest: String = text
                .split_inclusive('\n')
                .filter(|l| !l.starts_with("- [ ] "))
                .filter(|l| {
                    let line = l.trim_end_matches('\n');
                    line.trim().is_empty() || !frozen.iter().any(|f| f == line)
                })
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
            u.written.push(file.clone());
            if !apply {
                // the files the move writes, where `items::add` writes them
                for line in &open {
                    u.written
                        .push(format!("{}{}", ctx.prefix, topic_file(list, ctx.kb, line)));
                }
                if let Some(s) = &slice {
                    u.written.push(format!("{}{s}", ctx.prefix));
                }
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
        }
        misfiled(ctx, u, list, apply)?;
    }
    Ok(())
}

/// The topic file an open item's line belongs in (D-124).
fn topic_file(list: crate::items::List, kb: bool, line: &str) -> String {
    format!("{}/{}.md", list.dir(kb), crate::items::topic_of(line))
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
        u.written.push(file.clone());
        if !apply {
            u.written.extend(
                lines
                    .iter()
                    .map(|l| format!("{}{}", ctx.prefix, topic_file(list, ctx.kb, l))),
            );
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

/// routes: the layout routes the type directories (D-123), so the route lines
/// an earlier 0.16 build appended leave the index — exactly those lines;
/// every other line stays where it is (R-172).
fn routes(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    let path = ctx.root.join("index.md");
    let Ok(text) = fs::read_to_string(&path) else {
        return Ok(());
    };
    let gone = text
        .lines()
        .filter(|l| crate::migrate::DIRECTORY_ROUTES.contains(l))
        .count();
    if gone == 0 {
        return Ok(());
    }
    let file = format!("{}index.md", ctx.prefix);
    u.item(
        "auto",
        "routes",
        &file,
        format!(
            "{gone} type-directory route line(s) docsys wrote taken out: the layout routes those directories (D-123)"
        ),
    );
    if apply {
        let new: String = text
            .split_inclusive('\n')
            .filter(|l| {
                !crate::migrate::DIRECTORY_ROUTES.contains(&l.trim_end_matches(['\n', '\r']))
            })
            .collect();
        fs::write(&path, new).map_err(|e| e.to_string())?;
    }
    u.written.push(file);
    Ok(())
}

/// dates: a docsys/0.5 page's date is its last content change in history
/// (D-122), so the `updated:` lines leave every page and template —
/// a structural change (R-172). A re-run absorbs a line a branch from before
/// the move still wrote.
fn dates(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    // every page wherever it sits, but the records, which are never edited
    // (R-023)
    let mut files: Vec<String> = ctx
        .tree
        .pages
        .iter()
        .filter(|p| p.kind != Kind::Raw)
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
        }
        u.written.push(format!("{}{rel}", ctx.prefix));
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

/// A docsys/0.5 page carries no verification (D-130): a workflow's
/// verify-on-approval job leaves with the move. What it says of the change,
/// empty when there was no job.
fn without_approval_job(params: &mut crate::workflow::Workflow, to: u32) -> String {
    use crate::workflow::Verify;
    let from = params.ci.verify;
    if !Era(to).page_verification() && from != Verify::Off {
        params.ci.verify = Verify::Off;
        return format!(
            "; its `{}` approval job leaves — a docsys/0.5 page carries no verification (D-130)",
            from.name()
        );
    }
    String::new()
}

/// The 1-based lines `region` takes in `source`: the lines 0.15 extracted, in
/// a row.
fn lines_at(source: &str, region: &str) -> Option<(usize, usize)> {
    let all: Vec<&str> = source.lines().collect();
    let part: Vec<&str> = region.lines().collect();
    let first = (0..all.len()).find(|&i| all.get(i..i + part.len()) == Some(&part[..]))?;
    Some((first + 1, first + part.len().max(1)))
}

/// The page with every pin's `hash:` line gone and, for each `(path,
/// symbol)` in `whole`, that entry's `symbol:` line gone too — the
/// frontmatter only, each entry's other lines as written. `None` when
/// nothing changes.
fn rewrite_pins(text: &str, whole: &[(String, String)]) -> Option<String> {
    fn flush(entry: &mut Vec<&str>, whole: &[(String, String)], out: &mut String) -> bool {
        let pair = |l: &str| -> Option<(String, String)> {
            let (k, v) = l.get(4..)?.split_once(':')?;
            Some((
                k.trim().to_string(),
                v.trim().trim_matches(['"', '\'']).to_string(),
            ))
        };
        let value = |key: &str| {
            entry
                .iter()
                .filter_map(|l| pair(l))
                .find(|(k, _)| k == key)
                .map(|(_, v)| v)
        };
        let target = (value("path"), value("symbol"));
        let drop = |l: &str| match pair(l) {
            Some((k, _)) if k == "hash" => true,
            Some((k, _)) if k == "symbol" => {
                matches!(&target, (Some(p), Some(s)) if whole.iter().any(|(wp, ws)| wp == p && ws == s))
            }
            _ => false,
        };
        let kept: Vec<&str> = entry.iter().copied().filter(|l| !drop(l)).collect();
        let changed = kept.len() != entry.len();
        for (i, line) in kept.iter().enumerate() {
            out.push_str(if i == 0 { "  - " } else { "    " });
            out.push_str(line.get(4..).unwrap_or(""));
        }
        entry.clear();
        changed
    }
    let mut lines = text.split_inclusive('\n');
    let first = lines.next()?;
    if first.trim_end_matches(['\n', '\r']) != "---" {
        return None;
    }
    let mut out = String::from(first);
    let (mut frontmatter, mut verifies, mut changed) = (true, false, false);
    let mut entry: Vec<&str> = Vec::new();
    for line in lines {
        let bare = line.trim_end_matches(['\n', '\r']);
        if frontmatter && verifies && (bare.starts_with("  - ") || bare.starts_with("    ")) {
            if bare.starts_with("  - ") {
                changed |= flush(&mut entry, whole, &mut out);
            }
            entry.push(line);
            continue;
        }
        changed |= flush(&mut entry, whole, &mut out);
        verifies = false;
        if frontmatter && bare == "---" {
            frontmatter = false;
        } else if frontmatter && bare.starts_with("verifies:") {
            verifies = true;
        }
        out.push_str(line);
    }
    changed |= flush(&mut entry, whole, &mut out);
    changed.then_some(out)
}

/// pins: the move keeps 0.15's verdict on each pin that carries a `hash:`
/// (D-119). One 0.15 read as fresh is acknowledged at the region 0.5 reads —
/// its declaration, wherever 0.15 had bound the symbol — or, where 0.5 finds
/// no declaration of the symbol, at the whole file, the entry's `symbol:`
/// leaving the page's frontmatter. One stale on 0.15 stays stale, listed. No
/// pin is refreshed: every region acknowledged is one 0.15 called fresh.
fn pins(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    let (repo, root) = (ctx.repo, ctx.root);
    for page in ctx.tree.pages.iter().filter(|p| p.kind == Kind::Permanent) {
        let Some(fm) = &page.fm else { continue };
        let held: Vec<crate::fresh::Pin> = crate::fresh::pins_of(fm)
            .into_iter()
            .filter(|p| !p.hash.is_empty())
            .collect();
        if held.is_empty() {
            continue;
        }
        let file = format!("{}{}", ctx.prefix, page.rel);
        let id = fm
            .fields
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let mut acks: Vec<String> = Vec::new();
        let mut whole: Vec<(String, String)> = Vec::new();
        for pin in held {
            let label = pin.label();
            let symbol = pin
                .symbol
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let source = fs::read_to_string(repo.join(&pin.path)).ok();
            let old = source.as_deref().and_then(|s| {
                crate::fresh::region(s, &pin.path, symbol)
                    .ok()
                    .filter(|o| crate::fresh::content_hash(o) == pin.hash)
            });
            let (Some(source), Some(old)) = (source, old) else {
                u.item(
                    "manual",
                    "pins",
                    &file,
                    format!("`{label}`: stale before the upgrade, still stale — re-read the page against it"),
                );
                u.completed_by(format!("docsys pin --refresh {}", page.rel));
                continue;
            };
            if id.is_none() {
                u.item(
                    "manual",
                    "pins",
                    &file,
                    format!("`{label}`: {} has no `id` (R-050) — acknowledgements are kept by page id; give it one", page.rel),
                );
                continue;
            }
            match (
                crate::fresh::declared_region(&source, &pin.path, symbol),
                symbol,
            ) {
                (Ok(new), _) if new == old => {
                    acks.push(crate::ack::region_hash(&new, &pin.path));
                    u.item(
                        "auto",
                        "pins",
                        &file,
                        format!("`{label}`: acknowledged beside the page"),
                    );
                }
                (Ok(new), Some(sym)) => {
                    acks.push(crate::ack::region_hash(&new, &pin.path));
                    let span = |r: Option<(usize, usize)>| {
                        r.map_or("?".to_string(), |(a, b)| format!("L{a}-L{b}"))
                    };
                    u.item(
                        "auto",
                        "pins",
                        &file,
                        format!(
                            "`{label}`: fresh as 0.15 read it, {}; acknowledged where 0.5 reads its declaration, {}",
                            span(lines_at(&source, &old)),
                            span(crate::symbols::resolve(&source, &pin.path, sym).ok())
                        ),
                    );
                }
                (Ok(new), None) => {
                    acks.push(crate::ack::region_hash(&new, &pin.path));
                    u.item(
                        "auto",
                        "pins",
                        &file,
                        format!("`{label}`: acknowledged beside the page"),
                    );
                }
                (Err(_), Some(sym)) => {
                    acks.push(crate::ack::region_hash(&source, &pin.path));
                    whole.push((pin.path.clone(), sym.to_string()));
                    u.item(
                        "auto",
                        "pins",
                        &file,
                        format!("`{label}`: fresh as 0.15 read it; pinned to the whole file: 0.5 finds no declaration of `{sym}`"),
                    );
                }
                (Err(why), None) => u.item(
                    "manual",
                    "pins",
                    &file,
                    format!("`{label}`: {why} — re-read the page against the file"),
                ),
            }
        }
        // every `hash:` leaves the page, whatever the outcome: the commit
        // carries each page
        u.written.push(file);
        if !acks.is_empty() {
            u.written.push(format!("{}{}", ctx.prefix, crate::ack::DIR));
        }
        if !apply {
            continue;
        }
        if let Some(id) = id {
            for h in &acks {
                crate::ack::write(root, id, h).map_err(|e| e.to_string())?;
            }
        }
        let path = root.join(&page.rel);
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if let Some(new) = rewrite_pins(&text, &whole) {
            fs::write(&path, new).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// The steps of the move from docsys/0.4 to docsys/0.5 (`migrations/0.4-0.5.tsv`).
fn move_0_4_to_0_5(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
    let (repo, root, tree, kb) = (ctx.repo, ctx.root, &ctx.tree, ctx.kb);
    let prefix = ctx.prefix.as_str();
    pins(ctx, u, apply)?;

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
        u.written.push(file);
    }

    // ledgers: open items into their topics' files, after the separators are
    // ASCII (D-124)
    ledgers(ctx, u, apply)?;
    // journal: history from here on, the files frozen under _archive/ (D-125)
    journal(ctx, u, apply)?;
    // no page keeps a verification, after the pins step read `verifies:`
    // (D-130)
    no_verification(ctx, u, apply)?;

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

/// The upgrade commit's message: the move, the notes it crosses, and the
/// journal lines it carries. It also describes a pull request.
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
    if !u.carried.is_empty() {
        out.push_str("\n\nJournal entries a branch from before the move wrote (D-125):\n");
        out.push_str(&u.carried.join("\n"));
    }
    out
}

/// What the move left at `file`: the sha256 of its bytes, `-` where it
/// removed the file.
fn left_at(repo: &Path, file: &str) -> String {
    fs::read(repo.join(file)).map_or_else(|_| "-".to_string(), |b| crate::fresh::sha256_hex(&b))
}

/// The record of a written move, beside the index (R-177): each path the
/// move wrote and what it left there, one `path<TAB>sha256` line each; a
/// directory it wrote is recorded file by file.
pub fn record_text(repo: &Path, files: &[String]) -> String {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .collect();
        names.sort();
        for n in names {
            let (path, rel) = (dir.join(&n), format!("{rel}/{n}"));
            if path.is_dir() {
                walk(&path, &rel, out);
            } else {
                out.push(rel);
            }
        }
    }
    let mut paths = Vec::new();
    for f in files {
        if repo.join(f).is_dir() {
            walk(&repo.join(f), f, &mut paths);
        } else {
            paths.push(f.clone());
        }
    }
    paths.dedup();
    paths
        .iter()
        .map(|p| format!("{p}\t{}\n", left_at(repo, p)))
        .collect()
}

/// A record read back: each path, with what the move left there when the
/// record says it.
pub fn read_record(text: &str) -> Vec<(String, Option<String>)> {
    text.lines()
        .filter(|l| !l.is_empty())
        .map(|l| match l.split_once('\t') {
            Some((p, h)) => (p.to_string(), Some(h.to_string())),
            None => (l.to_string(), None),
        })
        .collect()
}

/// The recorded paths that no longer hold what the move left: each holds an
/// edit of the person's now.
pub fn edited_since(repo: &Path, record: &[(String, Option<String>)]) -> Vec<String> {
    record
        .iter()
        .filter(|(p, h)| h.as_ref().is_some_and(|h| *h != left_at(repo, p)))
        .map(|(p, _)| p.clone())
        .collect()
}

/// Whether a path the move writes is `file` or holds it.
pub fn covers(written: &str, file: &str) -> bool {
    file == written || file.starts_with(&format!("{}/", written.trim_end_matches('/')))
}

/// A record of a written move left in the git directory, its move committed
/// by hand or discarded since: named, as taken out.
pub fn record_taken_out(u: &mut Upgrade, record: &str) {
    u.items.insert(
        0,
        Item {
            strategy: "auto",
            step: "upgrade-record",
            file: record.to_string(),
            what: "the record of a move written by `--apply` alone, committed by hand or discarded since — taken out".to_string(),
            command: None,
        },
    );
}

/// Commit what the upgrade wrote as one commit (R-177), under the current
/// identity.
pub fn commit(repo: &Path, u: &Upgrade) -> Result<(), String> {
    commit_files(repo, &u.written, &message(u))
}

/// The move's own commit: the files it wrote, with its message. Nothing to
/// commit is no error — the move was committed already.
pub fn commit_files(repo: &Path, files: &[String], message: &str) -> Result<(), String> {
    if files.is_empty() {
        return Ok(());
    }
    let git_ok = |args: &[&str]| {
        crate::git::cmd(repo)
            .args(args)
            .output()
            .is_ok_and(|o| o.status.success())
    };
    let in_index = |f: &str| git_ok(&["ls-files", "--error-unmatch", "--", f]);
    // a path git ignores and does not track stays out: the person keeps it out
    let ignored = |f: &str| !in_index(f) && git_ok(&["check-ignore", "-q", "--", f]);
    let to_stage: Vec<&String> = files
        .iter()
        .filter(|f| (repo.join(f).exists() || in_index(f)) && !ignored(f))
        .collect();
    let added = to_stage.is_empty()
        || crate::git::cmd(repo)
            .args(["add", "-A", "--"])
            .args(&to_stage)
            .status()
            .is_ok_and(|s| s.success());
    if !added {
        return Err("git could not stage the files the move wrote".into());
    }
    // the move's paths and no other — a removal an earlier try staged is in
    // HEAD alone, and a file of the person's staged beside them stays staged
    let paths: Vec<&String> = files
        .iter()
        .filter(|f| {
            !ignored(f)
                && (repo.join(f).exists()
                    || in_index(f)
                    || git_ok(&["cat-file", "-e", &format!("HEAD:{f}")]))
        })
        .collect();
    if paths.is_empty() {
        return Ok(());
    }
    let staged = crate::git::cmd(repo)
        .args(["diff", "--cached", "--quiet", "--"])
        .args(&paths)
        .status()
        .is_ok_and(|s| !s.success());
    if !staged {
        return Ok(());
    }
    let ok = crate::git::cmd(repo)
        .args(["commit", "-q", "-m"])
        .arg(message)
        .arg("--")
        .args(&paths)
        .status()
        .is_ok_and(|s| s.success());
    if ok {
        Ok(())
    } else {
        Err("git refused the move's commit — git's output above says why".into())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// Only a pin's `hash:` leaves, and the `symbol:` of an entry pinned to
    /// its whole file; an entry's first line hands its marker on.
    #[test]
    fn a_rewritten_pin_entry_keeps_its_other_lines() {
        let page = "---\nid: p\nverifies:\n  - path: a.rs\n    symbol: f\n    hash: \"sha256:x\"\n  - symbol: g\n    path: b.rs\n    block: 1a2b\n    hash: \"sha256:y\"\nsources: []\n---\nbody\n    hash: stays\n";
        let want = "---\nid: p\nverifies:\n  - path: a.rs\n    symbol: f\n  - path: b.rs\n    block: 1a2b\nsources: []\n---\nbody\n    hash: stays\n";
        assert_eq!(
            rewrite_pins(page, &[("b.rs".into(), "g".into())]).as_deref(),
            Some(want)
        );
        assert_eq!(rewrite_pins(want, &[]), None);
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
            // the plan's header leaves to the note how the move is applied
            assert!(
                note(m.release).is_some_and(|n| n.contains("`docsys upgrade --apply")),
                "CHANGELOG [{}] has no Upgrading section naming `docsys upgrade --apply`",
                m.release
            );
        }
    }
}
