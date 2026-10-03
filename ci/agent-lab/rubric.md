# Rubric

One row per artifact and criterion; `pass` / `partial` / `fail` / `n/a`.
Any lint WARN in the final tree fails the lint row (the strict rule: a
warning is a finding). Rows marked **auto** are filled by `agent/checks.sh`
into `auto.tsv`; the rest are read by a person into `score.tsv` (same
columns: `artifact<TAB>row<TAB>score<TAB>note`), in under a minute each.

Every row names two sources: the SPEC rule it measures and the installed
text the agent read it in. A row with no installed text is a docsys gap and
is closed before the lab runs (D-087) — the task text never carries it.

## Where the agent read it (the gate before running)

| expectation | rule | installed text |
|---|---|---|
| a note that fits no domain stays in the inbox, a proposal goes to open questions | R-026 | `kb-ingest` step 1 |
| one type per page; steps AND a why are two pages | R-031 | `kb-ingest` step 2 |
| frontmatter with `sources:` | R-024 | `kb-ingest` step 3 |
| a consumed project's page is cited as `@namespace/id` | D-078 | `kb-ingest` step 3 · `AGENTS.md` → Sources beyond the inbox |
| the opening stands alone; the page is routed | R-032 / R-035 | `kb-ingest` steps 4–5 |
| the note moves with the same name and bytes; citing pages are rewritten | R-023 / R-027 | `kb-ingest` step 6 (`docsys raw move`) |
| a page is read against its sources when a person asks, corrected in an ordinary commit, and nothing records the reading | D-130, D-131 | the rules block's cross-check line · `/docsys-crosscheck` |
| a duplicate note updates the same page, both in `sources:` | R-024 | `kb-ingest` step 3 ("author or update") — measured |
| a contradicting note is surfaced, never silently resolved | R-151 | — measured; a silent resolution adds a sentence to `kb-ingest` step 3 |
| `inbox pull`: choose the span and say why; the second pull lands nothing | §20 / D-080 | `AGENTS.md` → Sources beyond the inbox |
| R-049 table, destinations first, byte-exact, never retyped, `confirmed:` needs the human's word | R-049 / R-099 / R-090 / R-081 | docsys skill → Graduate |
| link, not move, when the knowledge already exists | R-093 | docsys skill → Graduate |
| covered → stop; nothing names it → one question, else a `question` row | seed | `/docsys-seed` §1, §3 |
| an answer that conflicts with the evidence stays open → a `question` row naming the evidence | seed | `/docsys-seed` §3 |
| `SEED.tsv` outside `docs/`, never committed; apply after the word; the absent-builder case | R-003, D-091 | `/docsys-seed` §4 |
| the one authored page: `explanation/<feature>-overview`, from the evidence | D-131 | `/docsys-seed` §4b |
| a page you write from evidence, or change in substance, names its `sources:` | R-024 | docsys skill → Sources |
| lint 0/0 before a commit | R-097 | docsys skill → Always · the pre-commit relay · the Stop relay |

## A · an ingest page (one set per wiki page the session wrote)

| row | criterion | how |
|---|---|---|
| A1 faithfulness | every claim traces to a line of a listed source; `partial` = one unsupported qualifier; `fail` = any invented fact, number or step | read (checks.sh prints page and sources side by side) |
| A2 verbatim | every token of `fixtures/notes/tokens.tsv` for the sources used survives unchanged | **auto** |
| A3 place | domain and type match `fixtures/notes/expected.tsv`; `split` has two pages; `stay` notes are untouched in the inbox | **auto** for location; type fit read |
| A4 opening | the first sentences say what the page is and when to read it; `fail` if they assume the note was read | read |
| A5 frontmatter | `id type domain sources`, and no `verification:` or `updated:` (D-122, D-130) — every note used is listed | **auto** (lint R-024/R-026/R-029) + read for completeness |
| A6 router | a line in `wiki/<domain>/index.md`; the domain in `wiki/index.md`; no R-034/R-035 | **auto** |
| A7 raw relocated | each processed note under `raw/<domain>/` with the same basename and sha256; `stay` notes still in the inbox; nothing deleted | **auto** |
| A8 lint | `0 error(s), 0 warning(s)` at the final commit | **auto** |
| A9 committed | commits after `seed`, no leftovers, no AI signature in a message | **auto** |
| A10 stress | duplicates merged (one page, two sources); the contradiction surfaced; noise and no-domain notes left with an open-questions line; the split | **semi-auto** (page count per domain/type, `grep` in `wiki/open-questions.md`) + read |

## B · an audit session (withdrawn)

A docsys/0.5 page carries no verification, so no session audits one (D-130). Reading pages
against their sources is the cross-check a person asks for, measured by `M-crosscheck` (rows C*
under the stranger test below).

## C · graduation

| row | criterion | how |
|---|---|---|
| C1 mapping | blocks landed where `fixtures/project/expected-dispositions.tsv` says (destination holds the block, source holds the link) | **auto** |
| C2 byte-exact | every line the destination gained is a line of the source before graduation | **auto** |
| C3 prepared | destinations made with `page new`, an authored opening, routed from `index.md` | lint + read |
| C4 link | the duplicate `## Notes` is `Already documented: [[reference/keys\|keys]].`, `reference/keys.md` unchanged | **auto** |
| C5 status | F2-graduate: every `status:` unchanged; F2-confirm: `cart-key` carries `confirmed: owner, <date>` (and `graduated` only if empty of value), the other two untouched | **auto** + read |
| C6 escape | nothing is both moved and kept; an ambiguity is linked or left with a `questions.md` item | read |
| C7 lint + commit | 0/0, committed, no leftovers | **auto** |

## D · brownfield seeding

| row | criterion | how |
|---|---|---|
| D1 refusals honored | no `work/research/cli.md`; `auth` became a question, not a page | **auto** |
| D2 tokens | every sha in the landed rows resolves; each `answer` text equals the person's text byte for byte; real-repo runs carry no `answer` row | **auto** |
| D3 nothing invented | research and postmortem pages hold only the tool's skeleton and attributed blockquotes | **auto** (non-template, non-quote body lines = 0) |
| D4 conflict surfaced | answer 3 ("born in 2020") is a `question` row naming the birth evidence, not an `answer` | **auto** grep + read |
| D5 chronology | journal rows dated by the commit's own date, newest first (R-104) | **auto** (lint) |
| D6 items | debt and question items in R-108 grammar, each traceable to a body or a TODO | lint **auto** + read |
| D7 numbers | `4096`, `30`, `2021-11-04` unchanged where quoted | **auto** |
| D8 language | the Turkish subject quoted untranslated | **auto** |
| D9 lint + commit | 0/0, `SEED.tsv` not under `docs/`, committed | **auto** |
| D10 overview draft | one `explanation/<feature>-overview` per seeded feature, `sources:` naming the research page's `git:` locators, routed, body from the evidence only, no verification field (D-130) | **auto** for frontmatter and routing; faithfulness read |

## E · a base learning from a repository

| row | criterion | how |
|---|---|---|
| E1 pull | `inbox pull` ran with a span and a stated reason; a second pull would land nothing | **auto** count + read |
| E2 sources | pages cite records (and `@namespace/id` where the project is consumed); R-059 clean | **auto** |
| E3 distilled | the page answers one R-031 question about what was decided and why, not "commit X did Y" | read |
| E4 numbers | the commit body's number survives | **auto** |
| E5 noise | no bookkeeping commit landed in `raw/inbox/` | **auto** |

## Scoring a pair (sonnet vs opus)

A difference is reported when a row is `pass` for one and `fail` for the
other, when a docsys gap was hit by one model only, when cost differs more
than 5× or turns more than 2×, when a hook fired for one only, or when one
stopped before committing despite the sandbox. Wording, router order, id
choice and commit-message style are not differences. A split that does not
survive one rerun of the failing model is labelled `variance`.

## M · the stranger test (backlog M3)

A fresh session, told only what a person who installed docsys would say. `M-stranger` starts from
`fixtures/gen-stranger.sh`, where nothing is adopted; `M-interview` starts from the same repository
already adopted.

| row | criterion | how | installed text |
|---|---|---|---|
| S1 adopted | `docs/.docmeta.yml` committed | **auto** | `docsys --help`, first lines |
| S2 type | the policy's page is `reference/` or `explanation/` | **auto** (directory) + read for fit | P/R-031 in the rules block; ADOPTION.md step 2 |
| S3 pins | the page pins symbols of `src/retry.ts` in `pins:`; a whole-file pin is `partial` | **auto** | the rules block's binding line; ADOPTION.md step 3 |
| S5 no state | no `verification:` field (D-130) | **auto** | the rules block |
| S6 sources | `sources:` is non-empty and resolves (no R-059) | **auto** | the docsys skill → Sources |
| S7 lint · refs | 0 errors, 0 warnings; refs clean | **auto** | the pre-commit relay; the gate |
| S8 no restatement | no signature or parameter list retold from the code | read | P/R-033 in the rules block |
| S9 committed | commits after `seed`, clean tree, no AI signature | **auto** | the preamble |
| I1 the interview | the session reached for `/docsys-interview` (or its first step, `seed gaps`) | **auto** | the interview's description; the first-turn routing; ADOPTION.md step 1 |
| I2 into work/ | what landed is under `work/` | **auto** | `/docsys-interview`, `/docsys-seed` §4 |
| I3 no page before confirmation | nothing permanent beyond the overview draft | **auto** | `/docsys-seed` §4, §4b |
| C1 the mode | `M-crosscheck`: the session ran `docsys crosscheck`, as `/docsys-crosscheck` says | **auto** (transcript) | the rules block's cross-check line; `/docsys-crosscheck` |
| C2 corrected | the retry page says what `src/retry.ts` says now: 5 attempts | **auto** | `/docsys-crosscheck` step 4 |
| C3 no state | no verification field in a page, no `Verifies:` or `Approved-by:` in a commit | **auto** | `/docsys-crosscheck` step 6 |
| C4 committed | the correction is committed; lint 0/0 | **auto** | `/docsys-crosscheck` step 6 |
