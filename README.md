# docsys

[![ci](https://github.com/FatihErtugral/docsys/actions/workflows/ci.yml/badge.svg)](https://github.com/FatihErtugral/docsys/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/docsys.svg)](https://crates.io/crates/docsys)

**A documentation system that agents and humans keep honest — mechanically.**

Plain markdown + git. A single zero-dependency binary enforces the mechanics;
an LLM handles only the judgment calls, following authored decision procedures.
Nothing lives in two places, nothing goes stale silently, and nothing blocks
your commit unless the damage would be irreversible or silently wrong.

Built spec-first: every behavior traces to a numbered rule in [SPEC.md](SPEC.md)
(133 normative rules, survived six adversarial audit rounds by six independent
models), every implementation-defined choice is registered in
[corpus/DECISIONS.md](corpus/DECISIONS.md), and the conformance corpus keeps the
binary from drifting looser *or* noisier than the rules.

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
    PERM -.->|"verifies: hash pin<br/>code drift = loud"| CODE
    HOW -->|"compile<br/>(mature only)"| SKILL
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
        T1[lint · refs<br/>validation]
        T2[init · migrate · graduate<br/>movement + scaffolding]
        T3[rules · agents<br/>generated agent layer]
    end
    subgraph MODEL["LLM  (judgment — never moves bytes)"]
        M1[classify: which type?]
        M2[route: which block where?]
        M3[write: openings, distillations]
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

`docsys agents` installs four warn-only hooks and a thin skill; `docsys rules`
generates the agent-facing text **from the embedded spec** — there is no
hand-maintained rules copy to drift (R-155).

```mermaid
flowchart LR
    S([session starts]) --> SI["session-intent hook<br/>classify work type once"]
    SI --> WORK["agent works<br/>judgment via docsys rules --procedures"]
    WORK -- "edits docs page" --> PU["post-edit hook<br/>bumps updated:"]
    WORK -- "commit" --> PC["pre-commit hook<br/>contract surface changed,<br/>docs didn't? → WARN + lint"]
    WORK -- "turn ends" --> ST["stop hook<br/>code moved, docs didn't? → remind"]
    PC --> GATE{"docsys lint"}
    GATE -- "errors" --> FIX[fix in the same session]
    GATE -- "clean/warnings" --> DONE([commit])
```

Hooks **warn and never block** — hard blocking gets hooks disabled entirely,
which removes the protection completely (R-150, a field lesson). Blocking is
reserved for the two outcomes that earn it: irreversible, or silently wrong
(R-151) — a dangling reference blocks, because an agent that cannot resolve a
reference invents an answer.

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
stamp carries its fetch date so a stale composition is visible.

## Commands

| Command | What it does |
|---|---|
| `docsys adopt [--repo .] [--root docs]` | One-command integration: docmeta (or the full init skeleton on a fresh project), agent assets, `settings.json` when absent, AGENTS.md managed block, git pre-commit gate (warn-mode), and an `ADOPTION.md` report whose checklist carries every judgment call. Idempotent. |
| `docsys lint [--root docs] [--json]` | Full tree validation: frontmatter, ids, links, journal discipline, templates, list grammars — both profiles. Errors exit 1, warnings don't. |
| `docsys init [--root docs] [--profile …]` | Greenfield skeleton. `project`: router, journal, debt. `knowledge-base`: the record layer (`raw/inbox/`) and the wiki root. |
| `docsys migrate inventory / apply` | Brownfield adoption: evidence-rich plan → approved mapping → mechanical move with link rewriting on both sides of the docs boundary. |
| `docsys refs --repo .` | Validate every `doc: <id>` in the code base against the tree (typos stop being invisible). |
| `docsys graduate plan / apply` | Byte-exact block movement from work files to the permanent layer. |
| `docsys export plan / product / feature` | Compose a document from permanent pages: a draft map from the tree's own evidence, a whole product from an authored map, or one feature by identifier (`--follow` widens a hop). Bodies verbatim, source-stamped, `--audience` and `--lang` aware. |
| `docsys export manifest` | Publish what this namespace exports — id, type, title, summary, content hash, no bodies. A few KB where a clone is megabytes. |
| `docsys fetch` | Materialize consumed namespaces into `.federation/`: manifest first, unchanged pages skipped, provenance recorded. |
| `docsys rules --agents-md / --procedures` | Agent text generated from the embedded spec: a ~33-line always-loaded block, and the fifteen decision procedures. |
| `docsys agents [--kb]` | Install the agent layer: hooks + `/doc-sync` + the docsys and export skills for a project, or the four knowledge-base organs (capture · ingest · audit · lookup) with `--kb`. |

## Quick start

```sh
cargo install docsys      # zero dependencies → one static binary on your PATH
docsys help
```

No Rust toolchain? Grab a prebuilt binary for Linux (static musl,
x86_64/aarch64), macOS (Intel/Apple Silicon), or Windows from the
[releases page](https://github.com/FatihErtugral/docsys/releases) and put it
on your PATH.

### 1 · Feel it on a clean project (5 minutes)

```sh
mkdir demo && cd demo && git init -q
docsys init --root docs      # skeleton: .docmeta.yml, router, journal, debt
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

### 2 · The agent layer (where it becomes a system)

```sh
docsys adopt                           # assets + settings.json + AGENTS.md block + git gate + report
docsys rules --procedures | less       # the 15 authored decision procedures
```

Scope note: `docsys lint` reads the documentation root only; `.claude/rules/*.md`,
`AGENTS.md` and code are the province of `docsys refs --repo .`, which checks the
`doc:` references they carry. `adopt` writes `.claude/settings.json` only when the file does not exist — an
existing one may carry MCP servers and permission lists, so the merge snippet
lands on the `ADOPTION.md` checklist instead of being clobbered. Then open an
agent session in that directory and try the loop:

- open with something ambiguous ("let's look at the timer") → the
  session-intent hook asks for the work type, once
- have it edit a `docs/reference/` page → `updated:` bumps itself
- change code and try to commit without touching docs → the pre-commit hook
  warns, naming what should have moved
- type `/doc-sync` → a drift report over `docsys lint` + `docsys refs`

### 3 · A real repository, safely (clone first)

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

### 4 · A personal knowledge base (the other profile)

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
or deleted record is an error; relocation is the expected flow), every
`sources:` entry must resolve, a `verified` page must record `verified_by:` and
`verified_rev:`, and a page that changes drops back to `unverified`.

## What keeps it honest

- **Severity is doctrine.** Warn by default; block only what is irreversible
  or silently wrong (§2.2, R-151). Every warning names the file that must
  change (R-152).
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
  reason (R-193) — 38 decisions and counting, most of them forced by real
  repositories: a formatter that reflowed a config field, a build tree that
  turned 147 findings into 9,171, an example citation that failed the rule it
  was teaching.

## Repository layout

```
SPEC.md               the specification — 133 normative rules + experimental §13
src/                  the reference implementation (Rust, stdlib only)
corpus/
├── DECISIONS.md      register of implementation-defined choices (R-193)
└── cases/            conformance corpus: tree + exact expected findings
tests/                behavior locks for migrate · refs · graduate · agents ·
                      knowledge base (git-observable) · export · federation
```

## Status

Core (layout, identity, lifecycle, graduation, journal, freshness, agent
layer) is implemented and field-proven: a firmware repository adopted end to
end, and a personal knowledge base whose constitution predated the spec and
matched it. Both profiles — `project` and `knowledge-base` — are checked.

Federation (§13) stays marked **experimental** in the spec, and the
implementation now has its first working slice: manifests, `fetch` over
filesystem paths and git URLs, and `@namespace/id` composition, proven between
repositories on one machine. Consuming a provider over HTTP without a
checkout, and the consumer-impact report for a retired identifier, are the
next slices — the rules there bind nothing until a second real estate exists.

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
