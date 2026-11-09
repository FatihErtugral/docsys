# docsys/0.5 → docsys/0.6

The conformance case R-179 asks for: a team's repository as docsys 0.17.0
adopted it, moved by `docsys upgrade --apply --commit`, compared file by file
with the expected repository. `tests/team_move.rs` runs it.

| Path | What it is |
|---|---|
| `before/` | The repository as docsys 0.17.0 wrote it: a dangling `doc:` in code, so its tracked `.githooks/` gate is in warn mode; its pin moved back to 0.16.1; and the owner's own workflow, in the shape the team repositories keep — a `push:` with no filter, a release install pinned to 0.16.1, `lint`, `refs` and `gate --range`. |
| `plan.txt` | What `docsys upgrade` prints before anything is written. |
| `after/` | The repository after the move's commit. |
| `lint.txt` | `docsys lint` on the moved tree. |

A top-level `dot-<name>` stands for `.<name>` (`dot-claude/` is `.claude/`), so
an agent working in this repository never loads the case's skills or hooks.

The release is a `file://` directory with one fixed sha256 a target, so the
installer the move writes is the same bytes on every run. After the
comparison the test commits through the hardened gate — an unrelated change
passes, naming the debt as inherited; a new dangling citation is stopped — and
runs the gate and the prompt relay with no docsys on the PATH.
`DOCSYS_BLESS=1 cargo test --test team_move` rewrites the expected files from
the current build. Read the diff before you commit it.
