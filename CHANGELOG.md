# Changelog

All notable changes to docsys are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/). Release notes are extracted from this file
by the release workflow — the tag's section becomes the GitHub release body.

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
