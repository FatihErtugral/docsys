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
| D-016 | **Brownfield signal.** A tree whose markdown files all sit outside the layout — a router counts as inside — warns under R-020 (`layout`): an unmigrated tree must not read as a clean one (second pilot: 63 flat files, four findings, misleading silence). |
| D-017 | **Migration plan format.** `path<TAB>target` rows, target ∈ reference/howto/explanation/tutorial/archive/keep; `#` comment lines carry classification evidence (first heading, in/out link counts). The tool never fills a target — classification is judgment (R-003). |
| D-018 | **Migrated identifiers.** `id` derives from the filename in kebab-case (`AppManifests.md` → `app-manifests`); `type` is the plan's target; `updated` is the migration date. |
| D-019 | **Out-of-tree links survive migration.** A relative link escaping the docs root is depth-corrected to keep pointing where it pointed (R-172: the migration rewrites what its own moves invalidated) — and is then judged by lint on its own merits, which surfaces pre-existing R-075 debt instead of silently breaking or hiding it. |
| D-020 | **Inbound repo references.** `--repo` extends migration to the other side of the boundary: inventory reports every file referencing into the docs tree (Phase A risk report), apply rewrites exact moved-path strings across the repo — including inside URLs that carry the repo's own paths — and reports what it could not map (directory-level and generated-path references) as RISK lines for judgment. A match preceded by `/` or a word character is someone else's path, never rewritten, never a risk. |
| D-021 | **`refs` scope.** The code scan covers every readable text file in the repo minus the docs tree (lint owns that side), minus `scan_exclude` prefixes from `.docmeta.yml` (R-077 — the owner's word on archived sub-projects and tooling). Token grammar and resolution are byte-identical with the docs-side check: one shared index, one shared tokenizer, so the two sides can never disagree. Supersedes D-012. |
| D-022 | **Agent text rendering (R-155).** SPEC.md is embedded in the binary at build time; `rules --procedures` extracts §14.3 verbatim, `rules --agents-md` derives the always-loaded block mechanically (every `agent` rule's number + first sentence, the R-057 pattern) plus fixed tool-gate lines. Nothing is hand-authored, so the text cannot drift from the spec. The budget floor (R-165) is the rendered summary itself. |
| D-023 | **Agent assets (`docsys agents`).** Four hooks (pre-commit surface warning, stop reminder, updated-bump, session-intent routing), /doc-sync command, and the thin skill install under `.claude/`; every hook warns and never blocks (R-150), names what must change (R-152), and carries its edit-with-the-tree pattern note (R-011's dead-scan lesson). settings.json is printed for hand-merge, never written — it is a protected file. |
| D-024 | **Graduation plan format.** `# source:` header; per-block evidence comments (`# block <n> · L<a>-L<b> · fnv:<16hex> · "snippet"`); rows `<n><TAB>keep|link:<dest>|move:<dest>`. The fnv checksum is plan integrity, not the R-113 content hash: any drift between plan and apply refuses with "re-plan". `move:` destinations must pre-exist with an id (R-099's authored step is never done blindly); template-section headings stay in the source (R-098) and only bodies move; `graduated_to` is updated in place and may sit on an `active` file (R-091). |
| D-025 | **Heading map — universal, zero embedded languages.** `.docmeta.yml` `headings: [Context=Bağlam, ...]` declares the displayed form of each canonical template heading; lint (R-048) and graduation keep-heading detection match through the map. The tool translates nothing and knows no language — the tree declares its own. |

## Open (assigned, not yet decided)

- R-028 verification-record field names — decided with the knowledge-base release
- Migration data schema (R-173) — decided with the first migration
- Compiled-skill location/format (R-094/095) — decided with `compile-skill`
- Lock file name/format (R-154) — decided with the first multi-writer feature
