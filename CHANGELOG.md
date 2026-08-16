# Changelog

All notable changes to docsys are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/). Release notes are extracted from this file
by the release workflow — the tag's section becomes the GitHub release body.

## [Unreleased]

### Added

- Audience modes (D-033): a page may declare `audience:`, the vocabulary is
  the tree's own (`audiences:` in `.docmeta.yml`, the domains pattern), and an
  undeclared page reads as `developer` — no migration, no guessing. `--audience`
  on `export plan/product/feature` selects by declaration: a *named* page of
  the wrong audience is refused, a `--follow`ed one becomes a named gap
  warning, and a draft for an audience nobody writes for is an error naming
  the whole-tree gap. The tool never writes and never re-audiences prose —
  the modes select, the agent authors.
- The `docsys-export` skill (installed by `agents`/`adopt`): turns "create the
  end-user doc for X" into a procedure — discover with `export plan`, compose,
  close gaps by authoring pages with approval, translate under R-122/R-123.
  Without it the prompt lives in the user's head, the hand-maintained
  knowledge D-022 exists to prevent.
- `docsys fetch` — federation's first working slice (D-034): a consumer
  declares its providers as one `consume_base:` template plus a list of names
  (three hundred services, one line — or `<ns>=<location>` per service), where
  a location is a filesystem path or a git URL with `#<subdir>`; fetch
  shallow-clones git providers and materializes each provider's exported
  pages under `.federation/<ns>/` (reconstructed frontmatter,
  verbatim body, provenance sidecar; `internal: true` never crosses), and a
  `@ns/<id>` map or feature entry now composes across repositories — two
  repos sharing one feature produce one document. Foreign entries resolve
  only against the verified local state: unfetched and tampered
  materializations are refused by name, and every foreign stamp carries its
  fetch date.

## [0.3.0] - 2026-08-16

The knowledge-base profile is checked, not just recognized. Shaped by the first
real knowledge base: a personal wiki whose constitution predated the spec and
matches it — the tool met a tree that was already living the rules.

### Added

- `profile: knowledge-base` is fully linted (supersedes D-006's v0 narrowing).
  The layout (`raw/` record layer, `wiki/<domain>/<type>/` pages, `wiki/index.md`
  plus domain indexes as routers), the page contract (`domain`, `verification`,
  `sources` — R-024), undeclared domains (R-026), and the directory's type
  segment (R-029) are all checked; wiki-links resolve against the permanent
  layer root, the field convention (D-030).
- R-028's verification record has names: `verified_by:` and `verified_rev:`.
  A `verified` page without them is reported — a verification nobody can audit
  is a claim, not a record.
- R-059: every `sources:` entry must resolve; a severed evidence trail is an
  error — it is exactly the silent failure R-027 names.
- R-023: `raw/` content-immutability is checked at the gate (D-031): uncommitted
  modification or deletion of a tracked raw file is an error; relocation — the
  basename reappearing under `raw/` — is the expected flow and passes. Outside
  git the hook layer owns the promise.
- The record layer never blocks (R-194, extended to the profile): a dangling
  link or reference inside a raw note is reported, never an error, and the
  path scan skips raw notes — they are quoted source material.
- `scan_exclude` now excludes from the docs-side walk too: a template library
  inside a knowledge base is tooling, not documentation.
- `adopt` and `graduate` refuse a knowledge-base tree by name instead of
  half-running: adoption is its own release; graduation there is distillation
  (R-092), an authored rewrite no command may fake.
- `export plan` / `export product` (D-032) — the product half of the founding
  goal: a product-level document composed from the tree's permanent pages.
  `plan` drafts a product map from the tree's evidence (R-057 titles and
  summaries, grouped by type) — a proposal, never a decision; the map is plain
  markdown (H1 name, H2 sections, router-shaped entries whose targets are
  `doc:` identifiers). `product` is fully mechanical: bodies carried verbatim
  with headings shifted (prose never rewritten), a source stamp per page
  (identifier, file, drift hash, `updated`), and a hard refusal to
  half-compose — a foreign, flowing, retired or unknown identifier fails the
  run with the complete list. Foreign (`@ns/`) entries compose once federation
  consumption exists. `--lang` states the document's intended content
  language: pages whose declared language differs are warned by name, and a
  mixed composition without a stated intent is warned once — the tool reads
  declarations, determines no language, and translates nothing (R-120–R-123;
  translation is agent work).
- `export feature <id>…` — a slice of the tree with no map file: the named
  identifiers, optionally widened one hop along their wiki-links (`--follow`).
  One command exports one feature out of a large code base.
- `--out` never rewrites an unchanged document: content is compared without
  the dated header line, so a no-op regeneration leaves the file, its honest
  generation date, and every downstream consumer (watchers, builds, a
  translating agent) untouched. Regeneration itself stays stateless — a cache
  is state, and state drifts (R-002); the per-page stamps are what let
  downstream re-process only the sections whose hash moved.
- `AGENTS.md` — the repository's own ground rules: English-only content, no
  external project names, spec-first, corpus-locked.

## [0.2.3] - 2026-08-16

A formatter ran over an adopted tree and the tool lost a field. Real projects
run prettier; a checker that silently drops configuration when they do is worse
than one that never read it.

### Fixed

- `.docmeta.yml` and frontmatter follow an inline list that a formatter
  reflowed across lines, including the shape where the opening bracket lands on
  its own line. A reflowed `scan_exclude` had been silently dropped, bringing
  back four already-resolved findings.
- A trailing comma leaves punctuation, not an item: the empty slot is skipped.
  Read as an item it became an empty path prefix — "exclude everything" — and
  the scan inspected zero files. R-011's dead-scan rule caught it on the real
  tree, which is the whole reason that rule exists.

## [0.2.2] - 2026-08-16

Warnings got the same treatment errors did: on a real tree, more than half of
them were the checker misreading the repository. 311 warnings became 0, and the
tool learned six things.

### Added

- `.docmeta.yml` accepts `list_labels: [deferred=<local form>, repay when=…]`
  (R-108), the same shape `headings` already had: a tree writing its
  documentation in another language must not be forced to embed English words
  in its own prose to satisfy a checker.

### Fixed

- A token containing `<`, `>`, `{` or `}` is a metasyntactic placeholder, not a
  reference (R-073). Prose documenting the citation form — including this
  project's own generated agent text — was failing the convention it teaches.
- Closed work (`graduated`, `abandoned`) is exempt from the section template
  (R-048): the template guides open work and makes graduation mechanical, so
  demanding it of a finished page asks for structure that can do nothing.
- A root-level page's full path is its bare name, so `[[roadmap]]` is not a
  short-name link (R-070).
- `graduated_to` resolves through the shared resolver (R-056): a destination
  named through a `defines:` family is as real as a page id, and two resolvers
  over one identifier space eventually disagree.

## [0.2.1] - 2026-08-16

The same real repository, one layer deeper: 212 errors became 2, and every
step of the drop was the tool learning, not the repository changing.

### Fixed

- R-079: a family member may be written in the register's own short form —
  a page listing `(utf8-text)` under `defines: ADR-*` defines `ADR-utf8-text`,
  and demanding the prefix twice made pages fail to define what they plainly
  define. An occurrence that is itself a `doc:` citation no longer counts as
  evidence: a reference cannot prove its own target (the hole withdrawn R-064
  was meant to close).
- The historical layer never blocks. A journal entry, an archive slice, or an
  `_archive/` page citing a graduated, flowing, or missing identifier is
  reported, not an error: a dated record cannot be corrected by editing it, and
  the legitimate repairs — a tombstone, a distilled page — are what the report
  names. An error nobody may honestly clear is one people learn to bypass.
- A citation to an archived page resolves and is reported; the record is real,
  dated content, exactly as an explicit `[[_archive/…]]` link already was.
- A graduated page that records `graduated_to` is a signpost, not a husk:
  citing it is reported, not blocked. Only a graduated page with nowhere to
  point remains an error.
- Inline-code parity is counted from the start of the line, so a second
  reference on the same line parses (it used to swallow the closing backtick).

## [0.2.0] - 2026-08-16

First release shaped by a real adoption: every change below came from running
the tool against an existing repository and finding the tool, not the repo, at
fault.

### Added

- `adopt` — one-command integration: init skeleton or docmeta upgrade, agent
  assets, `settings.json` when absent, the AGENTS.md managed block, a warn-mode
  git pre-commit gate (hooksPath-aware), and an `ADOPTION.md` report whose
  checklist carries every judgment call. Idempotent; refuses an unmigrated tree.
- R-194: a `doc:` reference resolves against every page id, tracked work
  included. A citation to a graduated page is an error (its value moved on); any
  other flowing citation is reported, naming the distillation still owed.
- `.docmeta.yml` accepts `journal_entry_max_lines` (R-101, default 5) so a tree
  can state a discipline it actually keeps, and whole-line `#` comments.

### Fixed

- Repo-side scans list files through `git ls-files`, so `.gitignore` decides
  what belongs to the project (D-029). A build tree or a nested worktree used to
  turn 147 real findings into 9,171.
- A relative `--root` is anchored to `--repo`; the docs root is excluded by
  identity, not spelling (D-027) — it used to be scanned as code.
- R-073: a reference opened inside an inline-code span ends at the closing
  backtick, so a grammatical suffix glued to it (a case ending, a possessive) is
  no longer read into the identifier.
- R-100: one bracketed annotation may sit between the date and the separator —
  entry counters and channel tags are field conventions that keep the date first.
- R-075: an escaping link whose target exists is reported, not an error;
  documentation routing to a file beside the code is working, not broken.

## [0.1.0] - 2026-08-15

Initial public release.

### The specification

- `SPEC.md`: 133 normative rules across layout, identity, lifecycle,
  graduation, journal discipline, freshness, and the agent layer; survived
  six adversarial audit rounds by six independent models. Federation (§13)
  ships as EXPERIMENTAL and binds nothing until a reference implementation
  and a second real estate exist.
- `corpus/DECISIONS.md`: every implementation-defined choice registered
  with its reason (D-001..D-028).
- Conformance corpus with exact expected findings — an extra finding fails
  a case as hard as a missing one, so the checker cannot drift noisy.

### The binary (zero dependencies, `unsafe` forbidden)

- `adopt` — one-command integration: init skeleton or docmeta upgrade,
  agent assets, `settings.json` when absent, AGENTS.md managed block,
  warn-mode git pre-commit gate (hooksPath-aware), and an `ADOPTION.md`
  report whose checklist carries every judgment call. Idempotent.
- `lint` — full tree validation: frontmatter, ids, links, journal
  discipline, templates, list grammars. Warn by default; block only what
  is irreversible or silently wrong.
- `refs` — every `doc: <id>` in the code base validated against the tree.
- `migrate inventory/apply` — brownfield adoption: evidence-rich plan,
  approved mapping, mechanical move with link rewriting on both sides of
  the docs boundary.
- `graduate plan/apply` — byte-exact block movement from work files to the
  permanent layer; content is moved, never rewritten.
- `rules` / `agents` — agent-facing text generated from the embedded spec
  (no hand-maintained copy to drift) and the thin `.claude/` adapter:
  four warn-only hooks, `/docsys-sync`, skill.

### Field-proven

- Piloted against three real repositories; a 63-file flat tree migrated
  with 29 references rewritten, and a full legacy documentation system
  swapped out with zero memory loss.
