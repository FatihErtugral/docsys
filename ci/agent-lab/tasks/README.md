# Task texts

What a person would type, nothing more. A task file carries no procedure —
which organ to use, how `@namespace/id` is spelled, where a plan file goes,
that blocks move byte-exactly — because the installed layer (`.claude/`,
`AGENTS.md`, `docsys rules --procedures`) has to say all of that (D-087). A
task that passes only when its text carries a procedure is a docsys finding,
and the sentence moves into the installed layer; the task text stays.

`_preamble.md` is prepended to every task and states only the sandbox:
committing is allowed, pushing is not, nobody answers, end with a report.

Placeholders `${REPO_PATH}` and `${FEATURE}` are filled by the runner
(`agent/run-task.sh`), never by hand.
