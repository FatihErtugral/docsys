# The ideas docsys is built on

A plain document, outside the documentation tree on purpose: it is not linted,
not routed, not graduated. It explains the handful of ideas everything else
follows from, in the order they were adopted, so a reader can judge a rule by
the idea behind it. The binding text is `SPEC.md`; the reasons behind each
turn are in `corpus/DECISIONS.md`; this file is the map.

## 1 · Knowledge has two states: flowing and permanent

Work produces notes, drafts, half-decisions. Readers need contracts, procedures
and reasons that are still true. docsys keeps the two apart on disk — a
flowing layer (`work/` in a project, `raw/` in a knowledge base) where anything
may be written at any time, and a permanent layer (`reference/`, `howto/`,
`explanation/`, `tutorial/`; `wiki/<domain>/<type>/` in a base) that holds
only what earned its place. The move between them is the one act that carries
authority; everything else is free.

## 2 · The tool never writes prose

docsys moves, checks, derives and refuses. It does not author a sentence a
reader will take as knowledge: graduation moves blocks byte for byte;
seeding lands commit subjects, bodies and questions verbatim; a router line is
derived from a page's own summary. Whenever a decision is needed — which
type, which page, is this still true — a person or a model decides, and the
tool records that it was decided (R-003, D-053). This is why a seeded project
looks like skeletons and questions until someone answers: the tool has nothing
of its own to say.

## 3 · Mechanics enforce; judgment gets a procedure

Every rule is either mechanical (lint, gate, a command refuses) or a judgment
call written as a procedure the agent follows (`docsys rules --procedures`:
evidence, question, options, escape). The agent layer installed into a project
is thin on purpose: relays that call the binary, a skill that adds only the
approval gates, commands for the two conversations (seeding, interviewing).
Nothing an agent needs to run a tree lives in a prompt; if a task succeeds only
because the person's message supplied a procedure, that sentence moves into
the installed layer (D-087).

## 4 · Honest over complete

A documentation system fails two ways: it says nothing, or it says something
wrong with confidence. docsys treats the second as the worse failure (R-151):
a check blocks only when the outcome is irreversible or silently wrong; a
composition refuses to half-compose; no page carries a stamp that it was
checked, so none outlives the reading; "not in the base" is a complete answer. In this
repository the owner went further and reads every warning about
documentation as an error.

## 5 · Freshness is a hash, not a reviewer

Documentation drifts because nothing measures the distance between a page and
what it describes. docsys pins a page to a code region by content hash
(`pins:`) and dates a page by its last content change in history — when the
code moves, the page is stale by name, not by someone's memory. A compiled skill is pinned to the page it came
from and fails when the page moves.

## 6 · A page is read, not stamped

Every way of recording that a page was right cost a team a flow: a record in
the page conflicted on merge, an approval in history needed a host's
settings, and each needed a list of who may vouch. So a page carries no
verification state (D-130). It names in `sources:` what it rests on, and its
pins say when the code it describes moved. When a person wants pages
checked, an agent reads them against their sources and the code
(`/docsys-crosscheck`), corrects what is wrong in an ordinary commit, and
records what it cannot settle as a question (D-131). The worry the old
contract answered — on a team not everyone who writes knows — is answered by
a reading on demand, not by a stamp that ages.

## 7 · The human word, recorded

Some transitions are nobody's to infer: a work file is `done` or `graduated`
only on a person's explicit confirmation, recorded as `confirmed:` (R-081);
graduation asks the one question a human can answer — does any still-true
knowledge here exist nowhere else? The default commit gate asks once and lets
the same commit through, because a wall gets hooks disabled; a team that wants
the wall declares `commit_policy: require`, and then no commit lands without
its documentation, the end of a turn holds until the work is recorded — the
only moment the conversation that holds the reasons still exists — and a
bypass leaves a visible debt (R-209, D-093).

## 8 · A knowledge base is an assistant's memory

The second profile turns the same mechanics into a personal brain: notes land
in `raw/inbox/` in the person's own words; ingest distils them into wiki pages
and archives the note byte-for-byte (`raw/` is content-immutable, relocation
is the expected flow); a cross-check reads pages against their sources when
the person asks; lookup answers with the page or says the base does not have it. What the base
cannot settle is one dated line in `wiki/open-questions.md`. Forgetting makes
a topic unknown, never unrecorded; the character is set on the first turn, in
the person's language; every file under `wiki/` keeps the base's language,
whatever the conversation speaks.

## 9 · Learning from projects, and staying current

A base consumes projects: their exported pages are materialized under
`.federation/` and cited as `@namespace/id`; the git connector lands one
record per commit worth reading through the same write gate as any note, and
never the same item twice; `status` is the digest an assistant reads first;
`docsys assistant` does all of it in one command. When a consumed source moves,
`docsys crosscheck --since` names the pages that rest on it. Connectors to mail,
calendars and chats need what docsys refuses to carry — network, tokens,
schedules — so they live in another project and meet docsys at one seam, the
write gate (ROADMAP.md).

## 10 · Brownfield: history is evidence, not documentation

A repository with years of commits and no docs is seeded from what history
can prove — births, scopes, manifests, root causes, the code's own comment
blocks, tags — and nothing else. The builder confirms, corrects and adds what
history cannot say; an answer that conflicts with the evidence becomes a
question, not a record. When nobody can answer, the evidence rows still land,
and the session may author one page per feature from the evidence —
readable on day one, never the tool's claim.

## 11 · One tree at a time, one seam between trees

Trees couple only through references (§7); federation is fetch-direct over
paths and git, materialized locally, composed only from local state; export
composes documents for a declared audience and refuses to half-compose. A
foreign page is never edited by hand, and a stale composition shows its age.

## 12 · Zero dependencies, deterministic, offline

One binary, no crates, no network, no timers, no secrets. Every check gives
the same answer on every machine for the same tree — including the hashes.
This is what lets CI run the whole harness in minutes and lets a person trust
a finding without a service behind it.

## 13 · Spec first, then code, then the test that pins it

Every behaviour lands in this order: a sentence in `SPEC.md` (or a decision in
`corpus/DECISIONS.md` when the spec already allows it), the code, the test —
a corpus case for lint rules, an integration test for git-backed behaviour,
an `e2e.sh` step for a first-run flow. `CHANGELOG.md` and `README.md` are
part of the same change. A rule without a check is a debt (R-010); a check
without a rule is a bug.

## 14 · The system is tested against itself, not assumed

`ci/agent-lab/` holds two ways of testing the flows that matter: a mechanical
harness of exact expectations on dated, reproducible fixtures (what the binary
guarantees), and headless agent sessions on the same fixtures and on real
repositories (what an agent achieves with only the installed layer). Task
texts are what a person would say. Every gap the lab finds goes back into
docsys — as a decision, a rule, a sentence in the installed text — and the
finding stays in `ci/agent-lab/FINDINGS.md` with its evidence.

## Where each idea lives

| idea | spec | decisions |
|---|---|---|
| two layers | §3, §5 | D-030 |
| the tool never writes prose | R-003, R-090, R-156 | D-024, D-053 |
| mechanics vs judgment; the installed layer | §14.3, R-155 | D-051, D-087 |
| honest over complete | R-151, R-152 | D-032 |
| freshness by hash | §11 | D-066–D-073, D-119, D-122 |
| a page is read, not stamped | §3.2, R-024 | D-130, D-131 |
| the human word; the commit gate | R-081, R-097, R-209 | D-040, D-043, D-093 |
| the knowledge base | §3.1, R-023–R-029 | D-030, D-076, D-083, D-084, D-090 |
| learning from projects | §13, §20 | D-074–D-082 |
| brownfield seeding | `/docsys-seed` | D-053, D-054, D-091, D-092 |
| federation and export | §7, §13 | D-032–D-034 |
| zero dependencies | D-001 | — |
| spec first | R-010, §17 | — |
| the lab | `ci/agent-lab/README.md` | D-087 |
