# Roadmap

Where docsys stands after 0.14.0, what it will not grow into, and the next
slices in order. Decisions that bind land in `corpus/DECISIONS.md`; this file
is the intent, revised when the intent changes.

## Where it stands

- **Project profile** — complete for daily use: adoption in one command, the
  agent layer (three relays, five commands, two skills), graduation, seeding,
  capture and navigation, export and federation, mechanical freshness —
  `pins:`, history-dated pages, compiled skills, the range gate and the CI
  workflow (§11, D-066–D-073) — and the cross-check a person asks for
  (D-131).
- **Knowledge-base profile** — an assistant's memory: `docsys assistant` in
  one command, consumed projects as sources (`@namespace/id`), the git
  connector through the write gate, the digest (`status`), the cross-check
  of pages whose consumed sources moved, the character survey on the first
  turn, and `forget` (D-074–D-084, D-131).
- **Experimental** — federation (§13) and connectors (§20) bind nothing until a
  second real estate and a second real connector settle them.
- **Tested, not assumed** — `ci/agent-lab/` holds the distillation test
  campaign: dated fixtures for the four flows, a mechanical harness of exact
  expectations that CI runs, and the headless-agent leg whose findings go
  back into docsys (FINDINGS.md).

## The boundary: connectors are another project

docsys stays what it is: one zero-dependency binary, deterministic, no network,
no timers, no secrets (D-001, R-204, R-205). Connectors to calendars,
mailboxes, ticket trackers and chats need the opposite — OAuth, HTTP clients,
credentials, polling or webhooks, retries — so they live in a separate project
and meet docsys at one seam:

| Layer | Lives in | Owns |
|---|---|---|
| rules and mechanics | `docsys` | the record grammar (§20.1), `inbox add` and its `(source, source_id)` key, the git connector, `status`, lint |
| connectors | a separate project | source adapters, OAuth and tokens, cursors ("last item fetched"), the schedule |
| memory | the person's base | records and pages only — never a token, never a cursor |

The contract is the write gate: every connector lands records through
`docsys inbox add` (or, in Rust, through the `docsys` crate's `inbox::add`),
never by writing into `raw/` itself, so deduplication and provenance keep one
owner. A connector's conformance test is one line: run it against a fixture
source, then `docsys lint` the base.

The connector project has two faces over one core: CLI adapters a scheduler
runs (`cron`, `systemd`, `launchd`), and an MCP server exposing the same
adapters as tools an agent can call in a session — including outbound actions
(send, create, change), which stay skills run with the person's confirmation
(R-206). A long-running listener comes only when push sources (webhooks, chat)
need one, and it fronts the same adapters.

## Next slices, in order

1. **`docsys inbox check`** — validate a connector's records against the §20.1
   grammar (provenance fields, the key, the date), so a connector project can
   prove conformance without reading the spec. Small, in docsys.
2. **The connector project, opened** — a workspace depending on the `docsys`
   crate; the chat connector first ("note this" from any session, no OAuth),
   then calendar (read-only, lowest risk), then mail; the MCP face alongside
   the CLI face; the conformance script.
3. **`compile @namespace/id`** — a consumed project's howto compiled
   into the base's skills, so an assistant runs a project's procedure without
   opening the project.
4. **The nightly routine, as a template** — `docsys assistant` on a schedule,
   then an ingest session and a cross-check of what moved; documented once,
   outside the tree (R-205), with the morning briefing from `status`.
5. **Federation over HTTP** — a provider consumed without a checkout, and the
   consumer-impact report for a retired identifier (R-140).
6. **Second real estate, second real connector** — the evidence that lets §13
   and §20 leave EXPERIMENTAL, or forces the rules that must change first.
7. **A lone `-` as a file name** — `docsys rules --agents-md --write -` writes
   a file named `-`; refuse it by name, or read it as standard output. Found
   by the 0.16.0 release test; small, in docsys.

The follow-ups the 0.16.1 adoptions found, in seven real repositories. None
causes a merge conflict, so none is urgent:

8. **Pins the move widens** — the 0.4→0.5 move turns a symbol pin it cannot
   resolve into a whole-file pin (5 in one repository, 8 in another, on files
   of 1,200 lines and more); on a busy file that turns other teams' pull
   requests red on R-111. The move should list such pins instead.
9. **A retry in the CI install** — the 0.5 install step's download has no
   retry, so a transient error at the release host fails the docs job.
10. **Another patch of the same spec** — with a docsys other than the pinned
    one installed, every relay says "install it" on every command, and the
    guard's exit 1 does not block in the agent host, so a commit goes through
    without the lint gate. A newer patch on the same spec should not demand
    the older one; the regenerated gate should keep an owner's hardening (a
    blocking exit, no dead variable, `--skipped` not silenced).
11. **The template stamp alone** — every version rewrites the
    `# docsys-template:` line in six tracked files when nothing else changed.
12. **A clone's gate the project declined** — `upgrade --apply` in a clone
    with no git gate writes adopt's gate even where the project decided
    against a local pre-commit gate.
13. **The first leftover check after an upgrade commit** says "1 automatic";
    every later run says "no leftover" (seen twice in one repository).
14. **What the leftover check misses** — a 0.4 `verify-on-approval` CI job
    still calling `docsys verify --range … --commit` (refused with exit 2,
    hidden by `|| echo`, the job keeping `contents: write`), and
    retired-concept prose in work files ("stay unverified").
15. **The git-hook block under `set -e`** — `docsys_pin=$(head -n 1 …)` ends an
    owner's `set -e` hook silently when `.docsys-version` is absent.
16. **Generated text a formatter touches** — a formatter reformats generated
    files, after which the upgrade reads them as owner-edited; and the
    generated `docsys-sync.md` nests backticks that render wrong.
17. **Links lint lets through** — `[[work/debt]]` and `[[work/questions]]`,
    now folders, are accepted; a wiki-link inside a blockquote is not checked
    at all.
18. **`/docsys-crosscheck` pre-allows `git commit`** — the commit should be the
    person's call.
19. **The relay merge matches by script path** — `agents --kb` and
    `assistant` registered every relay twice where the owner's settings call
    it as `cd "$CLAUDE_PROJECT_DIR" && .claude/hooks/<relay>.sh`; and the
    `assistant` closing hint says `git add -A`, where a base whose sessions
    share one checkout stages by path.
20. **`page new` in a knowledge base** — `docsys page new howto <slug>` writes
    `howto/<slug>.md` at the base's top instead of `wiki/<domain>/howto/`;
    found by a stranger test of 0.17.0, present in 0.16.1.

## What will not be built here

- No network, no OAuth, no scheduler, no secrets store in the binary.
- No prose authored by the tool: openings, distillations, characters and
  briefings stay the model's, from what the tool derives.
- No erasure of history: `forget` makes a topic unknown; `git filter-repo` is
  a person's act.
