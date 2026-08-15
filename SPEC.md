# docsys Specification

**Version:** 0.1
**Status:** frozen — rule numbers are permanent; rule text may be clarified, not
redefined, until 1.0

This document defines a documentation system for software projects and personal
knowledge bases. It is implementation-independent: any tool that satisfies the
rules below is conformant.

The reference implementation is `docsys`, a single static binary.

---

## 1. Scope

### 1.1 What this specifies

- The on-disk layout of a documentation tree
- The frontmatter schema of a page
- The identifier and link grammar
- The lifecycle of a unit of work, and how knowledge graduates out of it
- How independent repositories reference each other's documentation
- Which rules are mechanically enforced and which require judgment

### 1.2 What this does not specify

- Prose style, tone, or content
- Rendering, publishing, or site generation
- Any storage format other than plain files in a version-controlled tree
- Which agent, editor, or CI system is used

### 1.3 Design constraints

**R-001** `advisory` — Documentation MUST be plain text files in a
version-controlled tree. No database, no proprietary format.

**R-002** `advisory` — A fact MUST have exactly one home. A stale copy is more
dangerous than a missing page, because it is trusted.

**R-003** `advisory` — Deterministic checks belong to tooling; classification and
contradiction belong to a human or a model.

**R-004** `advisory` — Capture requires no discipline. Processing requires full
discipline.

**R-005** `advisory` — Derived artifacts are generated: indexes, routers, graphs,
backlinks, timestamps, coverage reports. **Prose is never generated.** A system
that offers to write documentation produces exactly the confident stale copy that
R-002 forbids.

---

## 2. Conformance and rule notation

### 2.1 Rule format

Every normative rule is written as:

```
**R-NNN** `enforcement` · LEVEL — rule text
```

`LEVEL` is MUST, SHOULD, or MAY as defined in RFC 2119.

`enforcement` declares who is responsible for the rule:

| Tag | Meaning |
|---|---|
| `lint` | A local static check catches violations |
| `ci` | A cross-repository or pipeline gate catches violations |
| `cmd` | A command guarantees the outcome by construction |
| `agent` | Requires judgment; no mechanical check exists |
| `advisory` | Guidance only; not checked |

Rule numbers are permanent. A withdrawn rule keeps its number and is marked
`WITHDRAWN`; numbers are never reused.

### 2.2 Coverage requirement

**R-010** `ci` · MUST — Every rule tagged `lint`, `ci`, or `cmd` MUST be covered
by at least one conformance test. An implementation MUST be able to report, per
rule, which check covers it.

**R-011** `ci` · MUST — A check that inspected zero units MUST report failure,
not success. Silent non-matching is the most dangerous failure mode of a
documentation checker.

> Rationale for R-011: in field use, a hook whose path patterns no longer matched
> anything stayed green for weeks while the rule it enforced was dead.

**R-012** `advisory` · MUST — A checker MUST NOT be looser than the rule text. If
a rule cannot be checked, it is tagged `agent` or `advisory`, never left to an
incomplete check.

### 2.3 Version declaration

**R-013** `lint` · MUST — A conformant tree MUST declare `spec: docsys/0.1` in
its `.docmeta.yml`.

**R-014** `lint` · MUST — A tool encountering a major version it does not
implement MUST refuse to operate rather than degrade silently.

---

## 3. Profiles

A profile selects the layer names. Everything else in this specification is
shared across profiles.

**R-020** `lint` · MUST — A tree MUST declare exactly one profile in
`.docmeta.yml`: `project` or `knowledge-base`.

| | `project` | `knowledge-base` |
|---|---|---|
| Flowing layer | `work/` | `raw/` |
| Permanent layer | `reference/` `howto/` `explanation/` `tutorial/` | `wiki/<domain>/<type>/` |
| Grouping axis | none | domain |
| Extra field | — | `verification` |

**R-021** `advisory` — Profiles are independent systems. A tree of one profile
MUST NOT require the existence of a tree of another profile.

**R-022** `advisory` — The only coupling between trees is a reference
(§7). Absent a reference, there is no dependency and no shared state.

---

## 4. Layout

### 4.1 Permanent layer

**R-030** `lint` · MUST — A permanent page MUST be exactly one Diátaxis type:
`reference`, `howto`, `explanation`, or `tutorial`.

**R-031** `agent` · MUST — A page MUST NOT mix types. A page that answers more
than one of the questions below is split.

| Question the reader is asking | Type |
|---|---|
| What is X? Which value, which format? | `reference` |
| How do I do X? | `howto` |
| Why X and not Y? | `explanation` |
| Teach me X from zero | `tutorial` |

**R-032** `agent` · MUST — A page MUST open with one or two sentences that
establish its own context ("This page describes X; read it when Y"). Readers
arrive in the middle, from a search or a link.

**R-033** `agent` · MUST NOT — Documentation MUST NOT restate what the code
already states. Signatures and parameter lists are not copied; generated API
documentation owns those.

**R-034** `lint` · MUST — Every permanent page MUST be reachable from the root
router (`index.md`). An unreachable page is an orphan and is an error.

### 4.2 Flowing layer

**R-040** `lint` · MUST — The `project` profile defines six core work
categories:

```
work/
├── journal.md      chronology — what happened, when
├── journal/        archive slices
├── features/       work requiring a design decision
├── postmortems/    an incident with a lesson
├── research/       explored, no decision reached
├── debt.md         deliberately deferred
└── questions.md    not known
```

**R-041** `lint` · MUST — `features/`, `postmortems/`, and `research/` files
carry `status` (§8). `journal.md`, `debt.md`, and `questions.md` are list files
and MUST NOT carry `status`.

**R-042** `lint` · MAY — A tree MAY declare additional work categories in
`.docmeta.yml`. Additional categories are subject to the `status` rules but have
no defined graduation target.

**R-043** `advisory` — A directory is created when its first file is needed. An
empty directory reads as an obligation and produces filler content.

### 4.3 Reserved directories

**R-044** `lint` · MUST — The following names are reserved and excluded from
orphan and type checks: `_archive/`, `_templates/`, `_unsorted/`,
`.federation/`.

**R-045** `lint` · MUST NOT — Content MUST NOT be deleted. Obsolete content moves
to `_archive/`.

**R-046** `agent` · SHOULD — `_unsorted/` is temporary. Content that cannot be
classified goes there and is reported, never force-fitted into a type.

---

## 5. Frontmatter

### 5.1 Permanent pages

```yaml
---
id: token-ttl            # required
type: reference          # required
updated: 2026-08-15      # required
owner: team-identity     # required when federation is enabled
lang: tr                 # optional
internal: true           # optional — excluded from export
provides: "ADR-*"        # optional — this page defines an ID family
verifies:                # optional — freshness pin (§11)
  - path: src/auth/refresh.rs
    symbol: refresh_token
    hash: a3f9c1
---
```

**R-050** `lint` · MUST — A permanent page MUST carry `id`, `type`, and
`updated`.

**R-051** `lint` · MUST — `owner` MUST be present on any page exported to other
namespaces. An unowned shared contract is an error.

> Rationale: in field use, a wire protocol between two components went
> undocumented for months because neither side considered itself the owner.

**R-052** `cmd` · MUST — `updated` MUST be maintained by tooling, not by hand.

**R-053** `lint` · MAY — Router files (`index.md`), trackers (`roadmap.md`), and
`README.md` are exempt from frontmatter but MUST follow their own format rules.

### 5.2 Work files

```yaml
---
status: active                     # required
updated: 2026-08-15                # required
epic: "@company/checkout-v2"       # optional
abandoned_reason: "..."            # required when status: abandoned
graduated_to: [token-ttl]          # required when status: graduated
---
```

**R-054** `lint` · MUST — A work file MUST carry `status` and `updated`.

**R-055** `lint` · MUST — `status: abandoned` MUST carry `abandoned_reason`.

**R-056** `lint` · MUST — `status: graduated` MUST carry `graduated_to` with at
least one identifier.

---

## 6. Identifiers

### 6.1 Grammar

```
local-id   ::= [a-z0-9]+ ( "-" [a-z0-9]+ )*
namespace  ::= [a-z0-9]+ ( "-" [a-z0-9]+ )*
foreign-id ::= "@" namespace "/" local-id
```

**R-060** `lint` · MUST — An `id` MUST match `local-id`. ASCII lowercase,
digits, and hyphens only, regardless of the page's content language.

**R-061** `lint` · MUST — An `id` MUST be unique within its namespace.

**R-062** `advisory` · MUST — **The identifier is the contract; the filename is
cosmetic.** An `id` MUST NOT change when a file is renamed, moved, or
translated.

> Rationale: code once referenced documentation by file path. The first rename
> broke every reference, silently — no compiler, test, or reviewer saw it.

**R-063** `lint` · MAY — A page MAY declare `provides:` with a glob to register
itself as the definition site of an identifier family (for example `ADR-*`).

**R-064** `advisory` · MUST — A `provides:` pattern MUST match the *definition*
form, not the *citation* form. A pattern that also matches citations reports
broken references as healthy.

---

## 7. Links and references

### 7.1 Documentation to documentation

**R-070** `lint` · MUST — A doc-to-doc link MUST be a wiki-link carrying the full
path from the documentation root: `[[reference/token-ttl]]` or
`[[reference/token-ttl|alias]]`. Short-name links are invalid.

**R-071** `lint` · MUST — Every link target MUST resolve.

### 7.2 Code to documentation

**R-072** `lint` · MUST — Code MUST reference documentation by identifier, never
by path:

```
// doc: token-ttl
// doc: @svc-auth/token-ttl
```

**R-073** `lint` · MUST — The token following `doc:` is read up to the first
whitespace. A reference intended for a specific record writes that identifier
directly.

### 7.3 Documentation to code

**R-074** `lint` · MUST NOT — Documentation MUST NOT contain source paths as
references. Code moves; documentation stays. Where code must be shown, the
snippet is embedded with a comment explaining why it is there.

**R-075** `lint` · MUST NOT — Absolute paths (`/home/...`, `C:\...`) and links
that escape the tree root are errors.

> R-074 does not conflict with §11: `verifies` declares an *audit binding*, not a
> reader-facing reference, and it fails loudly when it breaks.

---

## 8. Work lifecycle

### 8.1 States

```
draft ──► active ──► done ──► graduated
  │         │  ▲
  │         │  └──────┘  (reopened)
  └─────────┴──► abandoned ──► graduated
```

| State | Meaning |
|---|---|
| `draft` | Opened, not started |
| `active` | In progress |
| `done` | Finished and confirmed |
| `graduated` | Permanent value extracted; not loaded into agent context |
| `abandoned` | Discontinued; reason recorded |

**R-080** `lint` · MUST — `status` MUST be one of the five values above.

**R-081** `agent` · MUST — `done` is set only on explicit human confirmation.
Passing tests or a green build means `active`.

**R-082** `lint` · MUST — `graduated` is terminal. No transition leaves it.

**R-083** `cmd` · MUST — A transition to `abandoned` MUST record a reason.

**R-084** `advisory` — Abandonment has two forms and they differ in value. Work
abandoned as unnecessary is archived. Work abandoned *after being tried* carries
expensive knowledge and SHOULD graduate to `explanation/` as a rejected
alternative.

**R-085** `lint` · SHOULD — A file that has been `active` without modification
beyond a configured threshold (default 90 days) SHOULD be reported. Undeclared
abandonment is the common case, not the rare one.

### 8.2 Work types are not categories

**R-086** `advisory` — A bug is a *type of work*, not a category. Categories are
determined by output. A bug's output depends on one question: **can it recur?**

| Situation | Destination |
|---|---|
| One-off mistake, a wrong line | `journal.md` entry |
| Can recur, but the code fix is the permanent answer | `journal.md` entry |
| Recurrence is prevented by a rule | invariant in `reference/` |
| Root cause was systemic, or the cost exceeded the configured threshold | `postmortems/` |

**R-087** `lint` · MAY — The postmortem threshold is configured in
`.docmeta.yml`. Without a threshold, every bug becomes a postmortem and the
directory stops being read.

---

## 9. Graduation

Graduation moves permanent knowledge out of the flowing layer.

**R-090** `cmd` · MUST — Graduation MUST move content, never rewrite it. A
conformant implementation copies blocks byte-for-byte; the model selects the
mapping and never retypes the text.

> This converts a rule that depended on model discipline into a mechanical
> guarantee.

**R-091** `cmd` · MUST — After graduation the source file MUST retain a link to
the destination and MUST record `graduated_to`.

**R-092** `agent` · MUST — Order matters. Content that already exists on a
permanent page is deduplicated first (mechanical, batchable). Content that exists
nowhere permanent is written to its destination *before* the source is shrunk.

**R-093** `agent` · MUST — Before archiving anything, the question is asked: does
this information exist anywhere else? If not, and it is still true, it graduates
first.

| Source | Usual destination |
|---|---|
| `features/` | `reference/` (contract), `explanation/` (decision) |
| `postmortems/` | `reference/` (invariant), `howto/` (runbook) |
| `research/` | `explanation/` |
| `abandoned` | `explanation/` (rejected alternative) |

### 9.1 Compilation to executable form

The graduation chain has one more link: flowing → permanent → executable.

**R-094** `cmd` · MAY — A `howto` page whose steps have stabilized MAY be
compiled into an executable agent skill.

**R-095** `cmd` · MUST — A compiled skill MUST record the source identifier and
content hash. When the source page changes, the compiled skill is reported as
stale (§11).

**R-096** `agent` · MUST NOT — Compilation MUST NOT invent steps. A procedure not
fully written on the page is not ready to be compiled.

---

## 10. Journal

**R-100** `lint` · MUST — A journal entry opens with `## YYYY-MM-DD — title`.

**R-101** `lint` · SHOULD — An entry is 2–5 lines: what triggered it, what
changed, which gate passed, which page holds the permanent content. An entry
longer than 15 lines is reported.

**R-102** `lint` · MUST NOT — Measurements, tables, algorithms, API lists,
register maps, and rejected alternatives MUST NOT be written to the journal.
They belong to the permanent layer; the journal links to them.

> Rationale: a journal reached 3,800 lines in nine days and became 31% of all
> documentation. One constant appeared in eight places and none were ever
> updated. Permanent knowledge kept in a chronology silently becomes a lie.

**R-103** `cmd` · MUST — When the active journal exceeds 500 lines, the oldest
whole days are moved to an archive slice. Slices are cut when full, not by
calendar.

**R-104** `lint` · MUST — Entries are ordered newest first. A retrospective entry
is inserted at its own date, not appended.

---

## 11. Freshness

A permanent page may pin itself to a region of code.

```yaml
verifies:
  - path: src/auth/refresh.rs
    symbol: refresh_token
    hash: a3f9c1
```

**R-110** `lint` · MAY — A page MAY declare `verifies`. When the hash of the
referenced region no longer matches, the page is reported as stale.

**R-111** `lint` · MUST — A stale page is a loud failure, never a silent one. It
is the only mechanism in this specification that detects code-documentation
drift mechanically.

**R-112** `advisory` — `verifies` is an audit binding, not a reader-facing
reference. It does not license writing code paths into prose (R-074).

---

## 12. Language

**R-120** `lint` · MUST — Structure is always English: directory names, file
names, frontmatter field names, `id` values, `status` values, link syntax.

**R-121** `agent` · MAY — Content language is free and declared as
`default_content_language` in `.docmeta.yml`. A page may override it with
`lang:`.

**R-122** `agent` · MUST — When editing an existing page, its language is
preserved. Languages are never mixed within a page.

**R-123** `agent` · MUST NOT — Code identifiers, protocol names, library names,
and quotations are never translated.

**R-124** `agent` · MUST NOT — Migration never translates. Content moves as it
is.

---

## 13. Federation

Federation lets independent repositories reference each other's documentation
without sharing a repository, a database, or a server.

### 13.1 Declaration

**R-130** `lint` · MUST — A tree participating in federation MUST declare
`namespace` in `.docmeta.yml`. The namespace is the only field written by hand.

**R-131** `cmd` · MUST — `provides` and `consumes` MUST be derived, never
authored. `provides` comes from pages carrying `id`; `consumes` comes from
`doc: @ns/id` references found in code and documentation.

> A hand-maintained dependency list goes stale by construction (R-002).

**R-132** `lint` · MAY — In a monorepo, a namespace is defined per directory. All
federation rules apply unchanged.

### 13.2 Export

**R-133** `cmd` · MUST — `export` produces a machine-readable manifest containing
namespace, spec version, and for each provided identifier: type, title, a
one-sentence summary, content hash, and owner.

**R-134** `cmd` · MUST NOT — The manifest MUST NOT contain page prose. Metadata
crosses the repository boundary; content does not.

**R-135** `lint` · MAY — A page marked `internal: true` is excluded from the
manifest.

### 13.3 Consumption

**R-136** `cmd` · MUST — A consumed page is materialized under `.federation/` as
a read-only copy carrying the source hash.

**R-137** `lint` · MUST NOT — Files under `.federation/` MUST NOT be edited
locally. They are derived artifacts.

**R-138** `advisory` — The `.federation/` copy does not violate R-002. It is
read-only, derived, hash-pinned, and machine-refreshed. What R-002 forbids is an
*unchecked* copy that can silently lie.

### 13.4 Enforcement

**R-139** `ci` · MUST — A reference to a nonexistent foreign identifier MUST fail
the consuming repository's pipeline. An agent that cannot resolve a reference
invents an answer; this failure is silent and harmful.

**R-140** `ci` · MUST — Removing a provided identifier MUST report which
namespaces consume it, before the change lands.

**R-141** `cmd` · SHOULD — When a consumed page's hash changes, the consuming
repository SHOULD receive an automated change proposal showing the old and new
content side by side.

**R-142** `advisory` — Federation requires no central service. Every check above
runs in the repository that owns the problem. A central pass is needed only to
detect providers that nobody consumes.

### 13.5 Asymmetric membership

**R-143** `lint` · MAY — A namespace MAY declare `federation_role: consume-only`.
It reads manifests but publishes none, and no other namespace may reference it.

**R-144** `advisory` · MUST — A private or machine-local tree MUST be
`consume-only`. A reference to it resolves on its owner's machine and is dead
everywhere else, which is a silent failure of the worst kind (R-151).

---

## 14. Automation levels

**R-150** `advisory` · MUST — Automated checks warn by default. Hard blocking
creates friction, and friction is resolved by disabling the check entirely —
which removes the protection completely.

**R-151** `advisory` · MUST — A check escalates to blocking only when the outcome
is **irreversible** or **silently wrong**. Everything else warns.

> The second criterion was added after federation was designed: a broken
> reference is reversible but produces a confidently wrong answer, which is worse
> than a visible failure.

**R-152** `lint` · MUST — A warning MUST name the file that needs to change.

> Rationale: in a single 1,979-line commit an unnamed warning went unnoticed and
> five contracts shipped undocumented.

**R-153** `advisory` · MUST — A vital rule lives in two places: the always-loaded
summary and a mechanical check. Guidance may fail to load; the check always runs.

**R-154** `cmd` · SHOULD — Concurrent writing sessions in one tree SHOULD be
detected and reported. Interleaved commits from parallel sessions are
misdiagnosed as tool failures.

**R-155** `cmd` · MUST — Rule text presented to an agent MUST be generated from
this specification, not maintained as a hand-written copy.

---

## 15. `.docmeta.yml`

```yaml
spec: docsys/0.1                 # required
profile: project                 # required — project | knowledge-base
default_content_language: en     # required
created: 2026-08-15              # required

namespace: svc-auth              # required when federation is enabled
federation_role: publish         # publish | consume-only  (R-143)

phase: mvp                       # mvp | mature
type: app                        # app | library | embedded
mode: solo                       # solo | team

work_categories: []              # additional categories (R-042)
postmortem_threshold: "4h"       # R-087
stale_active_days: 90            # R-085
```

**R-160** `lint` · MUST — `.docmeta.yml` MUST exist at the documentation root and
MUST declare `spec`, `profile`, `default_content_language`, and `created`.

---

## 16. Versioning and migration

A tree created under one version of this specification must be able to move to
the next without hand editing.

### 16.1 Compatibility

**R-170** `cmd` · MUST — A major version the tool does not implement causes a
refusal to operate (R-014). A minor version difference MUST NOT block operation.

**R-171** `cmd` · MUST — Every command reports a version difference in one line
and names the command that resolves it. A migration the user never hears about
does not happen.

### 16.2 What a migration may change

**R-172** `cmd` · MUST — A migration changes structure only: frontmatter field
names, directory locations, default values. It MUST NOT modify prose. This is
R-090 extended to version upgrades.

**R-173** `cmd` · MUST — A migration MUST be declared as data, so it can be
reviewed, diffed, and tested independently of the implementation that runs it.

**R-174** `cmd` · MUST — Every migration step declares a strategy:

| Strategy | Behavior |
|---|---|
| `auto` | Applied mechanically |
| `suggest` | A value is proposed; a human confirms |
| `manual` | Listed only; never applied automatically |

**R-175** `cmd` · MUST NOT — A change requiring judgment MUST NOT be applied
automatically. Semantic changes are always `manual`.

### 16.3 Execution

**R-176** `cmd` · MUST — The default action is a plan. Nothing is written without
an explicit apply.

**R-177** `cmd` · MUST — Chained migrations apply one version at a time, each as
its own commit. A single combined commit destroys the information about which
step broke.

**R-178** `advisory` — Rollback uses version control. No separate rollback
mechanism is specified.

**R-179** `ci` · MUST — Every migration MUST have a conformance test: a corpus
tree at the source version, migrated, then compared against the expected tree at
the target version.

### 16.4 Manifest versioning

**R-180** `cmd` · MUST — The federation manifest format is versioned
independently of this specification and changes more slowly. Repositories are not
upgraded on the same day; a manifest format that moved with every specification
release would break federation continuously.

**R-181** `cmd` · MUST — An implementation MUST read manifest versions older than
its own.

---

## Appendix — open questions for 0.2

These are known gaps, recorded so they are not rediscovered.

### Audience-facing documentation

This specification covers documentation written for the people who build a
system. Documentation written for the people who *use* it needs three additions,
designed but deliberately deferred:

**Product layer.** A namespace is one service. A product is usually several. A
product declares its members:

```yaml
product: checkout
namespaces: [svc-cart, svc-payment, svc-inventory]
```

**Audience field.** `internal: true` today is only an export filter. Three
audiences actually exist and they need different pages:

```yaml
audience: internal | integrator | end-user
```

**Derivation, distinct from graduation.** A user-facing page cannot be a copy of
an internal one — the assumed knowledge differs. It cannot be independent either,
or it goes stale. So it is rewritten but tracked:

```yaml
id: checkout-setup
audience: end-user
derives_from:
  - id: deploy-runbook
    hash: 8c21f0
```

Graduation *moves* content (R-090). Derivation *rewrites and tracks* it. When the
source changes the hash no longer matches and the user-facing page is reported
stale — which addresses the standard failure mode of product documentation.

### Other gaps

- Brownfield ingestion: which signals in version-control history are worth
  turning into documentation, and which are noise
- Whether `tutorial` earns its place in the `project` profile, or is
  knowledge-base only
- Graph export format for feature-to-code-to-documentation relations
- Whether `verification: unverified|verified` (knowledge-base profile) should
  apply to the `project` profile as well
- Epic status aggregation rules when legs disagree
- Generated API references (OpenAPI and equivalents): linked from the router, or
  addressed by identifier like any other page
