---
name: docsys
description: Documentation system operations — set up, migrate, audit, and curate a docsys tree. Use when the user asks to create or migrate documentation structure, audit docs health, graduate work files, or when starting work in a repo that has docs/.docmeta.yml.
---

# docsys — the thin skill

This skill adds judgment and approval gates to the commands; never skip a
gate.

## Always

- Under `commit_policy: require` (D-093) the gate refuses code with no record
  of why, and the end of a turn holds until it is written: the session may be
  gone when the commit lands, so the knowledge is captured while the session
  is here.

## Set up (new tree)

`docsys init --root docs` then generate the agent block:
`docsys rules --agents-md >> AGENTS.md` (review the diff first).

## Migrate (existing docs anywhere in the repo)

1. `docsys migrate inventory --root <dir> --repo .` → plan skeleton with
   evidence lines and inbound-reference report.
2. Fill each TODO target — this is YOUR judgment call, per page, using the
   evidence and the P/R-031 procedure. **STOP: show the plan, get approval.**
3. `docsys migrate apply --plan <plan> --root <dir> --repo .` — the tool
   moves, rewrites links (in-tree and inbound), scaffolds. Review RISK lines:
   each is a judgment item, resolve or record as debt.

## Audit (report only)

Read the findings of the rules block's two checks with the R-049 / R-086
tables in hand. Change nothing; the user decides.

## Graduate (curation)

For each `status: done` work file: ask the R-093 question (does any still-true
information here exist nowhere else?). Route sections by the R-049 table.
Destination pages are prepared first (R-099); you select the mapping, you
never retype the text (R-090): `docsys graduate plan <work-file>`, then
`docsys graduate apply`. The file graduates only on the human's explicit word
(P/R-081), and every `doc:` citation of its id moves to a destination in the
same commit (D-127).

## Verification (who vouches)

Anyone writes — you included — and nothing you write is the truth yet. A
permanent page you author from evidence, or change in substance, names what it
rests on in `sources:`. It is verified once a maintainer approves it after its
last change (D-126); nothing about it is written into the page, and `docsys
help verify` says how an approval is recorded. When `.docmeta.yml` declares
`maintainers:`, an approval and `confirmed:` must name one of them (R-208):
the people who review the code are the people who vouch for the page. A
reader — a person or an agent — sees the state and reads accordingly.

## Compile (a howto into a skill)

A `howto/` page whose steps are complete — every step written, nothing you
would fill from memory (P/R-096) — compiles (`docsys compile`). A gap found
while running the skill is reported on the page, never patched in the skill.
