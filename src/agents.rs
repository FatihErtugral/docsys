//! `docsys agents` — installs the agent layer into a project: hooks that keep
//! documentation alive during sessions, the /doc-sync command, and the thin
//! skill. Every hook WARNS and never blocks (R-150: hard blocking gets hooks
//! disabled entirely, which removes the protection completely), and every
//! warning names what needs to change (R-152).

use crate::hook::Json;
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
# pre-commit-docs.sh — PreToolUse gate on `git commit`; the decision is made
# by `docsys hook pre-tool-use` (D-051): lint errors block, the code-without-docs
# question is asked once per change set. DOCSYS_SKIP=1 bypasses once.
# In a knowledge base the same relay guards raw/: an existing record is never
# overwritten or edited through Write/Edit (R-023, D-076).
command -v docsys >/dev/null || exit 0
exec docsys hook pre-tool-use --root "${DOCS_ROOT:-docs}"
"#;

/// End-of-turn reminder: code moved, docs did not. Reads the working tree
/// AND the commits not yet pushed: an agent that commits as it goes leaves a
/// clean tree, and a reminder that read only the tree stayed silent through a
/// whole session of code-only commits (D-041).
const STOP_DOCS_REMINDER: &str = r#"#!/usr/bin/env bash
# stop-docs-reminder.sh — end-of-turn nudge; warns, never blocks (R-150).
# Reads the working tree and the commits not yet pushed (`docsys hook stop`).
command -v docsys >/dev/null || exit 0
exec docsys hook stop --root "${DOCS_ROOT:-docs}" --stdin
"#;

/// Keep `updated:` honest after a docs edit (R-052: maintained by tooling).
const POST_EDIT_UPDATED: &str = r#"#!/usr/bin/env bash
# post-edit-updated.sh — bump `updated:` on the edited docs page (R-052),
# via `docsys hook post-tool-use` (reads the PostToolUse payload on stdin).
command -v docsys >/dev/null || exit 0
exec docsys hook post-tool-use --root "${DOCS_ROOT:-docs}"
"#;

/// Route documentation by work type, once per session, asking only when the
/// intent is genuinely ambiguous (a survey every session becomes noise).
const SESSION_INTENT: &str = r#"#!/usr/bin/env bash
# session-intent.sh — UserPromptSubmit hook; the routing text once per
# session, from `docsys hook user-prompt-submit` (work types for a project,
# the four organs for a knowledge base — the root's profile decides).
command -v docsys >/dev/null || exit 0
exec docsys hook user-prompt-submit --root "${DOCS_ROOT:-docs}"
"#;

const DOC_SYNC: &str = r#"---
description: Scan code↔doc drift and un-graduated done work; propose debt items as a diff
allowed-tools: Bash(git log:*), Bash(git diff:*), Bash(git show:*), Bash(docsys *), Read, Grep, Glob, Edit
---

# /docsys-sync — documentation drift check

Manual, never automatic. Report; propose `docs/work/debt.md` items as a diff
and wait for approval. Commit nothing.

1. Mechanical pass: `docsys lint --root docs --repo .` and `docsys refs --repo .` —
   include both outputs (one line each if green). Freshness errors are drift
   by definition: a stale pin names the region that moved, `updated:` behind
   history names a hand edit, an untouched draft names abandonment.
2. Drift suspects: `docsys seed plan --repo . --root docs --since <date of
   the newest journal entry>` — every feature history touched since, with
   its coverage. For each covered feature with commits, `git show --stat
   <sha> -- docs/`: did its page move with the code? Name the page that
   should have changed. An uncovered feature with commits is a seeding
   candidate, not drift.
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
  Inside the repository it also checks freshness: a stale pin (R-111) is
  re-read against the code, then `docsys pin --refresh <page>` — never
  refreshed blind; `updated:` behind history (R-106) is one date; a draft
  untouched past `stale_active_days` (R-085) is abandoned with a reason,
  graduated, or worked on.
- Never rewrite content — move it. Never translate. Never invent.
- The work has a type — feature, bug, improvement, research — and a record:
  a work file under `work/<category>/` or, at minimum, a journal entry that
  links the files and says why. Under `commit_policy: require` (D-093) the
  gate refuses a commit without it and the end of a turn holds until it is
  written: the session may be gone when the commit lands, so the knowledge
  is captured while the session is here.
- A question about the tree starts with `docsys lookup <words>` — every page,
  local and consumed (`@namespace/id`), naming the words — then the page.
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

## Verification (who vouches)

Anyone writes — you included — and nothing you write is the truth yet. A
permanent page you author from evidence, or change in substance, carries
`verification: unverified` and `sources:` (what it rests on); `docsys page
new <type> <id> --unverified` writes that frontmatter. Only an independent
session sets `verified` (R-025), recording `verified_by:` and `verified_rev:`
(R-028) — `docsys verify <page>` writes that record for whoever runs it (a
maintainer, from their git identity; refused otherwise), and `docsys verify
<page> --revoke` takes a page back to `unverified` when its body moved; a
`verified` page whose body then changes is an error until it is `unverified`
again (R-024). The maintainer in the session needs no second session: when
the person you work with is a declared maintainer and says the page is right,
run `docsys verify <page>` — it records them, not you (D-096). When
`.docmeta.yml` declares `maintainers:`,
`verified_by:` and `confirmed:` must name one of them (R-208): the people
who review the code are the people who vouch for the page. A reader — a
person or an agent — sees the state and reads accordingly.

## Compile (a howto into a skill)

A `howto/` page whose steps are complete — every step written, nothing you
would fill from memory (P/R-096) — compiles: `docsys compile <id>`. The skill
is the page body byte for byte, pinned to the page's content hash; lint fails
when the page moves until you re-read it and compile again (R-095). A gap
found while running the skill is reported on the page, never patched in the
skill.
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
   never invent a domain for a single note. A note that holds nothing to
   keep (noise) stays too, with one dated line in `wiki/open-questions.md`
   naming it, so the inbox never grows in silence; deleting is never yours.
   An open-questions line is `- [ ] YYYY-MM-DD …` (R-108), in the base's
   language; the file's header is not yours to rewrite.
2. **Pick the type** — `reference` (facts, values), `howto` (steps),
   `explanation` (why), `tutorial` (guided first run). Never mix types on one
   page (R-031); if a page starts holding steps AND concepts, split it.
3. **Author or update** `wiki/<domain>/<type>/<slug>.md` with frontmatter:
   `id` (stable, kebab-case, never renamed), `type`, `domain`,
   `verification: unverified`, `updated`, `sources: [raw/…]`. A claim that
   rests on a consumed project's own page cites it as `@namespace/id`
   (materialized under `.federation/`; AGENTS.md → Sources beyond the inbox);
   a claim that rests on a connector record cites the record's path like any
   note. A page that changes drops back to `unverified` — a verification
   describes content that no longer exists otherwise.
4. Open with one or two sentences that stand alone (R-032): a reader arrives
   here from a search, not from the top of a chain.
5. **Route it**: add the page to `wiki/<domain>/index.md`, and the domain to
   `wiki/index.md` if new (R-035 grammar).
6. **Archive the source**: `docsys raw move raw/inbox/<note> <domain>
   --root <base>` — the note lands in `raw/<domain>/` with the same filename
   and the same bytes (R-023), and every `sources:` entry that pointed at the
   old path is rewritten by the tool (R-027). Never `git mv` and edit
   `sources:` by hand: the hand edit is where evidence trails were severed.
7. Gate: `docsys lint --root <base>` — finish clean or report what blocks.

Never verify your own work (R-025) — that is kb-audit's job, in another
session — unless the person you are working with is a declared maintainer
(`.docmeta.yml` `maintainers:`, the working copy's git identity) and says the
page is right: then `docsys verify <page>` records their word, not yours
(D-096).
"#;

const KB_AUDIT: &str = r#"---
name: kb-audit
description: Independently verify knowledge-base pages against their sources — "audit my wiki", "verify these pages", "check the unverified pages". Records the audit or demotes the page.
---

# kb-audit — the independent eye

R-025: the session that produced a page never verifies it on its own
judgment. If you authored a page in this session, say so and stop —
verification needs another session, or the maintainer in this one: a declared
maintainer who reads the page and says it is right verifies it with
`docsys verify <page>` (their identity, their word — D-096).

For each `verification: unverified` page (or the ones named):

1. Read the page and every file in its `sources:`.
2. Judge faithfulness: is every claim supported? Any contradiction? A missing
   or empty source is a failure, not a pass.
3. **Faithful** → `docsys verify <page> --by "<who or which session>"` sets
   `verification: verified` and records the audit (R-028): `verified_by:` and
   `verified_rev:` (the base's current revision; the page must be committed
   as it is). Without that record the claim is unauditable.
4. **Not faithful** → leave/return it to `unverified` and append one line to
   `wiki/open-questions.md` naming the specific discrepancy —
   `- [ ] YYYY-MM-DD …` (R-108), in the base's language. Never edit the
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

1. `docsys lookup <words> --root <base>` — the mechanical first hop: every
   page, local and consumed (`@namespace/id`), that names all the words,
   scored by where they occur. Read the page it points at; a consumed page
   is another tree's contract and is cited as `@namespace/id`.
2. Nothing? `wiki/index.md` → the domain → `wiki/<domain>/index.md` → the
   page; then grep `wiki/` for tags and headings.
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
/// The marker the base's constitution carries until its character is set;
/// the first-turn hook reads it and runs the survey (D-083).
pub const CHARACTER_UNSET: &str = "<!-- character: unset";

const KB_AGENTS_MD: &str = r#"# Knowledge base — the contract

A personal knowledge base: plain markdown and git, no database, no lock-in.
`docsys` enforces the mechanics; this file carries what only people decide.

## Layers

- `raw/` — the record. `raw/inbox/` is where notes land; `raw/<domain>/` is
  where processed sources are archived. **Content-immutable**: bytes are never
  edited and nothing is deleted; relocation is the expected flow.
- `wiki/` — distilled knowledge, `wiki/<domain>/<type>/`. The single source of
  truth. Only ingest writes here.

## Character

<!-- character: unset — the first session proposes one and asks; replace this whole block with the answers, keep the headings around it -->

- Name: (unset — what the person calls the assistant)
- Address: (unset — how the assistant addresses the person: name, formal or informal)
- Tone: (unset — plain and brief, warm, formal; humor or none)
- Languages: the conversation mirrors the person's language, turn by turn;
  every file under `wiki/` — pages, indexes, `open-questions.md` — keeps the
  base's `default_content_language`, whatever language the person or the
  session's own settings speak; code identifiers, commands and quotations
  are never translated
- Never: invent what the base does not hold · act outward without the
  person's confirmation · edit a record · verify its own page

## The loop

capture → `raw/inbox/` · ingest → a wiki page + archived source · audit →
`verified` with a record · lookup → an answer with its source.

Rules that are not mechanical:
- Nothing is verified by the session that wrote it.
- A changed page is `unverified` again.
- A note that fits no domain stays in the inbox; a domain is proposed in
  `wiki/open-questions.md` and earns its place only after several notes.
- `wiki/open-questions.md` is the base's questions ledger: one dated line
  per item, `- [ ] YYYY-MM-DD …` (R-108); lint reads its grammar and
  `status` counts it — never rewrite the file, append to it.
- Never invent. "Not in the base" is a complete answer.

## Sources beyond the inbox

- **Projects the base consumes** — `docsys consume add <path|git-url>` (or
  `docsys consume discover <dir>` to list the candidates under a directory)
  names a project in `.docmeta.yml`; `docsys fetch` materializes its
  exported pages under `.federation/<namespace>/`, committed as the baseline.
  A wiki page that rests on such a page cites it as `@namespace/id` in
  `sources:`; lint says when that source moved after the page was verified.
- **The git connector** — `docsys inbox pull <repo> [--since <date>]
  [--limit <n>]` lands one record per commit worth reading (bookkeeping
  commits — no body, docs only — are skipped unless `--all`) through the
  same write gate as any note; a second pull lands nothing twice. Choose the
  span and say why; then ingest the records like notes: what the project
  decided, not what it did.
- **The digest** — `docsys status` first: the inbox, pages by state, open
  items, consumed namespaces, findings. `docsys assistant --root .
  --projects <dir>` stood this base up and keeps its consumed projects
  current, in one command.

## Hooks

`docsys agents --kb` wires four relays into `.claude/settings.json` (an
existing file is merged into, never overwritten): the first message of a
session gets the organ routing; a `Write`/`Edit` on an existing `raw/`
record is blocked (R-023) — new knowledge is a new file in `raw/inbox/`,
relocation is `docsys raw move`; an edited wiki page gets its
`updated:` bumped; `git commit` runs the gate; the end of a turn names what
waits in the inbox. Everything warns and nothing blocks, except the two
guards on the irreversible: the record and the commit.

## Forgetting

Only on the person's explicit word. `docsys forget <page|record> --reason
"…"` moves a page to `_archive/` with a tombstone (its identifier is never
reused) and a record to `raw/_forgotten/` (still a record, never read again,
never captured again); the ledger `.forgotten.yml` says when and why. Forget
the page before the records it rests on. It makes a topic unknown to every
organ; it does not erase history — that is a person's `git filter-repo`.

## Gate

`docsys lint --root .` — before any commit, after any change. Inside the
repository it also checks that a `verified` page still holds the body that
was verified (R-024): a changed body is an error until the page is
`unverified` again.
"#;

#[derive(Debug)]
pub struct Installed {
    pub written: Vec<String>,
    pub skipped: Vec<String>,
    /// what was decided rather than written: the gate's mode, a settings
    /// file left alone
    pub notes: Vec<String>,
}

/// The knowledge-base agent layer. Installed beside the base (`--kb`), never
/// mixed with the project layer: a knowledge base has no code to gate, and a
/// project has no inbox to ingest.
pub fn install_kb(claude_dir: &Path, base_dir: &Path, force: bool) -> Result<Installed, String> {
    let mut out = Installed {
        written: Vec::new(),
        skipped: Vec::new(),
        notes: Vec::new(),
    };
    // The hooks name the base relative to where the agent runs — the
    // directory holding `.claude/` — and that is `.` for a base that is its
    // own repository (D-076).
    let repo = claude_dir
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let root_arg = {
        let base_c = base_dir
            .canonicalize()
            .unwrap_or_else(|_| base_dir.to_path_buf());
        let repo_c = repo.canonicalize().unwrap_or_else(|_| repo.clone());
        match base_c.strip_prefix(&repo_c) {
            Ok(rel) if rel.as_os_str().is_empty() => ".".to_string(),
            Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
            Err(_) => base_dir.to_string_lossy().replace('\\', "/"),
        }
    };
    // The same four relays as a project — the binary reads the profile and
    // guards the record layer instead of asking the code-without-docs question.
    for (rel, template) in [
        ("hooks/pre-commit-docs.sh", PRE_COMMIT_DOCS),
        ("hooks/stop-docs-reminder.sh", STOP_DOCS_REMINDER),
        ("hooks/post-edit-updated.sh", POST_EDIT_UPDATED),
        ("hooks/session-intent.sh", SESSION_INTENT),
    ] {
        let path = claude_dir.join(rel);
        if path.exists() && !force {
            out.skipped.push(rel.to_string());
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let content =
            template.replace("${DOCS_ROOT:-docs}", &format!("${{DOCS_ROOT:-{root_arg}}}"));
        fs::write(&path, stamp(&content)).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o755));
        }
        out.written.push(rel.to_string());
    }
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
    // settings.json wires the hooks: written whole when absent, merged when
    // present (D-086) — MCP servers, permissions and the owner's own hooks
    // stay. Only a file that is not JSON is left alone, with the wiring in a note.
    let settings = claude_dir.join("settings.json");
    match wire_settings(&settings, KB_SETTINGS_SNIPPET)? {
        Wired::Created => out.written.push("settings.json".to_string()),
        Wired::Merged(n) => {
            out.written.push("settings.json".to_string());
            out.notes.push(format!(
                "settings.json: merged {n} docsys hook wire(s) into the existing file — MCP \
                 servers, permissions and your own hooks kept (D-086)"
            ));
        }
        Wired::AlreadyWired => out.skipped.push("settings.json".to_string()),
        Wired::Unparsable => out.notes.push(
            "settings.json: not valid JSON — untouched; wire the four hooks by hand: \
             UserPromptSubmit → session-intent.sh, PreToolUse matcher `Bash|Write|Edit` → \
             pre-commit-docs.sh, PostToolUse matcher `Write|Edit` → post-edit-updated.sh, Stop → \
             stop-docs-reminder.sh"
                .to_string(),
        ),
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
    // The git gate, as for a project: hard when the base lints clean inside
    // its repository, warn-mode while it carries debt (D-072).
    if repo.join(".git").exists() {
        let clean = crate::adopt::gate_clean(base_dir, &repo);
        let gate = crate::adopt::ensure_git_gate(&repo, &root_arg, clean);
        let mode = if clean {
            "hard"
        } else {
            "warn-mode until lint and refs are clean"
        };
        out.notes
            .push(format!("git pre-commit gate: {gate} ({mode})"));
    } else {
        out.notes
            .push("git pre-commit gate: skipped (not a git repository)".to_string());
    }
    Ok(out)
}

/// The knowledge-base wiring: the same four relays, PreToolUse also on
/// `Write|Edit` so the record layer is guarded before a byte moves (D-076).
pub const KB_SETTINGS_SNIPPET: &str = r#"{
  "hooks": {
    "UserPromptSubmit": [
      { "hooks": [ { "type": "command", "command": ".claude/hooks/session-intent.sh" } ] }
    ],
    "PreToolUse": [
      { "matcher": "Bash|Write|Edit",
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

pub fn install(claude_dir: &Path, force: bool) -> Result<Installed, String> {
    install_with_preamble(claude_dir, force, "")
}

/// `install`, with the owner's generated-file preamble (D-056) placed in
/// every markdown asset — never in a shell hook.
pub fn install_with_preamble(
    claude_dir: &Path,
    force: bool,
    preamble: &str,
) -> Result<Installed, String> {
    let files: [(&str, &str, bool); 9] = [
        ("hooks/pre-commit-docs.sh", PRE_COMMIT_DOCS, true),
        ("hooks/stop-docs-reminder.sh", STOP_DOCS_REMINDER, true),
        ("hooks/post-edit-updated.sh", POST_EDIT_UPDATED, true),
        ("hooks/session-intent.sh", SESSION_INTENT, true),
        ("commands/docsys-sync.md", DOC_SYNC, false),
        ("commands/docsys-seed.md", DOCSYS_SEED, false),
        ("commands/docsys-interview.md", DOCSYS_INTERVIEW, false),
        ("skills/docsys/SKILL.md", SKILL_MD, false),
        ("skills/docsys-export/SKILL.md", EXPORT_SKILL, false),
    ];
    let mut out = Installed {
        written: Vec::new(),
        skipped: Vec::new(),
        notes: Vec::new(),
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
            crate::migrate::with_preamble(content, preamble)
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

/// What `wire_settings` did to a settings file.
#[derive(Debug, PartialEq, Eq)]
pub enum Wired {
    /// no file: the snippet was written whole
    Created,
    /// an existing file: this many entries were appended to their events
    Merged(usize),
    /// every docsys command was already wired: nothing written
    AlreadyWired,
    /// not JSON (or `hooks` is not an object of arrays): never touched
    Unparsable,
}

/// Put the docsys hook wires into `.claude/settings.json` (D-086). Absent →
/// the snippet as it is. Present → parsed with the binary's own JSON reader
/// (D-051); for every event in the snippet, an entry whose commands are not
/// yet wired is appended to that event's list, and nothing else in the file
/// changes — MCP servers, permissions, the owner's hooks, key order. A file
/// the reader cannot parse is left exactly as it is: guessing at a person's
/// configuration is the clobbering D-028 refused.
pub fn wire_settings(path: &Path, snippet: &str) -> Result<Wired, String> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, snippet).map_err(|e| e.to_string())?;
        return Ok(Wired::Created);
    }
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let Some(mut doc) = crate::hook::parse_json(&text) else {
        return Ok(Wired::Unparsable);
    };
    let want = crate::hook::parse_json(snippet).ok_or("the hook snippet is not JSON")?;
    let Some(added) = merge_hook_wires(&mut doc, &want) else {
        return Ok(Wired::Unparsable);
    };
    if added == 0 {
        return Ok(Wired::AlreadyWired);
    }
    fs::write(path, doc.render()).map_err(|e| e.to_string())?;
    Ok(Wired::Merged(added))
}

/// Append the snippet's entries whose commands `doc` does not wire yet.
/// `None` when either side is not shaped `{"hooks": {<event>: [entries]}}`.
fn merge_hook_wires(doc: &mut Json, want: &Json) -> Option<usize> {
    let Json::Obj(fields) = doc else { return None };
    let Json::Obj(want_fields) = want else {
        return None;
    };
    let Json::Obj(want_events) = &want_fields.iter().find(|(k, _)| k == "hooks")?.1 else {
        return None;
    };
    if !fields.iter().any(|(k, _)| k == "hooks") {
        fields.push(("hooks".to_string(), Json::Obj(Vec::new())));
    }
    let Json::Obj(events) = &mut fields.iter_mut().find(|(k, _)| k == "hooks")?.1 else {
        return None;
    };
    let mut added = 0;
    for (event, entries) in want_events {
        let Json::Arr(want_entries) = entries else {
            return None;
        };
        if !events.iter().any(|(k, _)| k == event) {
            events.push((event.clone(), Json::Arr(Vec::new())));
        }
        let Json::Arr(list) = &mut events.iter_mut().find(|(k, _)| k == event)?.1 else {
            return None;
        };
        for entry in want_entries {
            let wanted = commands_of(entry);
            let wired = wanted
                .iter()
                .all(|c| list.iter().any(|e| commands_of(e).contains(c)));
            if !wired {
                list.push(entry.clone());
                added += 1;
            }
        }
    }
    Some(added)
}

/// The `command` strings of one hook entry (`{"matcher": …, "hooks": [{"type": "command", "command": …}]}`).
fn commands_of(entry: &Json) -> Vec<&str> {
    let Json::Obj(fields) = entry else {
        return Vec::new();
    };
    let Some(Json::Arr(hooks)) = fields.iter().find(|(k, _)| k == "hooks").map(|(_, v)| v) else {
        return Vec::new();
    };
    hooks
        .iter()
        .filter_map(|h| h.string_at(&["command"]))
        .collect()
}

/// The settings.json snippet for a project (the same wires `wire_settings`
/// merges into an existing file).
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

/// The seeding conversation, as a command: research by the tool, plain
/// questions by the agent, nothing written before the builder's word.
const DOCSYS_SEED: &str = r#"---
description: Seed documentation for one feature of an existing project — research in git, ask the builder plainly, write only what was confirmed
allowed-tools: Bash(docsys *), Bash(git log:*), Bash(git show:*), Bash(git status:*), Read, Grep, Glob, Write, Edit
---

# /docsys-seed <feature> — seed one feature

For a project whose documentation does not exist. The tool does the
research; you present it; the builder confirms, corrects and adds what
history cannot say. **Nothing is written before the builder says so.**

## 1 · Research (tool)

`docsys seed plan --repo . --root docs --target <feature>` — commits with
their bodies, files by touch count and the other features they serve, the
birth, manifests, `doc:` citations, the code's own comment blocks, tags.
If it is refused ("already covered by …"), stop: that page is the system's
now; drift goes through `/docsys-sync`, not through seeding.
If it says nothing names the feature, ask ONE question: where does it live
(a path, a scope, a symbol)? Then run it again with what you learned.

## 2 · Present ("what I found")

One block, in the tree's language (`default_content_language`), every line
carrying its evidence (`path:line`, `sha`): what the feature does, how it is
built, when it was born and moved, what broke and was fixed, what the
manifests declare, what the code's comments say about WHY. Derive; do not
ask what the code already answers.

## 3 · Ask — at most four questions, one at a time

Plain, single-meaning, answerable in a sentence, never a metaphor, never a
question that creates a conflict. The bank:

- Is this summary right? What is wrong or missing?
- <a specific fact the code cannot settle — a requirement vs a fallback, a
  product decision, an audience>
- <what the builder does with it that the code does not show>
- What is next for it — or is it finished and not to be touched?

If an answer conflicts with the evidence, show the evidence (`file:line`,
the test, the commit) and keep the question open until it is clear: it is
not an `answer` row yet — it becomes a `question` row that names the
evidence, and only the builder's next word settles it. An answer the
builder cannot give becomes a `question` row, dated today.

## 3b · Your own notes are questions, never text

If this machine holds agent memory for the repository (Claude Code keeps
`memory/*.md` under `~/.claude/projects/<repo-slug>/`), run the plan with
`--memory <that dir>`: each note's name and description becomes one line
of evidence and ONE question — "my notes say X; is it still true, and where
should it live?" The builder's answer is the source; the note is not. Never
paste a note into the tree.

## 4 · Approve, then land (tool)

Write the rows the conversation produced into a plan file OUTSIDE `docs/`
(`SEED.tsv`, never committed), show it, and wait for the explicit word. Then:
`docsys seed apply --plan SEED.tsv --repo . --root docs`.
When no builder can answer — a repository whose people are gone, a person
who says "land what history says, I will answer later" — the rows that need
nobody's memory still land on that person's word: `research` (the evidence,
reserved), `journal` (the chronology), `postmortem` (a commit's own account)
and `question` (everything the builder would have been asked). Only `answer`
rows wait for a builder; a plan with none is not a plan withheld.
Rows (TAB-separated; `docsys seed plan` prints the grammar): `research
<feature> <shas>` reserves the feature; `answer <feature> <who> <text>`
records the builder's words verbatim; `journal <date> <sha> <title>`
back-fills chronology at its own date; `postmortem <slug> <sha>` quotes an
incident's commit; `debt` and `question` add dated items.
Everything lands under `work/`. The permanent page comes later, through
graduation, when the builder confirms.

## 4b · The overview draft (the one page you may author)

After the rows land, one permanent page per seeded feature may be yours:
`docsys page new explanation <feature>-overview --unverified`, routed from
`index.md`, body written from the evidence only — what the feature is, how
it is built, when it was born and moved, what broke and why, what the
manifests and the code's own comments say — in the tree's language, with
`sources:` naming the same `git:` locators and files the research page
cites. It carries `verification: unverified`, and you never verify it:
a maintainer does, in another session (R-025, R-208). When the builder's
answers arrive, graduation moves them in byte-exact; the draft is where a
reader starts on day one, not the truth.

Never write prose of your own into the tree beyond that one page. Never
mark anything done or verified.
"#;

/// Rounds of the seeding interview across features — resumable, evidence
/// first, never a question git already answers.
const DOCSYS_INTERVIEW: &str = r#"---
description: Run the seeding interview across a project's undocumented features, one feature per round, resumable
allowed-tools: Bash(docsys *), Bash(git log:*), Bash(git show:*), Read, Grep, Glob, Write, Edit
---

# /docsys-interview — the seeding survey, round by round

`docsys seed gaps --repo . --root docs` lists every candidate feature with
its size, span and coverage. Uncovered features are the survey; covered
ones are the system's and are never asked about.

Each round is one feature, run exactly as `/docsys-seed <feature>`: research
by the tool, one "what I found" block, at most four plain questions, then
the builder's word before anything lands. Order: the largest uncovered
feature by commit count first, unless the builder names one. Stop when
the builder says stop; the next session resumes from `docsys seed gaps` —
what landed is reserved (`work/research/<feature>.md`, active) and will not
be asked again.

Rules that never bend: derive what history and code can say; ask only what
they cannot; a question is plain and single-meaning; a conflicting answer
is talked through, not recorded; nothing is written before approval; the
builder's words land verbatim, attributed and dated; the permanent layer is
never written here.
"#;

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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn stamp_goes_under_the_shebang_or_first() {
        let s = stamp("#!/usr/bin/env bash\nset -u\n");
        let mut lines = s.lines();
        assert_eq!(lines.next(), Some("#!/usr/bin/env bash"));
        assert_eq!(
            lines.next(),
            Some(format!("# docsys-template: {TEMPLATE_VERSION}").as_str())
        );
        assert_eq!(lines.next(), Some("set -u"));
        assert!(stamp("echo x\n").starts_with("# docsys-template: "));
    }

    #[test]
    fn every_hook_template_is_stamped_and_parseable() {
        let dir = std::env::temp_dir().join(format!("docsys-stamp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        install(&dir, false).unwrap();
        for rel in HOOK_FILES {
            let p = dir.join(rel);
            assert_eq!(
                template_version(&p).as_deref(),
                Some(TEMPLATE_VERSION),
                "{rel}"
            );
        }
        assert!(stale_hooks(&dir).is_empty());
        // an older stamp and a missing stamp are both named
        let hook = dir.join("hooks/session-intent.sh");
        fs::write(
            &hook,
            "#!/usr/bin/env bash\n# docsys-template: 0.0.1\nexit 0\n",
        )
        .unwrap();
        fs::write(
            dir.join("hooks/stop-docs-reminder.sh"),
            "#!/usr/bin/env bash\nexit 0\n",
        )
        .unwrap();
        let stale = stale_hooks(&dir);
        assert!(
            stale.contains(&("hooks/session-intent.sh".into(), "0.0.1".into())),
            "{stale:?}"
        );
        assert!(
            stale.contains(&("hooks/stop-docs-reminder.sh".into(), "unversioned".into())),
            "{stale:?}"
        );
        assert_eq!(stale.len(), 2);
        // stamp only within the first three lines — a mention deeper in a
        // script is not a stamp
        fs::write(
            &hook,
            "#!/usr/bin/env bash\nset -u\necho\n# docsys-template: 9.9.9\n",
        )
        .unwrap();
        assert_eq!(template_version(&hook), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn force_rewrites_and_plain_install_keeps() {
        let dir = std::env::temp_dir().join(format!("docsys-force-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        install(&dir, false).unwrap();
        let hook = dir.join("hooks/pre-commit-docs.sh");
        fs::write(&hook, "custom\n").unwrap();
        let kept = install(&dir, false).unwrap();
        assert_eq!(kept.written.len(), 0);
        assert_eq!(fs::read_to_string(&hook).unwrap(), "custom\n");
        let forced = install(&dir, true).unwrap();
        assert_eq!(forced.written.len(), 9);
        assert!(fs::read_to_string(&hook)
            .unwrap()
            .contains("docsys hook pre-tool-use"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hook_templates_are_valid_bash() {
        // The check needs a bash that can run: on a Windows runner `bash`
        // resolves to the WSL launcher, which has no distribution and fails
        // every script. A parser that cannot parse inspected nothing (R-011),
        // so the case is skipped there, not failed — the same templates are
        // parsed on the Unix legs of the matrix.
        let probe = std::process::Command::new("bash")
            .args(["-c", "echo bash-ok"])
            .output();
        let usable = probe
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("bash-ok"))
            .unwrap_or(false);
        if !usable {
            eprintln!("no usable bash on this host — template parse check skipped");
            return;
        }
        for (name, src) in [
            ("pre-commit", PRE_COMMIT_DOCS),
            ("stop", STOP_DOCS_REMINDER),
            ("post-edit", POST_EDIT_UPDATED),
            ("session-intent", SESSION_INTENT),
        ] {
            let p =
                std::env::temp_dir().join(format!("docsys-bash-n-{name}-{}", std::process::id()));
            fs::write(&p, src).unwrap();
            let ok = std::process::Command::new("bash")
                .arg("-n")
                .arg(&p)
                .status()
                .unwrap()
                .success();
            let _ = fs::remove_file(&p);
            assert!(ok, "{name} does not parse");
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod wire_tests {
    use super::*;

    #[test]
    fn merging_appends_only_the_unwired_entries_and_keeps_the_rest() {
        let mut doc = crate::hook::parse_json(
            r#"{"permissions":{"allow":["x"]},"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":".claude/hooks/pre-commit-docs.sh"}]}]}}"#,
        )
        .unwrap();
        let want = crate::hook::parse_json(KB_SETTINGS_SNIPPET).unwrap();
        let added = merge_hook_wires(&mut doc, &want).unwrap();
        assert_eq!(added, 3, "pre-commit-docs.sh was wired already");
        assert_eq!(merge_hook_wires(&mut doc, &want), Some(0), "idempotent");
        let out = doc.render();
        assert!(out.find("\"permissions\"").unwrap() < out.find("\"hooks\"").unwrap());
        assert_eq!(out.matches("pre-commit-docs.sh").count(), 1, "{out}");
        assert!(out.contains("session-intent.sh") && out.contains("stop-docs-reminder.sh"));
        assert!(merge_hook_wires(&mut Json::Arr(Vec::new()), &want).is_none());
        let mut no_hooks = crate::hook::parse_json(r#"{"permissions":{}}"#).unwrap();
        assert_eq!(merge_hook_wires(&mut no_hooks, &want), Some(4));
    }
}
