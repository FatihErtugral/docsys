# Findings

Every docsys gap the lab found, with the evidence that found it, the rule it
touches, and what was done. Pre-registered candidates carry the status
`open` until a session confirms or rejects them. Evidence under `out/` is
referenced by path shape only; nothing named is copied here.

| # | finding | found by | rule | status |
|---|---|---|---|---|
| 1 | R-027 (`cmd · MUST`) had no command: ingest taught `git mv` plus a hand edit of `sources:` | exploration before the harness | R-027 | fixed — `docsys raw move` (D-085) |
| 2 | an existing `.claude/settings.json` left the hooks unwired (`adopt`/`agents --kb` never merged) | a real repository with a settings file | D-028 | fixed — merged into, never overwritten (D-086) |
| 3 | the installed knowledge-base layer said nothing about consumed projects, `@namespace/id` at ingest, `inbox pull`, `status` | the rubric → installed-text gate | D-078, §20 | fixed — `AGENTS.md` → Sources beyond the inbox, `kb-ingest` step 3 (D-087) |
| 4 | the pre-commit gate went hard on "lint clean" while the gate itself also runs `refs`: a brownfield repo with a dangling `doc:` could not commit its own adoption | `mech/brownfield.sh` step 1 | R-076, D-072 | fixed — the mode follows lint and refs (D-088) |
| 5 | R-082 (`lint · MUST`, graduated is terminal) had no check at all | `mech/graduate.sh` step 6 | R-082 | fixed — checked at the gate against HEAD (D-089) |
| 6 | `seed plan --target <x>` returned an empty skeleton (`commits 0`) instead of its escape line when the word occurred in a diff but no commit was attributed | `mech/brownfield.sh` step 3 (`auth`) | seed, `/docsys-seed` §1 | fixed — the escape line prints; the diff hit is named as a mention |
| 7 | `tests/jarvis.rs` dated its pages with a literal day and failed the morning after | the first full test run of the day | R-106 | fixed — pages carry the run's date |
| 8 | `seed plan` does not read a manifest whose `"name"` is not at the start of a line (a one-line `package.json`) | `mech/brownfield.sh` step 4 | seed | open — low; real manifests are multi-line |
| 9 | the git connector lands a 201-file commit as a record with 201 file lines; `seed` excludes the same commit by rule (mega-commit) | `mech/brownfield.sh` step 7 | §20, D-054 | open — decide whether the connector should share the exclusion |
| 10 | `page new <type> <id>` in a knowledge base writes outside `wiki/<domain>/<type>/` and without the KB fields | exploration | R-024, R-029 | open — measure first: does any ingest session ship such a page? |
| 11 | `graduate apply` refuses an untracked plan file inside the repository as a dirty tree | plan design | R-097 | open — measure: does an agent put the plan under `docs/`? |
| 12 | connector record titles double the namespace (`"relay: relay: backoff …"`) when the subject already carries it | `mech/learn.sh` step 2 | §20.1 | open — cosmetic; decide |
| 13 | `/docsys-seed` as a slash command in `claude -p` | plan design | — | open — measure |
| 14 | how often the user-level "never commit without asking" rule stops a sandboxed session despite the preamble | plan design | — | open — measure |
| 15 | a base created by `docsys assistant` inside another repository shares that repository's history silently; the lab's own fixtures once nested under `ci/agent-lab/out/` and a `git add -A` in the base staged the lab's checkout | `mech/run.sh`, the first combined run | D-081 | fixed in the lab (fixtures live outside every repository, guarded); docsys now says `git: inside repository …` |
| 17 | the dry run (sonnet, F1-ingest, 0.80 USD, 34 turns): five pages, the duplicate merged, the split made, the contradiction logged — but the two notes left in the inbox got no open-questions line; the installed text said what to do with a note that fits no domain, nothing about noise | `agent/run-task.sh kb F1-ingest sonnet` | R-026, D-087 | fixed — `kb-ingest` step 1 names the noise case (the sentence went to the text, not the task) |
| 18 | sonnet's ingest added two qualifiers to the ledgerkit page ("the smallest size observed", "rounded down for a reason") that the record does not hold; the independent audit session caught both against the source and demoted the page | matrix sonnet-run1, F1-ingest → F1-audit | R-025, R-028 | measured — the two-session design works as intended; A1 fail on the ingest, B3 pass on the audit |
| 19 | the re-audit session rewrote `wiki/open-questions.md` in Turkish with a format preamble and items without checkboxes although `default_content_language: en` — the operator's Claude Code `language: turkish` setting leaked into page content; lint stayed clean because the file had no grammar at all | matrix sonnet-run1, F3-reaudit | R-108, R-120, D-083 | fixed — `wiki/open-questions.md` is a list file under R-108 (D-090), so the rewrite is now an error; the installed texts say every file under `wiki/` keeps the base's language whatever the session's setting says. The language itself stays a measured row (the setting is the person's) |
| 20 | three of five pages the learning session wrote were "Why <project> documents a promise": the first-docs commit retold as an explanation — a changelog line, not a decision | matrix sonnet-run1, F3-learn-ingest | R-031, D-087 | open — decide whether `AGENTS.md` → Sources beyond the inbox should say that a record whose only content is "documentation was added" holds nothing to distil |
| 21 | `seed apply` accepts `research auth -` for a feature `seed plan` said nothing names; the session both raised the question and reserved an empty research page | matrix sonnet-run1, F4-seed | seed, `/docsys-seed` §1 | open — decide whether a `research` row without evidence and without a path from the builder should be refused |
| 22 | the builder's answer "we measured 60 losing order on 2021-11-04" conflicts with the fixture's own history (2021-11-04 is the buffer fix; the window changed in 2022) — the session raised it as a question naming both commits instead of recording it | matrix sonnet-run1, F4-seed | `/docsys-seed` §3 | measured — correct behaviour; the fixture's answer 2 is now a second, unplanned conflict and stays |
| 23 | editing a lab script while a session runs breaks the running bash (incremental read) — twice; the child `claude` dies with its parent and leaves a transcript without a result event | dry run, sonnet-run1 | — | fixed in the lab — every runner re-executes from a private copy; `checks.sh` marks a transcript without a result event invalid |
| 24 | `/docsys-seed` §3 said a conflicting answer "keeps the question open" but not that it is no `answer` row: opus recorded the disputed "born in 2020" verbatim as an answer and raised the conflict beside it; sonnet turned it into a question only | matrix, F4-seed, both models | seed | fixed — the command text says it becomes a `question` row naming the evidence |
| 25 | the F2-graduate task text said "I have not confirmed anything yet" while the work files carry `confirmed: owner` — which R-081 requires for `status: done`; opus flagged the apparent contradiction in `questions.md`, sonnet did not notice. The fixture is right; the sentence was ambiguous (file-level transition vs. the done record) | matrix, F2-graduate | R-081 | fixed in the task text — "done and confirmed … I have not decided the file-level transition yet"; both models ran with the old wording |
| 26 | sonnet, F4-seed: the tool said nothing names `auth`; the first run raised the question and reserved an empty page, the rerun on the corrected fixture wrote neither — variance across runs, and a miss against `/docsys-seed` §1 both times | matrix sonnet-run1, F4-seed v1 and v2 | seed | measured — the escape (a `question` row) is in the installed text; the model does not take it reliably |
| 27 | a `pkill -f` by pattern from the operator's shell killed a running lab session (its private script copy matched) and left a transcript without a result event | sonnet-run1, F4-kb-pull-ingest v2 | — | fixed in the lab's practice — sessions are stopped by their run directory's pid, never by pattern; the cut run is kept as `.v2-cut-at-budget` and rerun |
| 28 | the matrix handed F4-kb-pull-ingest a repository path ending in `tree`, so the connector's default namespace was `tree`: opus renamed it with `--as ledgerkit`, sonnet pulled as `tree` and its pages and questions call the project "tree"; the real leg did the same with a directory named `seed` — opus reported it as a provenance question | matrix F4-kb-pull-ingest, real round 1 M1 learn | §20 | fixed in the lab — working copies carry the repository's own name; a difference in reading the situation, not in the tool |
| 29 | on a real repository with no builder, the session researched the feature, wrote `SEED.tsv` — and committed it, landing nothing under `docs/`: `/docsys-seed` §4 said "nothing is written before the builder says so", which makes a repository whose people are gone unseedable | real round 1, S1 sonnet | `/docsys-seed` §4, R-003 | fixed — §4 names the absent-builder case (evidence rows land on the person's word; only `answer` rows wait); the real task text now carries that word. Round 1 ran with the old text |
| 30 | `docsys seed plan` (no target) found 0 candidate features on a real medium repository (M2: firmware, no conventional-commit scopes — 4 of 41 subjects — and top-level directories outside FEATURE_ROOTS), so the runner skipped it; a named `--target <one of its modules>` returns a full history, a birth and a symbol trail | real round 1, M2 | seed, D-053 | open — the escape (name the feature) works and `/docsys-seed` §1 already asks where it lives; consider having the no-target inventory suggest top code directories when it finds zero scopes, and having the lab runner fall back to them |
| 31 | seeding a repository with nobody present left only skeletons under `work/` — honest, not readable; and the person's worry from the field: documentation that forms during development would carry a junior's guess as the project's word | real round 1 (every seed session), the person's question | R-003, R-025, R-081 | fixed — D-092: a project page may carry `verification:` (the wiki page's contract, D-077 included), `page new --unverified`, `.docmeta.yml maintainers:` with R-208 (name half in lint, author half in history), `/docsys-seed` §4b's one authored `explanation/<feature>-overview` page, `unverified`, for a maintainer to verify |
| 32 | the installed git pre-commit gate ran `docsys lint`, then `docsys refs`, in a script without `-e`: the last command's exit code decided, so a lint failure was masked whenever refs passed — every "hard" gate installed so far let a red tree commit; surfaced when D-093 appended `docsys gate` as the last line and `mech/learn.sh`'s deliberately red commit stopped landing | `mech/run.sh` after D-093 | R-097, D-072 | fixed — the block accumulates the three statuses and exits on any failure in hard mode; `adopt` rewrites an older block in place (`upgraded`); the lab script now follows D-077's order (unverified first, then the fetch commit, then the audit) |
| 33 | D-092/D-093 tried on the medium monorepo (M1) after the fact: `adopt` with the new binary merged the hooks and installed a hard gate; `commit_policy: require` and a declared maintainer set. A seed session (sonnet) landed research, two postmortems, journal, four questions AND the one authored page — an `explanation/<feature>-overview` page, `unverified`, from the evidence with `git:` locators, routed, its opening saying nobody confirmed it — and did not commit `SEED.tsv`. Two "make this one-line change and commit it" sessions (sonnet, opus) each committed the code WITH its documentation in one commit (a journal line; opus also a reference page, `unverified`, with `verifies:` pins and two questions) — the gate never had to refuse. Mechanical probes on the same copy: the relay refuses a code-only commit (exit 2), the git hook refuses it at the terminal (exit 1), the Stop hook holds once and passes on the `stop_hook_active` retry, `DOCSYS_SKIP=1` lands and leaves the debt line | out/real/d093-m1 | D-092, D-093 | measured — works in the field; no session wrote `confirmed:`/`verified_by:` |
| 16 | `--since YYYY-MM-DD` means "that day at this hour" to git (a bare date), years ≥ 2100 mean no bound, and the walk stops at the first commit older than the bound — `inbox pull --since <today>` lands nothing | `tests/seed.rs` while pinning `--since` | §20, seed | fixed — a bare date is normalized to the start of that day; the other two are git's and documented in the lab README |

## How a finding lands

`fixed` means: spec first (SPEC sentence or DECISIONS row), then the code,
then the test that pins it (`tests/*.rs`, a corpus case, or an `e2e.sh`
step), then CHANGELOG and README — in the same commit. `open` findings name
the measurement that decides them.

## What the synthetic matrix says (2026-09-03)

Same seeds, same binary, one model per chain; `REPORT-agent.md` holds every
row. The reading, in one place:

| | sonnet | opus |
|---|---|---|
| sessions (valid) | 11 (+3 cut and rerun) | 11 + the cross audit |
| cost, USD | ≈ 9.3 valid (11.05 with the cut runs) | 15.37 + 1.01 |
| turns per session | 27–70 | 19–40 |
| committed, lint 0/0 | every valid session | every session |
| automatic rows failing after review | 3 (auth escape, two numbers) | 4 (two rubric readings of the owner's word, a defensible R-093 call, one number) |

- **The two-session design holds.** Sonnet's ingest added two qualifiers
  the record does not carry; its own audit session and opus's cross audit
  both found them against the source, demoted the page and left the claims
  unedited (findings 18, cross run). No session verified a page it wrote.
- **Graduation is mechanical enough.** Both models moved blocks byte-exact,
  chose `link:` for the duplicate block, kept every `status:` until the
  owner's word, and added the word only where it was given. Opus read the
  fixture harder than the task text (finding 25); sonnet followed the table.
- **Where the models differ:** opus picked reference for contracts and
  explanation for decisions, left bodiless records in the inbox with a line,
  and named conflicts with commit ids; sonnet wrote three "why X documents a
  promise" pages (finding 20), missed the `auth` escape twice (finding 26)
  and once rewrote `open-questions.md` in the session's language (finding
  19 → D-090). Opus costs about twice per session and finishes in half the
  turns.
- **What went back into docsys from this leg:** D-087 (the layer carries
  every instruction), D-088 (gate verdict), D-089 (R-082), D-090
  (`open-questions.md` a list file), the `/docsys-seed` §3 sentence, the
  `kb-ingest` noise sentence, the `seed plan` escape line, `--since` bare
  dates, the `assistant` enclosing-repository line.
- **Environment, not docsys:** the operator's `language: turkish` setting
  reaches every session; reports come back in Turkish, pages stayed in
  English in every session but one (now an error by grammar).

## What real repositories added (round 1: small + medium)

Five of the user's own repositories, cloned read-only, adopted, and seeded by
a session with no builder present; M2 was skipped by feature-discovery
(finding 30). The synthetic flows held on real history:

- **Seeding is honest under a real absence.** Every session reserved the
  research page from the tool's evidence and turned what a builder would have
  answered into dated `question` rows — often sharper than the fixture's: an
  import nobody uses, a palette named one way in the code and another in
  `CLAUDE.md`, three brand renames in two days, a hard rule with no hook
  enforcing it, config files tracked in a scope but gone from HEAD. No session
  invented prose.
- **The one real gap the leg found:** `/docsys-seed` said "nothing is written
  before the builder says so", so two sessions committed the plan file and
  landed nothing under `docs/`. Fixed: §4 names the absent-builder case and
  the gate blocks a committed plan (D-091).
- **D-086 held in the field:** the medium monorepo already carried a
  `.claude/settings.json`; `adopt` merged the four hook wires into it,
  permissions kept, and the session ran with the hooks live.
- **Learning distils on real history:** 86 commits pulled, twelve pages that
  are decisions (a producer/consumer contract, a reversed internationalisation
  decision, production pitfalls of the product's language, a privacy-law
  choice), the rest of the pull left in the inbox with a reason, and the
  connector's namespace-from-directory wart caught as a provenance question.
- **Environment, not docsys:** the operator's `language: turkish` setting made
  every report come back in Turkish; pages stayed in the base's language.

**Round 1 spend:** ≈ 13.8 USD (8 sonnet + 6 opus sessions). Large repos
(round 2) wait on the user's word.
