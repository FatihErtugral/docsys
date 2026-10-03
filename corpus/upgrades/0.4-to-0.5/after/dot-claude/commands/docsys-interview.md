---
description: Collect what people know — the team's know-how, decisions and reasons — about one feature or every undocumented one, round by round
allowed-tools: Bash(docsys *), Bash(git log:*), Bash(git show:*), Read, Grep, Glob, Write, Edit
---

# /docsys-interview — the seeding survey, round by round

The survey is the list `docsys seed gaps --repo . --root docs` gives, the
largest feature first unless the builder names one. Each round is one
feature, run exactly as `/docsys-seed <feature>`. Stop when the builder says
stop; the next session resumes from `docsys seed gaps` —
what landed is reserved (`work/research/<feature>.md`, active) and will not
be asked again.

When the survey stops, name the next step: what the builder confirmed
graduates into permanent pages (`docsys graduate plan <work-file>`), and each
page about code is bound to its region as the rules block says.
