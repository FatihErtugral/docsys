//! `docsys agents` — installs the agent layer into a project: hooks that keep
//! documentation alive during sessions, the /doc-sync command, and the thin
//! skill. Every hook WARNS and never blocks (R-150: hard blocking gets hooks
//! disabled entirely, which removes the protection completely), and every
//! warning names what needs to change (R-152).

use std::fs;
use std::path::Path;

/// Warn when the contract surface changes without a documentation change.
/// The design decisions this carries came from the field (doc-hooks addon):
/// warn-don't-block, contract-surface-only triggers, patterns reviewed
/// together with the tree.
const PRE_COMMIT_DOCS: &str = r#"#!/usr/bin/env bash
# pre-commit-docs.sh — WARNS when the contract surface changes with no docs
# change staged. Never blocks (R-150). Adjust the surface patterns with the
# tree: a hook that silently matches nothing is worse than none (R-011).
set -uo pipefail

DOCS_ROOT="${DOCS_ROOT:-docs}"
# Contract surface: public APIs, data models, build/config. EDIT PER PROJECT.
SURFACE='\.h$|include/|schema|migrations/|Cargo\.toml$|package\.json$|CMakeLists\.txt$|pyproject\.toml$'

staged=$(git diff --cached --name-only)
[ -z "$staged" ] && exit 0
surface_hits=$(printf '%s\n' "$staged" | grep -E "$SURFACE" | grep -v "^$DOCS_ROOT/" || true)
docs_hits=$(printf '%s\n' "$staged" | grep "^$DOCS_ROOT/" || true)

if [ -n "$surface_hits" ] && [ -z "$docs_hits" ]; then
  echo "docs: contract surface changed with no documentation change:" >&2
  printf '  %s\n' $surface_hits >&2
  echo "  → update the affected page under $DOCS_ROOT/ in this session (see AGENTS.md)" >&2
fi
command -v docsys >/dev/null && docsys lint --root "$DOCS_ROOT" >&2 || true
exit 0
"#;

/// End-of-turn reminder: code moved, docs did not.
const STOP_DOCS_REMINDER: &str = r#"#!/usr/bin/env bash
# stop-docs-reminder.sh — end-of-turn nudge; warns, never blocks (R-150).
set -uo pipefail
DOCS_ROOT="${DOCS_ROOT:-docs}"
dirty=$(git status --porcelain 2>/dev/null | awk '{print $2}')
[ -z "$dirty" ] && exit 0
code=$(printf '%s\n' "$dirty" | grep -v "^$DOCS_ROOT/" | grep -vE '\.(md)$' || true)
docs=$(printf '%s\n' "$dirty" | grep "^$DOCS_ROOT/" || true)
if [ -n "$code" ] && [ -z "$docs" ]; then
  echo "docs: this session changed code but no documentation — if a contract" >&2
  echo "moved, the page moves in the SAME session; at minimum add the journal line." >&2
fi
exit 0
"#;

/// Keep `updated:` honest after a docs edit (R-052: maintained by tooling).
const POST_EDIT_UPDATED: &str = r#"#!/usr/bin/env bash
# post-edit-updated.sh — bump `updated:` on the edited docs page (R-052).
# Reads the edited path from the hook payload on stdin (Claude Code PostToolUse).
set -uo pipefail
DOCS_ROOT="${DOCS_ROOT:-docs}"
payload=$(cat)
file=$(printf '%s' "$payload" | grep -oE '"file_path"[[:space:]]*:[[:space:]]*"[^"]+"' | head -1 | sed 's/.*:[[:space:]]*"//; s/"$//')
case "$file" in
  *"$DOCS_ROOT"/_archive/*|*"$DOCS_ROOT"/_templates/*) exit 0 ;;
  *"$DOCS_ROOT"/*.md) ;;
  *) exit 0 ;;
esac
today=$(date +%F)
if grep -q '^updated:' "$file" 2>/dev/null; then
  sed -i "s/^updated:.*/updated: $today/" "$file"
fi
exit 0
"#;

/// Route documentation by work type, once per session, asking only when the
/// intent is genuinely ambiguous (a survey every session becomes noise).
const SESSION_INTENT: &str = r#"#!/usr/bin/env bash
# session-intent.sh — UserPromptSubmit hook; fires once per session.
set -euo pipefail
payload="$(cat)"
session_id="$(printf '%s' "$payload" | grep -oE '"session_id"[[:space:]]*:[[:space:]]*"[^"]+"' | head -1 | sed 's/.*"\([^"]*\)"$/\1/')"
marker="${TMPDIR:-/tmp}/.docsys-intent-${session_id:-unknown}"
[ -f "$marker" ] && exit 0
touch "$marker"
cat <<'EOF'
<session-doc-routing>
First turn. Classify the work type before anything else. If the message makes
it clear, state it in one line and proceed — ask only when genuinely ambiguous
(feature / bug / refactor / idea-note).

Routing: feature needing a design decision → work/features/ (status: draft);
bug → root cause first: wrong line = journal line, wrong assumption = invariant
in reference/ or a postmortem (test: can it recur?); refactor touching a public
surface → reference/ updated, and always record WHY; idea → journal or roadmap
line, never the permanent layer before it becomes a decision.

Contract-surface changes update their documentation in the SAME session.
End of session: journal line (≤5 lines, links not content). Gate: docsys lint.
Judgment calls follow the procedures: docsys rules --procedures.
</session-doc-routing>
EOF
"#;

const DOC_SYNC: &str = r#"---
description: Scan code↔doc drift and un-graduated done work; propose debt items as a diff
allowed-tools: Bash(git log:*), Bash(git diff:*), Bash(git show:*), Bash(docsys *), Read, Grep, Glob, Edit
---

# /doc-sync — documentation drift check

Manual, never automatic. Report; propose `docs/work/debt.md` items as a diff
and wait for approval. Commit nothing.

1. Mechanical pass: `docsys lint --root docs` and `docsys refs --repo .` —
   include both outputs (one line each if green).
2. Drift suspects: `git log --oneline -20`; for each commit touching the
   contract surface, check `git show --stat <sha> -- docs/` — did the docs
   move with it? Name the page that should have changed.
3. Graduation debt: `grep -rl '^status: done' docs/work/` — for each, what
   still-true knowledge exists nowhere permanent? Say concretely which section
   goes to which page (the R-049 table decides).
4. Propose debt items (`- [ ] <debt> -- deferred: <reason> -- repay when:
   <trigger>`) as an Edit diff; do not apply without approval.

No findings → say so; never invent debt.
"#;

const SKILL_MD: &str = r#"---
name: docsys
description: Documentation system operations — set up, migrate, audit, and curate a docsys tree. Use when the user asks to create or migrate documentation structure, audit docs health, graduate work files, or when starting work in a repo that has docs/.docmeta.yml.
---

# docsys — the thin skill

The mechanics live in the `docsys` binary; this skill adds only judgment and
approval gates. Never re-implement what a command does; never skip a gate.

## Always

- Gate: `docsys lint --root docs` — before any commit, after any docs change.
- Never rewrite content — move it. Never translate. Never invent.
- Judgment calls follow the authored procedures: `docsys rules --procedures`.
  When no option fits, take the escape; never force.

## Set up (new tree)

`docsys init --root docs` then generate the agent block:
`docsys rules --agents-md >> AGENTS.md` (review the diff first).

## Migrate (existing docs anywhere in the repo)

1. `docsys migrate inventory --root <dir> --repo .` → plan skeleton with
   evidence lines and inbound-reference report.
2. Fill each TODO target — this is YOUR judgment call, per page, using the
   evidence and the P/R-031 procedure. **STOP: show the plan, get approval.**
3. `docsys migrate apply --plan <plan> --root <dir> --repo .` — the tool
   moves, rewrites links (in-tree and inbound), scaffolds. Review RISK lines:
   each is a judgment item, resolve or record as debt.

## Audit (report only)

`docsys lint` + `docsys refs --repo .` and read the findings with the R-049 /
R-086 tables in hand. Change nothing; the user decides.

## Graduate (curation)

For each `status: done` work file: ask the R-093 question (does any still-true
information here exist nowhere else?). Route sections by the R-049 table.
Destination pages are prepared first (R-099), blocks move byte-exactly — you
select the mapping, you never retype the text (R-090). `confirmed:` requires
the human's explicit word (P/R-081).
"#;

pub struct Installed {
    pub written: Vec<String>,
    pub skipped: Vec<String>,
}

pub fn install(claude_dir: &Path, force: bool) -> Result<Installed, String> {
    let files: [(&str, &str, bool); 6] = [
        ("hooks/pre-commit-docs.sh", PRE_COMMIT_DOCS, true),
        ("hooks/stop-docs-reminder.sh", STOP_DOCS_REMINDER, true),
        ("hooks/post-edit-updated.sh", POST_EDIT_UPDATED, true),
        ("hooks/session-intent.sh", SESSION_INTENT, true),
        ("commands/doc-sync.md", DOC_SYNC, false),
        ("skills/docsys/SKILL.md", SKILL_MD, false),
    ];
    let mut out = Installed {
        written: Vec::new(),
        skipped: Vec::new(),
    };
    for (rel, content, executable) in files {
        let path = claude_dir.join(rel);
        if path.exists() && !force {
            out.skipped.push(rel.to_string());
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&path, content).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        if executable {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o755));
        }
        out.written.push(rel.to_string());
    }
    Ok(out)
}

/// The settings.json snippet the user merges by hand — automated JSON merging
/// without a parser risks clobbering their configuration (protected file).
pub const SETTINGS_SNIPPET: &str = r#"{
  "hooks": {
    "UserPromptSubmit": [
      { "hooks": [ { "type": "command", "command": ".claude/hooks/session-intent.sh" } ] }
    ],
    "PostToolUse": [
      { "matcher": "Write|Edit",
        "hooks": [ { "type": "command", "command": ".claude/hooks/post-edit-updated.sh" } ] }
    ],
    "Stop": [
      { "hooks": [ { "type": "command", "command": ".claude/hooks/stop-docs-reminder.sh" } ] }
    ]
  }
}"#;
