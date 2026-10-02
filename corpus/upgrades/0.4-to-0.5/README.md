# docsys/0.4 → docsys/0.5

The conformance case R-179 asks for: a repository as docsys 0.15 left it,
moved by `docsys upgrade --apply --commit`, compared file by file with the
expected repository. `tests/upgrade.rs` runs it.

| Path | What it is |
|---|---|
| `before/` | The repository's files as 0.15.1 wrote them, plus an owner's edits: one relay and one command changed by hand, a ledger item with em-dash markers, a `raw/` record. `@REV@` stands for the first commit. |
| `edits/` | The second commit: one verified page's body moves, and the code under one pin changes. |
| `hooks/pre-commit` | This clone's git gate as 0.15.1 wrote it, in warn mode. |
| `plan.txt` | What `docsys upgrade` prints before anything is written. |
| `after/` | The repository after the upgrade commit. |
| `hooks/pre-commit.after` | The gate after the upgrade: rewritten, its mode kept. |
| `expected.tsv` | The findings on the upgraded tree. |

A top-level `dot-<name>` stands for `.<name>` (`dot-claude/` is `.claude/`), so
an agent working in this repository never loads the case's skills or hooks.

The test builds the history first: `before/` committed on 2026-09-01, `@REV@`
replaced with that commit's short id, `edits/` committed on 2026-09-02, then
the gate installed. `DOCSYS_BLESS=1 cargo test --test upgrade` rewrites the
expected files from the current build. Read the diff before you commit it.
