---
name: docsys
description: Documentation system operations — set up, migrate, audit, and curate a docsys tree. Use when the user asks to create or migrate documentation structure, audit docs health, graduate work files, or when starting work in a repo that has docs/.docmeta.yml.
---

# docsys — the thin skill

The mechanics live in the `docsys` binary; this skill adds only judgment and
approval gates. Never re-implement what a command does; never skip a gate.

## Always

- Gate: `docsys lint --root docs` — before any commit, after any docs change.
  Inside the repository it also checks freshness: a stale pin (R-111) is
  re-read against the code, then `docsys pin --refresh <page>` — never
  refreshed blind; `updated:` behind history (R-106) is one date; a draft
  untouched past `stale_active_days` (R-085) is abandoned with a reason,
  graduated, or worked on.
- Never rewrite content — move it. Never translate. Never invent.
- The work has a type — feature, bug, improvement, research — and a record:
  a work file under `work/<category>/` or, at minimum, a journal entry that
  links the files and says why. Under `commit_policy: require` (D-093) the
  gate refuses a commit without it and the end of a turn holds until it is
  written: the session may be gone when the commit lands, so the knowledge
  is captured while the session is here.
- A question about the tree starts with `docsys lookup <words>` — every page,
  local and consumed (`@namespace/id`), naming the words — then the page.
- Judgment calls follow the authored procedures: `docsys rules --procedures`.
  When no option fits, take the escape; never force.
- Which command for which intent: knowledge only people have → `/docsys-interview`
  (the answers land verbatim under `work/`); existing code with no pages →
  `/docsys-seed <feature>`; pages that drifted from the code → `/docsys-sync`.
- Pin a region (`docsys pin`) when a change to it would likely make the page
  false: a symbol, `Class.method` for a member, never a large file whole —
  every unrelated edit to it stales the page.
- Not known → a dated `work/questions.md` item, never a guess left on a page.
  Agent memory is a question for the person, never a source (D-062).
- When docsys is wrong or in your way: `docsys feedback --draft`, fill it, and
  ask the person before filing it — filing publishes.

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
`verification: unverified` and `sources:` (what it rests on); `docsys page
new <type> <id> --unverified` writes that frontmatter. Only an independent
session sets `verified` (R-025), after reading every claim against its
sources and the code they name, recording `verified_by:` and `verified_rev:`
(R-028) — `docsys verify <page>` writes that record for whoever runs it (a
maintainer, from their git identity; refused otherwise), and `docsys verify
<page> --revoke` takes a page back to `unverified` when its body moved; a
`verified` page whose body then changes is an error until it is `unverified`
again (R-024). The maintainer in the session needs no second session: when
the person you work with is a declared maintainer and says the page is right,
run `docsys verify <page>` — it records them, not you (D-096). On a docsys/0.5 tree
a session that wrote none of the page records its own claim-by-claim reading
with `docsys check <page> --by <session>` — a check, never a verification
(R-214): `verified` stays a maintainer's word. When
`.docmeta.yml` declares `maintainers:`,
`verified_by:` and `confirmed:` must name one of them (R-208): the people
who review the code are the people who vouch for the page. A reader — a
person or an agent — sees the state and reads accordingly.

## Compile (a howto into a skill)

A `howto/` page whose steps are complete — every step written, nothing you
would fill from memory (P/R-096) — compiles: `docsys compile <id>`. The skill
is the page body byte for byte, pinned to the page's content hash; lint fails
when the page moves until you re-read it and compile again (R-095). A gap
found while running the skill is reported on the page, never patched in the
skill.
