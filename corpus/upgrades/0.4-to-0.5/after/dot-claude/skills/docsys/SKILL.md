---
name: docsys
description: Documentation system operations — set up, migrate, audit, and curate a docsys tree. Use when the user asks to create or migrate documentation structure, audit docs health, graduate work files, or when starting work in a repo that has docs/.docmeta.yml.
---

# docsys — the thin skill

The mechanics live in the `docsys` binary; this skill adds only judgment and
approval gates. Never re-implement what a command does; never skip a gate.

## Always

- The work has a type — feature, bug, improvement, research — and a record:
  a work file under `work/<category>/` or, at minimum, a commit message that
  says why — the journal is history, and a commit that changes no page
  carries `Docs: <why>` (D-125). Under `commit_policy: require` (D-093) the
  gate refuses a commit without it and the end of a turn holds until it is
  written: the session may be gone when the commit lands, so the knowledge
  is captured while the session is here.
- A question about the tree starts with `docsys lookup <words>` — every page,
  local and consumed (`@namespace/id`), naming the words — then the page.

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

`docsys lint` + `docsys refs --repo .` and read the findings with the R-049 /
R-086 tables in hand. Change nothing; the user decides.

## Graduate (curation)

For each `status: done` work file: ask the R-093 question (does any still-true
information here exist nowhere else?). Route sections by the R-049 table.
Destination pages are prepared first (R-099), blocks move byte-exactly — you
select the mapping, you never retype the text (R-090). `confirmed:` requires
the human's explicit word (P/R-081).

## Verification (who vouches)

Anyone writes — you included — and nothing you write is the truth yet. A
permanent page you author from evidence, or change in substance, carries
`sources:` (what it rests on); `docsys page new <type> <id> --unverified`
writes it. The page is verified once a maintainer approves it after its last
change (D-126): the `Approved-by:` line the approval job adds to a pull
request's description lands in the merge commit, and where no host does that,
`docsys verify <page>` makes the maintainer's own commit (their git identity;
refused otherwise). Nothing about it is written into the page. `docsys verify
--show <page>` lists what a re-verification reads; `docsys verify <page>
--revoke` takes the approval back. When `.docmeta.yml` declares
`maintainers:`, an approval and `confirmed:` must name one of them (R-208):
the people who review the code are the people who vouch for the page. A
reader — a person or an agent — sees the state and reads accordingly.

## Compile (a howto into a skill)

A `howto/` page whose steps are complete — every step written, nothing you
would fill from memory (P/R-096) — compiles: `docsys compile <id>`. The skill
is the page body byte for byte, pinned to the page's content hash; lint fails
when the page moves until you re-read it and compile again (R-095). A gap
found while running the skill is reported on the page, never patched in the
skill.
