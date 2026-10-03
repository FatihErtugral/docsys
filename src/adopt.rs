//! `docsys adopt` — one-command adoption. Everything mechanical happens here,
//! idempotently; everything that needs judgment lands in ADOPTION.md as a
//! checklist an agent burns down in a single session. Adoption must not be a
//! conversation.

use crate::workflow::{self, Ci, Verify};
use crate::{agents, lint, refs, rules, tree::DocTree};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const GATE_MARKER: &str = "docsys documentation gate";
const GATE_END: &str = "# --- end of the docsys gate";

/// The git pre-commit block. Every check runs, every failure counts, and the
/// mode decides whether a failure stops the commit: an earlier block let
/// `docsys refs`' exit code mask a lint failure (no -e, the last command
/// decided) — found by the agent lab. `gate` lints the tree itself, so the
/// block does not run `lint` too and each finding is said once; a pinned tree
/// under a docsys from before pins is named in one line (D-120). A skipped
/// commit whose record cannot be written says so. The template stamp names a
/// block behind the binary.
const GATE_BLOCK: &str = r#"
# --- docsys documentation gate ---------------------------------------------
# docsys-template: @VERSION@
@MODE@
# One-off skip: DOCSYS_SKIP=1 git commit ... (under commit_policy: require it leaves a debt item)
docsys_gate_exit=@EXIT@
if [ -z "${DOCSYS_SKIP:-}" ] && command -v docsys >/dev/null; then
  docsys_gate_status=0
  docsys_pin=$(head -n 1 "@ROOT@/.docsys-version" 2>/dev/null)
  if [ -n "$docsys_pin" ] && ! docsys --version >/dev/null 2>&1; then
    echo "docsys: this tree pins docsys $docsys_pin; install: cargo install docsys --version $docsys_pin --locked" >&2
    docsys_gate_status=1
  else
    docsys gate --repo . --root @ROOT@ || docsys_gate_status=1
    docsys refs --repo . --root @ROOT@ || docsys_gate_status=1
  fi
  if [ "$docsys_gate_status" -ne 0 ] && [ "$docsys_gate_exit" -ne 0 ]; then exit 1; fi
elif [ -n "${DOCSYS_SKIP:-}" ] && command -v docsys >/dev/null; then
  docsys gate --repo . --root @ROOT@ --skipped >/dev/null || echo "docsys: this skipped commit is not recorded — the line above says why" >&2
fi
# --- end of the docsys gate ---
"#;

/// The git commit-msg block on a docsys/0.5 tree: the message is the journal
/// entry, and the one place a hook can read it (D-125). Under
/// `commit_policy: require`, code with no documentation needs `Docs: <why>`.
const MESSAGE_BLOCK: &str = r#"
# --- docsys documentation gate ---------------------------------------------
# docsys-template: @VERSION@
@MODE@
# The commit message is the journal entry: `docsys gate --message` reads it (D-125).
if [ -z "${DOCSYS_SKIP:-}" ] && command -v docsys >/dev/null; then
  docsys gate --repo . --root @ROOT@ --message "$1" || [ "@EXIT@" -eq 0 ] || exit 1
fi
# --- end of the docsys gate ---
"#;

fn message_block(root_rel: &str, hard: bool) -> String {
    MESSAGE_BLOCK
        .replace("@VERSION@", agents::TEMPLATE_VERSION)
        .replace("@MODE@", if hard { HARD_MODE_LINE } else { WARN_MODE_LINE })
        .replace("@EXIT@", if hard { "1" } else { "0" })
        .replace("@ROOT@", root_rel)
}

/// Whether a docsys block is in warn mode.
fn is_warn(block: &[&str]) -> bool {
    block.iter().any(|l| {
        l.contains(" || true") || l.trim() == "docsys_gate_exit=0" || l.contains("[ \"0\" -eq 0 ]")
    })
}

fn gate_block(root_rel: &str, hard: bool) -> String {
    GATE_BLOCK
        .replace("@VERSION@", agents::TEMPLATE_VERSION)
        .replace("@MODE@", if hard { HARD_MODE_LINE } else { WARN_MODE_LINE })
        .replace("@EXIT@", if hard { "1" } else { "0" })
        .replace("@ROOT@", root_rel)
}
const WARN_MODE_LINE: &str =
    "# Warn-mode until the adoption debt is triaged; `docsys adopt` hardens it once lint is clean.";
const HARD_MODE_LINE: &str = "# Hard gate: lint errors and dangling references stop the commit.";

/// `namespace:` in the tree's `.docmeta.yml` — the repository's directory
/// name as a local-id — written when absent, kept when present (D-075).
fn ensure_namespace(root: &Path, repo: &Path) -> String {
    let dm = root.join(".docmeta.yml");
    let Ok(text) = fs::read_to_string(&dm) else {
        return "docmeta unreadable".to_string();
    };
    if let Some(existing) = text
        .lines()
        .find_map(|l| l.strip_prefix("namespace:"))
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return format!("{existing} (kept)");
    }
    let name = repo
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "project".to_string());
    let ns = crate::consume::local_id_of(&name);
    let ns = if ns.is_empty() {
        "project".to_string()
    } else {
        ns
    };
    // Right after the required trio, so the owner's own lines stay where they
    // were — at the end, verbatim.
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let at = lines
        .iter()
        .rposition(|l| {
            l.starts_with("spec:")
                || l.starts_with("profile:")
                || l.starts_with("default_content_language:")
        })
        .map_or(lines.len(), |i| i + 1);
    lines.insert(at, format!("namespace: {ns}"));
    lines.insert(
        at,
        format!("# The name a consumer uses for this tree — `consume: [{ns}]` (D-075)."),
    );
    let mut out = lines.join("\n");
    out.push('\n');
    if fs::write(&dm, out).is_err() {
        return "write failed".to_string();
    }
    format!("{ns} (written)")
}

/// What adopt did about the CI workflow, and what the checklist needs to know.
struct CiOutcome {
    summary: String,
    /// the file holds a verify-on-approval job (D-105)
    verify_job: bool,
    /// and that job opens a follow-up pull request
    opens_pull_requests: bool,
}

/// `.github/workflows/docsys.yml` when the repository has a `.github/`: lint
/// and refs on the default branch and every pull request, the
/// code-without-docs question over a pull request's range (D-072), the
/// verification of a merged one's approvals (D-105), shaped by the flags
/// (D-111). Written once: an existing file is kept, whatever the flags say;
/// nothing where no GitHub layout exists.
fn ensure_ci_workflow(repo: &Path, root_rel: &str, ci: Option<&Ci>) -> CiOutcome {
    if !repo.join(".github").is_dir() {
        return CiOutcome {
            summary: "skipped (no .github/)".to_string(),
            verify_job: false,
            opens_pull_requests: false,
        };
    }
    let file = repo.join(workflow::PATH);
    if file.exists() {
        let existing = fs::read_to_string(&file).unwrap_or_default();
        let note = if ci.is_some() {
            " — the --ci-* and --verify-on-approval flags shape a new workflow only"
        } else {
            ""
        };
        return CiOutcome {
            summary: format!("kept{note}"),
            verify_job: existing.contains("\n  verify-on-approval:\n"),
            opens_pull_requests: existing.contains("gh pr create"),
        };
    }
    let ci = ci.cloned().unwrap_or_default();
    let verify = ci.verify;
    let text = workflow::render(&workflow::Workflow {
        version: agents::TEMPLATE_VERSION.to_string(),
        branch: workflow::default_branch(repo),
        root: root_rel.to_string(),
        ci,
    });
    let written = file
        .parent()
        .is_some_and(|dir| fs::create_dir_all(dir).is_ok())
        && fs::write(&file, text).is_ok();
    if !written {
        return CiOutcome {
            summary: "failed".to_string(),
            verify_job: false,
            opens_pull_requests: false,
        };
    }
    CiOutcome {
        summary: format!("written (verify-on-approval: {})", verify.name()),
        verify_job: verify != Verify::Off,
        opens_pull_requests: verify == Verify::PullRequest,
    }
}

#[derive(Debug)]
pub struct AdoptOutcome {
    /// empty under `--no-report`
    pub report_path: String,
    pub summary: Vec<String>,
    /// what adopt prints instead of writing: the rules block when git ignores
    /// both files that could hold it, the report under `--no-report`
    pub printed: Vec<String>,
}

/// Where adopt puts what it writes outside the tree (D-110), and the shape of
/// the workflow (D-111); paths are relative to the repository.
#[derive(Debug, Default, Clone)]
pub struct Placement {
    /// `--rules-file`: the file the docsys:rules block goes to
    pub rules_file: Option<PathBuf>,
    /// `--report-dir`: the directory ADOPTION.md goes to
    pub report_dir: Option<PathBuf>,
    /// `--no-report`: the report is printed, nothing is written
    pub no_report: bool,
    /// the workflow flags; `None` when none was given
    pub ci: Option<Ci>,
}

fn md_count(dir: &Path) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .map(|p| {
            if p.is_dir() {
                md_count(&p)
            } else {
                usize::from(p.extension().is_some_and(|x| x == "md"))
            }
        })
        .sum()
}

fn ensure_docmeta(root: &Path, lang: &str) -> Result<&'static str, String> {
    let path = root.join(".docmeta.yml");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    if existing.is_empty() {
        // No docmeta and no pages: greenfield — the full init skeleton
        // (router, journal, debt) beats a bare config file. No docmeta but
        // existing pages: what they are is judgment (R-003), so adopt stops
        // and names the ways on (D-110).
        let pages = md_count(root);
        if pages > 0 {
            let shown = root.display().to_string();
            let r = shown.strip_prefix("./").unwrap_or(&shown);
            let again = if r == "docs" {
                String::new()
            } else {
                format!(" --root {r}")
            };
            return Err(format!(
                "`{r}` holds {pages} page(s) and no .docmeta.yml — adopt does not choose \
                 what they are; classification is judgment (R-003). Three ways on:\n\
                 \x20 1. migrate them: `docsys migrate inventory --root {r} > plan.tsv`, classify \
                 each row, `docsys migrate apply --plan plan.tsv --root {r}`, then \
                 `docsys adopt{again}`\n\
                 \x20 2. keep them in place as an unmigrated tree: `docsys init --root {r}`, then \
                 `docsys adopt{again}` — lint reports every page outside the layout (D-016)\n\
                 \x20 3. put the tree elsewhere: `docsys adopt --root <dir>`"
            ));
        }
        crate::migrate::init(root, lang)?;
        return Ok("created via init (router, templates)");
    }
    // Append only the missing required keys; the owner's lines stay verbatim.
    let mut prefix = String::new();
    if !existing.lines().any(|l| l.starts_with("spec:")) {
        prefix.push_str(&format!("spec: docsys/{}\n", rules::spec_version()));
    }
    if !existing.lines().any(|l| l.starts_with("profile:")) {
        prefix.push_str("profile: project\n");
    }
    if !existing
        .lines()
        .any(|l| l.starts_with("default_content_language:"))
    {
        prefix.push_str(&format!("default_content_language: {lang}\n"));
    }
    if prefix.is_empty() {
        return Ok("kept");
    }
    fs::write(&path, format!("{prefix}{existing}")).map_err(|e| e.to_string())?;
    Ok("upgraded")
}

/// Append the gate to the repo's pre-commit hook (idempotent). Hard — lint
/// errors and dangling references stop the commit — when the tree lints clean
/// at adoption; warn-mode when it carries debt, because a gate that blocks
/// every commit on day one gets bypassed forever (R-150). A warn-mode gate is
/// hardened by a later `adopt` once the tree is clean (D-072).
/// The verdict the gate itself will reach: no lint error AND no refs error
/// (a dangling `doc:` in code blocks a commit just as a broken page does,
/// D-088). The gate's mode — hard or warn — follows this, not lint alone.
pub(crate) fn gate_clean(root: &Path, repo: &Path) -> bool {
    let (lint, _) = crate::lint_in(root, Some(repo));
    if lint
        .findings
        .iter()
        .any(|f| f.severity == crate::model::Severity::Error)
    {
        return false;
    }
    let Ok(tree) = crate::tree::DocTree::load(root) else {
        return false;
    };
    !crate::refs::run(repo, &tree)
        .findings
        .iter()
        .any(|f| f.severity == crate::model::Severity::Error)
}

/// Where the gate lives once adopt has run here: git's hooks directory, or a
/// tracked `.githooks/` that nothing configures yet — adopt points
/// `core.hooksPath` at it (below), so the gate written there is the one a
/// commit runs, and the one an upgrade commits.
pub(crate) fn gate_hooks_dir(repo: &Path) -> Option<std::path::PathBuf> {
    if tracked_hooks_unset(repo) {
        return Some(repo.join(".githooks"));
    }
    crate::git::hooks_dir(repo)
}

/// A tracked `.githooks/` that nothing points git at yet — a fresh clone of
/// a repository that keeps its gate there: git runs none of it.
pub(crate) fn tracked_hooks_unset(repo: &Path) -> bool {
    let configured = crate::git::cmd(repo)
        .args(["config", "--get", "core.hooksPath"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| !o.stdout.trim_ascii().is_empty());
    !configured && repo.join(".githooks").is_dir()
}

/// `message`: the tree reads its journal from history and its gate has a
/// commit-msg half (D-125) — the tree's era, or the era an upgrade moves to.
pub(crate) fn ensure_git_gate(
    repo: &Path,
    root_rel: &str,
    clean: bool,
    message: bool,
) -> &'static str {
    // a base that is its own repository names itself `.`
    let root_rel = if root_rel.is_empty() { "." } else { root_rel };
    // A tracked .githooks/ is the project's own convention: when nothing
    // configures hooksPath, adopt sets it, so the gate fires on a fresh clone
    // exactly as the project's own setup step would. Then git says where its
    // hooks live (D-100) — hooksPath honoured, a worktree's being its common
    // directory's; a config-file text parse once missed another scope.
    let configured = crate::git::cmd(repo)
        .args(["config", "--get", "core.hooksPath"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| !o.stdout.trim_ascii().is_empty());
    if !configured && repo.join(".githooks").is_dir() {
        let _ = crate::git::cmd(repo)
            .args(["config", "core.hooksPath", ".githooks"])
            .status();
    }
    let Some(hooks_dir) = crate::git::hooks_dir(repo) else {
        return "failed";
    };
    let pre = ensure_block(&hooks_dir, "pre-commit", clean, &|hard| {
        gate_block(root_rel, hard)
    });
    if pre == "failed" || !message {
        return pre;
    }
    // the commit-msg block takes the pre-commit block's mode
    let pre_text = fs::read_to_string(hooks_dir.join("pre-commit")).unwrap_or_default();
    let pre_lines: Vec<&str> = pre_text.lines().collect();
    let hard = gate_span(&pre_lines)
        .and_then(|(s, e)| pre_lines.get(s..=e))
        .is_some_and(|b| !is_warn(b));
    match ensure_block(&hooks_dir, "commit-msg", hard, &|h| {
        message_block(root_rel, h)
    }) {
        "failed" => "failed",
        "written" | "upgraded" | "hardened" if pre == "kept" => "upgraded",
        _ => pre,
    }
}

/// Write the docsys block into one git hook (idempotent): rewritten when it
/// is not the binary's own, warn-mode hardened once `clean`, a hard block
/// kept hard, a new block placed right below the shebang.
fn ensure_block(
    hooks_dir: &Path,
    name: &str,
    clean: bool,
    render: &dyn Fn(bool) -> String,
) -> &'static str {
    let hook = hooks_dir.join(name);
    let existing = fs::read_to_string(&hook).unwrap_or_default();
    if existing.contains(GATE_MARKER) {
        // The block in place is rewritten when it is not the binary's own:
        // behind its template, or warn-mode on a tree that is now clean
        // (D-072). A hard gate stays hard.
        let lines: Vec<&str> = existing.lines().collect();
        let Some((s0, e0)) = gate_span(&lines) else {
            return "kept";
        };
        let old_block = lines.get(s0..=e0).unwrap_or(&[]);
        let was_warn = is_warn(old_block);
        let hard = clean || !was_warn;
        let fresh = render(hard);
        let fresh_lines: Vec<&str> = fresh.trim_matches('\n').lines().collect();
        if old_block == fresh_lines.as_slice() {
            return "kept";
        }
        let mut out: Vec<&str> = lines.get(..s0).unwrap_or(&[]).to_vec();
        out.extend(fresh_lines);
        out.extend(lines.get(e0 + 1..).unwrap_or(&[]));
        let mut text = out.join("\n");
        text.push('\n');
        return match (fs::write(&hook, text), was_warn && hard) {
            (Err(_), _) => "failed",
            (Ok(()), true) => "hardened",
            (Ok(()), false) => "upgraded",
        };
    }
    let block = render(clean);
    // The block goes right below the shebang, never at the end: an existing
    // hook usually ends in `exec` or `exit`, and a block appended below either
    // is dead code that looks installed — found live, twice (doctor's check).
    let text = if existing.is_empty() {
        format!("#!/usr/bin/env bash\nset -uo pipefail\n{block}")
    } else {
        let mut lines: Vec<&str> = existing.lines().collect();
        let mut at = 0usize;
        if lines.first().is_some_and(|l| l.starts_with("#!")) {
            at = 1;
            if lines
                .get(1)
                .is_some_and(|l| l.trim_start().starts_with("set "))
            {
                at = 2;
            }
        }
        lines.insert(at, &block);
        let mut t = lines.join("\n");
        t.push('\n');
        t
    };
    if fs::create_dir_all(hooks_dir).is_err() {
        return "failed";
    }
    if fs::write(&hook, text).is_err() {
        return "failed";
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&hook, fs::Permissions::from_mode(0o755));
    }
    "written"
}

pub fn run(repo: &Path, root: &Path, lang: &str) -> Result<AdoptOutcome, String> {
    run_placed(repo, root, lang, &Placement::default())
}

pub fn run_placed(
    repo: &Path,
    root: &Path,
    lang: &str,
    place: &Placement,
) -> Result<AdoptOutcome, String> {
    // A knowledge-base tree is lintable (0.3) but its adoption flow — id
    // backfill, legacy-checker delegation — is its own release. Refusing
    // beats half-adopting (the D-006 doctrine).
    let dm = fs::read_to_string(root.join(".docmeta.yml")).unwrap_or_default();
    if dm
        .lines()
        .any(|l| l.trim_start().starts_with("profile:") && l.contains("knowledge-base"))
    {
        return Err(
            "this is a knowledge-base tree — lint and refs already understand it; \
             `adopt` support for the profile lands with the knowledge-base adoption release"
                .to_string(),
        );
    }
    let mut summary = Vec::new();

    // 1 · configuration
    let dm = ensure_docmeta(root, lang)?;
    summary.push(format!(".docmeta.yml: {dm}"));
    // the tree as the repository names it, now that it exists — relative
    // always, so no relay or gate carries an absolute path (D-099)
    let root_rel = match crate::fresh::root_rel(repo, root) {
        r if r.is_empty() => ".".to_string(),
        r => r,
    };
    // 1b · the promised skeleton pieces an adopted tree usually lacks
    // (R-048 templates, the questions ledger) — written only when absent.
    let scaffolded = crate::migrate::scaffold_list_files_and_templates(root)?;
    if !scaffolded.is_empty() {
        summary.push(format!("scaffold: {} written", scaffolded.join(", ")));
    }
    // 1c · the tree's own name for any consumer (D-075) — in its docmeta,
    // once; never anywhere outside the repository
    summary.push(format!("namespace: {}", ensure_namespace(root, repo)));

    // 2 · agent layer (never-colliding names; existing files skipped)
    let claude = repo.join(".claude");
    let installed = agents::install_with_preamble(
        &claude,
        false,
        &crate::migrate::generated_preamble(root),
        &root_rel,
    )?;
    summary.push(format!(
        "agent assets: {} written, {} already present",
        installed.written.len(),
        installed.skipped.len()
    ));
    // Kept hooks may be behind the binary's templates — adopt never
    // overwrites, so it must at least say so (D-047).
    let stale = agents::stale_hooks(&claude);
    if !stale.is_empty() {
        let list: Vec<String> = stale.iter().map(|(r, v)| format!("{r}: {v}")).collect();
        summary.push(format!(
            "hooks: {} template(s) behind {} ({}) — run `docsys agents --force`",
            stale.len(),
            agents::TEMPLATE_VERSION,
            list.join(", ")
        ));
    }

    // 2b · settings.json: written whole when absent, merged when present
    // (D-086) — the docsys wires join the file, MCP servers, permissions and
    // the owner's own hooks stay as they are. Only a file that is not JSON is
    // left alone, and then the merge goes to the checklist.
    let settings = claude.join("settings.json");
    let settings_unparsable = match agents::wire_settings(&settings, agents::SETTINGS_SNIPPET)? {
        agents::Wired::Created => {
            summary.push("settings.json: created with docsys hook wires".to_string());
            false
        }
        agents::Wired::Merged(n) => {
            summary.push(format!(
                "settings.json: merged {n} docsys hook wire(s) into the existing file (MCP/permissions kept)"
            ));
            false
        }
        agents::Wired::AlreadyWired => {
            summary.push("settings.json: already wired".to_string());
            false
        }
        agents::Wired::Unparsable => {
            summary.push(
                "settings.json: not valid JSON — untouched; merge is on the checklist".to_string(),
            );
            true
        }
    };

    // 3 · the managed rules block (idempotent), in a file the repository
    // keeps (D-110)
    let mut printed = Vec::new();
    let preamble = crate::migrate::generated_preamble(root);
    let rules_printed = match rules_target(repo, place.rules_file.as_deref()) {
        Some(file) => {
            if let Some(dir) = file.parent() {
                fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            rules::write_agents_block_with(&file, &preamble)?;
            let shown = file.strip_prefix(repo).unwrap_or(&file);
            summary.push(format!(
                "{}: managed block written",
                shown.to_string_lossy().replace('\\', "/")
            ));
            false
        }
        None => {
            summary.push(
                "rules block: git ignores AGENTS.md and CLAUDE.md — printed below, not written"
                    .to_string(),
            );
            printed.push(rules::agents_block_with(&preamble));
            true
        }
    };

    // 4 · git pre-commit gate — hard when the tree is clean as the hook will
    // see it (inside the repository: pins and history included), warn-mode
    // while it carries debt (D-072)
    let clean = gate_clean(root, repo);
    let message = crate::era::Era::at(root).journal_from_history();
    let gate = ensure_git_gate(repo, &root_rel, clean, message);
    let mode = if clean {
        "hard"
    } else {
        "warn-mode until lint and refs are clean"
    };
    summary.push(format!("git pre-commit gate: {gate} ({mode})"));

    // 4b · CI: the same questions on the default branch and every pull request
    let ci = ensure_ci_workflow(repo, &root_rel, place.ci.as_ref());
    summary.push(format!(
        "ci workflow (.github/workflows/docsys.yml): {}",
        ci.summary
    ));

    // 5 · evidence: current findings + the existing layer
    let (lint_report, _) = lint(root);
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    let refs_report = refs::run(repo, &tree);
    let layer = agents::adoption_report(&claude);

    let count = |r: &crate::checks::Report, sev: crate::model::Severity| {
        r.findings.iter().filter(|f| f.severity == sev).count()
    };
    use crate::model::Severity::{Error, Warn};

    // 6 · the checklist — judgment goes here, not into a conversation.
    // The adoption's own record survives every later run (D-097): the first
    // `## Done` block is kept as written, and a re-run reports itself under
    // `## Last run` — "created" must not turn into "kept" in the only file
    // that says what adoption did.
    let today = crate::migrate::today();
    let report_path = report_target(repo, place.report_dir.as_deref());
    let existing_report = fs::read_to_string(&report_path).unwrap_or_default();
    let adoption_block = previous_adoption_block(&existing_report, root);
    let mut md = String::from(
        "# docsys adoption report\n\nGenerated by `docsys adopt`. \
        The mechanical steps below are DONE and idempotent; the checklist at the end \
        is the judgment work — burn it down in one agent session.\n\n",
    );
    match adoption_block {
        Some(block) => {
            md.push_str(&block);
            let _ = write!(md, "\n## Last run — {today}\n\n");
            for s in &summary {
                let _ = writeln!(md, "- {s}");
            }
        }
        None => {
            let _ = write!(md, "## Done — adoption ({today})\n\n");
            for s in &summary {
                let _ = writeln!(md, "- {s}");
            }
        }
    }
    let _ = write!(
        md,
        "\n## Current findings\n\n\
         | gate | errors | warnings |\n|---|---|---|\n\
         | `docsys lint` | {} | {} |\n| `docsys refs` | {} | {} |\n",
        count(&lint_report, Error),
        count(&lint_report, Warn),
        count(&refs_report, Error),
        count(&refs_report, Warn)
    );
    md.push_str("\n## Existing agent layer (mechanical inventory)\n\n");
    if layer.is_empty() {
        md.push_str("- none found\n");
    }
    for line in &layer {
        let _ = writeln!(md, "- {line}");
    }
    md.push_str(
        "\n## Judgment checklist (agent work, human approval)\n\n\
         - [ ] For every layer file above that *invokes* a legacy docs tool: keep the\n\
         \x20     owner's prose, repoint the mechanical call to the matching docsys\n\
         \x20     command, or retire the file to a `*-retired/` directory. Never delete.\n\
         - [ ] Rules/skills that RESTATE what docsys now enforces retire; rules that\n\
         \x20     carry project-specific conventions (roadmaps, catalogs, protocols)\n\
         \x20     split into `.claude/rules/doc-extensions.md` — the conventional,\n\
         \x20     language-neutral home for project doc contracts layered on docsys\n\
         \x20     (the file's content stays in the project's language).\n\
         - [ ] Triage the error findings: dangling references are usually decisions\n\
         \x20     cited but never distilled — graduate them (`docsys graduate plan`).\n\
         - [ ] When errors reach zero, run `docsys adopt` again: the pre-commit gate\n\
         \x20     hardens by itself (lint errors then stop the commit).\n\
         \n\
         An existing project documents itself in this order:\n\n\
         1. [ ] Collect what exists: what code and history say, per feature, with\n\
         \x20      `/docsys-seed <feature>`; what only people know with `/docsys-interview`.\n\
         2. [ ] Write pages by type — reference, howto, explanation — with\n\
         \x20      `docsys page new <type> <id> --unverified`.\n\
         3. [ ] Bind each page about code to the region it promises about with\n\
         \x20      `docsys pin`.\n\
         4. [ ] Name the maintainers in `.docmeta.yml` (`maintainers:`) and keep the CI\n\
         \x20      workflow green.\n\
         5. [ ] Start the verify flow: a maintainer reads each page against its\n\
         \x20      sources and records it with `docsys verify <page>`.\n",
    );
    if ci.summary.starts_with("skipped") {
        md.push_str(
            "- [ ] No `.github/` here: run `docsys lint --root <root> --repo .`, `docsys refs`\n\
             \x20     and `docsys gate --range <base>...HEAD` in the CI you have; `docsys adopt`\n\
             \x20     writes the GitHub workflow once `.github/` exists.\n",
        );
    }
    if ci.opens_pull_requests {
        md.push_str(
            "- [ ] The verify-on-approval job opens a follow-up pull request with the\n\
             \x20     records. Turn on Settings > Actions > General > Workflow permissions >\n\
             \x20     \"Allow GitHub Actions to create and approve pull requests\", or delete the\n\
             \x20     workflow and run `docsys adopt --verify-on-approval direct` (or `off`).\n",
        );
    }
    if ci.verify_job {
        let without: Vec<String> = crate::checks::maintainer_handles(&tree)
            .into_iter()
            .filter(|m| m.login.is_none())
            .map(|m| m.handle)
            .collect();
        if !without.is_empty() {
            let _ = write!(
                md,
                "- [ ] The verify-on-approval job knows a maintainer by the `@login` a host\n\
                 \x20     approval carries, and these `maintainers:` entries carry no `@login`: {}.\n\
                 \x20     Write each as `handle <email> @login` in .docmeta.yml.\n",
                without.join(", ")
            );
        }
    }
    if rules_printed {
        md.push_str(
            "- [ ] Git ignores both `AGENTS.md` and `CLAUDE.md`, so `docsys adopt` printed\n\
             \x20     the docsys:rules block instead of writing it: put it in a file your\n\
             \x20     agents load, or run `docsys adopt --rules-file <path>`.\n",
        );
    }
    if settings_unparsable {
        md.push_str(
            "- [ ] Merge the docsys hook wires into `.claude/settings.json` by hand or\n\
             \x20     agent (the file is not valid JSON, so the tool did not touch it). Snippet:\n\n\
             ```json\n",
        );
        md.push_str(agents::SETTINGS_SNIPPET);
        md.push_str("\n```\n");
    }

    // The report is regenerated, but a leading comment block on the existing
    // file is the owner's — a privacy classifier's marker, a review note —
    // and survives the rewrite (D-046).
    let (text, note) = managed_report_with(&existing_report, &md, &preamble);
    if let Some(n) = note {
        summary.push(n);
    }
    let text = crate::migrate::with_preamble(&text, &preamble);
    if place.no_report {
        printed.push(text);
        return Ok(AdoptOutcome {
            report_path: String::new(),
            summary,
            printed,
        });
    }
    if let Some(dir) = report_path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(&report_path, text).map_err(|e| e.to_string())?;

    Ok(AdoptOutcome {
        report_path: report_path.to_string_lossy().to_string(),
        summary,
        printed,
    })
}

/// The files git knows of under `repo` that match `pathspecs`, each with
/// whether it is tracked: tracked, untracked and ignored ones alike. Empty
/// outside a repository.
fn known_files(repo: &Path, pathspecs: &[&str]) -> Vec<(String, bool)> {
    let list = |args: &[&str]| -> Vec<String> {
        crate::git::cmd(repo)
            .args(["ls-files", "-z"])
            .args(args)
            .arg("--")
            .args(pathspecs)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .split('\0')
                    .filter(|p| !p.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut out: Vec<(String, bool)> = Vec::new();
    for (args, tracked) in [
        (&["-c"][..], true),
        (&["-o", "--exclude-standard"][..], false),
        (&["-o", "-i", "--exclude-standard"][..], false),
    ] {
        for p in list(args) {
            if !out.iter().any(|(q, _)| *q == p) {
                out.push((p, tracked));
            }
        }
    }
    out
}

fn git_ignores(repo: &Path, rel: &str) -> bool {
    crate::git::cmd(repo)
        .args(["check-ignore", "-q", rel])
        .output()
        .is_ok_and(|o| o.status.success())
}

const RULES_FILES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];

/// Where the docsys:rules block goes (D-110): the named file; else where its
/// markers already are — a tracked file first, then a root one, AGENTS.md
/// before CLAUDE.md — updated in place, never moved; else AGENTS.md, or
/// CLAUDE.md when git ignores AGENTS.md. `None` when git ignores both.
/// The file that holds the rules block, when one does.
pub fn rules_block_holder(repo: &Path) -> Option<PathBuf> {
    rules_target(repo, None)
        .filter(|p| fs::read_to_string(p).is_ok_and(|t| t.contains(crate::rules::BLOCK_BEGIN)))
}

pub(crate) fn rules_target(repo: &Path, named: Option<&Path>) -> Option<PathBuf> {
    if let Some(file) = named {
        return Some(repo.join(file));
    }
    let mut known = known_files(repo, &["*AGENTS.md", "*CLAUDE.md"]);
    for name in RULES_FILES {
        if !known.iter().any(|(p, _)| p == name) {
            known.push((name.to_string(), false));
        }
    }
    let mut holding: Vec<(bool, bool, usize, String)> = known
        .into_iter()
        .filter_map(|(rel, tracked)| {
            let base = rel.rsplit('/').next().unwrap_or(&rel);
            let rank = RULES_FILES.iter().position(|n| *n == base)?;
            fs::read_to_string(repo.join(&rel))
                .ok()?
                .contains(rules::BLOCK_BEGIN)
                .then(|| (!tracked, rel.contains('/'), rank, rel))
        })
        .collect();
    holding.sort();
    if let Some((.., rel)) = holding.into_iter().next() {
        return Some(repo.join(rel));
    }
    RULES_FILES
        .into_iter()
        .find(|name| !git_ignores(repo, name))
        .map(|name| repo.join(name))
}

/// Where ADOPTION.md goes (D-110): into `--report-dir`; else to the one git
/// knows of — tracked, untracked or ignored, so a report moved by hand or
/// kept out of git is found again — that holds the managed block; else to
/// the repository's root.
fn report_target(repo: &Path, dir: Option<&Path>) -> PathBuf {
    if let Some(dir) = dir {
        return repo.join(dir).join("ADOPTION.md");
    }
    let at_root = repo.join("ADOPTION.md");
    let holds = |p: &Path| fs::read_to_string(p).is_ok_and(|t| t.contains(REPORT_BEGIN));
    if holds(&at_root) {
        return at_root;
    }
    let mut known = known_files(repo, &["*ADOPTION.md"]);
    known.retain(|(rel, _)| rel.rsplit('/').next() == Some("ADOPTION.md"));
    known.sort_by_key(|(rel, tracked)| (!tracked, rel.matches('/').count(), rel.clone()));
    known
        .into_iter()
        .map(|(rel, _)| repo.join(rel))
        .find(|p| holds(p))
        .unwrap_or(at_root)
}

/// The first run's `## Done` block inside an existing managed report — kept
/// verbatim on every later run (D-097). Only a block inside the managed
/// markers counts: an unmarked legacy report is preserved whole by
/// `managed_report_with`, and importing its lines too would duplicate them.
fn previous_adoption_block(existing: &str, root: &Path) -> Option<String> {
    let (b, e) = (existing.find(REPORT_BEGIN)?, existing.find(REPORT_END)?);
    let managed = existing.get(b..e)?;
    let start = managed.find("\n## Done")? + 1;
    let after = managed.get(start..)?;
    let heading_end = after.find('\n')?;
    let heading = after.get(..heading_end)?;
    let body_start = heading_end + 1;
    let body = after.get(body_start..)?;
    let body_end = body.find("\n## ").unwrap_or(body.len());
    let body = body.get(..body_end)?.trim_end_matches('\n');
    let heading = if heading.contains("— adoption") {
        heading.to_string()
    } else {
        // a report written before D-097: the tree's own creation date, else no date
        let created = fs::read_to_string(root.join(".docmeta.yml"))
            .ok()
            .and_then(|t| {
                t.lines()
                    .find_map(|l| l.strip_prefix("created:").map(|v| v.trim().to_string()))
            });
        match created {
            Some(d) if !d.is_empty() => format!("## Done — adoption ({d})"),
            _ => "## Done — adoption".to_string(),
        }
    };
    Some(format!("{heading}\n{body}\n"))
}

/// The leading HTML-comment block of a generated file (with the blank lines
/// inside it), ending with a newline; empty when the file does not start with
/// a comment. Everything after the block is generator territory.
pub fn preserved_header(existing: &str) -> String {
    let mut out = String::new();
    let mut in_comment = false;
    for line in existing.lines() {
        let t = line.trim();
        if in_comment {
            out.push_str(line);
            out.push('\n');
            if t.contains("-->") {
                in_comment = false;
            }
            continue;
        }
        if t.starts_with("<!--") {
            out.push_str(line);
            out.push('\n');
            in_comment = !t.contains("-->");
        } else if t.is_empty() && !out.is_empty() {
            out.push('\n');
        } else {
            break;
        }
    }
    if out.trim().is_empty() {
        String::new()
    } else {
        // the block, then one blank line before the generated body
        format!("{}\n\n", out.trim_end())
    }
}

pub const REPORT_BEGIN: &str = "<!-- docsys:adoption:begin — generated, do not edit inside -->";
pub const REPORT_END: &str = "<!-- docsys:adoption:end -->";

/// The report lives in a managed block; everything outside it is the owner's
/// and survives every regeneration (R-045, D-057). An existing file without
/// the markers — written by an earlier version wholesale — is kept verbatim
/// below the new block rather than overwritten: nothing authored is lost,
/// and the note names what to trim. Returns the new text and a summary note.
pub fn managed_report(existing: &str, report: &str) -> (String, Option<String>) {
    managed_report_with(existing, report, "")
}

/// `managed_report`, with the owner's preamble (D-056) as the first line
/// inside the block, so every regeneration's diff carries it.
pub fn managed_report_with(
    existing: &str,
    report: &str,
    preamble: &str,
) -> (String, Option<String>) {
    let block = format!(
        "{REPORT_BEGIN}\n{preamble}{}\n{REPORT_END}\n",
        report.trim_end()
    );
    if existing.is_empty() {
        return (block, None);
    }
    if let (Some(b), Some(e)) = (existing.find(REPORT_BEGIN), existing.find(REPORT_END)) {
        if b < e {
            let head = existing.get(..b).unwrap_or("");
            let tail = existing
                .get(e + REPORT_END.len()..)
                .unwrap_or("")
                .trim_start_matches('\n');
            let tail = if tail.is_empty() {
                String::new()
            } else {
                format!("\n{tail}")
            };
            return (format!("{head}{block}{tail}"), None);
        }
    }
    let header = preserved_header(existing);
    let rest = existing
        .get(header.len()..)
        .unwrap_or(existing)
        .trim_start_matches('\n');
    let text = format!(
        "{header}{block}\n<!-- docsys: the previous ADOPTION.md, written before the report had a managed block, is kept verbatim below (R-045); trim what the block above now covers -->\n\n{rest}"
    );
    (
        text,
        Some(
            "ADOPTION.md: previous unmarked report kept verbatim below the managed block — trim it"
                .to_string(),
        ),
    )
}

/// `adopt --obsidian` (D-065): the three settings that let a docs root open
/// as an Obsidian vault without breaking a docsys rule — absolute link
/// format (R-070's full paths), `_archive/` and `.federation/` out of
/// search and graph, `_templates/` as the templates folder — plus one
/// `.base` view for stale work. Written only when absent.
pub fn obsidian(root: &Path) -> Result<Vec<String>, String> {
    let mut written = Vec::new();
    let dir = root.join(".obsidian");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let files: [(&str, &str); 3] = [
        (
            ".obsidian/app.json",
            "{\n  \"newLinkFormat\": \"absolute\",\n  \"useMarkdownLinks\": false,\n  \"userIgnoreFilters\": [\"_archive/\", \".federation/\"],\n  \"showUnsupportedFiles\": true\n}\n",
        ),
        (".obsidian/templates.json", "{\n  \"folder\": \"_templates\"\n}\n"),
        (
            "_templates/stale-work.base",
            "# Obsidian Bases view (1.9+): open work, oldest `updated` first — the\n# stale-work dashboard R-085 describes. Move or copy it anywhere in the vault.\nfilters:\n  and:\n    - file.inFolder(\"work\")\n    - status == \"active\"\nviews:\n  - type: table\n    name: Stale work\n    order:\n      - file.name\n      - status\n      - updated\n    sort:\n      - property: updated\n        direction: ASC\n",
        ),
    ];
    for (rel, text) in files {
        let path = root.join(rel);
        if path.exists() {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&path, text).map_err(|e| e.to_string())?;
        written.push(rel);
    }
    Ok(written.into_iter().map(str::to_string).collect())
}

/// The first and last line of the docsys block in a pre-commit hook: from the
/// marker line to the end line — or, for a block from before the end line, to
/// its outer `fi`.
fn gate_span(lines: &[&str]) -> Option<(usize, usize)> {
    let start = lines
        .iter()
        .position(|l| l.contains(GATE_MARKER) && l.starts_with("# ---"))?;
    let rest = lines.iter().skip(start);
    let end = rest
        .clone()
        .position(|l| l.starts_with(GATE_END))
        .or_else(|| rest.clone().position(|l| l.trim() == "fi"))?;
    Some((start, start + end))
}

/// Whether the docsys block in the repository's pre-commit hook is the one this
/// binary writes, its mode kept — `None` when there is no block. For `docsys
/// upgrade`'s plan, which rewrites a block behind the binary and never changes
/// its mode.
pub(crate) fn gate_current(repo: &Path, root_rel: &str, message: bool) -> Option<bool> {
    let root_rel = if root_rel.is_empty() { "." } else { root_rel };
    let hook = gate_hooks_dir(repo)?.join("pre-commit");
    let existing = fs::read_to_string(hook).ok()?;
    let lines: Vec<&str> = existing.lines().collect();
    let (s0, e0) = gate_span(&lines)?;
    let old = lines.get(s0..=e0)?;
    let was_warn = is_warn(old);
    let fresh = gate_block(root_rel, !was_warn);
    let fresh_lines: Vec<&str> = fresh.trim_matches('\n').lines().collect();
    if old != fresh_lines.as_slice() {
        return Some(false);
    }
    // a docsys/0.5 tree's gate has its commit-msg half too (D-125)
    if !message {
        return Some(true);
    }
    let msg = fs::read_to_string(gate_hooks_dir(repo)?.join("commit-msg")).unwrap_or_default();
    let mlines: Vec<&str> = msg.lines().collect();
    let fresh = message_block(root_rel, !was_warn);
    let fresh_lines: Vec<&str> = fresh.trim_matches('\n').lines().collect();
    Some(
        gate_span(&mlines)
            .and_then(|(s, e)| mlines.get(s..=e))
            .is_some_and(|b| b == fresh_lines.as_slice()),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::preserved_header;

    #[test]
    fn no_header_when_the_file_does_not_start_with_a_comment() {
        assert_eq!(preserved_header(""), "");
        assert_eq!(
            preserved_header("# docsys adoption report\n<!-- late -->\n"),
            ""
        );
        assert_eq!(preserved_header("\n\n# report\n"), "");
    }

    #[test]
    fn one_line_comment_is_kept_with_one_blank_line_after_it() {
        let h = preserved_header("<!-- restricted-context:public -->\n# report\nbody\n");
        assert_eq!(h, "<!-- restricted-context:public -->\n\n");
    }

    #[test]
    fn multi_line_and_stacked_comments_are_kept_whole() {
        let src = "<!-- a -->\n<!-- reviewed:\n     2026-08-26 -->\n\n<!-- b -->\n\n# report\n";
        let h = preserved_header(src);
        assert_eq!(
            h,
            "<!-- a -->\n<!-- reviewed:\n     2026-08-26 -->\n\n<!-- b -->\n\n"
        );
        // idempotent: header of (header + body) is the header
        assert_eq!(preserved_header(&format!("{h}# report\n")), h);
    }

    #[test]
    fn leading_whitespace_on_the_comment_line_is_tolerated() {
        assert_eq!(preserved_header("  <!-- x -->\ntext"), "  <!-- x -->\n\n");
    }

    #[test]
    fn managed_report_keeps_everything_outside_its_block() {
        use super::{managed_report, REPORT_BEGIN, REPORT_END};
        let (first, note) = managed_report("", "# report v1\n\n- a");
        assert!(note.is_none());
        assert!(first.starts_with(REPORT_BEGIN) && first.trim_end().ends_with(REPORT_END));
        let authored = format!("<!-- mine -->\n{first}\n## Closing note\n\nkept.\n");
        let (second, note) = managed_report(&authored, "# report v2");
        assert!(note.is_none());
        assert!(second.starts_with("<!-- mine -->\n"), "{second}");
        assert!(
            second.contains("# report v2") && !second.contains("# report v1"),
            "{second}"
        );
        assert!(second.ends_with("## Closing note\n\nkept.\n"), "{second}");
        assert_eq!(second.matches(REPORT_BEGIN).count(), 1);
        let legacy = "<!-- restricted-context:public -->\n# docsys adoption report\n\n## Done\n- x\n\n## Triage — 2026-08-26\n| a | b |\n";
        let (third, note) = managed_report(legacy, "# report v3");
        assert!(note.unwrap().contains("kept verbatim"));
        assert!(
            third.starts_with("<!-- restricted-context:public -->\n\n"),
            "{third}"
        );
        assert!(third.contains("# report v3"), "{third}");
        assert!(
            third.contains("## Triage — 2026-08-26\n| a | b |"),
            "{third}"
        );
        assert!(third.contains("kept verbatim below (R-045)"), "{third}");
    }

    #[test]
    fn an_unterminated_comment_swallows_to_end_of_file() {
        let h = preserved_header("<!-- open\nstill\n");
        assert_eq!(h, "<!-- open\nstill\n\n");
    }
}
