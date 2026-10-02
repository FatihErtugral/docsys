---
description: Run the seeding interview across a project's undocumented features, one feature per round, resumable
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

Rules that never bend: derive what history and code can say; ask only what
they cannot; a question is plain and single-meaning; a conflicting answer
is talked through, not recorded; nothing is written before approval; the
builder's words land verbatim, attributed and dated; the permanent layer is
never written here.
