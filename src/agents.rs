//! `docsys agents` — installs the agent layer into a project: hooks that keep
//! documentation alive during sessions, the /doc-sync command, and the thin
//! skill. Every hook WARNS and never blocks (R-150: hard blocking gets hooks
//! disabled entirely, which removes the protection completely), and every
//! warning names what needs to change (R-152).

use std::fs;
use std::path::Path;

/// The one channel that reliably reaches the model: a PreToolUse hook, where
/// exit 2 stops the tool call and the model reads stderr. Field report: every
/// warn-only channel (exit 0 stdout/stderr on PostToolUse/Stop) lands in the
/// transcript, not the model — a fully broken pipeline and a healthy one were
/// indistinguishable. Lint errors block outright (severity doctrine). The
/// code-without-docs invariant ASKS ONCE: the first attempt stops with the
/// question, re-running the same commit proceeds — a wall gets hooks disabled
/// (R-150); a question does not.
const PRE_COMMIT_DOCS: &str = r#"#!/usr/bin/env bash
# pre-commit-docs.sh — PreToolUse gate on `git commit` (docsys gate relayed).
set -uo pipefail

payload=$(cat)
cmd=$(printf '%s' "$payload" | grep -oE '"command"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1)
case "$cmd" in *"git commit"*) ;; *) exit 0 ;; esac

DOCS_ROOT="${DOCS_ROOT:-docs}"
command -v docsys >/dev/null || exit 0
[ -n "${DOCSYS_SKIP:-}" ] && exit 0

out=$(docsys gate --repo . --root "$DOCS_ROOT" 2>&1); code=$?
if [ "$code" -ne 0 ]; then
  printf '%s\n' "$out" >&2
  echo "docsys gate: lint errors block this commit — fix them first (DOCSYS_SKIP=1 to bypass once)" >&2
  exit 2
fi

line=$(printf '%s\n' "$out" | grep '^GATE ' || true)
if [ -n "$line" ]; then
  # The question is asked once per (HEAD, change set). The marker lives under
  # the git dir and stays until HEAD moves — a passing attempt must NOT consume
  # it: `git add … && git commit` stages inside the command, after this hook
  # ran, so a pass can still commit nothing, and the next attempt would have
  # been asked again (found live). The set is the staged files, or the working
  # tree when nothing is staged yet — the same scope the gate itself answered.
  head=$(git rev-parse -q --verify HEAD 2>/dev/null || echo none)
  set_=$(git diff --cached --name-only); [ -z "$set_" ] && set_=$(git diff --name-only)
  key=$(printf '%s\n%s\n' "$head" "$set_" | cksum | cut -d' ' -f1)
  dir="$(git rev-parse --git-dir 2>/dev/null || echo "${TMPDIR:-/tmp}")/docsys-gate"
  mkdir -p "$dir" 2>/dev/null
  find "$dir" -type f ! -name "$head.*" -delete 2>/dev/null
  marker="$dir/$head.$key"
  if [ ! -f "$marker" ]; then
    touch "$marker"
    printf '%s\n' "$line" >&2
    echo "code moves with no documentation change. If a contract moved, update the page (or add the journal line) and commit; if nothing user-visible moved, run the same commit again — this gate asks once." >&2
    exit 2
  fi
fi
exit 0
"#;

/// End-of-turn reminder: code moved, docs did not. Reads the working tree
/// AND the commits not yet pushed: an agent that commits as it goes leaves a
/// clean tree, and a reminder that read only the tree stayed silent through a
/// whole session of code-only commits (D-041).
const STOP_DOCS_REMINDER: &str = r#"#!/usr/bin/env bash
# stop-docs-reminder.sh — end-of-turn nudge; warns, never blocks (R-150).
# Scope: working tree + commits ahead of the upstream (@{u}..HEAD). Without an
# upstream only the tree is read.
set -uo pipefail
DOCS_ROOT="${DOCS_ROOT:-docs}"
# porcelain: strip the two status columns; a rename reports its NEW path
tree=$(git status --porcelain 2>/dev/null | sed 's/^...//; s/.* -> //')
ahead=$(git diff --name-only '@{u}..HEAD' 2>/dev/null || true)
changed=$(printf '%s\n%s\n' "$tree" "$ahead" | sed '/^$/d' | sort -u)
[ -z "$changed" ] && exit 0
code=$(printf '%s\n' "$changed" | grep -v "^$DOCS_ROOT/" | grep -vE '\.(md)$' || true)
docs=$(printf '%s\n' "$changed" | grep "^$DOCS_ROOT/" || true)
if [ -n "$code" ] && [ -z "$docs" ]; then
  where="this session"
  [ -n "$ahead" ] && where="this session (including commits not yet pushed)"
  echo "docs: $where changed code but no documentation — if a contract" >&2
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

# /docsys-sync — documentation drift check

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

/// The export skill: turns "create the end-user doc for X" into a procedure.
/// The binary selects and composes; the skill carries the judgment steps —
/// closing audience gaps by authoring pages (with approval) and translating
/// under R-122/R-123. Without this, the prompt lives in the user's head,
/// which is exactly the hand-maintained knowledge D-022 exists to prevent.
const EXPORT_SKILL: &str = r#"---
name: docsys-export
description: Produce an audience-shaped document (end-user, developer, designer, …) from the docs tree — "create the end-user doc for feature X", "make a user guide", "export the designer spec", "translate the product doc". Runs docsys export, closes audience gaps by authoring pages with approval, handles language intent.
---

# docsys-export — audience-shaped documents

The binary selects and composes; it never writes prose. YOU author what is
missing — with approval — and the tree keeps it fresh afterwards.

## 1. Discover

`docsys export plan --root docs --audience <a>` — what exists for this reader.
An error naming the whole-tree gap means the pages do not exist yet: go to §3.
The vocabulary is the tree's own (`audiences:` in `.docmeta.yml`); an
undeclared page reads as `developer` (D-033).

## 2. Compose

- One feature: `docsys export feature <id>… [--follow] --audience <a>
  --title "…" --root docs --out <file>`
- Whole product: author or update the product map (a markdown file OUTSIDE the
  docs root), then `docsys export product <map> --audience <a> --root docs
  --out <file>`

Read every WARN: `gap:` lines name related pages this reader cannot use yet;
an `unchanged — left untouched` result is success, not failure. A refusal
listing wrong-audience pages means §3, never `--force`-style workarounds.

## 3. Close a gap (the only judgment step)

For each missing page: distil from the EXISTING pages — invent nothing; if a
fact exists nowhere in the tree, ask, do not guess. The voice matches the
reader: an end-user page never names source files, classes, or tests. Give it
the tree's usual frontmatter plus `audience: <a>`, route it on the index, and
gate with `docsys lint --root docs` until clean. **Show the first page and get
approval before authoring the rest.**

## 4. Language

`--lang <code>` states the document's language; WARNs name the pages declared
otherwise. Translating a page is editing that page: structure stays, and code
identifiers, product names, protocol names, and quotations keep their original
form (R-122/R-123 — when unsure whether something is a proper name, keep it).
Re-run the export afterwards: the per-page stamps changed only where content
did, so only those sections needed the work.
"#;

// --- knowledge-base agent layer (`docsys agents --kb`) -----------------------
// The four organs of a personal knowledge base: capture writes, ingest
// distils, audit verifies independently, lookup answers. The binary enforces
// the contract (ids, sources, verification records, raw immutability); these
// carry the judgment. Field-shaped: every rule here was paid for by a real
// base whose constitution predated the spec and matched it.

const KB_CAPTURE: &str = r#"---
name: kb-capture
description: Save something into the knowledge base — "note this", "remember this", "add to my brain", "log this lesson". Writes to raw/inbox/ with zero classification; sorting is ingest's job.
---

# kb-capture — the write gate

Capture costs nothing or it does not happen. Never classify here, never ask
where it belongs, never open a wiki page.

1. Write ONE file to `raw/inbox/<YYYY-MM-DD>-<short-kebab-slug>.md`.
2. Content: the note in the user's own words, plus one line naming **why it is
   worth keeping** — that line is what makes it distillable later. Add a
   `suggested-domain:` line only if it is obvious; ingest decides.
3. Confirm in one sentence. Do not run lint, do not touch `wiki/`.

Never paraphrase away a specific: a number, a version, an error string, a
command is the part that will be worth having.
"#;

const KB_INGEST: &str = r#"---
name: kb-ingest
description: Process the knowledge-base inbox — "process my inbox", "empty the inbox", "file these notes". Distils raw notes into wiki pages, archives the source, runs the gate.
---

# kb-ingest — raw becomes knowledge

Distillation, not movement (R-092): the raw note is evidence and stays; the
wiki page is authored. Full discipline is applied HERE, so capture can stay
free.

For each file in `raw/inbox/`:

1. **Classify the domain** against `domains:` in `.docmeta.yml`. Fits none?
   Leave the note in the inbox and record the proposal in
   `wiki/open-questions.md` — never force a note into the nearest domain, and
   never invent a domain for a single note.
2. **Pick the type** — `reference` (facts, values), `howto` (steps),
   `explanation` (why), `tutorial` (guided first run). Never mix types on one
   page (R-031); if a page starts holding steps AND concepts, split it.
3. **Author or update** `wiki/<domain>/<type>/<slug>.md` with frontmatter:
   `id` (stable, kebab-case, never renamed), `type`, `domain`,
   `verification: unverified`, `updated`, `sources: [raw/…]`. A page that
   changes drops back to `unverified` — a verification describes content that
   no longer exists otherwise.
4. Open with one or two sentences that stand alone (R-032): a reader arrives
   here from a search, not from the top of a chain.
5. **Route it**: add the page to `wiki/<domain>/index.md`, and the domain to
   `wiki/index.md` if new (R-035 grammar).
6. **Archive the source**: move the note from `raw/inbox/` to
   `raw/<domain>/` — same filename, bytes untouched (R-023), and every
   `sources:` entry that pointed at the old path is rewritten (R-027).
7. Gate: `docsys lint --root <base>` — finish clean or report what blocks.

Never verify your own work (R-025) — that is kb-audit's job, in another
session.
"#;

const KB_AUDIT: &str = r#"---
name: kb-audit
description: Independently verify knowledge-base pages against their sources — "audit my wiki", "verify these pages", "check the unverified pages". Records the audit or demotes the page.
---

# kb-audit — the independent eye

R-025: the session that produced a page never verifies it. If you authored a
page in this session, say so and stop — verification needs another session.

For each `verification: unverified` page (or the ones named):

1. Read the page and every file in its `sources:`.
2. Judge faithfulness: is every claim supported? Any contradiction? A missing
   or empty source is a failure, not a pass.
3. **Faithful** → set `verification: verified` and record the audit (R-028):
   `verified_by:` (who or which session) and `verified_rev:` (the base's
   current revision). Without that record the claim is unauditable.
4. **Not faithful** → leave/return it to `unverified` and append one line to
   `wiki/open-questions.md` naming the specific discrepancy. Never edit the
   page's claims to make them pass — that is authoring, and it would need
   another audit.
5. Gate: `docsys lint --root <base>`.

Report page by page: verified, or demoted with the reason.
"#;

const KB_LOOKUP: &str = r#"---
name: kb-lookup
description: Answer from the knowledge base — "what do my notes say about X", "check my brain for X", "did I write anything about X". Read-only; answers with sources or says it is not there.
---

# kb-lookup — the read gate

Read-only. Never write, never fix what you find; report gaps instead.

1. `wiki/index.md` → the domain → `wiki/<domain>/index.md` → the page.
2. Not routed? grep `wiki/` for tags and headings.
3. Still nothing → **say it is not in the base.** Never answer from your own
   knowledge while implying the base said it; offer to capture the question.
4. Answer WITH the page path, and say plainly when the page is
   `unverified` — an unaudited page may be wrong, and the reader decides how
   much to lean on it.

`raw/` is evidence, not an answer: quote it only to show where a page came
from.
"#;

/// The knowledge base's constitution: the always-loaded contract, the part
/// that is judgment rather than rule text (the mechanical half is `docsys
/// rules --agents-md`, generated from the spec).
const KB_AGENTS_MD: &str = r#"# Knowledge base — the contract

A personal knowledge base: plain markdown and git, no database, no lock-in.
`docsys` enforces the mechanics; this file carries what only people decide.

## Layers

- `raw/` — the record. `raw/inbox/` is where notes land; `raw/<domain>/` is
  where processed sources are archived. **Content-immutable**: bytes are never
  edited and nothing is deleted; relocation is the expected flow.
- `wiki/` — distilled knowledge, `wiki/<domain>/<type>/`. The single source of
  truth. Only ingest writes here.

## The loop

capture → `raw/inbox/` · ingest → a wiki page + archived source · audit →
`verified` with a record · lookup → an answer with its source.

Rules that are not mechanical:
- Nothing is verified by the session that wrote it.
- A changed page is `unverified` again.
- A note that fits no domain stays in the inbox; a domain is proposed in
  `wiki/open-questions.md` and earns its place only after several notes.
- Never invent. "Not in the base" is a complete answer.

## Gate

`docsys lint --root .` — before any commit, after any change.
"#;

pub struct Installed {
    pub written: Vec<String>,
    pub skipped: Vec<String>,
}

/// The knowledge-base agent layer. Installed beside the base (`--kb`), never
/// mixed with the project layer: a knowledge base has no code to gate, and a
/// project has no inbox to ingest.
pub fn install_kb(claude_dir: &Path, base_dir: &Path, force: bool) -> Result<Installed, String> {
    let mut out = Installed {
        written: Vec::new(),
        skipped: Vec::new(),
    };
    for (rel, content) in [
        ("skills/kb-capture/SKILL.md", KB_CAPTURE),
        ("skills/kb-ingest/SKILL.md", KB_INGEST),
        ("skills/kb-audit/SKILL.md", KB_AUDIT),
        ("skills/kb-lookup/SKILL.md", KB_LOOKUP),
    ] {
        let path = claude_dir.join(rel);
        if path.exists() && !force {
            out.skipped.push(rel.to_string());
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&path, content).map_err(|e| e.to_string())?;
        out.written.push(rel.to_string());
    }
    // AGENTS.md is the owner's file: written only when absent (D-028's rule
    // for protected files), never merged over.
    let agents = base_dir.join("AGENTS.md");
    if agents.exists() && !force {
        out.skipped.push("AGENTS.md".to_string());
    } else {
        fs::write(&agents, KB_AGENTS_MD).map_err(|e| e.to_string())?;
        out.written.push("AGENTS.md".to_string());
    }
    Ok(out)
}

pub fn install(claude_dir: &Path, force: bool) -> Result<Installed, String> {
    let files: [(&str, &str, bool); 7] = [
        ("hooks/pre-commit-docs.sh", PRE_COMMIT_DOCS, true),
        ("hooks/stop-docs-reminder.sh", STOP_DOCS_REMINDER, true),
        ("hooks/post-edit-updated.sh", POST_EDIT_UPDATED, true),
        ("hooks/session-intent.sh", SESSION_INTENT, true),
        ("commands/docsys-sync.md", DOC_SYNC, false),
        ("skills/docsys/SKILL.md", SKILL_MD, false),
        ("skills/docsys-export/SKILL.md", EXPORT_SKILL, false),
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
        let content = if executable {
            stamp(content)
        } else {
            content.to_string()
        };
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
    "PreToolUse": [
      { "matcher": "Bash",
        "hooks": [ { "type": "command", "command": ".claude/hooks/pre-commit-docs.sh" } ] }
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

/// Adoption report: what agent layer already exists, and which shell commands
/// it invokes. Detection is mechanical; deciding what to delegate to docsys is
/// judgment and stays with an agent and a human (D-026).
pub fn adoption_report(claude_dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for sub in ["hooks", "commands", "skills", "rules"] {
        collect_files(&claude_dir.join(sub), &mut files);
    }
    files.sort();
    for f in files {
        let rel = f
            .strip_prefix(claude_dir)
            .unwrap_or(&f)
            .to_string_lossy()
            .replace('\\', "/");
        if rel.starts_with("skills/docsys/")
            || rel.starts_with("skills/docsys-export/")
            || rel.starts_with("hooks/pre-commit-docs")
            || rel.starts_with("hooks/stop-docs-reminder")
            || rel.starts_with("hooks/post-edit-updated")
            || rel.starts_with("commands/docsys-sync")
        {
            continue; // our own assets are not adoption surface
        }
        let Ok(text) = fs::read_to_string(&f) else {
            continue;
        };
        let mut calls: Vec<String> = Vec::new();
        for line in text.lines() {
            // allowed-tools: Bash(cmd ...) declarations
            let mut rest = line;
            while let Some(pos) = rest.find("Bash(") {
                let after = rest.get(pos + 5..).unwrap_or("");
                if let Some(end) = after.find(')') {
                    let inner = after.get(..end).unwrap_or("").trim();
                    let head = inner
                        .split_whitespace()
                        .take(2)
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !head.is_empty() && !calls.contains(&head) {
                        calls.push(head);
                    }
                    rest = after.get(end + 1..).unwrap_or("");
                } else {
                    break;
                }
            }
            // fenced/script invocation lines
            let trimmed = line.trim_start();
            for prefix in ["python3 ", "python ", "bash ", "sh ", "make "] {
                if trimmed.starts_with(prefix) {
                    let head = trimmed
                        .split_whitespace()
                        .take(2)
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !calls.contains(&head) {
                        calls.push(head);
                    }
                }
            }
        }
        if calls.is_empty() {
            out.push(format!("{rel} · no shell calls detected"));
        } else {
            out.push(format!("{rel} · invokes: {}", calls.join(" · ")));
        }
    }
    out
}

fn collect_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<std::path::PathBuf> =
        entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            collect_files(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// The version line written into every hook, right under the shebang, so a
/// tree can tell "behind the binary" from "hand-written" (D-047).
pub const TEMPLATE_VERSION: &str = env!("CARGO_PKG_VERSION");

fn stamp(content: &str) -> String {
    match content.split_once('\n') {
        Some((shebang, rest)) if shebang.starts_with("#!") => {
            format!("{shebang}\n# docsys-template: {TEMPLATE_VERSION}\n{rest}")
        }
        _ => format!("# docsys-template: {TEMPLATE_VERSION}\n{content}"),
    }
}

/// The template version a hook on disk carries; `None` for a hook written
/// before stamping or by hand.
pub fn template_version(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    text.lines()
        .take(3)
        .find_map(|l| l.strip_prefix("# docsys-template:"))
        .map(|v| v.trim().to_string())
}

/// Hooks whose template is not the binary's: (relative path, version found).
pub fn stale_hooks(claude_dir: &Path) -> Vec<(String, String)> {
    HOOK_FILES
        .iter()
        .filter(|rel| claude_dir.join(rel).is_file())
        .filter_map(|rel| {
            let found =
                template_version(&claude_dir.join(rel)).unwrap_or_else(|| "unversioned".into());
            (found != TEMPLATE_VERSION).then(|| (rel.to_string(), found))
        })
        .collect()
}

pub const HOOK_FILES: [&str; 4] = [
    "hooks/pre-commit-docs.sh",
    "hooks/stop-docs-reminder.sh",
    "hooks/post-edit-updated.sh",
    "hooks/session-intent.sh",
];
