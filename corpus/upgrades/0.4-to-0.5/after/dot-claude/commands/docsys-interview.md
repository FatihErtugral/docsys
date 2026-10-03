---
description: Collect what people know — the team's know-how, decisions and reasons — about one feature or every undocumented one, round by round; their words land verbatim under work/
allowed-tools: Bash(docsys *), Bash(git log:*), Bash(git show:*), Read, Grep, Glob, Write, Edit
---

# /docsys-interview — the seeding survey, round by round

`docsys seed gaps --repo . --root docs` lists every candidate feature with
its size, span and coverage. Uncovered features are the survey; covered
ones are the system's and are never asked about.

Each round is one feature, run exactly as `/docsys-seed <feature>`: research
by the tool, one "what I found" block, at most four plain questions, then
the builder's word before anything lands. Order: the largest uncovered
feature by commit count first, unless the builder names one. Stop when
the builder says stop; the next session resumes from `docsys seed gaps` —
what landed is reserved (`work/research/<feature>.md`, active) and will not
be asked again.

When the survey stops, name the next step: what the builder confirmed
graduates into permanent pages (`docsys graduate plan <work-file>`), and each
page about code is bound to its region as the rules block says.
