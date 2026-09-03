# The agent lab

Two ways of testing docsys's distillation flows — knowledge-base ingest
(raw → wiki), project graduation (work → permanent), a base learning from
consumed projects (`@namespace/id`), and the brownfield path (years of
history, no docs → adopt → seed → graduate; a base pulling that history
through the git connector) — that report separately:

- **the mechanical harness** (`mech/`): no agent, no network, exact strings,
  under three minutes; what the binary guarantees, pinned. CI runs it
  through `ci/e2e.sh`.
- **the agent lab** (`agent/`): headless `claude -p` sessions on the same
  fixtures, one model at a time (sonnet and opus from identical seeds),
  scored against `rubric.md`; what the agent layer achieves with only the
  installed instructions (D-087).

Everything here is committed except `out/` (gitignored): transcripts, trees,
real clones, costs, and any named material live there and nowhere else.

## Run

```sh
cargo build --release
ci/agent-lab/mech/run.sh                       # → out/mech/<ts>/results.tsv, REPORT-mechanical.md
ci/agent-lab/agent/run-task.sh kb F1-ingest sonnet    # one session
ci/agent-lab/agent/run-matrix.sh --models sonnet,opus # the synthetic matrix
ci/agent-lab/agent/run-real.sh out/real/repos.tsv --round 1
ci/agent-lab/agent/score.sh                    # → REPORT-agent.md
./run-all.sh [--only mech|agent] [--models …] [--real …]
```

`docsys` is the binary this checkout built (`target/release` first on PATH);
the lab never measures an installed copy. `claude` 2.1+ is needed for the
agent leg; the mechanical leg needs bash, git, coreutils, grep, awk.

## Layout

| path | what |
|---|---|
| `lib.sh` | shared helpers: dated commits (reproducible SHAs), lint summary, `check`/`expect_*` recording to `results.tsv` |
| `fixtures/gen-kb.sh` | a knowledge base with three domains, two pages (one unfaithful), eight inbox notes (`fixtures/notes/`) |
| `fixtures/gen-project.sh` | a project with three finished work files, destinations prepared, `fixtures/project/expected-dispositions.tsv` |
| `fixtures/gen-estate.sh` | three provider projects with dated, bodied and bookkeeping commits, plus a stranger base |
| `fixtures/gen-brownfield.sh` | `ledgerkit`: 2019–2026, scopes, manifests, a Turkish subject, a mega-commit, a dangling `doc:`, tags |
| `mech/<flow>.sh` | one script per flow; `mech/run.sh` runs them all and writes the report |
| `tasks/` | what a person would say — one or two sentences per task, no procedure |
| `rubric.md` | the criteria, which are automatic, and where the agent read each expectation |
| `agent/run-task.sh` | one session + mechanical capture + `checks.sh` |
| `agent/run-matrix.sh` | the four chains per model, the cross audit, the cost cap |
| `agent/run-real.sh` | the real-repository leg (clones from GitHub, remotes removed, codenames) |
| `agent/score.sh` | merges `auto.tsv` and the reader's `score.tsv` into `REPORT-agent.md` |
| `REPORT-mechanical.md` · `REPORT-agent.md` · `FINDINGS.md` | the committed results |

## Rules of the lab

- **Fixtures are dated.** Every commit carries an explicit author and
  committer date, so SHAs are a function of the inputs and R-106 has a
  stable answer. Regenerating a fixture gives the same tree.
- **The task text is the person's message.** It never carries a procedure.
  When a session succeeds only because the message supplied one, the
  sentence belongs in the installed layer and the finding goes to
  `FINDINGS.md` (D-087).
- **Outputs stay out of the fixture.** Scripts write to `$WORK/out`, never
  into the repository under test; a stray file is a dirty tree and
  `graduate apply` is right to refuse it (R-097).
- **Strict lint.** A warning in a final tree fails the lint row.
- **Real repositories** are cloned from GitHub into `out/real/<codename>/src`
  with `origin` removed; sessions work on copies; nothing is pushed, nothing
  named leaves `out/`. `agent/repos.example.tsv` shows the shape of the
  gitignored `out/real/repos.tsv`.
- **Model comparison** starts every chain from the same `seed` tag, keeps a
  chain on one model, records the resolved model id and the binary's sha,
  and reports only the differences `rubric.md` calls differences.

## What goes where afterwards

An exact expectation the harness pinned becomes an integration test
(`tests/*.rs`) or an `ci/e2e.sh` step; a docsys gap becomes a DECISIONS row,
a SPEC clarification or a change to the installed text — in that order,
spec first. The lab is where the expectation is discovered, not where it
lives.

## Notes on git the lab learned the hard way

- `--since YYYY-MM-DD` is "that day at this hour" to git; docsys normalizes
  a bare date to the start of the day (finding 16), other spellings pass.
- `--since` with a year of 2100 or later is no bound at all.
- `git log --since` stops walking at the first commit older than the bound:
  a history whose dates are out of topological order is cut short. The
  fixtures are dated in order for that reason.
- `git add -A` from a subdirectory stages the whole repository: a fixture
  that is not its own repository turns a session's commit into a commit on
  the enclosing one. Every fixture is its own repository, outside this one.
- bash reads a script incrementally: editing `agent/run-task.sh` while a
  session runs breaks the running copy at a shifted line. The runners
  re-execute themselves from a private copy (`LAB_SELF_COPY`), so the files
  can be edited between sessions without waiting.
- never stop a session with `pkill -f <pattern>`: the runners' private script
  copies and the operator's own shell match the same patterns. `run-task.sh`
  records nothing else than its output directory; stop a session by the pid
  of the `claude` process working in its fixture.
- the operator's own Claude Code settings reach every session: a
  `language` setting makes the agent report in that language, and once made
  it rewrite a wiki file in it (finding 19). The lab keeps the setting as it
  is — it is the person's real environment — and measures page language as
  a rubric row; docsys answers with grammar (R-108 on `open-questions.md`)
  and the installed sentence about the base's language.
