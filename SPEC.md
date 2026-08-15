# docsys Specification

**Version:** 0.2
**Status:** draft — rule numbers are permanent; rule text may be clarified, not
redefined, within a minor version

This document defines a documentation system for software projects and personal
knowledge bases. It is implementation-independent: any tool that satisfies the
rules below is conformant.

The reference implementation is `docsys`, a single static binary.

> **0.1 was never released.** It was audited by four independent models before
> publication and revised into this version. No migration path from 0.1 is
> provided or needed. See §18 for what changed and why.

---

## 1. Scope

### 1.1 What this specifies

- The on-disk layout of a documentation tree
- The frontmatter schema of a page
- The identifier and link grammar, and the lifecycle of an identifier
- The lifecycle of a unit of work, and how knowledge graduates out of it
- How independent repositories reference each other's documentation
- Which rules are mechanically enforced and which require judgment

### 1.2 What this does not specify

- Prose style, tone, or content
- Rendering, publishing, or site generation
- Any storage format other than plain files in a version-controlled tree
- Which agent, editor, or CI system is used

### 1.3 Design constraints

**R-001** `advisory` · MUST — Documentation MUST be plain text files in a
version-controlled tree. No database, no proprietary format.

**R-002** `advisory` · MUST — A fact MUST have exactly one home. A stale copy is
more dangerous than a missing page, because it is trusted. Derived, read-only,
hash-pinned materializations are not copies in this sense; see R-138.

**R-003** `advisory` · MUST — Deterministic checks belong to tooling;
classification and contradiction belong to a human or a model.

**R-004** `advisory` · SHOULD — Capture requires no discipline. Processing
requires full discipline.

**R-005** `advisory` · MUST NOT — Tooling MUST NOT author prose. It generates
only derived artifacts: indexes, routers, graphs, backlinks, timestamps,
coverage reports. Rendering normative text from this specification into another
format (R-155) is transformation, not authorship.

---

## 2. Conformance and rule notation

### 2.1 Rule format

A rule declaration is a line matching, at the start of the line:

```
^\*\*R-\d{3}\*\* `(lint|ci|cmd|agent|advisory)` · (MUST|MUST NOT|SHOULD|SHOULD NOT|MAY) — 
```

An example, indented so that it is not itself a declaration:

    **R-000** `lint` · MUST — rule text

`R-000` is reserved and is never a real rule.

The level is as defined in RFC 2119.

`enforcement` declares how the rule is verified:

| Tag | Meaning |
|---|---|
| `lint` | A local static check, which may read version-control history |
| `ci` | A cross-repository or pipeline gate |
| `cmd` | A command guarantees the outcome by construction |
| `agent` | Requires judgment; verified by a human or a model |
| `advisory` | Normative but not verifiable by tooling; assessed by human review |

`advisory` does not mean optional. It means no automated check can decide it.

Rule numbers are permanent. A withdrawn rule keeps its number and states where
its content went; numbers are never reused. A withdrawal declaration matches:

```
^\*\*R-\d{3}\*\* WITHDRAWN — 
```

A parser that counts rules counts both forms and reports withdrawals separately.

### 2.2 Coverage requirement

**R-010** `ci` · MUST — Every rule tagged `lint`, `ci`, or `cmd` MUST be covered
by at least one conformance test, unless the rule's level is `MAY` and the
implementation does not offer the optional capability. An implementation MUST be
able to report, per rule, which check covers it.

**R-011** `ci` · MUST — An *applicable* check that inspected zero units MUST
report failure, not success. A check is applicable when the tree declares the
feature it verifies: federation checks are applicable only when `namespace` is
declared, freshness checks only when at least one `verifies` block exists. An
inapplicable check reports "not applicable", never "passed".

> Rationale for R-011: in field use, a hook whose path patterns no longer matched
> anything stayed green for weeks while the rule it enforced was dead.

**R-012** `advisory` · MUST — A checker MUST NOT be looser than the rule text. If
a rule cannot be fully checked, it is tagged `agent` or `advisory`, never `lint`
with an incomplete check.

**R-013** `lint` · MUST — A conformant tree MUST declare `spec: docsys/<major>.<minor>`
in its `.docmeta.yml`. The major component MUST match an implemented major
version; the minor component MUST NOT be required to match.

**R-014** WITHDRAWN — merged into R-170.

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

**R-021** `advisory` · MUST — Profiles are independent systems. A tree of one
profile MUST NOT require the existence of a tree of another profile.

**R-022** `advisory` · MUST — The only coupling between trees is a reference
(§7). Absent a reference, there is no dependency and no shared state.

### 3.1 The knowledge-base profile

**R-023** `lint` · MUST — In the `knowledge-base` profile, `raw/` is append-only.
An existing file under `raw/` is never modified or deleted; new files enter
through `raw/inbox/` and are relocated to `raw/<domain>/` once processed.

**R-024** `lint` · MUST — A `wiki/` page carries `verification: unverified` or
`verification: verified`, and `sources:` listing the `raw/` paths it rests on. A
new or modified page is always `unverified`.

**R-025** `agent` · MUST — Only an independent session may set `verified`. The
session that produced a page never verifies it.

**R-026** `lint` · MUST — `domain` values are declared in `.docmeta.yml`. A page
whose domain is not declared is an error; content that fits no declared domain
stays in `raw/inbox/` rather than being forced into the nearest one.

---

## 4. Layout

### 4.1 Permanent layer

**R-030** `lint` · MUST — A permanent page MUST declare exactly one `type` from
the enumeration `reference`, `howto`, `explanation`, `tutorial`. This rule checks
the declaration, not the prose; see R-031.

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

**R-041** `lint` · MUST — Files under `features/`, `postmortems/`, and
`research/` are *tracked work* and carry `status` (§8). `journal.md`, `debt.md`,
and `questions.md` are *list files*: they hold many independent items, do not
track the life of one unit of work, and MUST NOT carry `status`.

**R-042** `lint` · MAY — A tree MAY declare additional tracked-work categories in
`.docmeta.yml`. They are subject to the `status` rules but have no defined
graduation target.

**R-043** `advisory` · SHOULD — A directory is created when its first file is
needed. Only `journal.md` and `debt.md` are created at initialization. An empty
directory reads as an obligation and produces filler content.

### 4.3 Reserved directories

**R-044** `lint` · MUST — The following names are reserved and excluded from
orphan and type checks: `_archive/`, `_templates/`, `_unsorted/`,
`.federation/`. Files under `.federation/` are additionally subject to R-149.

**R-045** `lint` · MUST NOT — A command MUST NOT delete content; obsolete content
moves to `_archive/`. Where version-control history is available, a check MAY
report content that disappeared without an archive counterpart. Removing a
secret, a credential, or content whose deletion is legally required is not a
violation of this rule.

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
title: Token lifetime    # optional — defaults to the first heading
summary: >               # optional — defaults to the first paragraph
  How long an access token stays valid and what refreshes it.
lang: tr                 # optional
internal: true           # optional — excluded from export
aliases: [token-lifetime]  # optional — retired identifiers (§6.2)
defines: "adr-*"         # optional — this page defines an identifier family
verifies:                # optional — freshness pin (§11)
  - path: src/auth/refresh.rs
    symbol: refresh_token
    hash: "sha256:a3f9c1…"
---
```

**R-050** `lint` · MUST — A permanent page MUST carry `id`, `type`, and
`updated`.

**R-051** `lint` · MUST — `owner` MUST be present on any page exported to other
namespaces. An unowned shared contract is an error.

> Rationale: in field use, a wire protocol between two components went
> undocumented for months because neither side considered itself the owner.

**R-052** `cmd` · MUST — `updated` MUST be maintained by tooling. Where a page's
`updated` is older than its last content change in version-control history, a
check reports it. Filesystem timestamps are never used; a fresh clone resets
them.

**R-053** `lint` · MAY — Router files (`index.md`), trackers (`roadmap.md`), and
`README.md` are exempt from frontmatter but MUST follow their own format rules.

**R-057** `lint` · MUST — `title` defaults to the text of the page's first
heading; `summary` defaults to its first paragraph, truncated at the first
sentence boundary. An implementation MUST use these defaults when the fields are
absent, so that two exporters produce identical manifests for identical trees.

### 5.2 Tracked work files

```yaml
---
status: active                     # required
updated: 2026-08-15                # required
epic: "@company/checkout-v2"       # optional
abandoned_reason: "..."            # required when status: abandoned
graduated_to: [token-ttl]          # required when status: graduated
---
```

**R-054** `lint` · MUST — A tracked-work file (R-041) MUST carry `status` and
`updated`. List files are exempt.

**R-055** `lint` · MUST — `status: abandoned` MUST carry a non-empty
`abandoned_reason`.

**R-056** `lint` · MUST — `status: graduated` MUST carry `graduated_to` with at
least one identifier that resolves.

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

**R-061** `lint` · MUST — An `id` MUST be unique within its namespace, counting
`aliases` and tombstones (§6.2) as occupied.

**R-062** `advisory` · MUST — **The identifier is the contract; the filename is
cosmetic.** An `id` MUST NOT change when a file is renamed, moved, or translated.

> Rationale: code once referenced documentation by file path. The first rename
> broke every reference, silently — no compiler, test, or reviewer saw it.

**R-063** `lint` · MAY — A page MAY declare `defines:` with a glob to register
itself as the definition site of an identifier family, for example `adr-*`. The
glob is matched against `local-id` values and therefore uses the same lowercase
grammar as R-060.

> `defines:` is authored. It is not the manifest field `provides` (R-131), which
> is derived. In 0.1 both were called `provides`, which four reviewers read as a
> contradiction.

**R-064** `advisory` · MUST — A `defines:` pattern MUST match the *definition*
form, not the *citation* form. A pattern that also matches citations reports
broken references as healthy.

### 6.2 Identifier lifecycle

Identifiers are stable but not immortal. Services are renamed, teams merge,
contracts retire. Without a lifecycle, the first rename is an estate-wide
outage.

**R-065** `lint` · MUST — A retired identifier is never reused. Renaming a page's
identifier means: the new `id` is assigned, and the previous one is added to
`aliases`. Both resolve; the alias resolves with a deprecation notice.

**R-066** `cmd` · MUST — Removing an exported identifier creates a tombstone: the
manifest continues to list it with `state: withdrawn`, the date, and optionally
`superseded_by`. A tombstoned identifier resolves to an explanatory error, never
to "not found".

**R-067** `lint` · MUST — A tombstone MUST be retained for at least the
deprecation window declared in `.docmeta.yml` (`deprecation_window`, default 180
days). Removing it earlier is a breaking change to consumers.

**R-068** `cmd` · MUST — Renaming a namespace follows the same rule: the previous
namespace is retained as an alias in the manifest for the deprecation window, and
foreign references to it resolve with a deprecation notice.

**R-069** `cmd` · MUST — Splitting or merging pages tombstones the retired
identifiers with `superseded_by` listing the replacements. A consumer of a split
identifier receives an error naming all successors, so a human can choose.

---

## 7. Links and references

### 7.1 Documentation to documentation

**R-070** `lint` · MUST — A doc-to-doc link MUST be a wiki-link carrying the full
path from the documentation root: `[[reference/token-ttl]]` or
`[[reference/token-ttl|alias]]`. Short-name links are invalid.

> Known tension: R-062 declares filenames cosmetic, yet this rule makes the path
> a contract between documents. An identifier-based link form is under
> consideration for 0.3; see §19.

**R-071** `lint` · MUST — Every link target MUST resolve. Resolution appends
`.md` when the path has no extension, and does not follow symlinks outside the
tree.

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

**R-076** `lint` · MUST — Both local and foreign references MUST resolve. A local
`doc: <local-id>` that matches no `id`, no alias, and no `defines:` family is an
error. In 0.1 only foreign references were required to resolve, so a typo in a
local reference was invisible.

### 7.3 Documentation to code

**R-074** `agent` · MUST NOT — Documentation MUST NOT contain source paths as
references. Code moves; documentation stays. Where code must be shown, the
snippet is embedded with a comment explaining why it is there. This requires
judgment: a path inside a quoted stack trace or an embedded snippet is not a
reference. The `verifies` block (§11) is an audit binding, not a reference, and
is exempt.

**R-075** `lint` · MUST NOT — Absolute paths (`/home/...`, `C:\...`) and links
that escape the tree root are errors.

**R-112** WITHDRAWN — its content is the exemption note now attached to R-074.

### 7.4 Scan scope

**R-077** `lint` · MUST — A tree declares the scan scope in `.docmeta.yml`.
Defaults: every UTF-8 text file tracked by version control, excluding paths
ignored by version control and excluding `.federation/`. An implementation MUST
NOT restrict scanning to a hardcoded set of file extensions.

> Rationale: a reference in a Helm chart or a build file is as real as one in
> source code. An implementation scanning only its own language's files silently
> under-reports `consumes`, and the provider then deletes a contract that is in
> use.

**R-078** `lint` · MUST — A scan MUST report the number of files inspected. Zero
inspected files fails under R-011.

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

**R-082** `lint` · MUST — `graduated` is terminal. Where version-control history
is available, a transition out of `graduated` is reported.

**R-083** `cmd` · MUST — A command MUST refuse a transition to `abandoned`
without a reason. R-055 checks the resulting file; this rule prevents the
transition from happening without one.

**R-084** `advisory` · SHOULD — Abandonment has two forms and they differ in
value. Work abandoned as unnecessary is archived. Work abandoned *after being
tried* carries expensive knowledge and SHOULD graduate to `explanation/` as a
rejected alternative.

**R-085** `lint` · SHOULD — A file whose last content change in version-control
history is older than `stale_active_days` (default 90) while `status: active`
SHOULD be reported. Undeclared abandonment is the common case, not the rare one.

### 8.2 Work types are not categories

**R-086** `advisory` · SHOULD — A bug is a *type of work*, not a category.
Categories are determined by output. A bug's output depends on one question:
**can it recur?**

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

### 9.1 Blocks

**R-098** `lint` · MUST — A **block** is the unit graduation moves. It is either:

- a heading together with all content until the next heading of equal or higher
  level, or
- a top-level CommonMark block element that is not inside such a section:
  paragraph, list, fenced code block, table, or block quote.

Blocks are addressed by their byte offsets in the source file, computed on the
canonical form defined in R-113.

### 9.2 Movement

**R-090** `cmd` · MUST — Graduation MUST move content, never rewrite it. A
conformant implementation copies whole blocks byte-for-byte; the model selects
which block goes where and never retypes the text.

> This converts a rule that depended on model discipline into a mechanical
> guarantee.

**R-091** `cmd` · MUST — After graduation the source file MUST retain a link to
the destination and MUST record `graduated_to`.

**R-092** `agent` · MUST — Order matters. A block whose content already exists on
a permanent page is removed from the source and replaced by a link. A block that
exists nowhere permanent is written to its destination *before* the source is
shrunk. Deciding whether two blocks say the same thing is judgment, not a
comparison of bytes.

**R-093** `agent` · MUST — Before archiving anything, the question is asked: does
this information exist anywhere else? If not, and it is still true, it graduates
first.

| Source | Usual destination |
|---|---|
| `features/` | `reference/` (contract), `explanation/` (decision) |
| `postmortems/` | `reference/` (invariant), `howto/` (runbook) |
| `research/` | `explanation/` |
| `abandoned` | `explanation/` (rejected alternative) |

**R-097** `cmd` · MUST — Every command that writes more than one file MUST be
atomic: either all writes land or none do. A command MUST refuse to start on a
dirty working tree unless explicitly forced, and MUST leave no state in which two
authoritative copies of the same content both pass lint.

### 9.3 Compilation to executable form

The graduation chain has one more link: flowing → permanent → executable.

**R-094** `cmd` · MAY — A `howto` page whose steps have stabilized MAY be
compiled into an executable agent skill.

**R-095** `cmd` · MUST — A compiled skill MUST record the source identifier and
the source content hash (R-113). When the source page changes, the compiled skill
is reported as stale.

**R-096** `agent` · MUST NOT — Compilation MUST NOT invent steps. A procedure not
fully written on the page is not ready to be compiled.

---

## 10. Journal

**R-100** `lint` · MUST — A journal entry opens with `## YYYY-MM-DD — title`.

**R-101** `lint` · SHOULD — An entry is 2–5 lines: what triggered it, what
changed, which gate passed, which page holds the permanent content. An entry
whose body exceeds 15 source lines is reported. Lines are counted in the source
file, not as rendered.

**R-102** `agent` · MUST NOT — Measurements, tables, algorithms, API lists,
register maps, and rejected alternatives MUST NOT be written to the journal. They
belong to the permanent layer; the journal links to them. Recognizing them
requires reading the prose, so this is not a static check.

> Rationale: a journal reached 3,800 lines in nine days and became 31% of all
> documentation. One constant appeared in eight places and none were ever
> updated. Permanent knowledge kept in a chronology silently becomes a lie.

**R-103** `cmd` · MUST — When the active journal exceeds 500 lines, the oldest
whole days are moved to an archive slice named
`work/journal/<first-date>--<last-date>.md`, carrying `status: graduated` and one
router line. Slices are cut on day boundaries; a single day larger than the limit
becomes its own slice.

**R-104** `lint` · MUST — Entries are ordered newest first. A retrospective entry
is inserted at its own date, not appended.

---

## 11. Freshness

A permanent page may pin itself to a region of code.

```yaml
verifies:
  - path: src/auth/refresh.rs
    symbol: refresh_token
    hash: "sha256:a3f9c1…"
```

**R-110** `lint` · MAY — A page MAY declare `verifies`. When the hash of the
referenced region no longer matches, the page is reported as stale.

**R-111** `lint` · MUST — A stale page is a loud failure, never a silent one. It
is the only mechanism in this specification that detects code-documentation
drift mechanically.

**R-113** `lint` · MUST — A **content hash** is `sha256` over the canonical form
of the content, written as `sha256:` followed by lowercase hex. The canonical
form is: UTF-8, NFC-normalized, LF line endings, trailing whitespace removed from
each line, exactly one trailing LF. Without a fixed definition two
implementations disagree about staleness for the same tree.

**R-114** `lint` · MUST — When `symbol` is absent, the hash covers the whole file
at `path`. When `symbol` is present, the implementation MUST declare how it
resolves symbols for that language, and MUST report an error rather than guess
when a symbol is ambiguous or unresolvable.

---

## 12. Language

**R-120** `lint` · MUST — Structural tokens MUST be ASCII: directory names, file
names, frontmatter field names, `id` values, `status` values, link syntax. This
is checked as a character-class and enumeration constraint, not as a language
determination.

**R-125** `advisory` · SHOULD — Structural tokens SHOULD read as English. No
check can determine the language of `masa`, so this is guidance, not a gate.

**R-121** `agent` · MAY — Content language is free and declared as
`default_content_language` in `.docmeta.yml`. A page may override it with
`lang:`.

**R-122** `agent` · MUST — When editing an existing page, its language is
preserved. A page has one prose language; code identifiers, protocol names,
product names, and quotations retain their original form and do not count as
mixing (R-123).

**R-123** `agent` · MUST NOT — Code identifiers, protocol names, library names,
and quotations are never translated.

**R-124** `agent` · MUST NOT — Migration never translates. Content moves as it
is. See also R-172, which forbids migrations from touching prose at all.

---

## 13. Federation

Federation lets independent repositories reference each other's documentation
without sharing a repository, a database, or a server.

### 13.1 Declaration

**R-130** `lint` · MUST — A tree participating in federation MUST declare
`namespace` in `.docmeta.yml`. The namespace is the only federation field written
by hand.

**R-131** `cmd` · MUST — The manifest fields `provides` and `consumes` MUST be
derived, never authored. `provides` comes from pages carrying `id`; `consumes`
comes from `doc: @ns/id` references found within the scan scope (R-077).

> A hand-maintained dependency list goes stale by construction (R-002). The
> authored, page-level declaration of an identifier family is a different thing
> and is called `defines:` (R-063).

**R-132** `lint` · MAY — In a monorepo, a namespace is defined per directory by
placing a `.docmeta.yml` at that directory's documentation root. Namespace roots
MUST NOT nest. All federation rules apply unchanged.

### 13.2 Export

**R-133** `cmd` · MUST — `export` produces a manifest containing the manifest
format version, the namespace, the spec version, and for each provided
identifier: type, title, summary, content hash, owner, and state (`active` or
`withdrawn`). Titles and summaries follow R-057.

**R-134** `cmd` · MUST NOT — The manifest MUST NOT contain page prose. Prose
travels through the content channel (§13.3), not the manifest.

**R-135** `lint` · MAY — A page marked `internal: true` is excluded from the
manifest.

### 13.3 Transport

The manifest says what exists. The content channel delivers the bytes. In 0.1
only the manifest was specified, which left the mandatory consumption flow with
no source of content.

**R-145** `lint` · MUST — A publishing namespace MUST declare, in
`.docmeta.yml`, where consumers obtain its manifest and its page content. The
mechanism is not fixed by this specification: a URL, a package artifact, a git
remote and revision, or a filesystem path are all conformant. What is fixed is
that the location is declared and resolvable without human intervention.

**R-146** `cmd` · MUST — The manifest is authoritative. Content fetched through
the content channel MUST be verified against the hash recorded in the manifest.

**R-147** `cmd` · MUST NOT — Content whose hash does not match the manifest MUST
NOT be materialized. A mismatch is reported as a failure, never silently
accepted.

**R-148** `cmd` · MUST — When a provider is unreachable, the last verified
materialization is retained and its age is reported. A transient outage MUST NOT
delete content a consumer already had.

**R-149** `lint` · MUST — Every file under `.federation/` MUST carry provenance:
source namespace, identifier, content hash, and the time it was fetched. A file
without provenance is an error, so hand-placed content cannot masquerade as
federated content.

### 13.4 Consumption

**R-136** `cmd` · MUST — A consumed page is materialized under `.federation/` as
a read-only copy carrying its provenance (R-149).

**R-137** `lint` · MUST NOT — Files under `.federation/` MUST NOT be edited
locally. A file whose content no longer matches its recorded provenance hash is
an error.

**R-138** `advisory` · MUST — R-002 does not apply to materializations under
`.federation/`. They are exempt because they are derived, read-only,
hash-pinned, and machine-refreshed. What R-002 forbids is an *unchecked* copy
that can silently lie.

### 13.5 Enforcement

**R-139** `ci` · MUST — A reference to a nonexistent foreign identifier MUST fail
the consuming repository's pipeline. This is an instance of the R-151 exception:
an agent that cannot resolve a reference invents an answer, so the failure is
silent and harmful.

**R-140** `ci` · MUST — Removing or tombstoning a provided identifier MUST report
the consuming namespaces *known to the local federation state* before the change
lands. Where that state is incomplete, the report MUST say so rather than imply
that no consumers exist.

**R-141** `ci` · MAY — Complete consumer coverage requires an index built from
every participating namespace's manifest. That index is an optional component. An
estate without one operates with the partial knowledge described in R-140.

**R-142** `advisory` · SHOULD — Every check that can run in the repository owning
the problem does run there; no central service is required for those. Complete
consumer coverage (R-141) is the single exception, and it is optional.

**R-143** `cmd` · SHOULD — When a consumed page's hash changes, the consuming
repository SHOULD receive an automated change proposal showing the old and new
content side by side. The old content is the previous verified materialization;
the new content comes through the content channel.

### 13.6 Asymmetric membership

**R-144** `lint` · MAY — A namespace MAY declare `federation_role: consume-only`.
It reads manifests but publishes none, and no other namespace may reference it.

> A private or machine-local tree must be `consume-only`. A reference to it
> resolves on its owner's machine and is dead everywhere else, which is exactly
> the silent failure R-151 exists to prevent.

---

## 14. Automation levels

**R-150** `advisory` · MUST — Automated checks warn by default. Hard blocking
creates friction, and friction is resolved by disabling the check entirely —
which removes the protection completely.

**R-151** `advisory` · MUST — A check escalates to blocking only when the outcome
is **irreversible** or **silently wrong**. Everything else warns. Every blocking
rule in this specification cites this rule.

> The second criterion was added after federation was designed: a broken
> reference is reversible but produces a confidently wrong answer, which is worse
> than a visible failure.

**R-152** `lint` · MUST — A warning MUST name the file that needs to change.

> Rationale: in a single 1,979-line commit an unnamed warning went unnoticed and
> five contracts shipped undocumented.

**R-153** `advisory` · MUST — A vital rule lives in two places: the always-loaded
summary and a mechanical check. Guidance may fail to load; the check always runs.

**R-154** `cmd` · SHOULD — Concurrent writing sessions in one tree SHOULD be
detected. A lock records host, process, and start time under the tree, and a
lock older than `lock_timeout` (default 4 hours) is reported as stale rather than
honored.

**R-155** `cmd` · MUST — Rule text presented to an agent MUST be generated from
this specification, not maintained as a hand-written copy. This is transformation
of existing normative text, not authorship (R-005).

---

## 15. `.docmeta.yml`

```yaml
spec: docsys/0.2                 # required
profile: project                 # required — project | knowledge-base
default_content_language: en     # required
created: 2026-08-15              # required

namespace: svc-auth              # required when federation is enabled
federation_role: publish         # publish | consume-only  (R-144)
manifest_url: "…"                # required when publishing (R-145)
content_url: "…"                 # required when publishing (R-145)

work_categories: []              # additional tracked-work categories (R-042)
domains: []                      # knowledge-base profile only (R-026)

scan_exclude: []                 # added to version-control ignores (R-077)
postmortem_threshold: "4h"       # R-087
stale_active_days: 90            # R-085
deprecation_window: 180          # days, R-067
lock_timeout: "4h"               # R-154
```

**R-160** `lint` · MUST — `.docmeta.yml` MUST exist at the documentation root and
MUST declare `spec`, `profile`, `default_content_language`, and `created`.

**R-161** `lint` · MUST — An implementation MUST reject unknown keys under a
different major version and MUST ignore unknown keys within the same major
version. Silent acceptance of a misspelled key is a configuration bug that
surfaces as missing enforcement.

**R-162** `lint` · MAY — An implementation MAY store its own configuration
elsewhere. Fields that affect no rule in this specification do not belong in
`.docmeta.yml`; in 0.1 three such fields (`phase`, `type`, `mode`) implied
behavior that no conformant tool provided.

---

## 16. Versioning and migration

A tree created under one version of this specification must be able to move to
the next without hand editing.

### 16.1 Compatibility

**R-170** `cmd` · MUST — An implementation MUST refuse to operate on a tree whose
major version it does not implement, rather than degrade silently. A minor
version difference MUST NOT block operation.

**R-171** `cmd` · MUST — Every command reports a version difference in one line
and names the command that resolves it. When no such command exists — an older
tool facing a newer tree — it says so instead of naming one.

### 16.2 What a migration may change

**R-172** `cmd` · MUST — A migration changes structure only: frontmatter field
names, directory locations, default values, and the *targets* of links and
identifier references that its own moves invalidated. It MUST NOT alter prose.
Rewriting `[[reference/x]]` to `[[refs/x]]` after moving that directory is a
structural change; rewording the sentence around it is not permitted.

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

**R-178** `advisory` · MAY — Rollback uses version control. No separate rollback
mechanism is specified.

**R-179** `ci` · MUST — Every migration MUST have a conformance test: a corpus
tree at the source version, migrated, then compared against the expected tree at
the target version.

### 16.4 Manifest versioning

**R-180** `cmd` · MUST — The federation manifest format carries its own version,
independent of this specification, and changes more slowly. Repositories are not
upgraded on the same day; a manifest format that moved with every specification
release would break federation continuously.

**R-181** `cmd` · MUST — An implementation MUST read manifest versions older than
its own, and MUST ignore unknown fields in manifest versions newer than its own
within the same major version. Backward reading alone would force the estate-wide
lockstep upgrade that R-180 exists to prevent.

**R-182** `cmd` · MUST — A manifest major version an implementation does not
implement causes a refusal to consume that namespace, reported by name, not a
silent skip.

---

## 17. Conformance testing

**R-190** `ci` · MUST — An implementation MUST ship a conformance corpus: trees
exercising each `lint`, `ci`, and `cmd` rule, with expected outputs.

**R-191** `ci` · MUST — The corpus MUST include at least one tree per rule that
*violates* it. A checker that never sees a violation is untested.

**R-192** `advisory` · SHOULD — The corpus is the portable part of this
specification. An independent implementation demonstrates conformance by passing
it.

---

## 18. Changes from 0.1

0.1 was audited by four independent models (one cloud, three local) before
release. 97 raw findings produced 28 verified issues. The material changes:

| Change | Rules |
|---|---|
| Federation content transport specified; the consumption flow had no source of bytes | R-145–R-149 |
| Identifier lifecycle added: aliases, tombstones, deprecation window, split/merge | R-065–R-069 |
| Five rules moved from `lint` to `agent` because no static check can decide them | R-074, R-102, and the split of R-120/R-125 |
| `advisory` redefined as "normative but unverifiable" instead of "guidance only" | §2.1 |
| Consumer reporting made honest; complete coverage declared to need an optional index | R-140–R-142 |
| Content hash defined: sha256 over a canonical form | R-113 |
| "Block" defined so graduation has a deterministic input | R-098 |
| Scan scope defined; extension-limited scanning forbidden | R-077, R-078 |
| Local `doc:` references must resolve; in 0.1 a typo was invisible | R-076 |
| `provides:` (authored) renamed to `defines:` to end the collision with the derived manifest field | R-063 |
| Version declaration parsed as major.minor; R-014 merged into R-170 | R-013, R-170 |
| Manifest forward compatibility added | R-181, R-182 |
| Zero-unit failure scoped to *applicable* checks | R-011 |
| `.federation/` exemption from R-002 made normative rather than explanatory | R-002, R-138 |
| Provenance required under `.federation/` so hand-placed files cannot pass | R-149 |
| Timestamps read from version-control history, never the filesystem | R-052, R-085 |
| Atomicity required for multi-file commands | R-097 |
| Title and summary defaults defined so exporters agree | R-057 |
| `knowledge-base` profile given a minimum definition | R-023–R-026 |
| Behavior-free configuration fields removed | R-162 |
| Rule-declaration grammar fixed so a parser cannot mistake the example for a rule | §2.1 |

---

## 19. Open questions

Recorded so they are not rediscovered.

### Identifier-based document links

R-062 declares filenames cosmetic; R-070 makes the path a contract between
documents. Renaming a file preserves code references and breaks documentation
links. A link form addressing the identifier directly would make the founding
principle uniform. Deferred because it changes every existing link and deserves
its own discussion.

### Audience-facing documentation

This specification covers documentation for the people who build a system.
Documentation for the people who *use* it needs three additions:

**Product layer.** A namespace is one service; a product is usually several.

```yaml
product: checkout
namespaces: [svc-cart, svc-payment, svc-inventory]
```

**Audience field.** `internal: true` is only an export filter. Three audiences
exist and need different pages:

```yaml
audience: internal | integrator | end-user
```

**Derivation, distinct from graduation.** A user-facing page cannot be a copy of
an internal one — the assumed knowledge differs — and cannot be independent
either, or it goes stale. So it is rewritten but tracked:

```yaml
id: checkout-setup
audience: end-user
derives_from:
  - id: deploy-runbook
    hash: "sha256:8c21f0…"
```

Graduation *moves* content (R-090). Derivation *rewrites and tracks* it. When the
source changes the hash no longer matches and the user-facing page is reported
stale — which addresses the standard failure mode of product documentation.

### Other gaps

- Brownfield ingestion: which signals in version-control history are worth
  turning into documentation, and which are noise
- Whether `tutorial` earns its place in the `project` profile
- Graph export format for feature-to-code-to-documentation relations
- Whether `verification` should extend to the `project` profile
- Epic status aggregation when legs disagree
- Generated API references: linked from the router, or addressed by identifier
- Authentication and integrity of the content channel beyond hash verification
  (R-146 verifies content matches the manifest, but not that the manifest itself
  is authentic)
