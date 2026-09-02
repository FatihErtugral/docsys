# docsys

[![ci](https://github.com/FatihErtugral/docsys/actions/workflows/ci.yml/badge.svg)](https://github.com/FatihErtugral/docsys/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/docsys.svg)](https://crates.io/crates/docsys)

**A documentation system that agents and humans keep honest — mechanically.**

Plain markdown + git. A single zero-dependency binary enforces the mechanics;
an LLM handles only the judgment calls, following authored decision procedures.
Nothing lives in two places, nothing goes stale silently, and nothing blocks
your commit unless the damage would be irreversible or silently wrong.

Built spec-first: every behavior traces to a numbered rule in [SPEC.md](SPEC.md)
(147 normative rules, survived six adversarial audit rounds by six independent
models, plus two experimental sections — federation and connectors — that bind
nothing until real use settles them), every implementation-defined choice is
registered in [corpus/DECISIONS.md](corpus/DECISIONS.md), and the conformance
corpus keeps the binary from drifting looser *or* noisier than the rules.

Two profiles, one binary. A **project** keeps its documentation next to its
code and honest against it; a **knowledge base** is a person's memory — and,
consuming the projects, an assistant's: `docsys assistant --root ~/jarvis
--projects ~/code` is the whole setup.

---

## The idea in one diagram

Knowledge flows one way: captured with zero discipline, processed with full
discipline, distilled into a permanent layer the code can point at.

```mermaid
flowchart LR
    subgraph WORK["work/ — flowing layer (has status, ends)"]
        direction TB
        F["features/ · postmortems/ · research/"]
        J["journal.md<br/>entries ≤ 5 lines"]
        J -->|"over 500 lines"| S["journal/<br/>archive slices"]
    end

    subgraph PERM["permanent layer (id: is the contract)"]
        direction TB
        REF["reference/ — facts code cannot state"]
        HOW["howto/ — procedures"]
        EXP["explanation/ — why · ADRs"]
    end

    CODE["source code"]
    SKILL["executable skill"]

    F ==>|"graduate — byte-exact,<br/>never rewritten"| PERM
    CODE -->|"doc: &lt;id&gt;<br/>never a path"| PERM
    PERM -.->|"verifies: SHA-256 pin (§11)<br/>lint fails when the region moves"| CODE
    HOW -->|"compile (complete steps only)<br/>skill pinned to the page's hash"| SKILL
```

Which section of a work file lands in which permanent type is a lookup, not a
guess — the template sections map to destinations (R-049): contract surface →
`reference/`, decisions and rejected alternatives → `explanation/`, procedural
lessons → `howto/`.

Two contracts carry everything:

- **The identifier is the contract, the filename is cosmetic.** Code cites
  documentation as `doc: <id>`; rename the file, nothing breaks (R-062).
- **Deterministic work belongs to the tool, judgment to the model.** Links,
  frontmatter, uniqueness, budgets, block movement → `docsys`. "Which type is
  this?", "does permanent value remain?" → an agent, following the fifteen
  authored procedures (R-003, §14.3).

## Who does what

```mermaid
flowchart TD
    subgraph TOOL["docsys  (deterministic — never authors prose)"]
        T1[lint · refs · gate · status<br/>validation and the digest]
        T2[init · adopt · migrate · graduate<br/>fetch · inbox — movement, scaffolding, records]
        T3[rules · agents · pin · compile<br/>generated layer, hashes, skills]
    end
    subgraph MODEL["LLM  (judgment — never moves bytes)"]
        M1[classify: which type?]
        M2[route: which block where?]
        M3[write: openings, distillations]
        M4[verify: in another session]
    end
    subgraph HUMAN["human  (authority)"]
        H1[approve plans]
        H2["confirm done / graduated"]
    end

    TOOL -- "plan skeletons<br/>+ evidence" --> MODEL
    MODEL -- "filled plans" --> HUMAN
    HUMAN -- approval --> TOOL
```

The plan file is the boundary: the tool emits an inventory with evidence, the
model fills the mapping, a human approves, the tool executes byte-exactly. The
model never retypes text; the tool never guesses a classification.

## Adopting an existing project (the common case)

```mermaid
sequenceDiagram
    participant U as human
    participant A as agent
    participant T as docsys

    U->>T: docsys migrate inventory --root documentation --repo .
    T-->>A: plan skeleton — per file: heading, link counts, target: TODO<br/>+ inbound-reference report (README, CI, code)
    A->>A: classify each file (Diátaxis, procedure P/R-031)
    A->>U: filled plan — STOP, approval gate
    U->>T: docsys migrate apply --plan plan.tsv --repo .
    T->>T: move files · inject frontmatter · rewrite links<br/>(in-tree + inbound, even inside URLs) · scaffold router & work/
    T-->>U: summary + RISK lines (judgment leftovers) + lint report
```

Proven on a real firmware repository: 63 flat files migrated, 16 in-tree and
13 inbound references rewritten (README, CONTRIBUTING, CI, even demo payloads
carrying the repo's own GitHub URLs), and the result linted down to exactly the
two pre-existing violations the old tree already had.

## The session loop (agent layer)

`docsys agents` installs four hooks, three commands (`/docsys-sync`,
`/docsys-seed`, `/docsys-interview`) and two skills. The hooks are two-line
relays: every decision is made by `docsys hook <event>` in the binary — a real
JSON parser for the payload, heredoc-aware command detection, git paths read
unquoted — and pinned by unit tests (D-051). `docsys rules` generates the
agent-facing text **from the embedded spec**; there is no hand-maintained rules
copy to drift (R-155).

```mermaid
flowchart LR
    S([session starts]) --> SI["session-intent hook<br/>classify work type once"]
    SI --> WORK["agent works<br/>judgment via docsys rules --procedures"]
    WORK -- "edits docs page" --> PU["post-edit hook<br/>bumps updated:"]
    WORK -- "commit" --> PC["pre-commit hook<br/>docsys gate"]
    WORK -- "turn ends" --> ST["stop hook<br/>code moved, docs or journal didn't?<br/>(tree + unpushed commits) → remind"]
    PC -- "lint errors" --> BLOCK[BLOCKED — fix first]
    PC -- "code moved, docs didn't" --> ASK["asks ONCE — the same<br/>commit again proceeds"]
    PC -- "docs moved too" --> DONE([commit])
```

One channel blocks — the pre-commit hook, where exit 2 stops the call and the
model reads the reason: lint **errors** block outright, and the
code-without-docs question **asks once** (the marker lives until HEAD moves,
so a retry that dropped its `git add` is caught, not waved through). The other
hooks warn and never block: a wall gets hooks disabled, a question does not
(R-150, R-151, D-040, D-043, D-049).

The same four relays serve a knowledge base (`docsys agents --kb`); the binary
reads the root's profile and changes what they guard: a `Write`/`Edit` on an
existing `raw/` record is blocked (the one irreversible write), the first turn
names the four organs instead of the work types, `updated:` bumps on wiki
pages only, and the end of a turn names what waits in the inbox (D-076).

## Graduation — the heart

```mermaid
flowchart LR
    PLAN["graduate plan<br/>block inventory:<br/>lines · checksum · snippet"] --> FILL["model fills:<br/>keep / link:dest / move:dest"]
    FILL --> APPROVE{human approves}
    APPROVE --> APPLY["graduate apply"]
    APPLY --> D1["destination written first<br/>bytes arrive exact"]
    APPLY --> D2["source keeps template headings,<br/>gains link + graduated_to"]
    APPLY --> D3["refuses: dirty tree ·<br/>drifted source · missing destination"]
```

"Content is never rewritten, only moved" stopped being a rule the model must
obey and became a guarantee the command enforces (R-090): the model selects
the mapping, the tool copies the bytes.

## Freshness — drift is a hash, not a reviewer

Documentation goes wrong quietly: the code moves, the page does not. Five
checks make that loud, all of them errors, none of them a reviewer's memory:

```mermaid
flowchart LR
    CODE["code region"] -- "verifies: sha256 pin" --> PAGE["permanent page"]
    PAGE -- "compile" --> SKILL[".claude/skills/&lt;id&gt;<br/>docsys_source_hash"]
    HIST["git history"] -- "last change vs updated:<br/>days since draft moved" --> PAGE
    PROV["provider page<br/>(consumed, @ns/id)"] -- "fetch: provenance hash" --> WIKI["wiki page<br/>verified at rev"]
    WIKI -- "body at verified_rev<br/>sources at verified_rev" --> LINT{lint}
    PAGE --> LINT
    SKILL --> LINT
    LINT -- "moved" --> ERR["ERROR, named:<br/>re-read, then pin --refresh / compile / audit"]
```

- **`verifies:`** — `docsys pin <page> <path> [--symbol <s>]` records a code
  region's SHA-256 on the page; lint recomputes it on every run and a moved
  region is an error until the page is re-read and `pin --refresh`ed (§11,
  R-111). Symbols resolve as brace or `def`/`class` blocks; an ambiguous one
  is an error, never a guess (D-069).
- **History** — one `git log` walk dates every page: `updated:` behind the
  page's last commit (R-106) and a `draft`/`active`/`done` file untouched
  beyond `stale_active_days` (R-085) are errors (D-070, D-071).
- **Compiled skills** — `docsys compile <howto>` carries the page's hash; the
  skill is an error once the page moved (R-095).
- **Verification** — a `verified` wiki page is checked against the body it
  held at `verified_rev` (D-077) **and** against the consumed sources it rested
  on (D-082): when a provider's page moves and `fetch` brings the new hash, the
  pages verified against the old one fail by name.
- **CI** — `adopt` writes `.github/workflows/docsys.yml` (lint, refs, and
  `gate --range` on a pull request), and the pre-commit gate is hard as soon
  as the tree lints clean (D-072).

## Export — a document for a reader, out of a large tree

A tree is written page by page; a reader wants one document. `export` composes
one, mechanically: bodies are carried **verbatim** (heading levels shift, prose
is never rewritten), every section carries a source stamp (identifier, file,
content hash, date), and the run **refuses to half-compose** — a missing,
flowing, retired or unfetched identifier fails with the complete list.

```sh
docsys export plan --root docs --audience end-user     # what exists for this reader
docsys export feature subghz-listen subghz-record \
  --audience end-user --lang tr --title "SubGHz Guide" \
  --root docs --out guide.md                            # one feature, no map file
```

Two declarations make the same tree serve different readers, and the tool
determines neither of them:

- **`audience:`** on a page (the vocabulary is the tree's own `audiences:`;
  undeclared reads as `developer`). A page of the wrong audience named on a map
  is refused; one reached by `--follow` becomes a named gap — "no end-user
  counterpart exists" is a work item, not a silent omission.
- **`lang:`** per page. `--lang` states the document's intent and warns page by
  page; the tool translates nothing — translation is agent work, and code
  identifiers, product names and quotations keep their original form.

The composer never writes prose. An end-user document exists when end-user
pages exist — the `docsys-export` skill carries that procedure (discover the
gap, author with approval, compose), so the workflow lives in the repository,
not in someone's head.

Regeneration is stateless — a cache is state, and state drifts — but an
unchanged result never touches the output file, so nothing downstream
re-triggers on a no-op.

## Federation — one feature, several repositories

A feature with a foot in three services should still read as one document. A
consumer declares its providers; `fetch` materializes their exported pages
locally; `@namespace/id` composes beside local identifiers.

```yaml
# docs/.docmeta.yml of the consuming (or a thin product) repository
consume_base: "git@github.com:acme/{ns}.git#docs"   # one template…
consume: [auth, billing, payments]                  # …and just names
```

```sh
docsys export manifest --root docs --out manifest.docsys   # each provider publishes
docsys fetch --root docs                                   # consumer materializes
docsys export feature app-side @auth/token-ttl --root docs --out guide.md
```

The manifest is why this scales: an index of ids, hashes, titles and summaries
— **no bodies** — so a refresh downloads what actually changed instead of
cloning estates. On a real 66-page tree the manifest is 20 KB where the
repository is 70 MB. Foreign pages compose only from the verified local state
(never a live query); an unfetched or locally edited materialization is refused
by name, `internal: true` pages never cross the boundary, and every foreign
stamp carries its fetch date so a stale composition is visible. A provider
that publishes no manifest is still consumable — the tree is the index then.

The consumer's list grows without hand-editing and lives nowhere but in its
own `.docmeta.yml`: `docsys consume discover ~/code` lists the docsys trees
under a directory, `docsys consume add <path|git-url>` appends one, reading
the provider's `namespace:` (which `adopt` writes into every tree). A consumed
page is evidence too: a knowledge-base page may cite `@namespace/id` in its
`sources:`, and `docsys lookup <words>` searches local and consumed pages in
one pass (D-074, D-075, D-078).

## An assistant's memory — the knowledge base, its connectors, staying current

The other profile is a person's memory. Consuming the person's projects, it
becomes an assistant's — and the mechanics are the same ones above, pointed
the other way.

```mermaid
flowchart LR
    subgraph IN["what comes in"]
        GIT["git connector<br/>inbox pull"]
        CONN["any connector<br/>inbox add --source --id"]
        NOTE["a note in a session<br/>capture"]
    end
    subgraph BASE["the base (knowledge-base profile)"]
        RAW["raw/inbox/ — records,<br/>content-immutable, provenance"]
        WIKI["wiki/&lt;domain&gt;/&lt;type&gt;/<br/>sources: raw/… or @ns/id"]
        FED[".federation/&lt;ns&gt;/<br/>consumed pages + provenance"]
    end
    PROJ["docsys projects<br/>(consume add · fetch)"]
    GIT & CONN & NOTE --> RAW
    RAW -- "ingest (session)" --> WIKI
    PROJ -- "fetch" --> FED
    FED -- "learn (session):<br/>sources: [@ns/id]" --> WIKI
    WIKI -- "audit (another session)" --> VER["verified_by · verified_rev"]
    FED -. "provider moved → fetch →<br/>lint: page stale, by name" .-> WIKI
    WIKI -- "compile (verified howto)" --> SKILL["skill"]
    BASE -- "status" --> BRIEF["morning briefing<br/>(the model's words)"]
```

- **Records, not pages.** A connector lands one file per item in `raw/inbox/`
  with its provenance (`source`, `source_id`, `title`, `captured`, `url`); the
  same item lands once, so a schedule is safe; no record is ever edited — the
  hook blocks the attempt. `docsys inbox add` is the gate every connector
  calls; the git connector (`docsys inbox pull <repo>`) is built in and skips
  bookkeeping commits unless `--all` (§20, D-079).
- **Learning from projects.** A page distilled from consumed pages cites them
  as `@namespace/id`; lint resolves the citation against the materialization
  and refuses an unfetched one (D-078).
- **Staying current.** The base pulls, nothing pushes (R-205): re-running
  `docsys assistant` (or `fetch` and `inbox pull` on a schedule) brings each
  project's new pages and commits; a page verified against a source that has
  since moved is an error until re-read and audited again (D-082); `docsys
  status` says what waits — inbox, unverified pages, moved sources, stale
  skills — and the assistant's morning words are the model's, from that
  (D-080).
- **What it may never do** is mechanical too: verify its own page (R-025),
  edit a record (R-023), answer from memory when the base does not have it
  (the lookup skill), act outward on its own (R-206).

## Commands

| Command | What it does |
|---|---|
| `docsys adopt [--repo .] [--root docs] [--lang <code>] [--obsidian]` | One-command integration: docmeta (or the full init skeleton on a fresh project) with the tree's `namespace:`, agent assets, `settings.json` when absent, AGENTS.md managed block, the git pre-commit gate (hard when the tree lints clean, warn-mode while it carries debt, hardened by a later run), `.github/workflows/docsys.yml` when `.github/` exists, and an `ADOPTION.md` report whose checklist carries every judgment call. Idempotent. |
| `docsys seed plan [--target <feature>] [--since <date>] [--memory <dir>]` · `docsys seed apply --plan <file> [--force]` · `docsys seed gaps [--since <date>]` | Brownfield seeding: evidence from history and code, refused when a page covers the feature; the approved rows land under `work/` as tokens and verbatim quotations (D-053, D-058). |
| `docsys debt close <n> [--note <line>]` · `docsys journal add <text> [--title <t>] [--date <d>] [--link <path>]` · `docsys page new <kind> <id> [--title <t>]` | Capture, mechanical: a repaid debt leaves the ledger with its journal line; an entry at its date; a page from its template (D-063). |
| `docsys backlinks <path\|id> [--repo .]` · `docsys mentions [<path\|id>]` · `docsys graph [--format dot\|json\|jsoncanvas] [--repo .]` | Derived navigation, never written into a page: who points at a page (code included), who names it without linking, the whole map (D-064). |
| `docsys adopt --obsidian` | The docs root as an Obsidian vault: absolute links, `_archive/` ignored, `_templates/` as templates, a `stale-work.base` view (D-065). Caveats: `aliases:` means retired ids here; keep Linter's `yaml-timestamp` off. |
| `docsys lint [--root docs] [--repo <dir>] [--json]` | Full tree validation: frontmatter, ids, links, journal discipline, templates, list grammars — both profiles. Inside a git repository (`--repo`, or detected) also the freshness rules: `verifies:` pins recomputed (R-111), `updated:` behind history (R-106), drafts untouched beyond `stale_active_days` (R-085). Errors exit 1, warnings don't. |
| `docsys lookup <word…> [--root docs] [--json]` | A question's first hop: every page, local and consumed (`@namespace/id`), that names every word, best first — identifier, title, tags, summary, body — with `status:` on a draft and `unverified` on an unaudited page. `raw/` is never listed. No hit exits 1: "not in the base" (D-074). |
| `docsys consume add <path\|git-url>[#subdir] [--as <ns>]` · `docsys consume discover <dir>` | Grow this tree's `consume:` list from a checkout or a git URL, reading the provider's `namespace:`; list the docsys trees under a directory as candidates without writing. The list lives in this tree's `.docmeta.yml` and nowhere else (D-075). |
| `docsys inbox add --source <name> --id <item> [--title <t>] [--url <u>] [--date <d>] [<file>\|-]` · `docsys inbox pull <repo> [--since <date>] [--limit <n>] [--as <ns>] [--all]` | The connector write gate (§20): one record into `raw/inbox/` with its provenance, the same item landing once; and the built-in git connector, one record per commit, bookkeeping commits skipped unless `--all` (D-079). |
| `docsys assistant [--root .] [--projects <dir>]… [--domains a,b] [--since 30.days] [--limit 3]` | An assistant's memory in one command: the base, its agent layer, every docsys project under the given directories consumed and fetched, their recent commits as records, the digest. Idempotent (D-081). |
| `docsys status [--root .] [--repo <dir>] [--json]` | The digest an assistant reads first: inbox, pages by state, open questions and debt, consumed namespaces and their fetch day, compiled skills, and lint's findings folded by rule. Derived on every run, never stored (D-080). |
| `docsys compile <howto> [--root docs] [--dir .claude] [--force]` | A howto whose steps are complete becomes an executable skill: the page body byte for byte under `.claude/skills/<id>/`, pinned to the page's content hash. Lint fails while the page has moved since the compile (R-094, R-095, D-073). |
| `docsys pin <page> <path> [--symbol <s>]` · `docsys pin --refresh <page>` | Pin a permanent page to a code region — the whole file or one symbol's block — with its SHA-256 (§11); refresh every pin after re-reading the page. Lint fails while a pinned region has moved. |
| `docsys init [--root docs] [--lang <code>] [--profile project\|knowledge-base]` | Greenfield skeleton. `project`: router, journal, debt. `knowledge-base`: the record layer (`raw/inbox/`) and the wiki root. |
| `docsys migrate inventory [--root <dir>] [--repo <dir>]` · `docsys migrate apply --plan <file> [--root <dir>] [--lang <code>] [--repo <dir>]` | Brownfield adoption: evidence-rich plan → approved mapping → mechanical move with link rewriting on both sides of the docs boundary. |
| `docsys refs --repo <dir> [--root <dir>] [--json]` | Validate every `doc: <id>` in the code base against the tree (typos stop being invisible). |
| `docsys graduate plan <work-file> [--root <dir>]` · `docsys graduate apply --plan <file> [--root <dir>] [--force]` | Byte-exact block movement from work files to the permanent layer; `--force` overrides the dirty-tree refusal. |
| `docsys export plan [--audience <a>]` · `docsys export product <map> [--out <file>] [--lang <code>] [--audience <a>]` · `docsys export feature <id>… [--follow] [--title <t>] [--out <file>] [--lang <code>] [--audience <a>]` | Compose a document from permanent pages: a draft map from the tree's own evidence, a whole product from an authored map, or one feature by identifier (`--follow` widens a hop). Bodies verbatim, source-stamped, `--audience` and `--lang` aware. |
| `docsys export manifest [--root <dir>] [--out <file>]` | Publish what this namespace exports — id, type, title, summary, content hash, no bodies. A few KB where a clone is megabytes. |
| `docsys fetch [--root <dir>]` | Materialize consumed namespaces into `.federation/`: manifest first, unchanged pages skipped, provenance recorded. |
| `docsys rules --agents-md \| --procedures [--max-lines <n>] [--write <file>]` | Agent text generated from the embedded spec: a ~40-line always-loaded block, and the fifteen decision procedures. `--max-lines` checks the block against the budget (R-165); `--write` lands it in a file instead of stdout. |
| `docsys agents [--kb] [--dir .claude] [--force]` · `docsys agents --report [--dir .claude]` | Install the agent layer: four relay hooks + `/docsys-sync`, `/docsys-seed`, `/docsys-interview` + the docsys and export skills for a project; with `--kb` the four knowledge-base organs (capture · ingest · audit · lookup), the same four relays guarding the record layer, `settings.json` when absent and the git gate (D-076). Hooks carry their template version; `--force` refreshes them. `--report` lists the layer already installed and the shell calls it makes. |
| `docsys hook pre-tool-use\|stop\|post-tool-use\|user-prompt-submit [--repo .] [--root docs]` | The hook logic itself, reading the agent harness payload on stdin (D-051). |
| `docsys gate [--repo .] [--root docs] [--range <a>...<b>]` · `docsys doctor [--repo .] [--root docs] [--dir .claude]` | The commit-time question the binary computes (lint + code-without-docs); with `--range`, the same question over a pull request, failing when unanswered — what the CI workflow runs. And the liveness check: every hook present, executable, wired under the right event, up to date (D-040, D-047). |

## Quick start

```sh
cargo install docsys      # zero dependencies → one static binary on your PATH
cd your-project           # any git repository, with or without documentation
docsys adopt              # the whole setup, one command — what it writes is listed below
docsys doctor             # is the pipeline alive? every hook present, wired, up to date
```

No Rust toolchain? Grab a prebuilt binary for Linux (static musl,
x86_64/aarch64), macOS (Intel/Apple Silicon), or Windows from the
[releases page](https://github.com/FatihErtugral/docsys/releases) and put it
on your PATH.

`adopt` is idempotent — re-run it after an upgrade and only what changed is
rewritten. It lands:

- `docs/` — the skeleton when none exists (`.docmeta.yml`, router, journal,
  debt, questions, `_templates/`); an existing tree is left as it is
- `.claude/hooks/`, `.claude/commands/`, `.claude/skills/` — four relay hooks,
  `/docsys-sync`, `/docsys-seed`, `/docsys-interview`, the docsys and export
  skills
- `.claude/settings.json` — the hook wiring, written only when the file does
  not exist (an existing one is never clobbered; the snippet to merge goes on
  the report)
- `AGENTS.md` — a managed block generated from the embedded spec
- `.git/hooks/pre-commit` — the lint gate, in warn mode until the tree is
  clean
- `ADOPTION.md` — the report, with a checklist of every judgment call left to
  you

Then open an agent session in that directory and work as usual. Three things
happen without being asked:

- the first message gets a routing block — name the work type (feature / bug /
  refactor / research / idea) and where each one lands
- editing a page under `docs/` bumps its `updated:` by itself
- committing code without touching docs is asked about once, naming what
  moved; the same commit again proceeds
- and CI asks the same questions of every push and pull request: `adopt`
  writes `.github/workflows/docsys.yml` when the repository has a `.github/`,
  and the git pre-commit gate is hard as soon as the tree lints clean

`docsys lint --root docs` is the check CI runs: errors exit 1, warnings do
not. Everything else — feeling the severity doctrine on a clean tree, seeding
a project that has no documentation, the capture commands, migrating an
existing tree, a personal knowledge base — is a guided tour below.

## Guided tours

### 1 · Feel it on a clean project (5 minutes)

```sh
mkdir demo && cd demo && git init -q
docsys init --root docs      # skeleton: .docmeta.yml, router, journal, debt, questions, _templates/
docsys lint --root docs      # green
```

Now break it on purpose and watch the severity doctrine work:

```sh
# a dangling wiki-link — silently wrong, so it BLOCKS
echo "See [[reference/ghost|ghost]]" >> docs/index.md
docsys lint --root docs      # ERROR R-071 · exit 1

# a bare permanent page — reversible, so it only WARNS
mkdir -p docs/reference && echo "naked page" > docs/reference/x.md
docsys lint --root docs      # WARN R-050 (frontmatter) + WARN R-034 (orphan)
```

### 2 · The agent layer, in detail

```sh
docsys rules --procedures | less       # the 15 authored decision procedures
docsys agents --report                 # what is installed, and the shell calls it makes
```

Test fixtures that carry a deliberately broken `doc:` reference (probe files, a
check's own negative case) go under a directory listed in `scan_exclude` — the
scanner reads every tracked text file (R-077), so nowhere else in the tree can hold
one. Scope note: `docsys lint` reads the documentation root only; `.claude/rules/*.md`,
`AGENTS.md` and code are the province of `docsys refs --repo .`, which checks the
`doc:` references they carry. `adopt` writes `.claude/settings.json` only when the file does not exist — an
existing one may carry MCP servers and permission lists, so the merge snippet
lands on the `ADOPTION.md` checklist instead of being clobbered. Then open an
agent session in that directory and try the loop:

- open with something ambiguous ("let's look at the timer") → the
  session-intent hook asks for the work type, once
- have it edit a `docs/reference/` page → `updated:` bumps itself
- change code and try to commit without touching docs → the pre-commit hook
  asks once, naming what moved; the same commit again proceeds
- type `/docsys-sync` → a drift report over `docsys lint`, `docsys refs` and
  `docsys seed plan --since`
- a pre-commit gate of your own that wants a marker in every generated file?
  declare it once — `generated_preamble: "<!-- … -->"` in `.docmeta.yml` — and
  every file docsys writes opens with it (D-056)

### 3 · A project with no documentation at all — seeding

```sh
docsys adopt                                   # skeleton, hooks, templates, questions ledger
docsys seed plan --repo . --root docs           # feature inventory: what history names, what is covered
docsys seed plan --repo . --root docs --target weather   # one feature's history as evidence
```

The plan is evidence, never prose: commits with their bodies, files by touch
count, the birth date, manifests, `doc:` citations, the code's own comment
blocks verbatim. `/docsys-seed <feature>` presents it to the builder — plain
questions, one at a time, nothing written until confirmed — and what the
builder adds is what history cannot say: why, what is still open, what comes
next. `docsys seed apply --plan SEED.tsv` then lands the approved rows under
`work/` as tokens and verbatim quotations (a reserved research page, the
builder's answers, dated journal entries, a postmortem quoting its commit,
debt and question items). `/docsys-interview` runs it feature by feature,
resumable. A feature a page already covers is refused by name; from there the
hooks keep it current.

### 4 · Capture and navigation

```sh
docsys journal add "Wire format settled; details on the page" --link reference/wire
docsys debt close 3 --note "measured twice, held"     # item leaves the ledger, journal records it
docsys page new feature dark-mode                      # from _templates/feature.md
docsys backlinks token-ttl --repo .                    # pages and code pointing at a page
docsys mentions                                        # prose naming a page without a link
docsys graph --format jsoncanvas --repo . > docs/map.canvas
docsys adopt --obsidian                                # the docs root as an Obsidian vault
```

Opening the tree in Obsidian works as-is with three settings `adopt --obsidian`
writes (absolute link format, `_archive/` and `.federation/` ignored,
`_templates/` as the templates folder). Two caveats: `aliases:` means retired
identifiers here and autocomplete names there; and keep the Linter plugin's
`yaml-timestamp` off — it fights `updated:` (D-065).

### 5 · A real repository, safely (clone first)

```sh
git clone <your-repo> /tmp/pilot && cd /tmp/pilot
docsys migrate inventory --root <docs-dir> --repo . > plan.tsv
```

Open `plan.tsv`: one evidence line per file (first heading, link counts,
inbound references from README/CI/code) and a `TODO` target. Filling the
targets is the judgment step — do it yourself, or hand it to an agent in that
directory; classification is exactly what the P/R-031 procedure is for. Then:

```sh
docsys migrate apply --plan plan.tsv --root <docs-dir> --repo .
```

It moves files, injects frontmatter, rewrites links on both sides of the docs
boundary (README included), reports what it could not map as RISK lines, and
lints the result. Don't like it? `git checkout . && git clean -fd` — it was a
clone; zero risk.

### 6 · A personal knowledge base (the other profile)

The same mechanics, a different layout: notes land with zero discipline, get
distilled with full discipline, and every claim keeps its evidence.

```sh
mkdir brain && cd brain && git init -q
docsys init --profile knowledge-base --root .   # raw/inbox/ + wiki/ + docmeta
docsys agents --kb --root .                     # capture · ingest · audit · lookup
$EDITOR .docmeta.yml                            # declare your domains:
```

Then work in natural language with an agent in that directory: *"note this"*
lands in `raw/inbox/`; *"process my inbox"* distils each note into
`wiki/<domain>/<type>/`, archives the source and routes the page; *"audit the
wiki"* verifies pages against their sources **in another session** and records
who verified what; *"what do my notes say about X"* answers with the page path
— or says the base does not have it.

What the binary guarantees underneath: `raw/` is content-immutable (an edited
or deleted record is an error; the hook blocks the attempt; relocation is the
expected flow), every `sources:` entry must resolve, a `verified` page must
record `verified_by:` and `verified_rev:`, and a verification is checked, not
trusted — a `verified` page whose body, or whose consumed source, no longer
hashes to what it held at `verified_rev` is an error until it is audited again
(D-077, D-082).

### 7 · Your own assistant (a base that learns from your projects)

The knowledge base is the memory; the projects it consumes are what it learns
from; connectors are how the outside world lands in it. Nothing below needs a
server, a database or a hand-written rule file — and the whole setup is one
command:

```sh
docsys assistant --root ~/jarvis --projects ~/code --domains coding,ops
```

It creates the base (a git repository, `raw/inbox/`, `wiki/`), installs the
four organs and the four relays, consumes every docsys project one level
under `~/code` (another knowledge base is skipped), materializes their pages,
lands their recent commits as records through the git connector, and prints
the digest. Run it again any time: new projects and new commits are picked
up, nothing is duplicated. The same thing by hand, when you want to see the
parts:

```sh
docsys init --profile knowledge-base --root .   # raw/inbox/ + wiki/
docsys agents --kb --root .                     # four organs, four relays, the gate
docsys consume discover ~/code --root .         # every docsys tree under ~/code
docsys consume add ~/code/relay --root .        # the ones you want, one line each
docsys fetch --root .                           # their pages, materialized
docsys lookup retry --root .                    # @relay/retry-policy, scored
docsys inbox pull ~/code/relay --since 7.days --root .   # this week's commits, as records
docsys status --root .                          # the digest before the morning briefing
```

The first session proposes the assistant's character — name, how it addresses
you, tone, languages, what it must never do — asks you to confirm in the
language you wrote in, writes the answers under `## Character` in the base's
`AGENTS.md`, and summarizes how it will talk from then on (D-083). It keeps
speaking your language turn by turn; the pages keep the base's declared one.

Then, in an agent session in that directory: *"study what my projects say
about failure handling and write it up"* — a wiki page whose `sources:` are
`@relay/retry-policy` and friends; another session audits it; *"process my
inbox"* distils the commits; a howto that matured compiles into a skill
(`docsys compile`). What the assistant may never do is also mechanical: no
record is edited (the hook blocks it), no page is verified by the session
that wrote it, no answer is given from memory when the base does not have
it. Connectors beyond git — calendar, mail, tickets, clips — call the same
gate: `docsys inbox add --source <name> --id <item>` (§20, experimental).

Staying current is a schedule, not a hope: run `docsys assistant` again (a
nightly job is enough — the tree holds records, never timers, R-205), and
`fetch` brings every project's changed pages while `inbox pull` lands its new
commits. The next `docsys lint` names every page that was verified against a
source that has since moved (D-082); the next `docsys status` lists it under
"sources"; the next session re-reads it and another one audits it.

## What keeps it honest

- **Severity is doctrine.** Warn by default; block only what is irreversible
  or silently wrong (§2.2, R-151). Every warning names the file that must
  change (R-152).
- **Drift is caught by a hash, not by a reviewer.** A page pins the code it
  describes (`verifies:`); when that region moves, lint fails until someone
  re-reads the page and refreshes the pin. History dates every page, so a
  freshness field that lies and a draft left to rot are errors too (§11,
  R-085, R-106, D-070).
- **A verification is checked, not trusted.** `verified` means "this body,
  against these sources, at this revision"; the body and the consumed sources
  are re-hashed against that revision on every lint, and a session never
  verifies its own page (R-024, R-025, D-077, D-082).
- **A check that inspected zero units fails** — a dead scan must never read as
  a clean tree (R-011). An unmigrated tree announces itself instead of passing
  silently.
- **Single source of truth, structurally.** Agent text is generated from the
  spec embedded in the binary; the revision history lives in git, not in a
  prose copy that can drift (R-155, §18).
- **The corpus cuts both ways.** Expected outputs are exact: an extra finding
  fails a case as hard as a missing one, so the checker can't grow noisy.
- **Every open decision has a home.** What the spec leaves to implementations
  is decided once, in [corpus/DECISIONS.md](corpus/DECISIONS.md), with the
  reason (R-193) — 82 decisions and counting, most of them forced by real
  repositories or by watching agents work: a formatter that reflowed a config
  field, a build tree that turned 147 findings into 9,171, an example citation
  that failed the rule it was teaching, a draft that took its own reference
  page's identifier, a base that read its own wiki as stray pages.

## Repository layout

```
SPEC.md               the specification — 147 normative rules + experimental §13 (federation), §20 (connectors)
src/                  the reference implementation (Rust, stdlib only — SHA-256 included)
corpus/
├── DECISIONS.md      register of implementation-defined choices (R-193)
└── cases/            conformance corpus: tree + exact expected findings
tests/                behavior locks for migrate · refs · graduate · adopt ·
                      doctor · hooks and kb hooks (executed for real) · seed ·
                      graph · knowledge base (git-observable) · export ·
                      federation · freshness (pins, history, range gate, CI) ·
                      compile · lookup and consume · the assistant's memory
ci/e2e.sh             a fresh box, twelve first-run flows, real paths
```

## Status

Core (layout, identity, lifecycle, graduation, journal, agent layer) is
implemented and field-proven: four repositories adopted end to end — the hook
layer hardened by a day of live reports, one per release from 0.4.2 to 0.5.1 —
and a personal knowledge base whose constitution predated the spec and matched
it. Both profiles — `project` and `knowledge-base` — are checked.

Brownfield seeding (0.6–0.8) reads a project's history and the code's own
comment blocks as evidence for a conversation with the builder, lands only what
was confirmed, and refuses a feature a page already covers. Capture commands and
derived navigation (0.9) make the right single-file write the cheap one and
give the tree backlinks, unlinked mentions and a graph. Freshness (0.11) is
mechanical: a page pins a code region (`verifies:`, §11) and lint recomputes
the hash on every run; history dates every page, so an `updated:` behind the
last commit and a draft nobody touched for `stale_active_days` are errors, not
hopes. `adopt` writes the CI workflow and hardens the pre-commit gate once the
tree is clean. A mature howto compiles into an executable skill that carries
its source hash, and goes stale with the page (0.12, R-094, R-095).

The assistant's memory (0.12–0.13) grew out of an agent lab: three sample
projects adopted, fifteen headless sessions watched, a knowledge base created
from the CLI alone that consumed the three, distilled a page whose sources
were their pages, audited it in another session, judged a batch of commit
records honestly, compiled a verified howto into a skill and gave a morning
briefing from `docsys status`. What that added: `lookup` across local and
consumed pages, `consume add`/`discover` with the list in the tree's own
docmeta, `@namespace/id` as a source, the knowledge-base hook layer, the
connector write gate with the git connector, `status`, and `assistant` as the
one command. A verification is now checked against its body and its consumed
sources at `verified_rev`, so a base stays current by fetching, and lint says
which pages fell behind.

Federation (§13) and connectors (§20) stay marked **experimental** in the
spec. Federation's working slice: manifests, `fetch` over filesystem paths and
git URLs, `@namespace/id` in compositions and in sources, proven between
repositories on one machine. Connectors' working slice: the write gate and
the git connector; calendar, mail, tickets and the rest are designed, not
built. Consuming a provider over HTTP without a checkout, the consumer-impact
report for a retired identifier, `compile @namespace/id`, and a second
connector against a real source are the next slices — those rules bind nothing
until real use settles them.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Unless you explicitly state
otherwise, any contribution intentionally submitted for inclusion in this work
by you, as defined in the Apache-2.0 license, shall be dual licensed as above,
without any additional terms or conditions.

## Lineage

Diátaxis (Procida) for the type system · Every Page Is Page One (Baker) for
openings · docs-as-code throughout · and field lessons from two production
repositories, encoded as rules: the 3,800-line journal that taught entry
budgets, the silent rename that taught id-over-path, the ownerless wire
protocol that taught contract ownership, the drowned warning that taught
warn-with-names.
