# Implementation decisions register (R-193)

Where SPEC.md deliberately leaves a decision to the implementation, the decision
is made here, once, and the corpus enforces it. A future spec release may fold a
line back into rule text; until then this file is the authority for `docsys`.

| # | Decision |
|---|---|
| D-001 | **Zero runtime dependencies.** The binary is stdlib-only: auditable, no supply chain, instant builds. Revisit only if a check becomes impossible without a dependency. |
| D-002 | **Frontmatter YAML subset.** `---` fences; `key: value` scalars (optional quotes, ` #` comments); inline `[a, b]` and block `- item` lists; keys `[a-z][a-z0-9_]*`. Nesting, multi-line scalars, anchors and duplicate keys are parse findings, never guesses. `.docmeta.yml` is the same subset without the fences. |
| D-003 | **Ledger format.** `.tombstones.yml` entries open with `- id: <local-id>`; `date:` and `superseded_by:` continuation lines are tolerated (v0 reads only the id). |
| D-004 | **Dates.** `updated` and all spec dates are plain ISO `YYYY-MM-DD`; no time, no timezone. History-derived freshness (R-106) is deferred to the VCS-aware release. |
| D-005 | **Exit codes.** 0 = clean or warnings only; 1 = at least one error finding; 2 = tree not operable (missing root or `.docmeta.yml`). |
| D-006 | **v0 profile scope.** `profile: project` only. `knowledge-base` is recognized and reported as not-yet-implemented, never half-checked. |
| D-007 | **`_archive/` scope.** v0 does not walk `_archive/` at all: its pages join no check population; wiki-link targets are resolved against it by direct path probe only (R-071's resolve-and-report). |
| D-008 | **`defines:` glob subset.** A single trailing `*` (prefix match). R-079 token membership requires the cited identifier to occur token-bounded on the defining page (`-` counts as a word character). |
| D-009 | **Reachability edges (R-034).** Edges are wiki-links; the walk starts at `index.md` and follows through any non-archived page, transitively. |
| D-010 | **Finding subjects (R-158).** Per rule: reference checks → the target token; frontmatter checks → the field name(s), comma-joined; line-anchored format checks → `line-N` / `entry-N`; tree-level findings → file `-`. |
| D-011 | **Expected-output format.** Corpus cases carry `expected.tsv`: one finding per line, `SEVERITY<TAB>RULE<TAB>FILE<TAB>SUBJECT`, sorted; a first line `EXIT<TAB>n` asserts the exit class. Comparison is exact set equality — extra findings fail the case as hard as missing ones. |
| D-012 | **Code scanning (R-077) not offered in v0.** `doc:` references are resolved inside the documentation tree only; the `refs` command brings the code scan and R-072 with it. |
| D-013 | **Router acceptance.** `- [[<path>|<title>]] -- <sentence>` with ` -- ` canonical; a spaced em dash is accepted on read, never emitted. |
| D-014 | **Router-entry detection.** Only `- ` lines containing `[[` are entries subject to R-035's grammar; plain bullets are prose, and whether prose belongs on a router is judgment (first pilot: three false positives on layout-description bullets). |
| D-015 | **Backtick strip.** The backtick joined R-073's trailing-punctuation set (folded back into the spec): `` `doc: x` `` written as inline code is the reference convention of a real field tree. |

## Open (assigned, not yet decided)

- R-028 verification-record field names — decided with the knowledge-base release
- Migration data schema (R-173) — decided with the first migration
- Compiled-skill location/format (R-094/095) — decided with `compile-skill`
- Lock file name/format (R-154) — decided with the first multi-writer feature
