//! `docsys adopt` — one-command adoption. Everything mechanical happens here,
//! idempotently; everything that needs judgment lands in ADOPTION.md as a
//! checklist an agent burns down in a single session. Adoption must not be a
//! conversation.

use crate::{agents, lint, refs, rules, tree::DocTree};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

const GATE_MARKER: &str = "docsys documentation gate";
const WARN_MODE_LINE: &str =
    "# Warn-mode until the adoption debt is triaged; `docsys adopt` hardens it once lint is clean.";
const HARD_MODE_LINE: &str = "# Hard gate: lint errors and dangling references stop the commit.";
const CI_MARKER: &str = "docsys documentation workflow";

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

/// `.github/workflows/docsys.yml` when the repository has a `.github/`: lint
/// and refs on every push, the code-without-docs question over a pull
/// request's range (D-072). Written once, never regenerated — it is the
/// project's file after that; nothing where no GitHub layout exists.
fn ensure_ci_workflow(repo: &Path, root_rel: &str) -> &'static str {
    if !repo.join(".github").is_dir() {
        return "skipped (no .github/)";
    }
    let dir = repo.join(".github/workflows");
    let file = dir.join("docsys.yml");
    if file.exists() {
        return "kept";
    }
    let text = format!(
        "# {CI_MARKER} — written by `docsys adopt`; edit freely, it is not regenerated.\n\
         name: docsys\n\n\
         on:\n  push:\n  pull_request:\n    types: [opened, synchronize, reopened, closed]\n\n\
         jobs:\n  docs:\n    runs-on: ubuntu-latest\n    steps:\n\
         \x20     - uses: actions/checkout@v5\n\
         \x20       with:\n\
         \x20         fetch-depth: 0\n\
         \x20     - run: cargo install docsys\n\
         \x20     - run: docsys lint --root {root_rel} --repo .\n\
         \x20     - run: docsys refs --repo . --root {root_rel}\n\
         \x20     - if: github.event_name == 'pull_request'\n\
         \x20       run: docsys gate --repo . --root {root_rel} --range \"origin/${{{{ github.base_ref }}}}...HEAD\"\n\
         \n\
         \x20 # A code review's approval is a maintainer's word (D-095): when a pull request\n\
         \x20 # merges, every page it touched that carries `verification:` is recorded as\n\
         \x20 # verified by each approver whose @login is in .docmeta.yml maintainers:, in a\n\
         \x20 # commit under that approver's identity. Approvers outside the list are skipped.\n\
         \x20 verify-on-approval:\n\
         \x20   if: github.event_name == 'pull_request' && github.event.pull_request.merged == true\n\
         \x20   runs-on: ubuntu-latest\n\
         \x20   permissions:\n\
         \x20     contents: write\n\
         \x20     pull-requests: read\n\
         \x20   steps:\n\
         \x20     - uses: actions/checkout@v5\n\
         \x20       with:\n\
         \x20         ref: ${{{{ github.event.pull_request.base.ref }}}}\n\
         \x20         fetch-depth: 0\n\
         \x20     - run: cargo install docsys\n\
         \x20     - env:\n\
         \x20         GH_TOKEN: ${{{{ github.token }}}}\n\
         \x20       run: |\n\
         \x20         range=\"${{{{ github.event.pull_request.base.sha }}}}...${{{{ github.sha }}}}\"\n\
         \x20         for login in $(gh api \"repos/${{{{ github.repository }}}}/pulls/${{{{ github.event.pull_request.number }}}}/reviews\" --jq '[.[] | select(.state == \"APPROVED\") | .user.login] | unique | .[]'); do\n\
         \x20           docsys verify --range \"$range\" --by \"@$login\" --commit --root {root_rel} || echo \"@$login: not a declared maintainer — skipped\"\n\
         \x20         done\n\
         \x20         git push\n"
    );
    if fs::create_dir_all(&dir).is_err() || fs::write(&file, text).is_err() {
        return "failed";
    }
    "written"
}

#[derive(Debug)]
pub struct AdoptOutcome {
    pub report_path: String,
    pub summary: Vec<String>,
}

fn contains_md(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for e in entries.filter_map(|e| e.ok()) {
        let p = e.path();
        if p.is_dir() {
            if contains_md(&p) {
                return true;
            }
        } else if p.extension().is_some_and(|x| x == "md") {
            return true;
        }
    }
    false
}

fn ensure_docmeta(root: &Path, lang: &str) -> Result<&'static str, String> {
    let path = root.join(".docmeta.yml");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    if existing.is_empty() {
        // No docmeta and no pages: greenfield — the full init skeleton
        // (router, journal, debt) beats a bare config file. No docmeta but
        // existing pages: an unmigrated tree — init would clobber its
        // index.md, and classification is migrate's job, not adopt's.
        if contains_md(root) {
            return Err(format!(
                "`{}` has pages but no .docmeta.yml — run `docsys migrate inventory` \
                 first, then adopt",
                root.display()
            ));
        }
        crate::migrate::init(root, lang)?;
        return Ok("created via init (router, journal, debt)");
    }
    // Append only the missing required keys; the owner's lines stay verbatim.
    let mut prefix = String::new();
    if !existing.lines().any(|l| l.starts_with("spec:")) {
        prefix.push_str("spec: docsys/0.4\n");
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

pub(crate) fn ensure_git_gate(repo: &Path, root_rel: &str, clean: bool) -> &'static str {
    // a base that is its own repository names itself `.`
    let root_rel = if root_rel.is_empty() { "." } else { root_rel };
    // Placement order: configured core.hooksPath → a tracked .githooks/ dir
    // (the project's own convention; we also set hooksPath so the gate fires
    // on a fresh clone, exactly what the project's own setup step would do)
    // → .git/hooks as the last resort.
    // git itself answers — a config-file text parse misses another scope or
    // another casing of the key (found live, from a field log).
    let configured = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["config", "--get", "core.hooksPath"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());
    let hooks_dir = match configured {
        Some(d) => d,
        None if repo.join(".githooks").is_dir() => {
            let _ = std::process::Command::new("git")
                .args(["config", "core.hooksPath", ".githooks"])
                .current_dir(repo)
                .status();
            ".githooks".to_string()
        }
        None => ".git/hooks".to_string(),
    };
    let hook = repo.join(&hooks_dir).join("pre-commit");
    let existing = fs::read_to_string(&hook).unwrap_or_default();
    // The block: every check runs, every failure counts, and the mode decides
    // whether a failure stops the commit. An earlier block let `docsys refs`'
    // exit code mask a lint failure (the script had no -e and the last command
    // decided) — found by the agent lab; the block is rewritten when it lacks
    // the status variable (D-093).
    let block_for = |hard: bool| -> String {
        let mode_line = if hard { HARD_MODE_LINE } else { WARN_MODE_LINE };
        let mut block = String::new();
        let _ = write!(
            block,
            "\n# --- {GATE_MARKER} ---------------------------------------------\n\
             {mode_line}\n\
             # One-off skip: DOCSYS_SKIP=1 git commit ... (under commit_policy: require it leaves a debt item)\n\
             docsys_gate_exit={}\n\
             if [ -z \"${{DOCSYS_SKIP:-}}\" ] && command -v docsys >/dev/null; then\n\
             \x20 docsys_gate_status=0\n\
             \x20 docsys lint --root {root_rel} || docsys_gate_status=1\n\
             \x20 docsys refs --repo . --root {root_rel} || docsys_gate_status=1\n\
             \x20 docsys gate --repo . --root {root_rel} || docsys_gate_status=1\n\
             \x20 if [ \"$docsys_gate_status\" -ne 0 ] && [ \"$docsys_gate_exit\" -ne 0 ]; then exit 1; fi\n\
             elif [ -n \"${{DOCSYS_SKIP:-}}\" ] && command -v docsys >/dev/null; then\n\
             \x20 docsys gate --repo . --root {root_rel} --skipped >/dev/null 2>&1 || :\n\
             fi\n",
            u8::from(hard)
        );
        block
    };
    if existing.contains(GATE_MARKER) {
        if !existing.contains("docsys_gate_exit=") {
            // an older block (masked statuses, or without the policy line): rewrite it in place
            let lines: Vec<&str> = existing.lines().collect();
            let start = lines
                .iter()
                .position(|l| l.contains(GATE_MARKER) && l.starts_with("# ---"));
            let end = start.and_then(|s| {
                lines
                    .iter()
                    .skip(s)
                    .position(|l| l.trim() == "fi")
                    .map(|i| s + i)
            });
            if let (Some(s0), Some(e0)) = (start, end) {
                let old_block = lines.get(s0..=e0).unwrap_or(&[]);
                let was_warn = old_block.iter().any(|l| l.contains(" || true"));
                let hard = clean || !was_warn;
                let mut out: Vec<String> = lines
                    .get(..s0)
                    .unwrap_or(&[])
                    .iter()
                    .map(|l| l.to_string())
                    .collect();
                // the block text starts with a blank line; the marker line replaces the old one
                let fresh = block_for(hard);
                out.extend(fresh.trim_start_matches('\n').lines().map(str::to_string));
                out.extend(
                    lines
                        .get(e0 + 1..)
                        .unwrap_or(&[])
                        .iter()
                        .map(|l| l.to_string()),
                );
                let mut text = out.join("\n");
                text.push('\n');
                return if fs::write(&hook, text).is_ok() {
                    "upgraded"
                } else {
                    "failed"
                };
            }
        }
        if clean && existing.contains("docsys_gate_exit=0") {
            let text = existing
                .replace("docsys_gate_exit=0", "docsys_gate_exit=1")
                .replace(WARN_MODE_LINE, HARD_MODE_LINE);
            return if fs::write(&hook, text).is_ok() {
                "hardened"
            } else {
                "failed"
            };
        }
        return "kept";
    }
    let block = block_for(clean);
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
    if fs::create_dir_all(hook.parent().unwrap_or(repo)).is_err() {
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
    let root_rel = root
        .strip_prefix(repo)
        .unwrap_or(root)
        .to_string_lossy()
        .replace('\\', "/");

    // 1 · configuration
    let dm = ensure_docmeta(root, lang)?;
    summary.push(format!(".docmeta.yml: {dm}"));
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
    let installed =
        agents::install_with_preamble(&claude, false, &crate::migrate::generated_preamble(root))?;
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

    // 3 · AGENTS.md managed block (idempotent)
    rules::write_agents_block_with(
        &repo.join("AGENTS.md"),
        &crate::migrate::generated_preamble(root),
    )?;
    summary.push("AGENTS.md: managed block written".to_string());

    // 4 · git pre-commit gate — hard when the tree is clean as the hook will
    // see it (inside the repository: pins and history included), warn-mode
    // while it carries debt (D-072)
    let clean = gate_clean(root, repo);
    let gate = ensure_git_gate(repo, &root_rel, clean);
    let mode = if clean {
        "hard"
    } else {
        "warn-mode until lint and refs are clean"
    };
    summary.push(format!("git pre-commit gate: {gate} ({mode})"));

    // 4b · CI: the same questions on every push and pull request
    let ci = ensure_ci_workflow(repo, &root_rel);
    summary.push(format!("ci workflow (.github/workflows/docsys.yml): {ci}"));

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
    let existing_report = fs::read_to_string(repo.join("ADOPTION.md")).unwrap_or_default();
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
         \x20     hardens by itself (lint errors then stop the commit).\n",
    );
    if ci.starts_with("skipped") {
        md.push_str(
            "- [ ] No `.github/` here: run `docsys lint --root <root> --repo .`, `docsys refs`\n\
             \x20     and `docsys gate --range <base>...HEAD` in the CI you have; `docsys adopt`\n\
             \x20     writes the GitHub workflow once `.github/` exists.\n",
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
    let report_path = repo.join("ADOPTION.md");
    let existing = fs::read_to_string(&report_path).unwrap_or_default();
    let pre = crate::migrate::generated_preamble(root);
    let (text, note) = managed_report_with(&existing, &md, &pre);
    if let Some(n) = note {
        summary.push(n);
    }
    fs::write(&report_path, crate::migrate::with_preamble(&text, &pre))
        .map_err(|e| e.to_string())?;

    Ok(AdoptOutcome {
        report_path: report_path.to_string_lossy().to_string(),
        summary,
    })
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
