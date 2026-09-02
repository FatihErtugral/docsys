# docsys Specification

**Version:** 0.4
**Status:** frozen core + experimental federation. Rule numbers are permanent;
rule text may be clarified, not redefined, within a minor version. From 0.4 the
fix policy is **deletion-first**: no change adds net normative rules — text
grows only when a smaller text was tried and failed.

This document defines a documentation system for software projects and personal
knowledge bases. It is implementation-independent: any tool that satisfies the
rules below is conformant.

The reference implementation is `docsys`, a single static binary.

> **No version before 0.4 was released.** The text was audited by up to six
> independent models across six adversarial rounds and revised each time; the
> full history, with reasons, is the version-control log (§18).

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
more dangerous than a missing page, because it is trusted. A materialization
under `.federation/` is exempt: it is derived, read-only, hash-pinned, and
machine-refreshed. What this rule forbids is an *unchecked* copy that can
silently lie.

**R-003** `advisory` · MUST — Deterministic checks belong to tooling;
classification and contradiction belong to a human or a model.

**R-004** `advisory` · SHOULD — Capture requires no discipline. Processing
requires full discipline.

**R-005** `advisory` · MUST NOT — Tooling MUST NOT author prose. It generates
only derived artifacts: indexes, routers, graphs, backlinks, timestamps,
coverage reports. Rendering normative text from this specification into another
format (R-155) is transformation, not authorship. The boundary is determinism,
not the binary the words pass through: a model exercising judgment is an author
under this rule, not tooling. The opening sentence R-099 requires is written by
the model as author; what no conformant tool does is generate prose
deterministically and present it as authored.

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

### 2.2 Severity vocabulary

This specification uses two words for check outcomes, and they map onto the
escalation criterion in R-151:

| Word in rule text | Behavior | When it is allowed |
|---|---|---|
| **is an error** / **fails** | Blocks: nonzero exit, pipeline stops | Only when the outcome is irreversible or silently wrong (R-151) |
| **is reported** | Warns: named in output, exit unaffected | Everything else |

**R-015** `lint` · MUST — An implementation MUST be able to list which rules it
treats as blocking, so two implementations can be compared on the behavior teams
actually experience.

**R-016** WITHDRAWN — restated §2.2's severity table and R-151's criterion; both
remain the authoritative statements.

**R-017** `advisory` · MUST — A rule that states no severity word warns. An
implementation MUST NOT invent blocking behavior for such a rule, and this
specification MUST NOT leave a rule unmarked whose violation is silently wrong.

**R-018** `ci` · MUST — Every guarantee tagged `cmd` whose violation leaves an
observable trace in the tree or its version-control history MUST have a `lint`
backstop that detects the same violation when it is produced by hand instead of
by the command. Two classes are verified by conformance tests (R-010, R-190)
instead: rules that constrain only how a command behaves — atomicity (R-097),
plan-before-apply (R-176) — and rules whose trace exists but is not mechanically
decidable, such as R-097's duplicate-content clause, where sameness is judgment
(R-092) and the backstop is the audit's duplicate check.

> Rationale: `cmd` describes what a tool does, not what a tree looks like. A
> human who deletes a page with `git rm` bypasses tombstone creation; a team that
> appends to `journal.md` in an editor bypasses rotation. Without a backstop the
> guarantee holds only for people who were already using the tool correctly —
> which is the population that did not need it.

**Experimental sections.** A section whose heading carries `(EXPERIMENTAL)` is
a design draft, not a conformance requirement. Its rules keep their numbers and
declaration form — the tags record *intended* enforcement — but no conformance
obligation (R-010, R-190, R-191) attaches to them, and no normative rule may
depend on one for its own meaning or enforcement. An experimental section
becomes normative only after a reference implementation exists and its rules
survive against real use: a complex system that works grows out of a simple
system that works, never out of more prose.

Rule numbers are permanent. A withdrawn rule keeps its number and states where
its content went; numbers are never reused. A withdrawal declaration matches:

```
^\*\*R-\d{3}\*\* WITHDRAWN — 
```

A parser that counts rules counts both forms and reports withdrawals separately.

### 2.3 Coverage requirement

**R-010** `ci` · MUST — Every rule tagged `lint`, `ci`, or `cmd` MUST be covered
by at least one conformance test, unless the rule's level is `MAY` and the
implementation does not offer the optional capability. An implementation MUST be
able to report, per rule, which check covers it.

**R-011** `ci` · MUST — Every check MUST report how many units it inspected,
and the distinction that matters is *empty population* versus *dead scan*. An
empty population — no tracked work yet, no `verifies` blocks, no foreign
references — reports zero and passes: a fresh tree is not a broken one. A
**dead scan** — the check's configured scope matched no files even though files
of that kind exist in the tree — MUST report failure, not success. A check
whose feature is not declared at all reports "not applicable", never "passed".

> Rationale for R-011: in field use, a hook whose path patterns no longer matched
> anything stayed green for weeks while the rule it enforced was dead.

**R-012** `advisory` · MUST — A checker MUST NOT be looser than the rule text. If
a rule cannot be fully checked, it is tagged `agent` or `advisory`, never `lint`
with an incomplete check.

**R-013** `lint` · MUST — A conformant tree MUST declare `spec: docsys/<major>.<minor>`
in its `.docmeta.yml`. The major component MUST match an implemented major
version; the minor component MUST NOT be required to match.

**R-014** WITHDRAWN — merged into R-170.

### 2.4 Definitions

**Content change.** A change to a page is a *content change* unless it touches
only: the `updated` field, the verification record (R-028), a `sources:` path
rewrite performed under R-027, or a structural target rewrite performed by a
migration under R-172. Every rule that reads "content change" — R-024, R-052,
R-082, R-085, R-106 — reads this definition. This is a definition, not a rule:
it cannot be violated, so it carries no enforcement tag and incurs no coverage
obligation.

> Without this, the `updated` repair (R-156) is itself a newer change than the
> value it writes, and the repair loop never terminates; and a mechanical
> `sources:` rewrite would flip every resting page to `unverified`.

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

**R-021** WITHDRAWN — a special case of R-022: absent a reference there is no
dependency, so no tree can require the existence of a tree of another profile.

**R-022** `advisory` · MUST — The only coupling between trees is a reference
(§7). Absent a reference, there is no dependency and no shared state.

### 3.1 The knowledge-base profile

**R-023** `lint` · MUST — In the `knowledge-base` profile, `raw/` is
**content-immutable**, not path-immutable. The bytes of an existing file are
never changed and no file is deleted, but relocation is permitted and expected:
new files enter through `raw/inbox/` and move to `raw/<domain>/` once processed.
Removing a secret or content whose deletion is legally required is not a
violation.

**R-027** `cmd` · MUST — Relocating a file under `raw/` MUST rewrite every
`sources:` entry that pointed at the old path. A relocation that severs the
evidence trail of a `wiki/` page is a silent failure of the kind R-151 forbids.

**R-024** `lint` · MUST — A `wiki/` page carries `verification: unverified` or
`verification: verified`, and `sources:` listing the `raw/` paths it rests on. A
page that undergoes a **content change** (§2.4) becomes `unverified`. The §2.4
exclusions are what make verification recordable at all: without them the act of
recording it, or a mechanical `sources:` path rewrite, would immediately undo it.
Where history is available the claim is checked, not trusted: a `verified` page
whose body no longer hashes to what it held at `verified_rev` **is an error**
until it is `unverified` again, and a `verified_rev` that does not hold the page
**is an error** under R-028 (D-077).

**R-025** `agent` · MUST — Only an independent session may set `verified`. The
session that produced a page never verifies it.

**R-028** `lint` · MUST — Setting `verified` MUST record, in the page's
frontmatter, who verified it and which source revision was verified. Without
this record no reviewer can establish whether verification was independent
(R-025) or whether it predates the current content.

**R-026** `lint` · MUST — `domain` values are declared in `.docmeta.yml`. A page
whose domain is not declared **is reported**; content that fits no declared
domain stays in `raw/inbox/` rather than being forced into the nearest one.

**R-029** `lint` · MUST — In `wiki/<domain>/<type>/`, the directory's type
segment MUST match the page's declared `type`. Readers navigate this profile by
directory; a `type: reference` page sitting under `howto/` misdirects everyone
who arrives that way.

**R-059** `lint` · MUST — Every `sources:` entry MUST resolve. R-071 covers only
wiki-links, so without this a manual move or deletion under `raw/` severs a
verified page's evidence trail with nothing reporting it — including when R-027's
command path was bypassed entirely. An entry may name version-control
evidence — `git:<sha>`, `tag:<ref>`, `git:<sha>:<path>[@L<a>-L<b>]` — resolved
against the repository the tree lives in. Where the verification doctrine
applies (a knowledge base's permanent pages) a severed entry **is an error**;
on any other page that declares sources (a seeded work file) it **is
reported**: evidence that moved must be seen, not block the tree.

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
router (`index.md`). An unreachable page is an orphan and **is reported**:
adding the router line afterwards is routine and reversible, so blocking here
would be the friction R-151 warns about. The check is applicable (R-011) only
when at least one permanent page exists.

**R-035** `lint` · MUST — A router line is:

```
- [[<path>|<title>]] -- <one sentence>
```

New entries are **appended at the end**; any richer ordering is human work. A
line not matching the grammar **is reported**. The format is normative because
three rules depend on it: reachability edges (R-034), the journal slice's
router line (R-103), and deterministic router repair (R-156). A router routes —
its entries are links and one-sentence hooks, never content.

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
`research/` are *tracked work* and carry `status` (§8). `journal.md`,
`debt.md`, `questions.md`, journal archive slices under `journal/`, and archived
list-file slices under `_archive/` (R-108) are *list files*: they hold many
independent items, do not track the life of one unit of work, and MUST NOT
carry `status`.

**R-042** `lint` · MAY — A tree MAY declare additional tracked-work categories in
`.docmeta.yml`. They are subject to the `status` rules but have no defined
graduation target.

**R-043** `advisory` · SHOULD — A directory is created when its first file is
needed. Only `journal.md` and `debt.md` are created at initialization. An empty
directory reads as an obligation and produces filler content.

### 4.3 Tracked-work templates

Templates bind *tracked work*, not capture. Capture — a journal line, an inbox
note — stays format-free (R-004): friction at capture is lost knowledge. A
tracked-work file is a processing artifact, and its structure is the input
format of graduation: standard sections turn "which block goes where" from a
guess into a lookup.

**R-048** `lint` · MUST — The three core tracked-work categories have templates,
installed into `_templates/` at initialization. A tracked-work file MUST carry
its category's section headings; a missing section **is reported**. A section
with nothing to say is left empty — it is never filled for completeness. Closed
work — `graduated` or `abandoned` — is exempt: the template guides work while it
is open and makes graduation mechanical (R-049), so demanding it of a page whose
value was already extracted or explicitly discontinued asks for structure that
can no longer do anything. A closed page that carries the headings keeps them
(R-098).

| Category | Required sections |
|---|---|
| `features/` | `## Context` · `## Decision` · `## Contract surface` · `## Rejected alternatives` |
| `postmortems/` | `## What happened` · `## Root cause` · `## Recurrence` · `## Lesson` |
| `research/` | `## Question` · `## Tried` · `## Learned` · `## Why no decision` |

Section headings are structural tokens (R-120): the heading line is fixed
English, the prose beneath it follows the content language (R-121).

**R-049** `advisory` · MUST — Every required section has a **declared
disposition**: it graduates to a destination, or it is explicitly retained and
archived with the file. A section is never silently dropped. Graduation reads
this table instead of inferring:

| Section | Disposition |
|---|---|
| `Context` | retained — frames the file |
| `What happened` | retained — the narrative is the record |
| `Root cause` | `explanation/` — the systemic why MUST survive the file |
| `Tried` | `explanation/` — negative results are knowledge |
| `Contract surface` | `reference/` |
| `Decision`, `Rejected alternatives` | `explanation/` (ADR) |
| `Recurrence` — the invariant and its test | `reference/` |
| `Lesson` | `howto/` when procedural, else `explanation/` |
| `Learned` | `explanation/` |
| `Question`, `Why no decision` — still open at closure | a **fresh** `questions.md` item (R-108 grammar) linking back; the section itself is retained. Writing the item is model authorship, licensed as in R-099 — a section cannot move byte-exactly into a one-line grammar |
| `Question`, `Why no decision` — closed with reasoning | `explanation/` |

List files are not templated — they hold many independent items, not sections —
but each has an **item grammar**:

**R-108** `lint` · MUST — A list-file entry MUST match its item grammar; a
non-matching entry **is reported**. The check is applicable (R-011) only when
the file has at least one entry. `journal.md` is governed by R-100/R-101. The
other two use `- [ ]` for open and `- [x]` for closed entries:

| File | Item grammar |
|---|---|
| `debt.md` open | `- [ ] YYYY-MM-DD <debt> -- deferred: <reason> -- repay when: <trigger>` |
| `debt.md` closed | `- [x] YYYY-MM-DD <debt> -- deferred: <reason> -- repay when: <trigger> -- resolved: <note or link>` |
| `questions.md` open | `- [ ] YYYY-MM-DD <question>` optionally ` -- <context or link>` |
| `questions.md` closed | `- [x] YYYY-MM-DD <question> -- answered: <link or one line>` |

An open item carries its opening date so its age can be measured (D-039).
`debt.md` holds items only: a heading or a paragraph after its first item **is
an error**. It is always one of three things written in the wrong form — a
closed debt kept as prose (it leaves the file; the journal line records the
repayment), a lesson (it goes to `work/postmortems/`), or an open debt without
its line (it becomes a dated `- [ ]` item). Prose before the first item is the
file's own preamble and is free. A list item without a checkbox — `- text`,
before or after the first item — is not preamble: it **is an error** in both
files, because it reads as an entry to a person and as nothing to every check
(no date, no `debt close` number, no age). Blocking is R-151's criterion met
exactly: a ledger that reads as a list while its content lives in prose or in
checkbox-less bullets is silently wrong, and the fix is one deletion or one
move.

The field labels above are canonical, not literal: `.docmeta.yml` MAY declare
`list_labels: [deferred=<local form>, repay when=<local form>, resolved=<local
form>, answered=<local form>]`, exactly as `headings` does for template sections
(D-025). A tree writing its documentation in another language must not be forced
to embed English words in its own prose to satisfy a checker; the tool knows no
language, so the tree declares its own.

The field markers are the literal strings ` -- deferred: `, ` -- repay when: `,
` -- resolved: `, ` -- answered: `, and bare ` -- `. They are matched **in the
declared order, at the first occurrence of each, left to right**; the last
field takes the remainder of the line, so field text may contain hyphens and
dashes freely. All separators are ASCII — one canonical spelling, because two
equivalent spellings make identical entries diff differently.

Closed entries remain valid indefinitely. When a list file grows past the
journal limit (R-103's threshold applies), closed entries MAY be moved in bulk
to an archive slice under `_archive/` — the location R-041 already names; the
slice remains a list file and its entries stay subject to this grammar. The R-093 question is
asked before the sweep — a closed entry whose `resolved:` points nowhere may be
the only record of the answer.

> A debt entry without a repayment trigger is never repaid — it is a wish, not a
> debt. A question without a date cannot be aged, and an unaged question list
> only grows.

### 4.4 Reserved directories

**R-044** `lint` · MUST — The following names are reserved and excluded from
orphan and type checks: `_archive/`, `_templates/`, `_unsorted/`,
`.federation/`. Files under `.federation/` are governed by §13 when federation
is active, and do not exist otherwise.
Files under `_archive/` are additionally excluded from resolution checks
(R-059, R-071, R-076): an archived page is a record, not a live claim, and its
references describe the tree as it was — erroring on them forever as their
targets retire would push teams to delete instead of archive (R-045).

**R-045** `agent` · MUST NOT — A command MUST NOT delete authored content;
obsolete content moves to `_archive/`. Derived content is exempt: refreshing a
`.federation/` materialization replaces bytes without archiving them, and so does
regenerating any other derived artifact. Removing a secret, a credential, or
content whose deletion is legally required is also not a violation. The
exemptions require judgment, which is why this is not a `lint` rule; where
version-control history is available a check reports disappeared authored content
for a human to assess.

**R-046** `agent` · SHOULD — `_unsorted/` is temporary. Content that cannot be
classified goes there and is reported, never force-fitted into a type.

**R-047** `lint` · MUST — A page under `_archive/` is not an authoritative home.
Its `id`, when present, MUST have a tombstone in the ledger (R-066) — archiving
retires the identifier, with `superseded_by` naming the graduation destination
when one exists (R-093). An archived identifier without a tombstone **is
reported**. `provides` (R-131) derives from live pages only, so an archived page
reaches the manifest solely through its tombstone, as `withdrawn` — never as an
active contract.

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
`updated`. Router files (`index.md`) and `README.md` are exempt; router lines
follow R-035.

**R-051** WITHDRAWN — absorbed by R-133 (EXPERIMENTAL): `owner` is a field of
the export manifest, and "exported" has no meaning outside federation, so the
requirement is checked where it is checkable — at export. The field lesson
behind it (a wire protocol nobody owned) stays in R-133's territory.

**R-052** `cmd` · MUST — `updated` MUST be maintained by tooling. Filesystem
timestamps are never used; a fresh clone resets them. R-106 backstops the hand
edit that skips the tooling.

**R-106** `lint` · MUST — Where version-control history is available, a page
whose `updated` is older than its last content change in history **is an
error**. This is the R-018 backstop for R-052: an edit made without the
tooling leaves exactly this trace, the freshness field is the one claim a
reader cannot check by hand, and the fix is one date (D-070, D-071).

**R-053** WITHDRAWN — folded into R-050. An exemption is not an optional
capability, and at `MAY` level R-010 let an implementation decline to test it
and then demand frontmatter on `index.md`.

**R-057** `lint` · MUST — `title` defaults to the text of the page's first
heading; `summary` defaults to its first paragraph, truncated at the first
sentence boundary. An implementation MUST use these defaults when the fields are
absent, so that one implementation produces identical manifests for identical
trees. The sentence-boundary algorithm is implementation-defined (§19), so
identity across *implementations* is not guaranteed; a tree that needs it
authors `title` and `summary` explicitly.

### 5.2 Tracked work files

```yaml
---
status: active                     # required
updated: 2026-08-15                # required
epic: checkout-v2                  # optional — declared in .docmeta.yml epics:
confirmed: "fatih, 2026-08-15"     # required at done and graduated (R-081)
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

**R-058** `lint` · MUST — `epic`, when present, MUST match `local-id` or
`foreign-id` (§6.1). A local value MUST appear in the `epics:` list declared in
`.docmeta.yml` — a typo has nowhere to hide, and a single non-federated
repository groups work without inventing a namespace. A foreign value **is
reported** as unsupported: the manifest cannot represent epics today, and
silently accepting an unresolvable grouping is the disease R-162 was written to
cure. The check is applicable (R-011) only when a tracked-work file declares an
`epic`.

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

**R-061** `lint` · MUST — An `id` MUST be unique within its namespace. The
uniqueness domain is: every page carrying `id` outside `_archive/` and
`.federation/`, plus `aliases`, plus tombstones (§6.2). A collision **is an
error**: two pages claiming one identifier make every reference to it ambiguous
and make the exported hash depend on directory traversal order. Two exclusions
are deliberate: an archived copy sharing its own tombstone's identifier is the
expected record pair, not a collision (R-047); and a materialized foreign page
never enters the local domain — its home namespace owns that identifier.

**R-062** `advisory` · MUST — **The identifier is the contract; the filename is
cosmetic.** An `id` MUST NOT change when a file is renamed, moved, or translated.

> Rationale: code once referenced documentation by file path. The first rename
> broke every reference, silently — no compiler, test, or reviewer saw it.

**R-063** `lint` · MAY — A page MAY declare `defines:` with a glob to register
itself as the definition site of an identifier family, for example `ADR-*`. A
family pattern carries **its own case**: `family-prefix ::= [A-Za-z0-9]+ ("-"
[A-Za-z0-9]+)* "-"` followed by `*`, matched case-sensitively. Family members
are not page ids — R-060's lowercase grammar governs ids, the declared pattern
governs its members — so the founding field convention (`ADR-mem-budget`,
`K-036`, entrenched across hundreds of code references) is legal by
declaration, while undeclared uppercase tokens remain grammar findings. Its
purpose is family resolution: it matches the identifiers by which members are
*cited* (R-076), and R-079 checks that a cited member actually exists on the
defining page.

> `defines:` is authored. It is not the manifest field `provides` (R-131), which
> is derived. In 0.1 both were called `provides`, which four reviewers read as a
> contradiction.

**R-064** WITHDRAWN — its definition-form/citation-form distinction contradicted
the purpose of R-063, whose glob exists precisely to match citations (R-076);
the failure it targeted — broken references reported as healthy — is prevented
by R-079's membership check.

### 6.2 Identifier lifecycle

Identifiers are stable but not immortal. Services are renamed, teams merge,
contracts retire. Without a lifecycle, the first rename is an estate-wide
outage.

**R-065** `lint` · MUST — A retired identifier is never reused. Renaming a page's
identifier means: the new `id` is assigned, and the previous one is added to
`aliases`. Both resolve; the alias resolves with a deprecation notice. Where
version-control history is available, an alias that disappeared from a page
without entering the ledger **is reported** — hand-tidying an alias frees a name
some cached reference still trusts.

**R-066** `cmd` · MUST — Removing an identifier creates a tombstone in the
**tombstone ledger** at `.tombstones.yml` in the documentation root, maintained
only by tooling. Each entry records the identifier, the withdrawal date, and
optionally `superseded_by`. (Federation adds an internality marking to ledger
entries — that is R-135's business, §13, not the core ledger's.) A tombstoned
identifier resolves to an explanatory error, never to "not found".

The ledger exists because a deleted page cannot supply its own tombstone, and
because the guarantee that a retired identifier is never reused (R-065) must hold
in a single repository with no federation at all — which is the most common
deployment.

**R-107** `lint` · MUST — Where version-control history is available, a deleted
page whose `id` has no ledger entry **is reported**. This is the R-018 backstop
for R-066: `git rm` bypasses tombstone creation, and the ledgerless
disappearance is its trace. Actual reuse of the freed identifier is the
collision R-061 blocks.

**R-067** `lint` · MUST — A tombstone is **never removed**, with one exception:
restoring the archived page it belongs to. Un-archiving returns the identifier
to its original owner and deletes its ledger entry in the same command — the
one case where removal cannot free the name for anyone else. Otherwise the
deprecation window declared in `.docmeta.yml` (`deprecation_window`, default
180 days) governs how a tombstoned identifier *resolves*, not how long the
entry lives: inside the window it resolves with a deprecation notice and **is
reported**; after it, resolution **is an error** naming `superseded_by` if
present.

> Pruning the ledger would free the identifier for reuse, since the ledger is the
> only thing that makes R-061 count a retired identifier as occupied. A reused
> identifier turns every cached reference into a confident lie (R-002) — the one
> outcome R-065 exists to prevent.

R-068, namespace renaming, lives in §13 (EXPERIMENTAL) — it has no meaning
outside federation.

**R-069** `cmd` · MUST — Splitting or merging pages tombstones the retired
identifiers with `superseded_by` listing the replacements. Resolution follows
R-067's window like any other tombstone; the notice — deprecation inside the
window, error after it — names all successors, so a human can choose rather
than a tool guessing one.

---

## 7. Links and references

### 7.1 Documentation to documentation

**R-070** `lint` · MUST — A doc-to-doc link MUST be a wiki-link carrying the full
path from the documentation root: `[[reference/token-ttl]]` or
`[[reference/token-ttl|alias]]`. Short-name links are invalid. A page that lives
at the root has no directory in its path, so `[[roadmap]]` for `roadmap.md` is
already the full path, not a short name — a bare name is a violation only when
no root-level page answers to it. A link addresses a page, never a heading: a
`#fragment` on the target is not resolved and **is reported** — the section is
named in the alias (`[[reference/token-ttl|Token TTL § Renewal]]`), where a
retitled heading costs a stale label, not a broken link.

> Known tension: R-062 declares filenames cosmetic, yet this rule makes the path
> a contract between documents. An identifier-based link form is deferred; see
> §19.

**R-071** `lint` · MUST — Every link target MUST resolve. Resolution appends
`.md` when the path has no extension, and does not follow symlinks outside the
tree. A target that moved to `_archive/` still resolves — to the archived
copy — and **is reported**: the live link is evidence of remaining interest, so
archiving never breaks a build; it surfaces the pages that still cared. A
target that exists nowhere is dangling, and a dangling target **is an error**:
a dangling wiki-link, a dangling `doc:` reference (R-076) and a dangling
foreign reference (under federation: R-139, §13) are one failure class — the
reference an agent cannot resolve and answers anyway — and silently wrong is
exactly R-151's criterion, so all carry the same severity. One
dangling reference yields **one finding** per referencing file and target
(R-158's identity), carrying every applicable rule number — never three
diagnostics for one break — and each finding names its referencing file, which
is what R-152 requires.

### 7.2 Code to documentation

**R-072** `lint` · MUST — Code MUST reference documentation by identifier, never
by path:

```
// doc: token-ttl
// doc: @svc-auth/token-ttl
```

**R-073** `lint` · MUST — A token containing `<`, `>`, `{` or `}` is a
metasyntactic placeholder, not a reference: prose that documents the citation form itself
(`doc: <id>`) writes one, and so does a code template that renders the citation
(`doc: {doc_id}`); reading either as an identifier makes documentation about the
convention fail the convention — this specification's own generated agent text
does exactly that. A consequence: this rule's counter-example cannot be
written literally — a page saying "do not write `doc: @real/name`" has written a
reference — so prose shows only the placeholder form, or puts the literal in a
fenced block or a `>` quotation, which are not scanned. Otherwise the token following `doc:` is read up to
the first whitespace, then stripped of trailing punctuation (`.` `,` `;` `:` `)` `]` `"`
`'` `?` `!` and the backtick) — a reference at the end of a sentence, or
written as inline code, is still a reference. A reference opened inside an
inline-code span ends at the closing backtick, and whatever the prose attaches
after it is not part of the identifier: an inflected language glues a case
suffix to the closing backtick, English glues a possessive, and reading those
into the token invents an identifier that was never written.

**R-076** `lint` · MUST — A local `doc: <local-id>` that matches no `id`, no
alias, and no `defines:` family **is an error** in the live layer, and **is
reported** in the historical one (the journal, its archive slices, `_archive/`):
a dated record cannot be corrected by editing it, and the legitimate repairs —
a tombstone for a renamed identifier (R-066), a page distilled at last — are
exactly what the report names. An error nobody may honestly clear is an error
people learn to bypass (R-150). In 0.1 local references were
not required to resolve, so a typo was invisible. Foreign references resolve
under federation (§13, R-139); in a tree with no federation state a foreign
reference **is reported** as unresolvable here, never silently accepted and
never a core error — the core cannot check what only a manifest knows.

**R-194** `lint` · MUST — A `doc:` reference resolves against **every** page
identifier in the tree, tracked work included — a work page's `id` is an `id`
(R-076). Because the flowing layer is temporary, the resolution carries a
severity of its own: a reference to a page whose `status` is `graduated` and
whose `graduated_to` names no resolvable destination **is an error** — that page
announced its permanent value moved elsewhere and is not loaded into agent
context (§5.2), so the citation is a dead end. When `graduated_to` does resolve,
the same citation **is reported**: the husk is a signpost, the reader is
mechanically redirected, and provenance ("this decision was carried out here")
is a legitimate reason to keep pointing at it; a reference to any other tracked-work page **is reported**,
naming the distillation that has not happened yet. A citation whose identifier belongs to an
archived page resolves and **is reported**: the page is a record, not a live
claim (R-059), and the same convention already governs an explicit `[[_archive/…]]`
link — a reader following it lands on real, dated content and knows what layer
it is. The journal is exempt from the error, active file and archive slices
alike: an entry is a dated statement
about what was true when it was written, the historical layer is never edited
retroactively (R-104), and demanding that a two-month-old entry cite a page that
did not exist yet asks history to be false. Field origin: the first real
adoption cited 33 work pages that exist, and reporting them as dangling made a
working tree look broken while the real debt — 54 identifiers with no page at
all — was buried in the same list.

**R-079** `lint` · MUST — A `doc:` reference resolved through a `defines:`
family MUST name a member that exists: the cited identifier occurs on the
defining page, as a heading or in its body — in full, or with the family prefix
removed, because a page that already declares the family commonly lists its
members in short form (`(utf8-text)` under a register of `ADR-*`) and demanding
the prefix twice makes a page fail to define what it plainly defines. A glob
match alone resolves nothing — otherwise `defines: adr-*` would make
`doc: adr-9999` resolve forever, and the short form does not weaken that: the
member still has to occur — so a family citation with no such member is
unresolved and falls under R-076. An occurrence that is itself a `doc:` citation
is not evidence: a reference cannot prove its own target, and without this the
defining page can cite any phantom member of its own family and have it read as
healthy — the exact failure withdrawn R-064 targeted. This demands no definition
syntax; it only refuses self-proof.

### 7.3 Documentation to code

**R-074** `agent` · MUST NOT — Documentation MUST NOT contain source paths as
references. Code moves; documentation stays. Where code must be shown, the
snippet is embedded with a comment explaining why it is there. This requires
judgment: a path inside a quoted stack trace or an embedded snippet is not a
reference. The `verifies` block (§11) is an audit binding, not a reference, and
is exempt.

**R-075** `lint` · MUST NOT — Absolute filesystem paths (`/home/...`, `C:\...`)
and **relative** links that traverse outside the tree root are errors, except
inside fenced code blocks and block quotes — the mechanically identifiable form
of the quoted material R-074 exempts. External URLs (`https://...`) are not
tree-escaping links and are permitted. Whether such quoted material belongs on the page at all remains
R-074's judgment.

An escaping link whose target **exists** is **reported** instead: documentation
that routes to a file living beside the code — a catalog README, a schema, a
build file — is a deliberate, working link, and the severity doctrine (§2.2)
blocks only what is irreversible or silently wrong. It stays reported because
the link breaks the day the docs tree moves, which is R-062's whole argument for
identifiers; a *broken* escaping link remains an error.

**R-112** WITHDRAWN — its content is the exemption note now attached to R-074.

### 7.4 Scan scope

**R-077** `lint` · MUST — A tree declares the scan scope in `.docmeta.yml`.
Defaults: every UTF-8 text file tracked by version control, excluding paths
ignored by version control and excluding `.federation/`. An implementation MUST
NOT restrict scanning to a hardcoded set of file extensions. A `scan_exclude`
entry is a path prefix from the scan root, matched on a path-component boundary
(`spec`, `tooling/`, `./spec/**` all name the directory `spec`); an entry the
prefix form cannot express — glob syntax, a parent-directory step — **is
reported**: an exclusion that silently excludes nothing is worse than one
rejected, because the findings it was meant to remove keep arriving unexplained.

> Rationale: a reference in a Helm chart or a build file is as real as one in
> source code. An implementation scanning only its own language's files silently
> under-reports `consumes`, and the provider then deletes a contract that is in
> use.

**R-078** WITHDRAWN — absorbed by R-011, which already requires every check to
report its inspected count and owns the zero-count semantics.

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

**R-080** `lint` · MUST — `status` MUST be one of the five values above. A file is
created as `draft` or `active`; any other initial value **is reported**.
Transitions follow this table, and a transition not listed **is reported**:

| From | Allowed to |
|---|---|
| `draft` | `active`, `abandoned` |
| `active` | `done`, `abandoned` |
| `done` | `graduated`, `active` (reopened) |
| `abandoned` | `graduated`, `active` (resumed) |
| `graduated` | — (terminal) |

> In 0.2 only the exit from `graduated` was constrained, so a file could be born
> `graduated` or jump `draft → graduated`, skipping the human confirmation R-081
> exists for. Status is reversible and visible, so these are reported rather than
> blocked (R-151).

**R-081** `agent` · MUST — `done` and the file-level transition to `graduated`
are set only on explicit human confirmation, recorded as `confirmed:` in the
file's frontmatter (§5.2). Passing tests or a green build means `active`. The
lint half: a file at `done` or `graduated` without `confirmed:` **is reported**
— the record is what lets a later audit distinguish a confirmed transition from
an agent's guess, the same reason R-028 exists for verification.

**R-082** `lint` · MUST — `graduated` is terminal, and a graduated file receives
no further **content change** (§2.4). Where version-control history is available,
a transition out of `graduated` or a content change in a graduated file **is
reported** — otherwise knowledge added there stays permanently outside agent
context.

**R-083** `cmd` · MUST — A command MUST refuse a transition to `abandoned`
without a reason. R-055 checks the resulting file; this rule prevents the
transition from happening without one.

**R-084** WITHDRAWN — absorbed by R-093, whose table already routes `abandoned`
work to `explanation/` as a rejected alternative and whose archive gate decides
the tried-versus-unnecessary split.

**R-085** `lint` · MUST — A file whose last content change in version-control
history is older than `stale_active_days` (default 90) while its status is
`draft`, `active` or `done` **is an error**. Undeclared abandonment is the
common case, not the rare one, and a flowing layer nobody closes is where
knowledge goes to die; the fix is one field — `abandoned` with its reason — or
the graduation the file was waiting for (D-070).

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

When the heading is a required template section (R-048), the addressed range
**begins after the heading line**: the heading stays in the source so the file
keeps satisfying its template, the section body is what moves, and the body is
replaced by the destination link (R-091). The destination's own heading is
written during page preparation (R-099), where model authorship is already
licensed.

Blocks are addressed by byte offsets **in the source file as it exists on disk**,
not in any normalized form. The canonical form (R-113) exists only for hashing;
using it for addressing would shift every offset in a file with CRLF endings or
decomposed Unicode and make R-090's byte-exact copy select the wrong range.

### 9.2 Movement

**R-090** `cmd` · MUST — In the `project` profile, graduation MUST move content,
never rewrite it. A conformant implementation copies whole blocks byte-for-byte;
the model selects which block goes where and never retypes the text. In the
`knowledge-base` profile graduation is distillation, not movement (R-092): the
wiki page is new text tracked to its evidence through `sources:`, no block is
moved, and byte-exact copying does not apply.

> This converts a rule that depended on model discipline into a mechanical
> guarantee.

**R-091** `cmd` · MUST — After graduation the source file MUST retain a link to
the destination and MUST record `graduated_to`. `graduated_to` may be present on
a file whose `status` is still `active`: graduating one block does not end the
work. The file-level transition to `status: graduated` happens only when nothing
of permanent value remains, and like `done` it requires explicit human
confirmation (R-081).

In the `knowledge-base` profile the source is content-immutable (R-023), so the
link runs the other way: the destination page records the source in `sources:`
and the raw file is left untouched.

**R-099** `cmd` · MUST — Creating a new destination page is a two-step operation.
First the page is prepared: `id`, `type`, `updated` and the context-establishing
opening (R-032) are written — the frontmatter by the tool, the opening sentence
by the model. Then blocks are moved into it byte-exactly (R-090). Without this
split, a page created purely by moving bytes can never satisfy R-050 and R-032,
and graduation into a page that does not yet exist would be impossible.

**R-092** `agent` · MUST — Order matters. A block whose content already exists on
a permanent page is removed from the source and replaced by a link. A block that
exists nowhere permanent is written to its destination *before* the source is
shrunk. Deciding whether two blocks say the same thing is judgment, not a
comparison of bytes.

In the `knowledge-base` profile nothing is removed from the source: the raw note
is evidence, not a draft. A `wiki/` page is not a copy of it but a distillation —
new text, written by a human or a model, tracked back to its evidence through
`sources:`. R-002 is satisfied because the wiki page is the single home of the
distilled fact; the raw note is the record of where it came from.

**R-093** `agent` · MUST — Before archiving anything, the question is asked: does
this information exist anywhere else? If not, and it is still true, it graduates
first.

| Source | Usual destination |
|---|---|
| `features/` | `reference/` (contract), `explanation/` (decision) |
| `postmortems/` | `reference/` (invariant), `howto/` (runbook), `explanation/` (systemic cause, negative results) |
| `research/` | `explanation/` |
| `abandoned` | `explanation/` (rejected alternative) |

**R-097** `cmd` · MUST — Every command that writes more than one file MUST be
atomic: either all writes land or none do. A command MUST refuse to start on a
dirty working tree unless explicitly forced, and **on completion** MUST leave no
state in which two authoritative copies of the same content both pass lint — the
prepared-but-not-yet-shrunk state *during* a graduation (R-092, R-099) is the
in-flight state atomicity governs, not a violation of this clause. List-file
appends (a journal line, a `questions.md` item) are single-file writes and are
exempt from the dirty-tree refusal: they are the capture path (R-004), and an
escape hatch that needs `--force` is not an escape hatch.

### 9.3 Compilation to executable form

The graduation chain has one more link: flowing → permanent → executable.

**R-094** `cmd` · MAY — A `howto` page whose steps have stabilized MAY be
compiled into an executable agent skill.

**R-095** `cmd` · MUST — A compiled skill MUST record the source identifier and
the source content hash (R-113). When the source page changes, or no longer
exists, the compiled skill **is an error** until it is compiled again from the
page as it now reads (D-070, D-073): a skill that runs steps its page has
since corrected is silently wrong on every invocation.

**R-096** `agent` · MUST NOT — Compilation MUST NOT invent steps. A procedure not
fully written on the page is not ready to be compiled.

---

## 10. Journal

**R-100** `lint` · MUST — A journal entry opens with `## YYYY-MM-DD` followed by a
separator and a title. The separator is `-`, `--` or an em dash; requiring a
character most keyboards cannot produce would put R-120's ASCII principle and
this rule in conflict for no benefit. Between the date and the separator a tree
MAY carry one bracketed annotation — `(448)`, `[ops]` — because entry counters
and channel tags are common field conventions that leave the date first and
machine-readable, which is all this rule protects.

**R-101** `lint` · SHOULD — An entry is **at most 5 source lines**: what
triggered it, what changed, which gate passed, which page holds the permanent
content. An entry exceeding the budget **is reported** — the check matches the
rule exactly, because a checker looser than its rule text is what R-012 forbids,
and the 3,800-line journal in R-102's rationale was reachable one tolerated
entry at a time. There is no lower bound: a one-line entry is a fine entry.
Lines are counted in the source file, not as rendered.

The budget is 5 lines unless `.docmeta.yml` declares `journal_entry_max_lines`,
which a tree MAY raise to state a discipline it actually keeps. Making it
configurable is not a loophole: the number stays in one visible place instead of
in an agent's memory, a tree that never declares it keeps the strict default,
and a tool that warns hundreds of times against a documented house rule teaches
people to ignore warnings — the failure R-150 exists to prevent.

**R-102** `agent` · MUST NOT — Measurements, tables, algorithms, API lists,
register maps, and rejected alternatives MUST NOT be written to the journal. They
belong to the permanent layer; the journal links to them. Recognizing them
requires reading the prose, so this is not a static check.

> Rationale: a journal reached 3,800 lines in nine days and became 31% of all
> documentation. One constant appeared in eight places and none were ever
> updated. Permanent knowledge kept in a chronology silently becomes a lie.

**R-103** `cmd` · MUST — When the active journal exceeds 500 lines, the oldest
whole days are moved to an archive slice named
`work/journal/<first-date>--<last-date>.md` — `<first-date>` is the earlier
calendar date, whatever order the entries appear in — which gets one router
line. Slices
are cut on day boundaries; a single day larger than the limit becomes its own
slice. A slice is a list file (R-041) and carries no `status`: it is a
chronology archive, not a unit of work that graduated anywhere.

**R-105** `lint` · MUST — An active `journal.md` whose length exceeds the limit
in R-103 **is reported**, naming the rotation that resolves it. This is the
R-018 backstop for R-103: appending in an editor bypasses rotation, and the
oversized file is the trace it leaves.

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

**R-111** `lint` · MUST — Staleness **is an error** on every run until resolved,
naming the page and the region that moved. A pin is a promise the author made
about a region of code; once the region moves the page is silently wrong until
someone re-reads it, which is R-151's criterion exactly. The cost is bounded:
the fix is one re-read followed by `docsys pin --refresh <page>`, or dropping a
pin that was never worth keeping (D-070). This is the only mechanism in this
specification that detects code-documentation drift mechanically.

**R-113** `lint` · MUST — A **content hash** is `sha256` over the canonical form
of the content, written as `sha256:` followed by lowercase hex. The canonical
form is: UTF-8, NFC-normalized, LF line endings, trailing whitespace removed from
each line, exactly one trailing LF. Without a fixed definition two
implementations disagree about staleness for the same tree. An implementation
that cannot normalize registers the gap and hashes the bytes as written
(D-068).

A **page's** content hash covers its **body** — everything after the closing
frontmatter delimiter; a file without frontmatter hashes whole. Frontmatter is
identity and bookkeeping, and a bookkeeping change (an `updated` bump) must not
churn manifests, compiled skills and materializations estate-wide. Frontmatter
is not left unguarded: for materialized pages it is reconstructed from the
manifest and checked by R-137; `verifies` hashes cover the referenced code
region, unchanged.

**R-114** `lint` · MUST — When `symbol` is absent, the hash covers the whole file
at `path`. When `symbol` is present, the implementation MUST declare how it
resolves symbols for that language, and MUST report an error rather than guess
when a symbol is ambiguous or unresolvable. The reference implementation's
resolution is registered as D-069.

---

## 12. Language

**R-120** `lint` · MUST — Structural tokens MUST be ASCII: directory names, file
names, frontmatter field names, `id` values, `status` values, link syntax, and
tracked-work template section headings (R-048). This is checked as a
character-class and enumeration constraint, not as a language determination.
(Structural tokens should also *read* as English — but no check can determine
the language of `masa`, so that stays guidance inside this note, not a rule.)

**R-125** WITHDRAWN — folded into R-120 as the note above; an unverifiable
SHOULD was consuming a rule number and a coverage slot.

**R-121** `agent` · MAY — Content language is free and declared as
`default_content_language` in `.docmeta.yml`. A page may override it with
`lang:`.

**R-122** `agent` · MUST — When editing an existing page, its language is
preserved. A page has one prose language; the terms R-123 protects retain their
original form and do not count as mixing.

**R-123** `agent` · MUST NOT — Code identifiers, protocol names, library and
product names, and quotations are never translated. This list is the single
definition; R-122 and P/R-123 cite it rather than restating it — two lists
drifted apart once already.

**R-124** WITHDRAWN — subsumed by R-172, which forbids migrations from altering
prose at all.

---

## 13. Federation (EXPERIMENTAL)

Federation lets independent repositories reference each other's documentation
without sharing a repository, a database, or a server.

> **Why experimental.** Six audit rounds produced findings here faster than
> fixes closed them, and the unresolved holes — manifest serialization, the
> namespace-to-location bootstrap, `consumes` representation, `.federation/`
> version-control status — are exactly the kind that only interoperating
> implementations settle. This section stabilizes against a reference
> implementation and a second real estate, not against further audit. Until
> then its rules bind nothing; a core tree that never declares `namespace` is
> untouched by everything below.

### 13.1 Declaration

**R-130** `lint` · MUST — A tree participating in federation MUST declare
`namespace` in `.docmeta.yml`, together with its `federation_role` (R-144) and,
when publishing, its transport locations (R-145). These are the authored
federation fields. What is never authored is the *dependency data* —
`provides` and `consumes` (R-131).

**R-131** `cmd` · MUST — The manifest fields `provides` and `consumes` MUST be
derived, never authored. `provides` comes from two sources: pages currently
carrying `id` outside `_archive/` (R-047), `.federation/`, `_unsorted/` and
`_templates/`, **and the tombstone ledger** (R-066) for identifiers that no
longer have a live page — excluding `internal: true` pages and ledger entries
marked internal (R-135). A materialized foreign page or an unclassified draft
must never be re-exported as this namespace's own contract. `consumes` comes
from `doc: @ns/id` references found within the scan scope (R-077) minus
`_archive/` — a dead reference in an archived record must not keep a foreign
contract alive — including references on `internal: true` pages:
the page is unexported, but its dependency is real, and hiding it lets a
provider retire a contract still in use. What `internal: true` removes is the
page's own identifier from `provides` (R-135), not its outgoing references.
Deriving `provides` from live pages alone would drop every tombstone on
the next export and break the deprecation window the moment it was needed.

> A hand-maintained dependency list goes stale by construction (R-002). The
> authored, page-level declaration of an identifier family is a different thing
> and is called `defines:` (R-063).

**R-132** `lint` · MAY — In a monorepo, a namespace is defined per directory by
placing a `.docmeta.yml` at that directory's documentation root. Namespace roots
MUST NOT nest. All federation rules apply unchanged.

### 13.2 Export

**R-133** `cmd` · MUST — `export` produces a manifest containing the manifest
format version, the namespace, the spec version, the namespace's own aliases
(R-068), the derived `consumes` list (R-131 — without it R-140's consumer
report and R-141's index have nothing to read), and for each identifier: type,
title, summary, content hash, owner,
state (`active` or `withdrawn`), and any `aliases` (R-065). A **withdrawn** entry
carries only identifier, state, withdrawal date and optional `superseded_by` —
the page that supplied type, title, summary, hash and owner no longer exists, so
requiring them would make a conformant manifest impossible after any deletion.
Pages excluded by R-135 appear in neither list. Titles and summaries follow
R-057. Without the alias and tombstone entries the lifecycle rules of §6.2 cannot
cross a repository boundary.

**R-134** `cmd` · MUST NOT — The manifest MUST NOT contain page **bodies**. Prose
travels through the content channel (§13.3). Title and the one-sentence summary
are identity metadata, not body content, and are the deliberate exception —
without them a consumer cannot tell what an identifier is before fetching it.

**R-135** `lint` · MUST — A page marked `internal: true` is excluded from the
manifest: its identifier appears in neither the active nor the withdrawn list,
and when the page is deleted its ledger tombstone (R-066) carries the internal
marking so the exclusion survives the page. Exclusion covers the page's own
identifier, not its outgoing references — those still count toward `consumes`
(R-131).

**R-167** `cmd` · MUST — Marking a previously **exported** page `internal: true`
in place MUST be refused. The identifier is public; taking it private is a
withdrawal, and the conformant path is the withdrawal path: retire the
identifier (R-066, with the R-140 consumer report), and give the internal
content a new identifier. The R-018 backstop: where version-control history is
available, a page that gained `internal: true` **after appearing in a published
manifest** is reported — a page that was never exported may flip freely, and
reporting it would only teach teams to ignore the check.

**R-019** `advisory` · MUST — A publisher MUST ensure its content channel does
not serve `internal: true` pages. No local check can observe what a remote
serves, and R-145 permits a plain git remote as a channel — which necessarily
serves every tracked file. A tree with internal pages and a whole-repository
content channel is therefore nonconformant even though lint cannot see it.
Confidentiality is a property of the channel, not of a frontmatter field.

### 13.3 Transport

The manifest says what exists. The content channel delivers the bytes. In 0.1
only the manifest was specified, which left the mandatory consumption flow with
no source of content.

**R-145** `lint` · MUST — A publishing namespace MUST declare, in
`.docmeta.yml`, where consumers obtain its manifest and its page content. The
mechanism is not fixed by this specification: a URL, a package artifact, a git
remote and revision, or a filesystem path are all conformant. What is fixed is
that the location is declared and resolvable without human intervention.

**R-146** WITHDRAWN — entailed by R-136 and R-147: a materialization must hash
to the manifest value, and a mismatch is never materialized — the verification
this rule restated.

**R-147** `cmd` · MUST NOT — Content whose hash does not match the manifest MUST
NOT be materialized. A mismatch is reported as a failure, never silently
accepted.

**R-148** `cmd` · MUST — When a provider is unreachable, the last verified
materialization is retained and its age is reported. A transient outage MUST NOT
delete content a consumer already had. That retained state is what existence
checks read (R-139).

**R-149** `lint` · MUST — Every materialized page under `.federation/` MUST have
provenance recording source namespace, identifier, content hash, and fetch time.
Provenance lives in a **sidecar** at `.federation/<namespace>/<id>.provenance.yml`,
never inside the page: writing it into the page would change the bytes and break
the hash that R-136 pins and R-137 protects. The path is fixed because it is
a cross-implementation surface — one tool's materialization must be readable by
another's checker. Sidecars are provenance records, not materialized pages, and
do not themselves require provenance. A materialized page without a sidecar is an
error, so hand-placed content cannot masquerade as federated content.

> A sidecar can be forged along with its page. Provenance proves what a file
> claims to be, not that the claim is true; authenticating the manifest itself is
> listed in §19 as an open problem.

### 13.4 Consumption

**R-136** `cmd` · MUST — A consumed page is materialized at
`.federation/<namespace>/<id>.md`: its **body** is the provider's page body,
whose canonical form (R-113) hashes to the manifest value; its **frontmatter is
reconstructed from the manifest fields** (R-133) — it is a derived artifact, not
the provider's bytes, so nothing in it escapes verification. Provenance lives in
the sidecar (R-149). Identity is defined by the canonical body hash, not raw
bytes: a checkout that converts line endings is still conformant. The
namespace-scoped path prevents two providers exporting the same `local-id` from
colliding.

**R-137** `lint` · MUST NOT — Files under `.federation/` MUST NOT be edited
locally. The check verifies both halves of a materialized page: a body whose
canonical hash no longer matches the provenance record, or frontmatter that no
longer equals the manifest-derived reconstruction (R-136), **is an error**.

**R-138** WITHDRAWN — the exemption is stated normatively in R-002 itself.

### 13.5 Enforcement

**R-139** `ci` · MUST — A reference to a nonexistent foreign identifier MUST fail
the consuming repository's pipeline. Existence is evaluated against the newest
*verified* federation state held locally — the last materialized manifest
(R-148) — never against a live provider query: an identifier absent from that
state does not exist, and one present in it exists even while its provider is
unreachable, with the state's age reported. This is an instance of the R-151
exception: an agent that cannot resolve a reference invents an answer, so the
failure is silent and harmful.

**R-140** `ci` · MUST — Removing or tombstoning a provided identifier MUST report
the consuming namespaces *known to the local federation state* before the change
lands. Where that state is incomplete, the report MUST say so rather than imply
that no consumers exist.

**R-141** `ci` · MAY — Complete consumer coverage requires an index built from
every participating namespace's manifest. That index is an optional component. An
estate without one operates with the partial knowledge described in R-140.

**R-142** WITHDRAWN — its content is fully stated by R-140 and R-141.

**R-143** `cmd` · SHOULD — When a consumed page's hash changes, the consuming
repository SHOULD receive an automated change proposal showing the old and new
content side by side. The old content is the previous verified materialization;
the new content comes through the content channel.

### 13.6 Namespace lifecycle

**R-068** `cmd` · MUST — Renaming a namespace follows R-067's shape at namespace
level: the previous namespace is retained as an alias in the manifest
**permanently**, and the deprecation window governs how foreign references to it
resolve, not how long the alias lives — inside the window they resolve with a
deprecation notice and **are reported**; after it, resolution **is an error**
naming the current namespace. Dropping the alias would free the name for another
estate and turn every cached `@old-ns/...` reference into a lie — the
namespace-level twin of the reuse R-067 prevents.

### 13.7 Asymmetric membership

**R-144** `lint` · MAY — A namespace MAY declare `federation_role: consume-only`.
It reads manifests but publishes none, and no other namespace may reference it.

> A private or machine-local tree must be `consume-only`. A reference to it
> resolves on its owner's machine and is dead everywhere else, which is exactly
> the silent failure R-151 exists to prevent.

### 13.8 Manifest versioning

**R-180** `cmd` · MUST — The federation manifest format carries its own version,
independent of this specification, and changes more slowly. Repositories are not
upgraded on the same day; a manifest format that moved with every specification
release would break federation continuously.

**R-181** `cmd` · MUST — Within a manifest major version it implements, an
implementation MUST read every older minor version, and MUST ignore unknown
fields in newer minor versions. Backward reading alone would force the
estate-wide lockstep upgrade that R-180 exists to prevent.

**R-182** `cmd` · MUST — A manifest **major** version an implementation does not
implement causes a refusal to consume that namespace, reported by name, not a
silent skip. R-181 governs minors within an implemented major; this rule governs
majors.

---

## 14. Automation levels

**R-150** WITHDRAWN — absorbed into R-151, whose escalation criterion entails
warn-by-default; the friction rationale now lives there.

**R-151** `advisory` · MUST — A check escalates to blocking only when the outcome
is **irreversible** or **silently wrong**. Everything else warns: hard blocking
elsewhere creates friction, and friction is resolved by disabling the check
entirely — which removes the protection completely. A rule blocks by using a
blocking word from §2.2, and R-015 requires the resulting choice to be visible.

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

### 14.1 Repair

Detection is defined above; repair has its own constraints. The characteristic
failure of automated repair is not a wrong fix but a loop: two tools — or a tool
and an agent — correcting each other forever.

**R-156** `cmd` · MUST NOT — Automatic repair MUST NOT touch authored prose. A
finding is repairable only when exactly one correct outcome is derivable from
the tree itself: a missing router line (format and position fixed by R-035; the
sentence is the R-057 summary — derived, not authored), a lagging `updated`
field (per the §2.4 definition), a link target moved **by the same command
invocation**, a provenance refresh. Everything else is a decision, and
decisions go to a human or a model (R-003).

**R-157** `cmd` · MUST — Repair MUST be idempotent: running the same repair
twice produces zero changes the second time. A repair that oscillates against
another tool's output fails this rule by construction.

**R-158** `cmd` · MUST — A finding's identity is the triple (rule, file,
subject), where *subject* is the key each rule's finding format declares — the
target of a reference, the field of a frontmatter check, the block of a
structure check — registered in the conformance corpus (R-193). Dangling
references are one finding class with the identity (R-071, referencing file,
target); R-076 and R-139 detections map onto it, the finding lists every
applicable rule, and the strongest applicable severity governs.

A repair pass MAY expose new findings — a repaired router line brings pages
into checks that could not see them before. What a pass MUST NOT do is
**reopen** a finding it previously closed: reopening is the oscillation signal,
and the loop stops and reports. The loop ends when a pass performs zero
repairs. Termination follows from two facts: findings are finite, and repair is
idempotent (R-157), so each (identity, repair) pair occurs at most once.

### 14.2 Decision procedures — the agent-facing rule format

R-155 says agent rule text is generated; this section fixes its shape. A
judgment cannot be made mathematically sharp — if it could, it would be a lint
rule (R-003). What it can be made is **decidable**: bounded input, a closed set
of outcomes, a default for indecision, and an escape that is always legitimate.
That combination is what lets a weak model fail *safely* instead of guessing.

```
EVIDENCE : what to read before answering — a bounded list, never "the repo"
QUESTION : one decidable question about observable evidence, not about intent
OPTIONS  : a closed, ordered set of outcomes — first match wins,
           and every outcome names its next action
DEFAULT  : the outcome chosen when the question cannot be answered
ESCAPE   : where the case goes when no option fits — always legitimate
VERIFY   : the mechanical check or independent review that catches a wrong answer
NEVER    : what is forbidden regardless of the answer
```

**R-163** `cmd` · MUST — Generated agent rule text (R-155) MUST present every
`agent`-tagged rule as a decision procedure, and the procedures are **authored
normative content — §14.3 of this specification**, rendered by the generator. A
generator MUST NOT invent OPTIONS, DEFAULT, ESCAPE or VERIFY absent from §14.3;
the R-018 backstop is a lint comparing emitted procedures against the authored
set. VERIFY MAY name the independent audit as the reviewer, but MUST name the
specific audit check.

**R-164** `advisory` · MUST — OPTIONS is closed and ordered; the first option
whose test passes wins, so overlapping options cannot produce two answers.
DEFAULT MUST be one of the OPTIONS **or the ESCAPE** — and where a wrong pick is
costlier than a delay, the escape *is* the right default. DEFAULT and ESCAPE
MUST NOT prescribe different actions for the same condition: one condition, one
answer. The escape is never a failure: a model that cannot decide routes to the
escape instead of forcing the nearest option — an honest "I don't know" stays
cheaper than a confident guess.

**R-165** `lint` · MUST — The generated always-loaded text MUST stay within
`agent_rules_max_lines` (default 200). The budget has a **floor**: the rendered
size of the authored mandatory set (§14.3). A configured value below the floor
**is an error**, stated here because R-161 covers unknown keys, not known keys
with impossible values — and a budget that silently drops mandatory procedures
is R-151's "silently wrong". The shipped default MUST be at or above the floor.
Past the budget, models degrade across *all* instructions uniformly — every
added line taxes every existing rule — so the ceiling is a hard gate, not a
style preference.

### 14.3 The authored procedures

This is the normative content R-163 renders — one procedure per `agent`-tagged
rule, indented so none parses as a rule declaration. Procedures name the
`project` profile's surfaces; in the `knowledge-base` profile, read
`questions.md` and "the journal" as a note in `raw/inbox/` — the profile's
capture surface — and read the type directories as `wiki/<domain>/<type>/`.

    P/R-031 — choose the type of a permanent page
    EVIDENCE : the content being placed; the four-question table (§4.1)
    QUESTION : which one question is the reader asking of this content?
    OPTIONS  : look up a value or format       → reference/
               follow steps to a goal          → howto/
               understand why it is this way   → explanation/
               learn from zero, guided         → tutorial/
    DEFAULT  : the escape — a default type would make explanation/ the
               dumping ground and silence the R-046 signal
    ESCAPE   : answers more than one → split (R-031); answers none, or cannot
               be split cleanly → _unsorted/ + a questions.md item
    VERIFY   : audit type-mixing review (reads each page against the table)
    NEVER    : force content into the nearest type

    P/R-025 — verify a knowledge-base page
    EVIDENCE : the page; its sources:; who authored it (history)
    QUESTION : did this session produce any of the page's content?
    OPTIONS  : yes → do not verify — leave for another session
               no  → check every claim against sources:, then set verified
    DEFAULT  : do not verify
    ESCAPE   : authorship unclear → do not verify + questions.md item
    VERIFY   : R-028 record; audit checks verifier differs from author
    NEVER    : verify your own output

    P/R-032 — open a page so it stands alone
    EVIDENCE : the page's first two sentences
    QUESTION : could a reader with no context tell what this is and when to
               read it?
    OPTIONS  : yes → done · no → write the two-sentence opening
    DEFAULT  : write the opening
    ESCAPE   : the page's purpose cannot be stated → it likely mixes types;
               run P/R-031 first
    VERIFY   : audit reads openings
    NEVER    : open with content that assumes the previous page was read

    P/R-033 — decide whether documentation repeats the code
    EVIDENCE : the content; the code region it describes
    QUESTION : does the code itself state this (signature, parameter list)?
               A measured value or an operating limit is NOT stated by code —
               reference/ exists precisely for those (R-031's "which value")
    OPTIONS  : yes → do not write it; the code owns it
               no  → write it; it is what the code cannot say
    DEFAULT  : do not write it — a duplicate becomes a trusted lie when the
               code changes (R-002); an omission is only an omission
    ESCAPE   : cannot tell who owns it → do not write + questions.md item
    VERIFY   : audit duplicate-source-of-truth check
    NEVER    : copy a signature or a parameter list

    P/R-045 — remove or archive
    EVIDENCE : the content; whether it is authored or derived
    QUESTION : is this authored content?
    OPTIONS  : authored → move to _archive/ · derived → regenerate freely
    DEFAULT  : treat as authored
    ESCAPE   : secret or legally required removal → delete, and record that a
               removal happened (never the content) in the journal
    VERIFY   : R-107 ledgerless-disappearance check; history review
    NEVER    : delete authored content for tidiness

    P/R-046 — revisit _unsorted/
    EVIDENCE : the item; the four-question table
    QUESTION : can it now be classified into exactly one type?
    OPTIONS  : yes → classify, frontmatter, router line · no → leave it
    DEFAULT  : leave it
    ESCAPE   : same as leaving — _unsorted/ is the escape
    VERIFY   : audit reports _unsorted/ age
    NEVER    : force it because it has waited long enough

    P/R-074 — a path appears in documentation
    EVIDENCE : the surrounding text; is it inside a fence or quotation? A
               path inside a `verifies:` block is an audit binding (§11),
               exempt by R-074's own text — not this procedure's business
    QUESTION : is the path a pointer the reader should follow, or quoted
               material (trace, log, example)?
    OPTIONS  : pointer → replace with an embedded snippet + why-comment
               quoted  → keep it inside its fence or quotation
    DEFAULT  : the escape — replacing quoted evidence destroys it, and
               fencing a live pointer merely hides it from lint
    ESCAPE   : unsure → leave the text untouched + questions.md item naming
               the file and line
    VERIFY   : R-075 lint on paths outside fences
    NEVER    : rewrite quoted evidence — a trace is a record, not prose

    P/R-081 — set done, or graduate the file
    EVIDENCE : the human's words in this session; which transition is asked
    QUESTION : did a human explicitly confirm THIS transition, in words?
    OPTIONS  : yes, done       → done + `confirmed: <who>, <date>` (§5.2)
               yes, graduated  → graduated + `confirmed:` updated — done was
               one confirmation, leaving context forever is another (R-091)
               no → status unchanged
    DEFAULT  : active
    ESCAPE   : ambiguous ("looks fine"?) → ask once; no answer → active
    VERIFY   : R-080 transition table; audit reads `confirmed:` records
    NEVER    : infer done from a green build or passing tests

    P/R-092 — route a block during graduation
    EVIDENCE : the block; the permanent layer searched by topic; R-049
    QUESTION : does this content already exist on a permanent page?
    OPTIONS  : yes → replace the block with a link to it
               no  → prepare the destination (R-099), then move (R-090)
    DEFAULT  : treat as new — a duplicate is caught by audit, a loss is not
    ESCAPE   : cannot tell whether two blocks say the same → move nothing,
               leave the block where it is + questions.md item; the work file
               stays active until the question closes (one home, R-002)
    VERIFY   : audit duplicate check; R-056 resolution of graduated_to
    NEVER    : shrink the source before the destination exists. In the
               knowledge-base profile nothing is ever moved or shrunk — the
               wiki page is new text and raw/ stays untouched (R-092)

    P/R-093 — archive gate
    EVIDENCE : the file; the permanent layer; the R-049 dispositions
    QUESTION : does any still-true information here exist nowhere else?
    OPTIONS  : yes → graduate it first, per R-049 · no → archive
    DEFAULT  : graduate first
    ESCAPE   : cannot judge whether it is still true → questions.md item, and
               do not archive yet
    VERIFY   : R-047 tombstone check; audit
    NEVER    : archive unique knowledge

    P/R-096 — compile a howto into a skill
    EVIDENCE : the howto page, start to finish
    QUESTION : is every step written, with no gap you would fill from memory?
    OPTIONS  : yes → compile · no → report the gaps, do not compile
    DEFAULT  : do not compile
    ESCAPE   : unsure whether a step is complete → do not compile
    VERIFY   : R-095 source-hash pin; execution failures surface gaps
    NEVER    : fill a gap from your own knowledge

    P/R-102 — write a journal entry
    EVIDENCE : the draft entry
    QUESTION : does it contain a measurement, table, API list, register map,
               algorithm, or rejected alternative? (R-102's list, complete)
    OPTIONS  : yes → that content goes to its permanent page; the journal
               keeps 2–5 lines + a wiki-link to it (R-070 — `doc:` is the
               code-side form, R-072)
               no  → write the entry
    DEFAULT  : move the content out
    ESCAPE   : no permanent home exists yet → it stays in the work file, and
               the journal links there
    VERIFY   : R-101 length report; audit duplicate check
    NEVER    : park permanent knowledge in the journal "for now"

    P/R-121 — choose a page's language
    EVIDENCE : .docmeta.yml default_content_language; the page's lang:
    QUESTION : is this a new page?
    OPTIONS  : new → default_content_language, or its explicit lang:
               existing → the page's current language (P/R-122)
    DEFAULT  : default_content_language
    ESCAPE   : an existing page is already mixed → pick its dominant
               language + questions.md item; never "fix" wholesale
    VERIFY   : audit language spot-check
    NEVER    : switch an existing page's language

    P/R-122 — edit an existing page
    EVIDENCE : the page's prose
    QUESTION : what language is it written in?
    OPTIONS  : that language → continue in it
    DEFAULT  : the page's existing language
    ESCAPE   : genuinely mixed → continue in the dominant language, and the
               questions.md item flags the page for a deliberate one-language
               pass — the mixed state is a debt, not a norm; fixing it
               wholesale mid-edit is a human decision, not a side effect
    VERIFY   : audit language spot-check
    NEVER    : mix languages within one page

    P/R-123 — a term that might be translated
    EVIDENCE : the term, against R-123's list: code identifier, protocol
               name, library or product name, quotation
    QUESTION : is it on R-123's list?
    OPTIONS  : yes → keep the original form · no → translate freely
    DEFAULT  : keep the original form
    ESCAPE   : unsure whether it is a proper name → keep the original
    VERIFY   : audit language spot-check
    NEVER    : translate an identifier or a quotation

---

## 15. `.docmeta.yml`

```yaml
spec: docsys/0.4                 # required
profile: project                 # required — project | knowledge-base
default_content_language: en     # required
created: 2026-08-15              # optional

# --- federation block: EXPERIMENTAL (§13) — omit it entirely in a core tree ---
namespace: svc-auth              # required when federation is enabled
federation_role: publish         # publish | consume-only  (R-144)
manifest_url: "…"                # required when publishing (R-145)
content_url: "…"                 # required when publishing (R-145)
# ---------------------------------------------------------------------------

work_categories: []              # additional tracked-work categories (R-042)
epics: []                        # declared epic labels (R-058)
domains: []                      # knowledge-base profile only (R-026)

scan_exclude: []                 # added to version-control ignores (R-077)
generated_preamble: []           # verbatim line(s) every generated file opens with (D-056)
postmortem_threshold: "4h"       # R-087
stale_active_days: 90            # R-085
deprecation_window: 180          # days, R-067
lock_timeout: "4h"               # R-154
agent_rules_max_lines: 200       # R-165
```

**R-160** `lint` · MUST — `.docmeta.yml` MUST exist at the documentation root and
MUST declare `spec`, `profile`, and `default_content_language`. `created` was
required in 0.2 but read by no rule, which R-162 forbids; it is now optional.

**R-161** `lint` · MUST — An unknown key MUST be **reported but not rejected**.
This keeps a newer tree usable by an older tool (R-170) while making a misspelled
key visible: silently ignoring `scan_exlude` would disable a check nobody
notices. Report-not-reject is the only behavior that serves both.

**R-162** `lint` · MUST NOT — `.docmeta.yml` MUST NOT contain fields that affect
no rule in this specification; implementations store their own configuration
elsewhere. In 0.1 three such fields (`phase`, `type`, `mode`) implied behavior no
conformant tool provided.

---

## 16. Versioning and migration

A tree created under one version of this specification must be able to move to
the next without hand editing.

### 16.1 Compatibility

**R-170** `cmd` · MUST — An implementation MUST refuse to operate on a tree whose
major version it does not implement, rather than degrade silently. A minor
version difference MUST NOT block operation.

> While the major version is 0, a minor release may tighten rules in ways that
> make a previously conformant tree report violations. Every such change ships
> with a migration (§16.2), and a tool encountering an unmigrated older tree
> reports the pending migration (R-171) instead of treating the tree as broken.
> After 1.0 this exception ends.

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

Moved to §13.8 (EXPERIMENTAL): the manifest is a federation surface, and its
versioning rules stabilize with the rest of federation.

---

## 17. Conformance testing

**R-190** `ci` · MUST — An implementation MUST ship a conformance corpus: trees
exercising each `lint`, `ci`, and `cmd` rule, with expected outputs.

**R-191** `ci` · MUST — The corpus MUST include at least one tree that *violates*
each rule it covers under R-010. A rule whose level is `MAY` and whose optional
capability the implementation does not offer is out of scope for both rules — it
cannot be violated by declining to implement it.

**R-192** WITHDRAWN — adds no obligation beyond R-190 and R-191.

**R-193** `advisory` · MUST — Where this specification leaves a decision to the
implementation, the implementation MUST record its decision in the corpus. The
corpus is therefore not only a test suite but the register of choices this
document deliberately did not make.

---

## 18. Revision history

The revision history of this specification is its version-control log. A prose
copy of it inside the document was deleted in 0.4: it restated rule text at a
second home, drifted from it twice, and once contradicted it — the exact failure
R-002 names.


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

### Deliberately left to the implementation

These are decisions this document does not make. Writing a tool forces each of
them; the decision is then recorded in the corpus (R-193) and folded back here
once a second implementation has to agree with it. Guessing them now would
produce another round of contradictions.

- Which lexical contexts count as a `doc:` reference: comments, strings, fenced
  code, generated files, quoted logs
- How `verifies: symbol` resolves per language, and what happens when a symbol is
  ambiguous
- The sentence-boundary algorithm behind R-057's summary default
- Which link forms create reachability edges for R-034
- Behavior when version-control history is unavailable: shallow clones, exported
  archives, non-Git systems
- The lock file's name and format (R-154)
- What evidence establishes an "independent session" (R-025)
- The YAML and CommonMark dialects, and duplicate-key policy

### Known gaps, deferred

- **Manifest authenticity.** Hash verification (R-136, R-147) proves that
  content matches the manifest, not that the manifest is genuine. Signing, trusted origin, and anti-replay are
  unaddressed; a forged manifest with matching content passes every check.
- **Bootstrap.** A consumer resolving `@svc-auth/token-ttl` must already know
  where `svc-auth` publishes. There is no namespace-to-location mapping.
- **Namespace ownership.** Nothing prevents two estates from claiming the same
  namespace.
- **Lockfile.** Consumers have no way to pin an accepted revision of a foreign
  contract.
- **Federation cycles.** A consumes B consumes A is undefined.
- **Publication atomicity.** A provider can publish a manifest before its content.
- **Cache bounds.** No maximum age for a retained materialization, and no
  fail-open or fail-closed policy when a provider is permanently gone.
- **Monorepo scan partitioning.** Several namespace roots in one repository each
  scan the whole tree.
- **Binary assets.** Diagrams and images are neither permitted nor forbidden.

### Other gaps

- Brownfield ingestion: which signals in version-control history are worth
  turning into documentation, and which are noise. First slice taken by the
  implementation (D-053, D-054): `seed plan` inventories features and, for a
  named feature, the history and code that carry it — as evidence for a
  conversation, never as prose; the language-free signals (tags, births,
  scopes, reverts, citations, comment blocks) are read, the word-based ones
  need a declared vocabulary, and the noise (merge and mega commits, vendored
  trees, a delete-and-restore pair) is excluded by rule
- Whether `tutorial` earns its place in the `project` profile
- Graph export format for feature-to-code-to-documentation relations —
  first slice taken by the implementation (D-064): `graph` exports
  page→page links, graduation and code→page citations as DOT, JSON or JSON
  Canvas; the cross-tree case waits on federation
- Whether `verification` should extend to the `project` profile
- Epic status aggregation when legs disagree
- Generated API references: linked from the router, or addressed by identifier

---

## 20. Connectors (EXPERIMENTAL)

A knowledge base becomes an assistant's memory when what happens outside it —
a repository's history, a calendar, a mailbox, a note dictated on the move —
lands inside it as records. A connector is the thing that carries one such
source into `raw/inbox/`. It is deliberately small: it writes records and
nothing else.

> **Why experimental.** One connector exists, the git connector built into the
> binary, and the write gate every other one would call. The record grammar,
> the deduplication key, the outbound boundary and the scheduling boundary need
> a second and a third connector against real sources before they bind. Until
> then these rules bind nothing; a base that never runs a connector is
> untouched by everything below.

### 20.1 The record

**R-200** `cmd` · MUST — A connector writes records, never pages. It lands one
file per item in `raw/inbox/`; classification, distillation and routing are
ingest's (R-092), in a session, with judgment.

**R-201** `lint` · MUST — A record a connector writes carries its provenance in
frontmatter: `source` (the connector's name, a local-id), `source_id` (the
item's identity at the source), `title`, `captured` (the day it landed) and
`url` when the source has one. A record without provenance is a note a person
wrote; a record with it can be traced, deduplicated and re-fetched.

**R-202** `cmd` · MUST — The same item lands once: `(source, source_id)` is the
key, checked across all of `raw/`, so a connector may run again at any time —
the second run names what is already there and writes nothing. Idempotence is
what makes a schedule safe.

**R-203** `cmd` · MUST NOT — A connector never edits or deletes a record, its
own included (R-023). A source that changes an item produces a new item.

### 20.2 The boundary

**R-204** `advisory` · MUST — Secrets never enter a record. A connector redacts
credentials, tokens and anything the source marks private before the record
lands; `internal: true` (R-135) and `scan_exclude` do not reach into `raw/`.

**R-205** `advisory` · MUST — The schedule lives outside the tree. A connector
is a command run by a scheduler, a hook or a person; the tree holds records,
never timers. Nothing in a base fires by itself.

**R-206** `advisory` · MUST NOT — Outbound actions — creating an event, sending
a message, changing a ticket — are never a connector's and never the tool's.
They are skills an agent runs with the person's confirmation; the record of the
action, if one is kept, arrives like any other record.

**R-207** `cmd` · MUST — The digest is derived, never stored. `docsys status`
reads the base — the inbox, pages by state, open items, consumed namespaces,
compiled skills, the findings lint would raise — and composes no prose. What
the assistant says in the morning is the model's, from that.

### 20.3 The built-in connector and the write gate

`docsys inbox pull <repo> --since <date>` is the git connector: one record per
commit — `source: git`, `source_id: <namespace>@<short sha>`, the subject as
title, the body, the files touched, the day of the commit. It exists because
docsys already reads git, and because a project's history is the source a base
most often learns from. `docsys inbox add --source <name> --id <item>` is the
write gate every other connector calls — a shell script over an API, an MCP
tool, a mail filter — with the same provenance fields and the same key.

### 20.4 Kinds of connectors (design, not implementation)

| Source | Item | `source_id` | Body |
|---|---|---|---|
| git history | a commit | `<ns>@<sha>` | subject, body, files touched |
| a consumed project's journal | an entry | `<ns>@journal:<date>:<slug>` | the entry's lines, in the author's words |
| calendar | an event | the provider's event id | when, where, who, the description |
| mail | a message | the `Message-ID` | sender, subject, the text |
| a ticket tracker | an issue or a comment | the tracker's id | title, state, the text |
| reading | a clip | the URL plus a content hash | the clipped text and where it came from |
| voice | a recording | the file's hash | the transcript |
| an agent session | a note the person asked to keep | the session id plus a counter | the note, in the person's words |

Every row lands through the same gate and is distilled by the same organ; a
connector's whole job is the left three columns.
