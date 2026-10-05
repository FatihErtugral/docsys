//! `docsys agents` — installs the agent layer into a project: hooks that keep
//! documentation alive during sessions, the /docsys-sync command, and the thin
//! skill. A hook warns, and blocks only where the tree asks for it with
//! `commit_policy: require` (R-209) — hard blocking by default gets hooks
//! disabled entirely, which removes the protection (R-150) — and every
//! warning names what needs to change (R-152).

use crate::hook::Json;
use std::fs;
use std::path::Path;

/// The one channel that reliably reaches the model: a PreToolUse hook, where
/// exit 2 stops the tool call and the model reads stderr. Field report: every
/// warn-only channel (exit 0 stdout/stderr on PostToolUse/Stop) lands in the
/// transcript, not the model — a fully broken pipeline and a healthy one were
/// indistinguishable. Lint errors block outright (severity doctrine). The
/// code-without-docs invariant ASKS ONCE under `ask`: the first attempt stops
/// with the question, re-running the same commit proceeds — a wall gets hooks
/// disabled (R-151); a question does not. Under `require` it refuses every
/// time (R-209).
const PRE_COMMIT_DOCS: &str = r#"#!/usr/bin/env bash
# pre-commit-docs.sh — PreToolUse gate on `git commit`; the decision is made
# by `docsys hook pre-tool-use` (D-051): lint errors block; code without
# documentation is a question asked once per change set, and under
# `commit_policy: require` a refusal every time (R-209). DOCSYS_SKIP=1 bypasses once.
# In a knowledge base the same relay guards raw/: an existing record is never
# overwritten or edited through Write/Edit (R-023, D-076).
command -v docsys >/dev/null || exit 0
@DOCSYS_GUARD@
exec docsys hook pre-tool-use --root "${DOCS_ROOT:-docs}"
"#;

/// End-of-turn reminder: code moved, docs did not. Reads the working tree
/// AND the commits not yet pushed: an agent that commits as it goes leaves a
/// clean tree, and a reminder that read only the tree stayed silent through a
/// whole session of code-only commits (D-041).
const STOP_DOCS_REMINDER: &str = r#"#!/usr/bin/env bash
# stop-docs-reminder.sh — end-of-turn reminder; it warns, and under
# `commit_policy: require` it holds the turn once (R-209).
# Reads the working tree and the commits not yet pushed (`docsys hook stop`).
command -v docsys >/dev/null || exit 0
@DOCSYS_GUARD@
exec docsys hook stop --root "${DOCS_ROOT:-docs}" --stdin
"#;

/// The page bookkeeping after a docs edit: a verified page whose body changed
/// is demoted (R-024).
const POST_EDIT_UPDATED: &str = r#"#!/usr/bin/env bash
# post-edit-updated.sh — the bookkeeping of the edited docs page: a verified
# page whose body changed turns unverified (R-024), via `docsys hook
# post-tool-use` (reads the PostToolUse payload on stdin).
command -v docsys >/dev/null || exit 0
@DOCSYS_GUARD@
exec docsys hook post-tool-use --root "${DOCS_ROOT:-docs}"
"#;

/// Route documentation by work type, once per session, asking only when the
/// intent is genuinely ambiguous (a survey every session becomes noise).
const SESSION_INTENT: &str = r#"#!/usr/bin/env bash
# session-intent.sh — UserPromptSubmit hook; the routing text once per
# session, from `docsys hook user-prompt-submit` (work types for a project,
# the four organs for a knowledge base — the root's profile decides).
command -v docsys >/dev/null || exit 0
@DOCSYS_GUARD@
exec docsys hook user-prompt-submit --root "${DOCS_ROOT:-docs}"
"#;

/// What every relay runs before it hands over to the binary (D-099). It works
/// from the project directory whatever the session's directory is. A docsys
/// that answers `--version` runs the tree's pinned version by itself (D-120);
/// one that does not predates pins, and under a pinned tree it is refused in
/// one line naming the version — it would otherwise answer with a flood of
/// findings about fields it cannot read.
const RELAY_GUARD: &str = r#"cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
docsys_pin=$(head -n 1 "${DOCS_ROOT:-docs}/.docsys-version" 2>/dev/null)
if [ -n "$docsys_pin" ] && ! docsys --version >/dev/null 2>&1; then
  echo "docsys: this tree pins docsys $docsys_pin; install: cargo install docsys --version $docsys_pin --locked" >&2
  exit 1
fi"#;

/// A relay as it is written to disk: the guard in place, the tree's own root
/// as the default (relative to the repository, never an absolute path), and
/// the template stamp.
pub fn render_relay(template: &str, root_arg: &str) -> String {
    let root_arg = if root_arg.is_empty() { "." } else { root_arg };
    stamp(
        &template
            .replace("@DOCSYS_GUARD@", RELAY_GUARD)
            .replace("${DOCS_ROOT:-docs}", &format!("${{DOCS_ROOT:-{root_arg}}}")),
    )
}

const DOC_SYNC: &str = r#"---
description: Scan code↔doc drift and un-graduated done work; propose debt items
allowed-tools: Bash(git log:*), Bash(git diff:*), Bash(git show:*), Bash(docsys *), Read, Grep, Glob, Edit
---

# /docsys-sync — documentation drift check

Manual, never automatic. Report, wait for approval, commit nothing.

1. Mechanical pass: the rules block's two checks — include both outputs (one
   line each if green). Freshness errors are drift
   by definition: a stale pin names the region that moved, an untouched draft
   names abandonment.
2. Drift suspects: `docsys seed plan --repo . --root docs --since <the date
   of the last commit to docs/, `git log -1 --format=%cs -- docs/`>` — every
   feature history touched since, with
   its coverage. For each covered feature with commits, `git show --stat
   <sha> -- docs/`: did its page move with the code? Name the page that
   should have changed. An uncovered feature with commits is a seeding
   candidate, not drift.
3. Graduation debt: `grep -rl '^status: done' docs/work/` — for each, say
   concretely which section goes to which page.
4. For each debt, its `docsys debt add` command, ready to run.

No findings → say so; never invent debt.
"#;

const SKILL_MD: &str = r#"---
name: docsys
description: Documentation system operations — set up, migrate, audit, and curate a docsys tree. Use when the user asks to create or migrate documentation structure, audit docs health, graduate work files, or when starting work in a repo that has docs/.docmeta.yml.
---

# docsys — the thin skill

This skill adds judgment and approval gates to the commands; never skip a
gate.

## Set up

`docsys adopt`; `docsys help adopt` says what it writes.

## Migrate (existing docs anywhere in the repo)

1. `docsys migrate inventory --root <dir> --repo .`
2. Fill each TODO target — this is YOUR judgment call, per page, using the
   evidence and the P/R-031 procedure. **STOP: show the plan, get approval.**
3. `docsys migrate apply --plan <plan> --root <dir> --repo .`, then review its
   RISK lines: each is a judgment item, resolve or record as debt.

## Audit (report only)

Read the findings of the rules block's two checks with the R-049 / R-086
tables in hand. Change nothing; the user decides.

## Graduate (curation)

For each `status: done` work file, route its sections by the R-049 table;
destination pages are prepared first (R-099): `docsys graduate plan
<work-file>`, then `docsys graduate apply`. Every `doc:` citation of the
file's id moves to a destination in the same commit (D-127).

## Sources

A permanent page you author from evidence, or change in substance, names what
it rests on in `sources:`.

## Compile (a howto into a skill)

A gap found while running a compiled skill is reported on its page, never
patched in the skill (P/R-096).
"#;

/// `/docsys-upgrade`: the person and the agent finish what `docsys upgrade`
/// lists for a person (D-104). The tool applies what is mechanical; the agent
/// reads; the person decides.
const DOCSYS_UPGRADE: &str = r#"---
description: Move this repository's docs tree to the docsys you run — the tool applies what is mechanical, you read what needs reading, the person decides
allowed-tools: Bash(docsys *), Bash(git status:*), Bash(git diff:*), Bash(git log:*), Read, Edit
---

# /docsys-upgrade — move the tree, then finish what needs a person

The tool applies what is mechanical and you read what needs reading; the
person decides whether the tree moves, each edit you propose, each line they
keep as it is, and whether the follow-ups go in a commit or a pull request.
Nothing is committed before they said yes.

1. Run `docsys upgrade --json` and show the person the notes and the plan:
   `auto` items the tool applies, `manual` items that are theirs, `info`.
   Then `docsys upgrade --apply --commit`.
2. A CI workflow its owner edited (`ci-workflow`): an `auto` item is the
   move's own — show the person its diff. A `manual` item at `file:line`
   names what to change there so the install reads the pin: propose that
   edit, and nothing else of theirs.
3. A pin listed for a re-read (`pins`): re-read it as the rules block says.
   If the page holds, run the item's `command`; if not, propose the page edit.
4. A relay, skill, command or contract its owner edited (a diff): propose
   one text that keeps the owner's lines and takes the new ones.
5. A line of the team's own text that names a retired concept
   (`retired-concepts`, at `file:line`): propose that line rewritten with the
   item's replacement, keeping the owner's other words.
6. A sha256 value is never written by hand: `docsys upgrade` reads each
   archive's from the release into the install step. When it could not read
   the release it says so; run it again once it can.
7. The follow-ups are described by the upgrade commit's message
   (`git log -1 --format=%B`).
8. Last, the audit: the move ended with the leftover list — what an earlier
   version left, each with its file and its fix. Walk the person through each
   item, and through `docsys lint`, until both are clean, or what is left is
   what they keep.
"#;

/// `/docsys-crosscheck`: the person names the pages; the agent reads each
/// against what it names and the code it pins, corrects and records, then
/// stops.
const DOCSYS_CROSSCHECK: &str = r#"---
description: Read pages against their sources and the code — the pages the person names, or every page changed since a revision; correct what is wrong in one commit, record what cannot be settled
allowed-tools: Bash(docsys *), Bash(git add:*), Bash(git commit:*), Bash(git diff:*), Bash(git log:*), Bash(git show:*), Bash(git status:*), Read, Grep, Glob, Edit
---

# /docsys-crosscheck [<page>… | <ref>] — read pages against their evidence

The person starts it; you read, correct and record, then stop. The evidence
is what a page names in `sources:` and the code its pins resolve to now.

1. The pages: the ones the person named, or every page changed since the
   revision they gave (a branch, a tag, a commit). Given neither, ask which.
2. Run `docsys crosscheck <page>…` or `docsys crosscheck --since <ref>`: each
   page, its `sources:`, and each pin's code lines now —
   `path:L<first>-L<last>`, a whole file, or why the pin does not resolve.
   With `--since`, a consumed source that moved since then is marked so, and
   its page is read even when the page itself did not change.
3. Read each page whole, then every statement in it against those sources
   and lines. Follow a source to what it names; read beyond a pin's lines
   only where they leave a statement open.
4. A statement the code or a source contradicts: correct that statement,
   and nothing the evidence does not touch.
5. What you cannot settle — a statement the evidence neither confirms nor
   contradicts, two sources that disagree, a pin that no longer resolves
   while the code does not say which side is wrong: record it as the rules
   block says, `docsys question add` or `docsys debt add` with
   `--topic <page-id>`, and leave the page's words as they are.
6. The corrections go in one ordinary commit whose message says what was
   corrected on which page. Nothing marks a page as cross-checked: write no
   verification field, trailer or record, in the page or anywhere else.
7. End with a short report: the pages read, what was corrected, and each
   question or debt item recorded.
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
the tree's usual frontmatter plus `audience: <a>`, and gate with `docsys lint --root docs` until clean. **Show the first page and get
approval before authoring the rest.**

## 4. Language

`--lang <code>` states the document's language; WARNs name the pages declared
otherwise. Translating a page is editing that page: its structure stays, and
P/R-123 decides each term — when unsure whether something is a proper name,
keep it.
Re-run the export afterwards: the per-page stamps changed only where content
did, so only those sections needed the work.
"#;

// --- knowledge-base agent layer (`docsys agents --kb`) -----------------------
// The organs of a personal knowledge base: capture writes, ingest distils,
// lookup answers (a docsys/0.4 base also audits). The binary enforces the
// contract (ids, sources, raw immutability); these carry the judgment. Field-shaped: every rule here was paid for by a real
// base whose constitution predated the spec and matched it.

const KB_CAPTURE: &str = r#"---
name: kb-capture
description: Save something into the knowledge base — "note this", "remember this", "add to my brain", "log this lesson". Writes one note to raw/inbox/ with zero classification, commits it, and files it in the same turn when its page is clear.
---

# kb-capture — the write gate

Capture costs nothing or it does not happen. Never ask where it belongs.

1. Write ONE file to `raw/inbox/<YYYY-MM-DD>-<short-kebab-slug>.md`.
2. Content: the note in the user's own words, plus one line naming **why it is
   worth keeping** — that line is what makes it distillable later. A note
   taken from a source the person gave — a document, a page, a link — names
   it; `docsys inbox add --source <name> --id <item> --url <link>` writes
   such a record with its provenance.
3. Commit the note alone, staged by its path.
4. When the page it belongs on is clear, run kb-ingest on it in this same
   turn; when it is not, leave it — the next session ingests it first.
5. Confirm in one sentence.

Never paraphrase away a specific: a number, a version, an error string, a
command is the part that will be worth having.
"#;

const KB_INGEST: &str = r#"---
name: kb-ingest
description: Process the knowledge-base inbox — "process my inbox", "empty the inbox", "file these notes", or the note just captured. Distils raw notes into wiki pages, archives the source, runs the gate.
---

# kb-ingest — raw becomes knowledge

Distillation, not movement (R-092): the raw note is evidence and stays; the
wiki page is authored. Full discipline is applied HERE, so capture can stay
free.

For each file in `raw/inbox/`, or the one note just captured:

1. **Classify the domain** against `domains:` in `.docmeta.yml`, the closest
   declared domain first. Fits none? Add the domain to `domains:`, create
   `wiki/<domain>/` with this note's page, and tell the person in one line.
   A note that holds nothing to keep (noise) stays in the inbox with one open
   question naming it (`docsys question add`), so the inbox never grows in
   silence; deleting is never yours. The question is written in the base's
   language.
2. **A job done once?** A note that records a job finished for the first
   time is archived to its domain (step 8) with no page. When a second
   record of the same job arrives, the two become a `howto`: its steps, its
   pitfalls, and each step's why in one clause — a longer why is an
   `explanation` page the howto links. Both records go in its `sources:`.
3. **Pick the type** — `reference` (facts, values), `howto` (steps),
   `explanation` (why), `tutorial` (guided first run). Never mix types on one
   page (R-031); if a page starts holding steps AND concepts, split it. A
   rule — a correction, an "always" or a "never" from the person — is one
   line with its why in the domain's `reference` page of rules.
4. **A contradiction?** When the note says otherwise than a page, never write
   over the page: show the person both, each with its source, and let them
   decide; then update the one page that holds it. With the person away,
   leave the page and record an open question.
5. **Author or update** `wiki/<domain>/<type>/<slug>.md` with frontmatter:
   `id` (stable, kebab-case, never renamed), `type`, `domain`,
   `sources: [raw/…]`. A claim that
   rests on a consumed project's own page cites it as `@namespace/id`
   (materialized under `.federation/`; AGENTS.md → Sources beyond the inbox);
   a claim that rests on a connector record cites the record's path like any
   note.
6. Open with one or two sentences that stand alone (R-032): a reader arrives
   here from a search, not from the top of a chain.
7. **Route it**: add the page to `wiki/<domain>/index.md`, and the domain to
   `wiki/index.md` if new (R-035 grammar).
8. **Archive the source**: `docsys raw move raw/inbox/<note> <domain>
   --root <base>` — the note lands in `raw/<domain>/` with the same filename
   and the same bytes (R-023), and every `sources:` entry that pointed at the
   old path is rewritten by the tool (R-027). Never `git mv` and edit
   `sources:` by hand: the hand edit is where evidence trails were severed.
9. Gate: `docsys lint --root <base>` — finish clean or report what blocks;
   then commit what this ingest wrote, staged by path.
"#;

/// The knowledge base's audit organ: a docsys/0.4 base's, as 0.15.1 wrote it,
/// retired with page verification from docsys/0.5 on (D-130).
pub const KB_AUDIT_SKILL: &str = "skills/kb-audit/SKILL.md";

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
description: Answer from the knowledge base — "what do my notes say about X", "check my brain for X", "did I write anything about X". Read-only; answers source first, then the base's own record, each labelled, or says it does not know.
---

# kb-lookup — the read gate

Read-only. Never write, never fix what you find; report gaps instead.

1. `docsys lookup <words> --root <base>` — the mechanical first hop: every
   page, local and consumed (`@namespace/id`), that names all the words,
   scored by where they occur. Read the page it points at; a consumed page
   is another tree's contract and is cited as `@namespace/id`.
2. Nothing? `wiki/index.md` → the domain → `wiki/<domain>/index.md` → the
   page; then grep `wiki/` for tags and headings.
3. **Source first.** A consumed page, or a record the person gave as a
   source, answers first, labelled as the source; a wiki page answers after
   it, labelled as the base's own record. Say which one each part of the
   answer rests on, with its path.
4. Neither → **say you do not know: it is not in the base.** Never answer
   from your own knowledge while implying the base said it; offer to capture
   the question.

Any other `raw/` record is evidence, not an answer: quote it only to show
where a page came from.
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
  every file under `wiki/` — pages, indexes, open questions — keeps the
  base's `default_content_language`, whatever language the person or the
  session's own settings speak; code identifiers, commands and quotations
  are never translated
- Never: invent what the base does not hold · act outward without the
  person's confirmation · edit a record

## The loop

capture → `raw/inbox/`, committed · ingest → a wiki page + archived source,
in the same turn when the page is clear · lookup → an answer from a source
first, then from the base's own record, each labelled. Before a task, the
base is looked up: a rule or a howto it holds is followed. A note waiting from
an earlier session is ingested before anything else.

What the base learns without being asked — how the work is done:
- A correction, or an "always" or a "never" from the person → a rule at once,
  one line with its why in the domain's `reference` page of rules.
- A job finished once → one record naming it and its steps, archived to its
  domain with no page; a second record of the same job → a `howto` with its
  steps, its pitfalls, and each step's why in one clause.
- A failure that cost real time → its root cause, found by asking why until
  the answer is a cause the work can change (five times, as a rule of thumb)
  → a rule, or a pitfall on the howto.
- Not recorded: events, the state of a task, a one-off measurement, and what
  a source the base already cites says.
- In doubt: one question at the end of the task, every doubt in it.

What it learns on the person's word: "learn X from <source>" → the source
captured as a record that names it, then ingested; every page it yields
cites it.

Rules that are not mechanical:
- Domains are open: the closest declared domain first; when none fits, add
  the domain to `domains:` in `.docmeta.yml`, create its folder with the
  note's page, and tell the person in one line.
- A claim that contradicts a page never overwrites it: the person sees both,
  each with its source, decides, and the one page that holds it is updated.
  With the person away, it is an open question.
- A howto followed again after it was written → `docsys compile <howto>`
  makes it a skill; from then on, run the skill.
- An open question is a line in `wiki/open-questions/<topic>.md`, written by
  `docsys question add --topic <domain>`; `status` counts them, and an
  answered one leaves with its commit's `Answered:` line (R-108).
- Never invent. "Not in the base" is a complete answer.

## Sources beyond the inbox

- **Projects the base consumes** — `docsys consume add <path|git-url>` (or
  `docsys consume discover <dir>` to list the candidates under a directory)
  names a project in `.docmeta.yml`; `docsys fetch` materializes its
  exported pages under `.federation/<namespace>/`, committed as the baseline.
  A wiki page that rests on such a page cites it as `@namespace/id` in
  `sources:`.
- **The git connector** — on the person's word, `docsys inbox pull <repo>
  [--since <date>] [--limit <n>]` lands one record per commit of the
  project's default branch worth reading (bookkeeping commits — no body,
  docs only — are skipped unless `--all`) through the same write gate as any
  note; a second pull lands nothing twice. Choose the span and say why; then
  ingest the records like notes: what the project decided, not what it did.
- **The digest** — `docsys status` first: the inbox, the pages, open
  items, consumed namespaces, findings. `docsys assistant --root .
  --projects <dir>` stood this base up and keeps its consumed projects'
  pages current, read from their default branches, in one command.

## Hooks

`docsys agents --kb` wires three relays into `.claude/settings.json` (an
existing file is merged into, never overwritten): the first message of a
session gets the organ routing; a `Write`/`Edit` on an existing `raw/`
record is blocked (R-023) — new knowledge is a new file in `raw/inbox/`,
relocation is `docsys raw move`; `git commit` runs the gate; the end of a
turn names what waits in the inbox. Everything warns and nothing blocks, except the two
guards on the irreversible: the record and the commit.

## Forgetting

Only on the person's explicit word. `docsys forget <page|record> --reason
"…"` moves a page to `_archive/` with a tombstone (its identifier is never
reused) and a record to `raw/_forgotten/` (still a record, never read again,
never captured again); the ledger `.forgotten.yml` says when and why. Forget
the page before the records it rests on. It makes a topic unknown to every
organ; it does not erase history — that is a person's `git filter-repo`.

## Gate

`docsys lint --root .` — before any commit, after any change.
"#;

/// The knowledge base's contract as `agents --kb` writes it, the version
/// section included (D-120).
pub fn kb_contract() -> String {
    format!("{KB_AGENTS_MD}\n{}", crate::rules::VERSION_SECTION)
}

#[derive(Debug)]
pub struct Installed {
    pub written: Vec<String>,
    pub skipped: Vec<String>,
    /// what was decided rather than written: the gate's mode, a settings
    /// file left alone
    pub notes: Vec<String>,
    /// each written file's path, in `written`'s order — what a command names
    /// from the repository's top (D-098)
    pub paths: Vec<std::path::PathBuf>,
}

/// What an agent layer is written for: its directory, the tree it serves,
/// and the profile asked for. `agents`, `agents --kb` and `assistant` write
/// a layer through `install_layer` alone.
pub struct Layer<'a> {
    pub dir: &'a Path,
    pub tree: &'a Path,
    /// the tree as the relays name it, relative to the repository
    pub root_arg: &'a str,
    pub kb: bool,
    pub force: bool,
    /// the owner's generated-file preamble (D-056), a project's markdown only
    pub preamble: &'a str,
}

/// Write an agent layer. It serves the tree it names and no other: a
/// knowledge base's layer is refused on a tree that is none, a project's on a
/// knowledge base, so a project's gates and wires are never rewritten with
/// another profile's; and the relays are the ones the tree's own spec runs —
/// no post-edit relay on docsys/0.5 (D-118, D-130).
pub fn install_layer(l: &Layer) -> Result<Installed, String> {
    let docmeta = crate::tree::docmeta_at(l.tree);
    let is_kb = docmeta.as_ref().is_some_and(|f| {
        f.fields.get("profile").and_then(crate::fm::Value::as_str) == Some("knowledge-base")
    });
    let shown = crate::place::shown(l.tree);
    let shown = shown.display();
    match (l.kb, docmeta.is_some(), is_kb) {
        (true, _, true) | (false, _, false) => {}
        (true, true, false) => {
            return Err(format!(
                "`{shown}` is a project tree — `docsys agents` installs its layer; `--kb` is a knowledge base's"
            ))
        }
        (true, false, _) => {
            return Err(format!(
                "`{shown}` is no knowledge base (no .docmeta.yml) — `docsys init --profile knowledge-base --root {shown}` makes one"
            ))
        }
        (false, _, true) => {
            return Err(format!(
                "`{shown}` is a knowledge base — `docsys agents --kb` installs its layer"
            ))
        }
    }
    let mut done = if l.kb {
        install_kb_layer(l.dir, l.tree, l.force)?
    } else {
        install_project_layer(l.dir, l.force, l.preamble, l.root_arg)?
    };
    done.paths = done
        .written
        .iter()
        .map(|f| {
            if f == "AGENTS.md" && l.kb {
                l.tree.join(f)
            } else {
                l.dir.join(f)
            }
        })
        .collect();
    Ok(done)
}

/// The knowledge-base agent layer. Installed beside the base (`--kb`), never
/// mixed with the project layer: a knowledge base has no code to gate, and a
/// project has no inbox to ingest.
pub fn install_kb(claude_dir: &Path, base_dir: &Path, force: bool) -> Result<Installed, String> {
    install_layer(&Layer {
        dir: claude_dir,
        tree: base_dir,
        root_arg: "",
        kb: true,
        force,
        preamble: "",
    })
}

fn install_kb_layer(claude_dir: &Path, base_dir: &Path, force: bool) -> Result<Installed, String> {
    let mut out = Installed {
        written: Vec::new(),
        skipped: Vec::new(),
        notes: Vec::new(),
        paths: Vec::new(),
    };
    // The hooks name the base relative to where the agent runs — the
    // directory holding `.claude/` — and that is `.` for a base that is its
    // own repository (D-076). `.claude` has the empty path as its parent,
    // which would have made the base's absolute path the relays' default.
    let repo = claude_dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
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
    // The same relays as a project — the binary reads the profile and guards
    // the record layer instead of asking the code-without-docs question.
    let post_edit = keeps_post_edit(&repo, &root_arg);
    for (rel, template) in [
        ("hooks/pre-commit-docs.sh", PRE_COMMIT_DOCS),
        ("hooks/stop-docs-reminder.sh", STOP_DOCS_REMINDER),
        (POST_EDIT, POST_EDIT_UPDATED),
        ("hooks/session-intent.sh", SESSION_INTENT),
    ] {
        if rel == POST_EDIT && !post_edit {
            continue;
        }
        let path = claude_dir.join(rel);
        if path.exists() && !force {
            out.skipped.push(rel.to_string());
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&path, render_relay(template, &root_arg)).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o755));
        }
        out.written.push(rel.to_string());
    }
    for (rel, content) in [
        ("commands/docsys-upgrade.md", DOCSYS_UPGRADE),
        (CROSSCHECK_COMMAND, DOCSYS_CROSSCHECK),
        ("skills/kb-capture/SKILL.md", KB_CAPTURE),
        ("skills/kb-ingest/SKILL.md", KB_INGEST),
        (KB_AUDIT_SKILL, KB_AUDIT),
        ("skills/kb-lookup/SKILL.md", KB_LOOKUP),
    ] {
        // `docsys crosscheck` reads a docsys/0.5 base alone, and its audit
        // organ is a docsys/0.4 base's (D-130)
        let v04 = crate::era::Era::at(base_dir).page_verification();
        if (rel == CROSSCHECK_COMMAND && v04) || (rel == KB_AUDIT_SKILL && !v04) {
            continue;
        }
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
    match wire_settings(&settings, &kb_settings_snippet(post_edit))? {
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
        fs::write(&agents, kb_contract()).map_err(|e| e.to_string())?;
        out.written.push("AGENTS.md".to_string());
    }
    // The git gate, as for a project: hard when the base lints clean inside
    // its repository, warn-mode while it carries debt (D-072). A linked
    // worktree's `.git` is a file; git says whether this is a repository.
    // The gate is the repository's: a base inside another repository, or a
    // gate that serves another tree, is left as it is.
    let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    match crate::git::toplevel(&repo) {
        None => out
            .notes
            .push("git pre-commit gate: skipped (not a git repository)".to_string()),
        Some(top) if canon(&top) != canon(&repo) => out.notes.push(
            "git pre-commit gate: skipped — the base sits inside a repository whose gate is its own"
                .to_string(),
        ),
        Some(_) => match crate::adopt::gate_root(&repo).filter(|r| *r != root_arg) {
            Some(other) => out
                .notes
                .push(format!("git pre-commit gate: kept — it serves `{other}`")),
            None => {
                let clean = crate::adopt::gate_clean(base_dir, &repo);
                let message = crate::era::Era::at(base_dir).journal_from_history();
                let gate = crate::adopt::ensure_git_gate(&repo, &root_arg, clean, message);
                let mode = if clean {
                    "hard"
                } else {
                    "warn-mode until lint and refs are clean"
                };
                out.notes
                    .push(format!("git pre-commit gate: {gate} ({mode})"));
            }
        },
    }
    Ok(out)
}

/// The knowledge-base wiring: the same four relays, PreToolUse also on
/// `Write|Edit` so the record layer is guarded before a byte moves (D-076).
pub const KB_SETTINGS_SNIPPET: &str = r#"{
  "hooks": {
    "UserPromptSubmit": [
      { "hooks": [ { "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/session-intent.sh" } ] }
    ],
    "PreToolUse": [
      { "matcher": "Bash|Write|Edit",
        "hooks": [ { "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/pre-commit-docs.sh" } ] }
    ],
    "PostToolUse": [
      { "matcher": "Write|Edit",
        "hooks": [ { "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/post-edit-updated.sh" } ] }
    ],
    "Stop": [
      { "hooks": [ { "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/stop-docs-reminder.sh" } ] }
    ]
  }
}"#;

pub fn install(claude_dir: &Path, force: bool) -> Result<Installed, String> {
    install_with_preamble(claude_dir, force, "", "docs")
}

/// `install`, with the owner's generated-file preamble (D-056) placed in
/// every markdown asset — never in a shell hook — and the tree's root,
/// relative to the repository, as the relays' default.
const UPGRADE_COMMAND: &str = "commands/docsys-upgrade.md";

const CROSSCHECK_COMMAND: &str = "commands/docsys-crosscheck.md";

pub fn install_with_preamble(
    claude_dir: &Path,
    force: bool,
    preamble: &str,
    root_arg: &str,
) -> Result<Installed, String> {
    let repo = claude_dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    install_layer(&Layer {
        dir: claude_dir,
        tree: &repo.join(root_arg),
        root_arg,
        kb: false,
        force,
        preamble,
    })
}

fn install_project_layer(
    claude_dir: &Path,
    force: bool,
    preamble: &str,
    root_arg: &str,
) -> Result<Installed, String> {
    let files: [(&str, &str, bool); 11] = [
        ("hooks/pre-commit-docs.sh", PRE_COMMIT_DOCS, true),
        ("hooks/stop-docs-reminder.sh", STOP_DOCS_REMINDER, true),
        ("hooks/post-edit-updated.sh", POST_EDIT_UPDATED, true),
        ("hooks/session-intent.sh", SESSION_INTENT, true),
        ("commands/docsys-sync.md", DOC_SYNC, false),
        ("commands/docsys-seed.md", DOCSYS_SEED, false),
        ("commands/docsys-interview.md", DOCSYS_INTERVIEW, false),
        (UPGRADE_COMMAND, DOCSYS_UPGRADE, false),
        (CROSSCHECK_COMMAND, DOCSYS_CROSSCHECK, false),
        ("skills/docsys/SKILL.md", SKILL_MD, false),
        ("skills/docsys-export/SKILL.md", EXPORT_SKILL, false),
    ];
    let mut out = Installed {
        written: Vec::new(),
        skipped: Vec::new(),
        notes: Vec::new(),
        paths: Vec::new(),
    };
    // `.claude` given relative to the working directory: the repository is `.`
    let repo = claude_dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    // a docsys/0.4 tree keeps the post-edit relay, and the assets 0.15.1
    // wrote: no `/docsys-upgrade`, and none of this docsys's own texts, which
    // would teach it 0.5 — the upgrade writes them (D-118, D-130)
    let before_05 = keeps_post_edit(repo, root_arg);
    let tree_04 = before_05 && repo.join(root_arg).join(".docmeta.yml").is_file();
    for (rel, content, executable) in files {
        if (rel == POST_EDIT && !before_05)
            || ((rel == UPGRADE_COMMAND || rel == CROSSCHECK_COMMAND) && before_05)
        {
            continue;
        }
        let path = claude_dir.join(rel);
        if path.exists() && !force {
            out.skipped.push(rel.to_string());
            continue;
        }
        if tree_04 {
            let shown = crate::place::shown(&path);
            out.notes.push(if path.exists() {
                format!(
                    "this docsys/0.4 tree keeps its {}; `docsys upgrade` moves the tree to 0.5 and writes it",
                    shown.display()
                )
            } else {
                format!(
                    "this docsys/0.4 tree is missing {}; `docsys upgrade` moves the tree to 0.5 and writes it",
                    shown.display()
                )
            });
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let content = if executable {
            render_relay(content, root_arg)
        } else {
            crate::migrate::with_preamble(&render_root(content, root_arg), preamble)
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

/// Whether the settings file wires every docsys relay already.
pub fn settings_wired(path: &Path, post_edit: bool) -> bool {
    let Some(mut doc) = fs::read_to_string(path)
        .ok()
        .and_then(|t| crate::hook::parse_json(&t))
    else {
        return false;
    };
    crate::hook::parse_json(&settings_snippet(post_edit))
        .is_some_and(|want| merge_hook_wires(&mut doc, &want) == Some(0))
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
            // a relay is wired whatever spelling wires it (D-099)
            let wired = wanted.iter().all(|c| {
                let name = relay_name(c);
                list.iter().any(|e| {
                    commands_of(e)
                        .iter()
                        .any(|have| have == c || (name.is_some() && relay_name(have) == name))
                })
            });
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

/// The relay a hook command runs, whatever its spelling: quotes, a leading
/// `bash `/`sh `, a leading `cd "$CLAUDE_PROJECT_DIR" &&`, `${…}` braces and a
/// leading `./` removed, what remains is exactly `.claude/hooks/<name>.sh`
/// (D-099). A command that wraps the relay in anything else is the owner's,
/// and `None`.
pub fn relay_name(command: &str) -> Option<&'static str> {
    let mut c = command
        .replace(['"', '\''], "")
        .replace("${CLAUDE_PROJECT_DIR}", "$CLAUDE_PROJECT_DIR");
    c = c.trim().to_string();
    if let Some(rest) = c.strip_prefix("cd $CLAUDE_PROJECT_DIR") {
        c = rest.trim_start().strip_prefix("&&")?.trim().to_string();
    }
    for runner in ["bash ", "sh "] {
        if let Some(rest) = c.strip_prefix(runner) {
            c = rest.trim().to_string();
        }
    }
    let c = c.strip_prefix("$CLAUDE_PROJECT_DIR/").unwrap_or(&c);
    let c = c.strip_prefix("./").unwrap_or(c);
    HOOK_FILES
        .iter()
        .find(|rel| c.strip_prefix(".claude/") == Some(**rel))
        .and_then(|rel| rel.strip_prefix("hooks/"))
}

/// Whether a command wires a relay by a path relative to wherever the session
/// stands — it runs only while the session sits at the repository's top level.
pub fn is_relative_wire(command: &str) -> bool {
    relay_name(command).is_some() && !command.contains("CLAUDE_PROJECT_DIR")
}

/// Rewrite every docsys wire in a settings document to the one form and fold
/// duplicates of the same relay under one event into one (D-099). Returns the
/// number of changes; `None` when the document is not shaped `{"hooks": …}`.
/// A command that wraps a relay in anything else is the owner's and is left
/// as it is.
pub fn canonicalize_wires(doc: &mut Json) -> Option<usize> {
    let Json::Obj(fields) = doc else { return None };
    let Some((_, Json::Obj(events))) = fields.iter_mut().find(|(k, _)| k == "hooks") else {
        return Some(0);
    };
    let mut changes = 0;
    for (_, entries) in events.iter_mut() {
        let Json::Arr(list) = entries else {
            return None;
        };
        // a duplicate is the same relay under the same matcher; a second
        // matcher is a second wire, not a duplicate
        let mut seen: Vec<(String, &'static str)> = Vec::new();
        let mut kept: Vec<Json> = Vec::new();
        for mut entry in list.drain(..) {
            let matcher = entry.string_at(&["matcher"]).unwrap_or("").to_string();
            if let Json::Obj(ef) = &mut entry {
                if let Some((_, Json::Arr(hooks))) = ef.iter_mut().find(|(k, _)| k == "hooks") {
                    let before = hooks.len();
                    hooks.retain_mut(|h| {
                        let Some(name) = h.string_at(&["command"]).and_then(relay_name) else {
                            return true;
                        };
                        if seen.contains(&(matcher.clone(), name)) {
                            return false;
                        }
                        seen.push((matcher.clone(), name));
                        let canonical = format!("\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/{name}");
                        if let Json::Obj(hf) = h {
                            for (k, v) in hf.iter_mut() {
                                if k == "command" && *v != Json::Str(canonical.clone()) {
                                    *v = Json::Str(canonical.clone());
                                    changes += 1;
                                }
                            }
                        }
                        true
                    });
                    changes += before - hooks.len();
                    if hooks.is_empty() {
                        continue;
                    }
                }
            }
            kept.push(entry);
        }
        *list = kept;
    }
    Some(changes)
}

/// Take every wire of one relay out of a settings document — the post-edit
/// relay a docsys/0.5 tree no longer runs (D-130). The number taken out.
pub fn remove_relay_wires(doc: &mut Json, relay: &str) -> usize {
    let name = relay.strip_prefix("hooks/").unwrap_or(relay);
    let Json::Obj(fields) = doc else { return 0 };
    let Some((_, Json::Obj(events))) = fields.iter_mut().find(|(k, _)| k == "hooks") else {
        return 0;
    };
    let mut removed = 0;
    for (_, entries) in events.iter_mut() {
        let Json::Arr(list) = entries else { continue };
        list.retain_mut(|entry| {
            if let Json::Obj(ef) = entry {
                if let Some((_, Json::Arr(hooks))) = ef.iter_mut().find(|(k, _)| k == "hooks") {
                    let before = hooks.len();
                    hooks.retain(|h| h.string_at(&["command"]).and_then(relay_name) != Some(name));
                    removed += before - hooks.len();
                    return !hooks.is_empty();
                }
            }
            true
        });
    }
    events.retain(|(_, entries)| !matches!(entries, Json::Arr(l) if l.is_empty()));
    removed
}

/// The settings.json snippet for a project (the same wires `wire_settings`
/// merges into an existing file).
pub const SETTINGS_SNIPPET: &str = r#"{
  "hooks": {
    "UserPromptSubmit": [
      { "hooks": [ { "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/session-intent.sh" } ] }
    ],
    "PreToolUse": [
      { "matcher": "Bash",
        "hooks": [ { "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/pre-commit-docs.sh" } ] }
    ],
    "PostToolUse": [
      { "matcher": "Write|Edit",
        "hooks": [ { "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/post-edit-updated.sh" } ] }
    ],
    "Stop": [
      { "hooks": [ { "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/stop-docs-reminder.sh" } ] }
    ]
  }
}"#;

/// The seeding conversation, as a command: research by the tool, plain
/// questions by the agent, nothing written before the builder's word.
const DOCSYS_SEED: &str = r#"---
description: Seed documentation for one feature of an existing project from what its code and history say — research in git, ask the builder plainly, write only what was confirmed
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

## 3b · Agent memory

If this machine holds agent memory for the repository (Claude Code keeps
`memory/*.md` under `~/.claude/projects/<repo-slug>/`), run the plan with
`--memory <that dir>`, and ask the builder about each note as the rules
block says.

## 4 · Approve, then land (tool)

Write the rows the conversation produced into a plan file OUTSIDE `docs/`
(`SEED.tsv`, never committed) and show it; on the builder's word,
`docsys seed apply --plan SEED.tsv --repo . --root docs`.
When no builder can answer — a repository whose people are gone, a person
who says "land what history says, I will answer later" — the rows that need
nobody's memory still land on that person's word: `research` (the evidence,
reserved), `postmortem` (a commit's own account) and `question` (everything
the builder would have been asked); the chronology is history's own. Only `answer`
rows wait for a builder; a plan with none is not a plan withheld.
Rows are TAB-separated; `docsys seed plan` prints their grammar.

## 4b · The overview draft (the one page you may author)

After the rows land, per seeded feature:
`docsys page new explanation <feature>-overview`, its body
written from the evidence only — what step 2 presented — in the tree's
language, with `sources:` naming the same `git:` locators and files the
research page cites. The draft is where a reader starts on day one.
"#;

/// The docsys skill's text, as `agents` installs it.
pub fn skill_text() -> &'static str {
    SKILL_MD
}

/// Rounds of the seeding interview across features — resumable, evidence
/// first, never a question git already answers.
const DOCSYS_INTERVIEW: &str = r#"---
description: Collect what people know — the team's know-how, decisions and reasons — about one feature or every undocumented one, round by round
allowed-tools: Bash(docsys *), Bash(git log:*), Bash(git show:*), Read, Grep, Glob, Write, Edit
---

# /docsys-interview — the seeding survey, round by round

The survey is the list `docsys seed gaps --repo . --root docs` gives, the
largest feature first unless the builder names one. Each round is one
feature, run exactly as `/docsys-seed <feature>`. Stop when the builder says
stop; the next session resumes from `docsys seed gaps` —
what landed is reserved (`work/research/<feature>.md`, active) and will not
be asked again.

When the survey stops, name the next step:
`docsys graduate plan <work-file>` for what the builder confirmed, and each
page about code bound to its region as the rules block says.
"#;

/// Adoption report: what agent layer already exists, and which shell commands
/// it invokes. Detection is mechanical; deciding what to delegate to docsys is
/// judgment and stays with an agent and a human (D-026).
/// `all_own`: a docsys/0.5 tree's inventory leaves out every asset docsys
/// writes; before, the list 0.15.1 knew.
pub fn adoption_report(claude_dir: &Path, all_own: bool) -> Vec<String> {
    let own: Vec<String> = HOOK_FILES
        .iter()
        .map(|h| (*h).to_string())
        .chain(
            [false, true]
                .into_iter()
                .flat_map(owned_assets)
                .map(|(a, _, _)| a.to_string()),
        )
        .collect();
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
        if all_own && own.contains(&rel) {
            continue;
        }
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

/// The post-edit relay. A docsys/0.4 tree keeps it; on a 0.5 tree a page's
/// date is history's and a page carries no verification, so it has nothing
/// left to do (D-122, D-130).
pub const POST_EDIT: &str = "hooks/post-edit-updated.sh";

/// Whether the tree at `<repo>/<root_arg>` keeps the post-edit relay.
pub fn keeps_post_edit(repo: &Path, root_arg: &str) -> bool {
    crate::era::Era::at(&repo.join(root_arg)).page_verification()
}

/// The relays a tree is wired with.
pub fn relays(post_edit: bool) -> Vec<&'static str> {
    HOOK_FILES
        .into_iter()
        .filter(|rel| post_edit || *rel != POST_EDIT)
        .collect()
}

/// The post-edit relay's wire in a settings snippet.
const POST_EDIT_WIRE: &str = r#"    "PostToolUse": [
      { "matcher": "Write|Edit",
        "hooks": [ { "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/post-edit-updated.sh" } ] }
    ],
"#;

/// The project's settings snippet, with the post-edit relay or without.
pub fn settings_snippet(post_edit: bool) -> String {
    if post_edit {
        SETTINGS_SNIPPET.to_string()
    } else {
        SETTINGS_SNIPPET.replace(POST_EDIT_WIRE, "")
    }
}

/// The knowledge base's settings snippet, with the post-edit relay or without.
pub fn kb_settings_snippet(post_edit: bool) -> String {
    if post_edit {
        KB_SETTINGS_SNIPPET.to_string()
    } else {
        KB_SETTINGS_SNIPPET.replace(POST_EDIT_WIRE, "")
    }
}

/// Every relay name docsys has written — the ones it recognises in a wire.
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
        let first = install(&dir, false).unwrap();
        let hook = dir.join("hooks/pre-commit-docs.sh");
        fs::write(&hook, "custom\n").unwrap();
        let kept = install(&dir, false).unwrap();
        assert_eq!(kept.written.len(), 0);
        assert_eq!(fs::read_to_string(&hook).unwrap(), "custom\n");
        // --force rewrites every asset a plain install writes
        let forced = install(&dir, true).unwrap();
        assert_eq!(forced.written, first.written);
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
    #[test]
    fn a_template_names_the_trees_own_root() {
        let text = "Run `docsys lint --root docs --repo .`; propose `docs/work/debt.md` items; `git show <sha> -- docs/`; docsys docs/x docsify --root docs-site\n";
        assert_eq!(render_root(text, "docs"), text);
        assert_eq!(
            render_root(text, "documentation"),
            "Run `docsys lint --root documentation --repo .`; propose `documentation/work/debt.md` items; `git show <sha> -- documentation/`; docsys documentation/x docsify --root docs-site\n"
        );
        assert_eq!(
            render_root("`docs/work/` and --root docs\n", "."),
            "`./work/` and --root .\n"
        );
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

/// A relay as this binary writes it, by its path under `.claude/` — for
/// `docsys upgrade`, which refreshes a relay still in a docsys template's shape.
pub fn relay_for(rel: &str, root_arg: &str) -> Option<String> {
    let template = match rel {
        "hooks/pre-commit-docs.sh" => PRE_COMMIT_DOCS,
        "hooks/stop-docs-reminder.sh" => STOP_DOCS_REMINDER,
        "hooks/post-edit-updated.sh" => POST_EDIT_UPDATED,
        "hooks/session-intent.sh" => SESSION_INTENT,
        _ => return None,
    };
    Some(render_relay(template, root_arg))
}

/// The markdown assets docsys owns, by path under `.claude/`, with this
/// binary's text and whether it is new since 0.15 (written where it is
/// absent). `docsys upgrade` refreshes a file a release wrote and nobody
/// edited (`released`), and leaves any other text to its owner.
pub fn owned_assets(kb: bool) -> Vec<(&'static str, &'static str, bool)> {
    let mut out = vec![
        ("commands/docsys-upgrade.md", DOCSYS_UPGRADE, true),
        (CROSSCHECK_COMMAND, DOCSYS_CROSSCHECK, true),
    ];
    if kb {
        out.extend([
            ("skills/kb-capture/SKILL.md", KB_CAPTURE, false),
            ("skills/kb-ingest/SKILL.md", KB_INGEST, false),
            (KB_AUDIT_SKILL, KB_AUDIT, false),
            ("skills/kb-lookup/SKILL.md", KB_LOOKUP, false),
        ]);
    } else {
        out.extend([
            ("commands/docsys-sync.md", DOC_SYNC, false),
            ("commands/docsys-seed.md", DOCSYS_SEED, false),
            ("commands/docsys-interview.md", DOCSYS_INTERVIEW, false),
            ("skills/docsys/SKILL.md", SKILL_MD, false),
            ("skills/docsys-export/SKILL.md", EXPORT_SKILL, false),
        ]);
    }
    out
}

/// A command's or a skill's text with the tree's own root where the template
/// says `docs`: in `--root docs` and at the start of a `docs/` path. The relays
/// default to it the same way (D-099); a template with no root left in it reads
/// the same in every tree.
pub fn render_root(text: &str, root_arg: &str) -> String {
    let root = if root_arg.is_empty() { "." } else { root_arg };
    if root == "docs" {
        return text.to_string();
    }
    let prefix = if root == "." {
        "./".to_string()
    } else {
        format!("{root}/")
    };
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("docs") {
        let (head, tail) = rest.split_at(i);
        out.push_str(head);
        let before = out.chars().last();
        let after = tail.get(4..).and_then(|s| s.chars().next());
        let starts = before.is_none_or(|c| matches!(c, ' ' | '`' | '(' | '\'' | '"' | '\n'));
        if starts && after == Some('/') {
            out.push_str(&prefix);
            rest = tail.get(5..).unwrap_or("");
        } else if starts
            && out.ends_with("--root ")
            && after.is_none_or(|c| !(c.is_alphanumeric() || c == '-' || c == '_'))
        {
            out.push_str(root);
            rest = tail.get(4..).unwrap_or("");
        } else {
            out.push_str("docs");
            rest = tail.get(4..).unwrap_or("");
        }
    }
    out.push_str(rest);
    out
}

/// A relay as the released hashes read it: without its template stamp and
/// with its root default read as `docs` — the two things a render varies.
fn relay_norm(text: &str) -> String {
    let mut out: String = text
        .split_inclusive('\n')
        .filter(|l| !l.starts_with("# docsys-template:"))
        .collect();
    let open = "${DOCS_ROOT:-";
    let mut at = 0;
    while let Some(i) = out.get(at..).and_then(|s| s.find(open)).map(|i| at + i) {
        let start = i + open.len();
        let Some(end) = out
            .get(start..)
            .and_then(|s| s.find('}'))
            .map(|j| start + j)
        else {
            break;
        };
        out.replace_range(start..end, "docs");
        at = start + "docs".len();
    }
    out
}

/// The knowledge base's texts earlier releases wrote, as data: what a
/// three-way merge reads an owner's file against (D-134). Each one's sha256
/// is its row in `assets-released.tsv`, and a test holds them together.
pub const EARLIER_KB_TEXTS: [(&str, &str, &str); 12] = [
    (
        "AGENTS.md",
        "0.4.0",
        include_str!("../migrations/released/AGENTS.md@0.4.0"),
    ),
    (
        "AGENTS.md",
        "0.12.0",
        include_str!("../migrations/released/AGENTS.md@0.12.0"),
    ),
    (
        "AGENTS.md",
        "0.14.0",
        include_str!("../migrations/released/AGENTS.md@0.14.0"),
    ),
    (
        "AGENTS.md",
        "0.15.0",
        include_str!("../migrations/released/AGENTS.md@0.15.0"),
    ),
    (
        "AGENTS.md",
        "0.16.0",
        include_str!("../migrations/released/AGENTS.md@0.16.0"),
    ),
    (
        "skills/kb-capture/SKILL.md",
        "0.4.0",
        include_str!("../migrations/released/skills__kb-capture__SKILL.md@0.4.0"),
    ),
    (
        "skills/kb-ingest/SKILL.md",
        "0.4.0",
        include_str!("../migrations/released/skills__kb-ingest__SKILL.md@0.4.0"),
    ),
    (
        "skills/kb-ingest/SKILL.md",
        "0.15.0",
        include_str!("../migrations/released/skills__kb-ingest__SKILL.md@0.15.0"),
    ),
    (
        "skills/kb-ingest/SKILL.md",
        "0.16.0",
        include_str!("../migrations/released/skills__kb-ingest__SKILL.md@0.16.0"),
    ),
    (
        "skills/kb-lookup/SKILL.md",
        "0.4.0",
        include_str!("../migrations/released/skills__kb-lookup__SKILL.md@0.4.0"),
    ),
    (
        "skills/kb-lookup/SKILL.md",
        "0.12.0",
        include_str!("../migrations/released/skills__kb-lookup__SKILL.md@0.12.0"),
    ),
    (
        "skills/kb-lookup/SKILL.md",
        "0.16.0",
        include_str!("../migrations/released/skills__kb-lookup__SKILL.md@0.16.0"),
    ),
];

/// An owner-edited knowledge-base asset, merged with this version's text.
pub struct Merged {
    /// the owner's file with this version's template changes in it
    pub text: String,
    /// template changes on lines the owner changed too, not applied
    pub clashes: Vec<crate::diff::Clash>,
    /// runs of the owner's lines an earlier release wrote and this version
    /// no longer has: what is still to resolve with the person
    pub retired: Vec<Vec<String>>,
}

/// An owner-edited knowledge-base asset brought to this version's text by a
/// three-way merge (D-134). The base is the released text the owner's file
/// keeps the most lines of; while the file still holds a line an earlier
/// release wrote and this version no longer has, only the earlier texts are
/// candidates, so what the merge could not place stays named until it is
/// resolved. `None` for an asset with no earlier text.
pub fn merged(asset: &str, owner: &str, want: &str) -> Option<Merged> {
    let earlier: Vec<&str> = EARLIER_KB_TEXTS
        .iter()
        .filter(|(a, _, _)| *a == asset)
        .map(|(_, _, text)| *text)
        .collect();
    if earlier.is_empty() {
        return None;
    }
    // a numbered item is the same line under another number (D-134)
    let key = crate::diff::item_key;
    let now: std::collections::HashSet<String> = want.lines().map(key).collect();
    let before: std::collections::HashSet<String> =
        earlier.iter().flat_map(|t| t.lines()).map(key).collect();
    let is_retired = |l: &str| {
        let k = key(l);
        !l.trim().is_empty() && before.contains(&k) && !now.contains(&k)
    };
    let retired_in = |text: &str| {
        let lines: Vec<&str> = text.lines().collect();
        let mut runs: Vec<Vec<String>> = Vec::new();
        let mut run: Vec<String> = Vec::new();
        for (k, l) in lines.iter().enumerate() {
            let blank_inside = l.trim().is_empty()
                && !run.is_empty()
                && lines.get(k + 1).is_some_and(|n| is_retired(n));
            if is_retired(l) || blank_inside {
                run.push(l.to_string());
            } else if !run.is_empty() {
                runs.push(std::mem::take(&mut run));
            }
        }
        if !run.is_empty() {
            runs.push(run);
        }
        runs
    };
    let lines = |t: &str| t.lines().map(str::to_string).collect::<Vec<_>>();
    let mine = lines(owner);
    let shared = |base: &str| {
        crate::diff::edits(&lines(base), &mine)
            .iter()
            .filter(|e| matches!(e, crate::diff::Edit::Keep(..)))
            .count()
    };
    let current = retired_in(owner).is_empty().then_some(want);
    let base = earlier
        .iter()
        .copied()
        .chain(current)
        .fold(None::<(usize, &str)>, |best, text| {
            let n = shared(text);
            match best {
                Some((m, _)) if m > n => best,
                _ => Some((n, text)),
            }
        })?
        .1;
    let (text, clashes) = crate::diff::merge3(base, owner, want);
    let text = crate::diff::renumbered(&text);
    let retired = retired_in(&text);
    Some(Merged {
        text,
        clashes,
        retired,
    })
}

/// The texts docsys releases wrote for the assets it owns, as data (R-173):
/// `asset <TAB> sha256 <TAB> release`.
const RELEASED: &str = include_str!("../migrations/assets-released.tsv");

/// The release that wrote `text` for `asset` (a path under `.claude/`, or a
/// knowledge base's `AGENTS.md`), the tree's generated preamble set aside;
/// `None` when no release wrote it — the file is its owner's.
pub fn released(asset: &str, text: &str, preamble: &str) -> Option<&'static str> {
    let bare = if asset.starts_with("hooks/") {
        relay_norm(text)
    } else {
        crate::migrate::without_preamble(text, preamble)
    };
    let hashes = [
        crate::fresh::sha256_hex(text.as_bytes()),
        crate::fresh::sha256_hex(bare.as_bytes()),
    ];
    RELEASED
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let mut cells = l.split('\t');
            Some((cells.next()?, cells.next()?, cells.next()?))
        })
        .find(|(a, h, _)| *a == asset && hashes.iter().any(|x| x == h))
        .map(|(_, _, release)| release)
}
