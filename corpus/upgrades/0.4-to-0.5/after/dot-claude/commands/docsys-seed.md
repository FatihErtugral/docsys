---
description: Seed documentation for one feature of an existing project from what its code and history say — research in git, ask the builder plainly, write only what was confirmed
allowed-tools: Bash(docsys *), Bash(git log:*), Bash(git show:*), Bash(git status:*), Read, Grep, Glob, Write, Edit
---

# /docsys-seed <feature> — seed one feature

For a project whose documentation does not exist. The tool does the
research; you present it; the builder confirms, corrects and adds what
history cannot say. **Nothing is written before the builder says so.**

## 1 · Research (tool)

`docsys seed plan --repo . --root docs --target <feature>` — commits with
their bodies, files by touch count and the other features they serve, the
birth, manifests, `doc:` citations, the code's own comment blocks, tags.
If it is refused ("already covered by …"), stop: that page is the system's
now; drift goes through `/docsys-sync`, not through seeding.
If it says nothing names the feature, ask ONE question: where does it live
(a path, a scope, a symbol)? Then run it again with what you learned.

## 2 · Present ("what I found")

One block, in the tree's language (`default_content_language`), every line
carrying its evidence (`path:line`, `sha`): what the feature does, how it is
built, when it was born and moved, what broke and was fixed, what the
manifests declare, what the code's comments say about WHY. Derive; do not
ask what the code already answers.

## 3 · Ask — at most four questions, one at a time

Plain, single-meaning, answerable in a sentence, never a metaphor, never a
question that creates a conflict. The bank:

- Is this summary right? What is wrong or missing?
- <a specific fact the code cannot settle — a requirement vs a fallback, a
  product decision, an audience>
- <what the builder does with it that the code does not show>
- What is next for it — or is it finished and not to be touched?

If an answer conflicts with the evidence, show the evidence (`file:line`,
the test, the commit) and keep the question open until it is clear: it is
not an `answer` row yet — it becomes a `question` row that names the
evidence, and only the builder's next word settles it. An answer the
builder cannot give becomes a `question` row, dated today.

## 3b · Agent memory

If this machine holds agent memory for the repository (Claude Code keeps
`memory/*.md` under `~/.claude/projects/<repo-slug>/`), run the plan with
`--memory <that dir>`, and ask the builder about each note as the rules
block says.

## 4 · Approve, then land (tool)

Write the rows the conversation produced into a plan file OUTSIDE `docs/`
(`SEED.tsv`, never committed) and show it; on the builder's word,
`docsys seed apply --plan SEED.tsv --repo . --root docs`.
When no builder can answer — a repository whose people are gone, a person
who says "land what history says, I will answer later" — the rows that need
nobody's memory still land on that person's word: `research` (the evidence,
reserved), `postmortem` (a commit's own account) and `question` (everything
the builder would have been asked); the chronology is history's own. Only `answer`
rows wait for a builder; a plan with none is not a plan withheld.
Rows are TAB-separated; `docsys seed plan` prints their grammar.

## 4b · The overview draft (the one page you may author)

After the rows land, per seeded feature:
`docsys page new explanation <feature>-overview --unverified`, its body
written from the evidence only — what step 2 presented — in the tree's
language, with `sources:` naming the same `git:` locators and files the
research page cites. The draft is where a reader starts on day one.
