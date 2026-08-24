//! `docsys doctor` — is the pipeline itself alive?
//!
//! Field report: five mechanisms, all silently failed — a hook on disk wired
//! to nothing, a gate block appended below `exec` (dead code), warn output on
//! a channel the model never reads. "Registered" is not "working": doctor
//! checks that each piece exists, is reachable, and speaks on a channel that
//! surfaces. The same class of failure was found twice in one day; a registry
//! check alone does not catch it — this does.

use std::fs;
use std::path::Path;

pub struct Diagnosis {
    pub lines: Vec<String>,
    pub failed: usize,
}

fn push(d: &mut Diagnosis, ok: bool, msg: String) {
    if ok {
        d.lines.push(format!("ok   {msg}"));
    } else {
        d.failed += 1;
        d.lines.push(format!("FAIL {msg}"));
    }
}

/// Section a script name sits under in settings.json: the last event key
/// occurring before it. Crude by design — zero-dep, and wrong wiring is
/// exactly a "name under the wrong key" mistake.
fn event_of<'t>(settings: &'t str, script: &str) -> Option<&'t str> {
    const EVENTS: [&str; 5] = [
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "Stop",
        "SessionStart",
    ];
    let pos = settings.find(script)?;
    EVENTS
        .iter()
        .filter_map(|e| settings[..pos].rfind(e).map(|p| (p, *e)))
        .max_by_key(|(p, _)| *p)
        .map(|(_, e)| e)
}

/// A top-level (column-0) `exec` or `exit` line makes everything below it
/// unreachable. Indented ones live inside conditionals and are fine — this is
/// a heuristic, and it is exactly the shape both field failures had.
fn dead_above(hook_text: &str, marker: &str) -> bool {
    for line in hook_text.lines() {
        if line.contains(marker) {
            return false;
        }
        if line.starts_with("exec ") || line == "exit 0" || line == "exit" {
            return true;
        }
    }
    false
}

pub fn run(repo: &Path, root: &Path, claude_dir: &Path) -> Diagnosis {
    let mut d = Diagnosis {
        lines: Vec::new(),
        failed: 0,
    };

    // 1. The tree is operable.
    let docmeta = root.join(".docmeta.yml").is_file();
    push(
        &mut d,
        docmeta,
        format!(".docmeta.yml at {}", root.display()),
    );
    if docmeta {
        let (report, _) = crate::lint(root);
        let errors = report
            .findings
            .iter()
            .filter(|f| f.severity == crate::model::Severity::Error)
            .count();
        d.lines.push(format!(
            "info lint: {errors} error(s), {} warning(s)",
            report.findings.len() - errors
        ));
    }

    // 2. Hook files exist and are executable.
    const HOOKS: [(&str, &str); 4] = [
        ("hooks/session-intent.sh", "UserPromptSubmit"),
        ("hooks/pre-commit-docs.sh", "PreToolUse"),
        ("hooks/post-edit-updated.sh", "PostToolUse"),
        ("hooks/stop-docs-reminder.sh", "Stop"),
    ];
    let settings = fs::read_to_string(claude_dir.join("settings.json")).unwrap_or_default();
    if settings.is_empty() {
        push(
            &mut d,
            false,
            format!(
                "{}/settings.json — hooks on disk run only when this file wires them",
                claude_dir.display()
            ),
        );
    }
    for (rel, want_event) in HOOKS {
        let path = claude_dir.join(rel);
        let exists = path.is_file();
        #[cfg(unix)]
        let runnable = exists && {
            use std::os::unix::fs::PermissionsExt;
            fs::metadata(&path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
        };
        #[cfg(not(unix))]
        let runnable = exists;
        push(&mut d, runnable, format!("{rel} present and executable"));
        if !settings.is_empty() {
            let name = rel.rsplit('/').next().unwrap_or(rel);
            match event_of(&settings, name) {
                Some(e) if e == want_event => {
                    push(&mut d, true, format!("{name} wired under {want_event}"));
                }
                Some(e) => push(
                    &mut d,
                    false,
                    format!("{name} wired under {e}, expected {want_event}"),
                ),
                None => push(
                    &mut d,
                    false,
                    format!("{name} on disk but not in settings.json — it never runs"),
                ),
            }
        }
    }

    // 3. The git gate exists AND is reachable. git itself answers where hooks
    // live — parsing the config file missed a hooksPath set in another scope
    // or spelled in another case, and doctor pointed at the wrong directory
    // (found live, from a field log).
    let hooks_dir = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["config", "--get", "core.hooksPath"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| ".git/hooks".to_string());
    let hook_path = repo.join(&hooks_dir).join("pre-commit");
    match fs::read_to_string(&hook_path) {
        Ok(text) if text.contains("docsys") => {
            if dead_above(&text, "docsys") {
                push(
                    &mut d,
                    false,
                    format!(
                        "{hooks_dir}/pre-commit: the docsys block sits below a top-level \
                         exec/exit — dead code that looks installed"
                    ),
                );
            } else {
                push(
                    &mut d,
                    true,
                    format!("{hooks_dir}/pre-commit gate reachable"),
                );
            }
        }
        Ok(_) => push(
            &mut d,
            false,
            format!("{hooks_dir}/pre-commit exists but carries no docsys gate"),
        ),
        Err(_) => push(
            &mut d,
            false,
            format!("no pre-commit hook under {hooks_dir} — the git gate never fires"),
        ),
    }

    // 4. Which channels actually reach the model — stated, so nobody has to
    // rediscover it. (Harness: Claude Code semantics.)
    d.lines.push(
        "info channels: PreToolUse exit 2 blocks and the model reads stderr; \
         UserPromptSubmit stdout joins context; PostToolUse/Stop exit 0 output \
         reaches the transcript only — never treat those two as enforcement"
            .to_string(),
    );
    d
}
