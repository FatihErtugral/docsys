# Changelog

All notable changes to docsys are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/). Release notes are extracted from this file
by the release workflow — the tag's section becomes the GitHub release body.

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
