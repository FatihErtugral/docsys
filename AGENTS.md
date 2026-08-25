# AGENTS.md — docsys

Ground rules for any agent or contributor working in this repository. They are
short because each one was paid for.

## This is a community project

- **Everything committed here is English**: code, comments, corpus trees, test
  fixtures, documentation, commit messages. The *tool* is language-neutral by
  design (D-025, R-108) — localized content appears in the corpus only where a
  case exists to prove that neutrality (e.g. `18-heading-map`,
  `23-list-labels`), never as the default voice.
- **No external project, company, or person names — ever.** Not in code, not
  in the corpus, not in comments, not in commit messages, and therefore not in
  git history. Field lessons are told anonymously: "a real repository",
  "the pilot", "the first adoption".

## How a change lands (R-012: spec first)

1. **SPEC.md** — behavior starts as rule text. SPEC 0.4 is *deletion-first*:
   no change adds net normative rules, so most changes are clarifications; new
   implementation choices go to `corpus/DECISIONS.md` (R-193) instead.
2. **Code** — the smallest change that satisfies the text.
3. **Corpus** — every behavior is locked by a case with exact expected
   findings (`expected.tsv`, D-011). An extra finding fails a case as hard as
   a missing one. Behavior a corpus tree cannot carry (git state, adoption
   flows) is locked by an integration test under `tests/` instead.

## Severity doctrine

Only the irreversible or the silently-wrong blocks (R-151). A red gate nobody
can honestly clear teaches people to bypass it (R-150) — that is why adoption
gates start in warn mode and harden when the debt reaches zero.

## Hard constraints

- Zero runtime dependencies; `unsafe` is forbidden (D-001).
- Green before any commit:
  `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
- Every published release bumps `Cargo.toml` **and** `Cargo.lock`, and adds a
  CHANGELOG section — the release workflow turns that section into the
  release notes.
- **Force pushes and branch deletions are forbidden in this repository — permanently, no exceptions (2026-08-26).**
  No `--force`, `--force-with-lease`, `+ref`, `--delete`, `--no-verify`; `.githooks/pre-push` rejects
  non-fast-forward updates. A wrong commit is reverted, history is never rewritten.
