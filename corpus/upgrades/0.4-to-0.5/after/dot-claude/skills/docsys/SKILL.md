---
name: docsys
description: Documentation system operations — set up, migrate, audit, and curate a docsys tree. Use when the user asks to create or migrate documentation structure, audit docs health, graduate work files, or when starting work in a repo that has docs/.docmeta.yml.
---

# docsys — the thin skill

This skill adds judgment and approval gates to the commands; never skip a
gate.

## Set up

A repository is set up with `docsys adopt`; `docsys help adopt` says what it
writes.

## Migrate (existing docs anywhere in the repo)

1. `docsys migrate inventory --root <dir> --repo .`
2. Fill each TODO target — this is YOUR judgment call, per page, using the
   evidence and the P/R-031 procedure. **STOP: show the plan, get approval.**
3. `docsys migrate apply --plan <plan> --root <dir> --repo .`, then review its
   RISK lines: each is a judgment item, resolve or record as debt.

## Audit (report only)

Read the findings of the rules block's two checks with the R-049 / R-086
tables in hand. Change nothing; the user decides.

## Graduate (curation)

For each `status: done` work file, route its sections by the R-049 table;
destination pages are prepared first (R-099): `docsys graduate plan
<work-file>`, then `docsys graduate apply`. Every `doc:` citation of the
file's id moves to a destination in the same commit (D-127).

## Verification (who vouches)

Anyone writes — you included — and nothing you write is the truth yet. A
permanent page you author from evidence, or change in substance, names what it
rests on in `sources:`. Nothing about its verification is written into the
page (D-126); `docsys help verify` says how an approval is recorded. When
`.docmeta.yml` declares
`maintainers:`, an approval and a confirmation must name one of them (R-208):
the people who review the code are the people who vouch for the page. A
reader — a person or an agent — sees the state and reads accordingly.

## Compile (a howto into a skill)

A gap found while running a compiled skill is reported on its page, never
patched in the skill (P/R-096).
