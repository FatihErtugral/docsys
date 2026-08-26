//! `docsys adopt` — one-command adoption. Everything mechanical happens here,
//! idempotently; everything that needs judgment lands in ADOPTION.md as a
//! checklist an agent burns down in a single session. Adoption must not be a
//! conversation.

use crate::{agents, lint, refs, rules, tree::DocTree};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

const GATE_MARKER: &str = "docsys documentation gate";

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

/// Append the warn-mode gate to the repo's pre-commit hook (idempotent).
/// Warn-mode is deliberate: a fresh adoption carries debt, and a gate that
/// blocks every commit on day one gets bypassed forever (R-150).
fn ensure_git_gate(repo: &Path, root_rel: &str) -> &'static str {
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
    if existing.contains(GATE_MARKER) {
        return "kept";
    }
    let mut block = String::new();
    let _ = write!(
        block,
        "\n# --- {GATE_MARKER} ---------------------------------------------\n\
         # Warn-mode until the adoption debt is triaged; then remove `|| true`.\n\
         # One-off skip: DOCSYS_SKIP=1 git commit ...\n\
         if [ -z \"${{DOCSYS_SKIP:-}}\" ] && command -v docsys >/dev/null; then\n\
         \x20 docsys lint --root {root_rel} || true\n\
         \x20 docsys refs --repo . --root {root_rel} || true\n\
         fi\n"
    );
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

    // 2 · agent layer (never-colliding names; existing files skipped)
    let claude = repo.join(".claude");
    let installed = agents::install(&claude, false)?;
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

    // 2b · settings.json: created only when absent — an existing file may carry
    // MCP servers, permissions, deny lists; merging those is judgment, so it
    // goes to the checklist instead of being overwritten.
    let settings = claude.join("settings.json");
    let settings_missing = !settings.exists();
    if settings_missing {
        fs::write(&settings, agents::SETTINGS_SNIPPET).map_err(|e| e.to_string())?;
        summary.push("settings.json: created with docsys hook wires".to_string());
    } else {
        summary.push(
            "settings.json: untouched (may carry MCP/permissions) — merge is on the checklist"
                .to_string(),
        );
    }

    // 3 · AGENTS.md managed block (idempotent)
    rules::write_agents_block(&repo.join("AGENTS.md"))?;
    summary.push("AGENTS.md: managed block written".to_string());

    // 4 · git pre-commit gate (warn-mode)
    let gate = ensure_git_gate(repo, &root_rel);
    summary.push(format!("git pre-commit gate: {gate}"));

    // 5 · evidence: current findings + the existing layer
    let (lint_report, _) = lint(root);
    let tree = DocTree::load(root).map_err(|e| e.to_string())?;
    let refs_report = refs::run(repo, &tree);
    let layer = agents::adoption_report(&claude);

    let count = |r: &crate::checks::Report, sev: crate::model::Severity| {
        r.findings.iter().filter(|f| f.severity == sev).count()
    };
    use crate::model::Severity::{Error, Warn};

    // 6 · the checklist — judgment goes here, not into a conversation
    let mut md = String::from(
        "# docsys adoption report\n\nGenerated by `docsys adopt`. \
        The mechanical steps below are DONE and idempotent; the checklist at the end \
        is the judgment work — burn it down in one agent session.\n\n## Done\n\n",
    );
    for s in &summary {
        let _ = writeln!(md, "- {s}");
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
         - [ ] When errors reach zero, harden the pre-commit gate: remove `|| true`.\n",
    );
    if !settings_missing {
        md.push_str(
            "- [ ] Merge the docsys hook wires into `.claude/settings.json` by hand or\n\
             \x20     agent (the tool never edits an existing settings file). Snippet:\n\n\
             ```json\n",
        );
        md.push_str(agents::SETTINGS_SNIPPET);
        md.push_str("\n```\n");
    }

    // The report is regenerated, but a leading comment block on the existing
    // file is the owner's — a privacy classifier's marker, a review note —
    // and survives the rewrite (D-046).
    let report_path = repo.join("ADOPTION.md");
    let header = preserved_header(&fs::read_to_string(&report_path).unwrap_or_default());
    fs::write(&report_path, format!("{header}{md}")).map_err(|e| e.to_string())?;

    Ok(AdoptOutcome {
        report_path: report_path.to_string_lossy().to_string(),
        summary,
    })
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
    fn an_unterminated_comment_swallows_to_end_of_file() {
        let h = preserved_header("<!-- open\nstill\n");
        assert_eq!(h, "<!-- open\nstill\n\n");
    }
}
