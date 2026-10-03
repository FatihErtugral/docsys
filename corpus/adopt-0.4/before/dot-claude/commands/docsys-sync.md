---
description: Scan code↔doc drift and un-graduated done work; propose debt items as a diff
allowed-tools: Bash(git log:*), Bash(git diff:*), Bash(git show:*), Bash(docsys *), Read, Grep, Glob, Edit
---

# /docsys-sync — documentation drift check

Manual, never automatic. Report; propose `docs/work/debt.md` items as a diff
and wait for approval. Commit nothing.

1. Mechanical pass: `docsys lint --root docs --repo .` and `docsys refs --repo .` —
   include both outputs (one line each if green). Freshness errors are drift
   by definition: a stale pin names the region that moved, `updated:` behind
   history names a hand edit, an untouched draft names abandonment.
2. Drift suspects: `docsys seed plan --repo . --root docs --since <date of
   the newest journal entry>` — every feature history touched since, with
   its coverage. For each covered feature with commits, `git show --stat
   <sha> -- docs/`: did its page move with the code? Name the page that
   should have changed. An uncovered feature with commits is a seeding
   candidate, not drift.
3. Graduation debt: `grep -rl '^status: done' docs/work/` — for each, what
   still-true knowledge exists nowhere permanent? Say concretely which section
   goes to which page (the R-049 table decides).
4. Propose debt items (`- [ ] <debt> -- deferred: <reason> -- repay when:
   <trigger>`) as an Edit diff; do not apply without approval.

No findings → say so; never invent debt.
