# Knowledge base — the contract

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
