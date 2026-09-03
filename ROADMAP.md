# Roadmap

Where docsys stands after 0.14.0, what it will not grow into, and the next
slices in order. Decisions that bind land in `corpus/DECISIONS.md`; this file
is the intent, revised when the intent changes.

## Where it stands

- **Project profile** — complete for daily use: adoption in one command, the
  agent layer (four relays, three commands, two skills), graduation, seeding,
  capture and navigation, export and federation, and mechanical freshness:
  `verifies:` pins, history-dated pages, compiled skills, the range gate and
  the CI workflow (§11, D-066–D-073).
- **Knowledge-base profile** — an assistant's memory: `docsys assistant` in
  one command, consumed projects as sources (`@namespace/id`), the git
  connector through the write gate, the digest (`status`), a verification
  checked against its body and its sources, the character survey on the first
  turn, and `forget` (D-074–D-084).
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
3. **`compile @namespace/id`** — a consumed project's verified howto compiled
   into the base's skills, so an assistant runs a project's procedure without
   opening the project.
4. **The nightly routine, as a template** — `docsys assistant` on a schedule,
   then an ingest session and an audit session; documented once, outside the
   tree (R-205), with the morning briefing from `status`.
5. **Federation over HTTP** — a provider consumed without a checkout, and the
   consumer-impact report for a retired identifier (R-140).
6. **Second real estate, second real connector** — the evidence that lets §13
   and §20 leave EXPERIMENTAL, or forces the rules that must change first.

## What will not be built here

- No network, no OAuth, no scheduler, no secrets store in the binary.
- No prose authored by the tool: openings, distillations, characters and
  briefings stay the model's, from what the tool derives.
- No erasure of history: `forget` makes a topic unknown; `git filter-repo` is
  a person's act.
