---
description: Scan code↔doc drift and un-graduated done work; propose debt items
allowed-tools: Bash(git log:*), Bash(git diff:*), Bash(git show:*), Bash(docsys *), Read, Grep, Glob, Edit
---

# /docsys-sync — documentation drift check

Manual, never automatic. Report, wait for approval, commit nothing.

1. Mechanical pass: the rules block's two checks — include both outputs (one
   line each if green). Freshness errors are drift
   by definition: a stale pin names the region that moved, an untouched draft
   names abandonment.
2. Drift suspects: `docsys seed plan --repo . --root docs --since <the date
   of the last commit to docs/, `git log -1 --format=%cs -- docs/`>` — every
   feature history touched since, with
   its coverage. For each covered feature with commits, `git show --stat
   <sha> -- docs/`: did its page move with the code? Name the page that
   should have changed. An uncovered feature with commits is a seeding
   candidate, not drift.
3. Graduation debt: `grep -rl '^status: done' docs/work/` — for each, say
   concretely which section goes to which page.
4. For each debt, its `docsys debt add` command, ready to run.

No findings → say so; never invent debt.
