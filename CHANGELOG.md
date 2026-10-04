# Changelog

All notable changes to docsys are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/). Release notes are extracted from this file
by the release workflow — the tag's section becomes the GitHub release body.

## [0.16.1] - 2026-10-04

### Upgrading from 0.16.0

- On a docsys/0.5 tree whose CI installs the release archive, run
  `docsys upgrade --apply --commit` once: it reads the four archives' sha256
  values from the 0.16.1 release and writes them, with the version, into the
  workflow's install step — that step alone, shown as a diff in the plan — so
  the pull request that carries the upgrade carries the values for review.
  Without network it leaves the workflow as it is, applies the rest and names
  the command to run again.

### Changed

- A docsys/0.5 release install holds the version `docsys upgrade` pinned and
  the sha256 values it read from that release, and checks the archive
  against them; nothing is read from the release when CI runs. A
  `.docsys-version` that names another version — a pull request that moves
  the pin alone — installs nothing, and the step says so in one line (D-111,
  D-120).
- `adopt --ci-install release` reads this version's values from the release
  the same way; without network it writes no workflow and says why.
- The release publishes each archive's `.sha256` file, which the upgrade
  reads, and no `SHA256SUMS` any more (D-111).

### Added

- `DOCSYS_RELEASES` names where `adopt` and `upgrade` read a release's
  `.sha256` files: a mirror, or a `file://` directory.

### Fixed

- `upgrade --apply` in a clone whose git runs no hooks (`core.hooksPath`
  naming no directory) says so once and applies the rest of the move; it
  stopped the whole move before (D-117).
- The upgrade knows the relays, commands, skills and knowledge-base contract
  0.16.0 wrote: untouched, they are refreshed, where they read as their
  owner's before; a relay that differs in its template line alone says so.
  A test holds every text a version writes in the registry of released
  texts, so a release cannot ship without it (D-117).
- The note a 0.4 tree's move prints describes the version that prints it,
  under its number: it is data beside the move's steps, where 0.16.0's
  CHANGELOG entry, kept as history, still promised a SHA256SUMS check and no
  later edit to CI (D-120).

## [0.16.0] - 2026-10-04

Every change below to a finding or a file format applies to a tree that
declares `spec: docsys/0.5`; a tree that declares `docsys/0.4` is served as
0.15.1 served it (D-118), but for the values 0.15.1 misread, which one
reader now reads alike under every spec (D-002). On a 0.4 tree
`commit_policy: require   # …` is `require` to the gate and the relays, a
pin's `path:` or `hash:` followed by a comment raises no R-111 or R-113, an
inline list closed before a comment is read and its entries checked (R-059),
and an inline list that never closes is named — R-050 in a page, R-161 in
`.docmeta.yml` — instead of taking the next fields, so a page whose
`sources:` swallowed `updated:` and `verifies:` loses that R-050 and shows
the R-111 they hid. While such a list stays open, a writer of that file
refuses, and an unclosed `maintainers:` names nobody, so `verify` refuses.
Lint's R-160 names a file it cannot read, and `debt add` or
`question add` refuses a ledger it cannot read instead of writing over it.
R-059 names the forms a source takes. Exit statuses and messages follow this
release on every tree.

### Upgrading from 0.15.x

- Nothing has to move on the day 0.16.0 is installed: a docsys/0.4 tree keeps
  its findings, its file formats and its hooks.
- Every clone must install docsys >= 0.16.0 before pulling this change; a gate
  under .git/hooks cannot warn an old binary.
- `docsys upgrade` prints the plan; `docsys upgrade --apply --commit` moves the
  tree in one commit. The items it lists for a person are theirs to finish.
- docsys/0.5 keeps no page verification: the move takes every verification
  field out of every page and carries none, `.docmeta.yml` names no
  maintainer, and `verify`, `page new --unverified` and an approval job are
  refused there. To have pages checked against their sources and code, a
  person runs `/docsys-crosscheck`; drift between code and a page still
  blocks through its pins (R-111, D-130, D-131).
- CI stops naming a docsys version or a sha256: `docsys upgrade` replaces
  what installs docsys in your workflow — the `env:` lines with the version
  and the archives' sha256 values, and the install step — with an install
  that reads `.docsys-version` and checks the archive against the
  `SHA256SUMS` its release publishes. The plan shows the diff; runners,
  triggers and every other step stay as you wrote them, and no later upgrade
  edits CI again.
- A branch opened before the move runs the same `docsys upgrade --apply
  --commit` once before it merges: it converts the branch's own additions the
  same way, and only a topic both sides added to can conflict. Merged without
  it, git stops on the old ledger (modify/delete) or carries its lines into
  the frozen slice and journal: keep the branch's ledger and commit, lint names
  the upgrade, and a re-run moves each item once and carries the branch's
  journal entries into its commit's message.

### Added

- `docsys upgrade [--apply] [--commit] [--force]`: a docsys/0.4 tree to
  docsys/0.5 in one commit; what needs a person is listed, never applied (D-117).
- `docsys upgrade` on a tree already at this version is the leftover check:
  it lists what an earlier version left, each with its file and its fix — a
  CI workflow whose install names a docsys version or a sha256, the route
  lines an earlier 0.16 build appended, the post-edit relay and its wire, the
  knowledge base's audit skill, `updated:` lines and verification fields in
  pages, a `verifies:` list or a `.verifies/` directory, a `maintainers:`
  list, a journal or ledger a late branch brought back, an agent layer below
  the repository's top, team text naming a retired concept, and the
  upgrade's own record left in the git directory — and a tree with none says
  so in one line (D-117, D-130).
- `upgrade --apply`, with `--commit` or without, ends with the leftover check
  on the moved tree, and `/docsys-upgrade` ends by walking the person through
  that list and lint, and names once what only the person decides (D-104,
  D-117).
- `/docsys-upgrade` and `docsys upgrade --json`: an agent finishes the items
  the upgrade lists for a person, with the person (D-104).
- `docsys crosscheck [<page>…] [--since <ref>] [--json]` and `/docsys-crosscheck`:
  on docsys/0.5 a person has an agent read pages — the ones named, or every
  permanent page changed since a revision, committed or not, or whose
  consumed `@namespace/id` source moved since then — against their
  `sources:` and the code their pins resolve to now. The command prints each
  page, its sources and each pin's lines (`path:L<first>-L<last>` for a
  symbol, by the resolver lint uses; the whole file; or the resolver's own
  refusal) and writes nothing; the agent corrects what the code or a source
  contradicts in one ordinary commit and records what it cannot settle with
  `docsys question add` or `docsys debt add`. The command comes with the agent
  layer of either profile, and `docsys upgrade` writes it where it is absent
  (D-131).
- `upgrade` lists each line of a team's own instructions, rules, skills and
  commands that still names a concept the move retires — the journal file, a
  ledger, `updated:`, a verification field, the post-edit relay — with what
  replaces it, and edits none of them; `/docsys-upgrade` proposes the rewrite
  (R-175, D-128).
- A tree pins the docsys it runs (`.docsys-version`): every command on it runs
  that version, installed once on first use; the relays, the gate and the CI
  workflow read the pin, and `upgrade` moves it (D-120).
- `docsys --version`: the binary and the spec it implements (D-099).
- `docsys --help` says what each command is for; `docsys <command> --help`, or
  `docsys help <command>`, gives its flags and an example, and so does an
  unknown flag or a group named alone (D-129).
- `docsys pin --gc`: removes the pin acknowledgements nothing needs (D-119).
- `docsys ledger fix`: em-dash ledger markers to R-108's ASCII (D-108).
- `docsys feedback [--draft]`: the issue format and a drafted issue; it files
  nothing, and refuses a draft's flags without `--draft` (D-116).
- `adopt --rules-file`, `--report-dir`, `--no-report` (D-110); `--ci-runner`,
  `--ci-install cargo|release` (`--ci-sha256` on a docsys/0.4 tree only),
  `--verify-on-approval pull-request|direct|off` on a docsys/0.4 tree (D-105,
  D-111, D-130).
- Every release publishes `SHA256SUMS` beside its archives: the release
  workflow hashes the archives the release holds and uploads the list, with
  no hand step (D-111).
- `adopt --ci-install release` on a docsys/0.5 tree: the release archive of
  the version the tree pins, checked against that release's `SHA256SUMS` with
  `sha256sum -c`, or `shasum -a 256 -c` where it is absent; no version and no
  sha256 is written into the workflow, and `--ci-sha256` is refused by name
  (D-111, D-120).
- The tree's own uncertainty markers and history headings are reported
  (R-210, R-211, D-115).
- `ci/compat.sh` and `ci/pin-replay.sh`: manual checks against the previous
  release and against a real clone.

### Changed

- No native Windows build is released any more, and CI tests Linux and macOS.
- A docsys/0.5 page carries no verification, in either profile: no field, no
  record, no approval and no maintainer, nothing about it in lint, status or
  lookup, and no approval job in the CI workflow. `docsys verify` is a
  docsys/0.4 tree's command; graduation's confirmation is the word of whoever
  runs it; the knowledge base has no audit skill. A person who wants pages
  checked asks for a cross-check (R-024 narrowed; R-025, R-028, R-208 and
  R-212 withdrawn; D-130).
- A pin 0.15 read as fresh stays fresh after the move, acknowledged at the
  region 0.5 reads or, where 0.5 finds no declaration of its symbol, pinned to
  the whole file, so a tree that linted clean lints clean (D-117, D-119).
- Of an owner's CI workflow the upgrade replaces only what installs docsys:
  the `env:` lines that name its version or sha256 values, with the comment
  right above each, and the install step, which becomes this version's and
  reads the pin. Runner labels, triggers, timeouts, other steps and other
  comments stay as written; the plan shows the diff, and an install it cannot
  tell apart is named at its line and left alone (D-111, D-117, D-120).
- An untouched release-install workflow from an earlier build is regenerated
  without its sha256 values (D-111).
- The step every clone takes after pulling an upgrade is said by the rules
  block alone, no longer by the upgrade's output and commit message (D-117,
  D-129).
- A page lists its pins in `pins:` (a docsys/0.4 page keeps `verifies:`), and
  a pin's evidence lives beside the page under `.pins/`; a refresh never
  writes the page, and pull requests that refresh pins merge cleanly; the
  post-edit relay leaves with the dates and the verification it kept (D-119,
  D-130).
- A pinned symbol resolves to its declaration, never to a use (D-106); in
  TS/JS a function written as a value — a property `name: (…) => …` or
  `name: function (…) {`, or a member assignment `a.name = function …` —
  declares its name where nothing else in the file does.
- The journal is the commit history: `docsys journal` renders every commit
  that changes the docs or carries a `Docs:` line, newest first, and
  `journal add` prints the `Docs:` line the commit ends with — a message of
  that line alone is one too; nothing is written beside history.
  A new `commit-msg` gate reads that message: under `require`, code with no
  documentation needs `Docs: <why>`, a removed item needs its trailer, and a
  `Docs:` entry over five lines is said; under `ask` it says nothing. `upgrade` moves `work/journal.md` and its slices under
  `_archive/journal/` byte for byte (§10, R-100, R-101, R-104, D-125; R-103
  and R-105 withdrawn).
- Debt and questions are directories of topic files: each open item is a line
  in the file of the page or feature its `[topic]` tag names, `general`
  without one, written by `docsys debt add` and `docsys question add`
  (`--topic`); closing removes the line and prints the `Resolved:` or
  `Answered:` line its commit carries, and `upgrade` groups the old ledgers'
  open items by tag and moves the rest, as written, under `_archive/`; a link
  to the list by its old name, `[[work/debt]]`, resolves to the list (R-071,
  R-108, D-124). D-109's vanished-item check is retired
  with the ledgers.
- Graduation ends by removing the work file: `docsys graduate apply --plan
  <file> --confirmed <who>` moves the last blocks byte for byte, removes the
  source, and prints the commit message, which names the destinations and
  carries `Confirmed-by:`; history keeps the file. It is refused while a block
  outside the sections R-049 retains would leave with the file. Without
  `--confirmed` the file stays as before;
  `upgrade` removes no graduated or archived work file (R-049, R-066, R-081,
  R-091, R-093, D-127).
- A router line may route a directory, and every page under it is reachable;
  the layout routes the four type directories, so a new page there adds no
  line to `index.md`, and nothing writes a route to a type directory (R-034,
  R-035, D-123).
- A page's date is its last content change in history: nothing writes
  `updated:`, a page that still carries it is reported, and `upgrade` removes
  the lines (R-050, D-122; R-052 and R-106 withdrawn).
- R-108 reads ledger separators as written (D-108).
- A code-side `doc:` counts at the start of a comment only (R-072, D-107); a
  wiki-link inside inline code is quoted material (R-071).
- A project's `raw/` is a record layer (R-023, D-112).
- The agent text carries the procedures an agent needs while it writes a
  page, and the first turn routes intents to commands; the rules
  block, the skill, the routing and help each say a thing once (D-114, D-129).
- The CI workflow `adopt` writes is pinned, least-privileged and regenerable.
  A docsys/0.5 tree's has no approval job; a docsys/0.4 tree's records the
  approval in a follow-up pull request (`pull-request`, the default there) or
  a push (`direct`), and a mode the tree cannot read is refused (D-105,
  D-111, D-130).
- `adopt` meets a repository as it is: the tree, the rules block and
  `ADOPTION.md` stay where they are, and `init` writes only absent files (D-110).

### Fixed

- The CI workflow names the repository's own base branch when no remote says
  which: a local `main` or `master` before the branch checked out.
- On a docsys/0.5 tree `adopt` reports only what is so: no docsys asset in the
  inventory, no error items on a clean tree, no `## Last run` and no byte
  around the rules block changed by a re-run that changes nothing (D-110).

- docsys works from any directory of the repository, and a hook finds its
  tree from its payload (D-098): a given `--repo` is the repository's top, a
  relative `--dir` or `--root` is read from there, and a path a command prints
  is named from there — `refs`, `backlinks`, `graph` and `migrate` read the
  whole repository (bare `backlinks` and `graph` the tree's own), `agents` and
  `compile` write the one agent layer at the top, `rules --agents-md --write`
  carries the tree's preamble — and `debt close` or `question close` outside a
  tree says R-160.
- An unknown command is named and exits 2, `help` of one too; a flag's value
  is never another flag (D-129).
- A reader that closes its end early (`| head`) ends a command quietly with a
  closed pipe's status (141), never a panic; `lint`, `refs`, `gate` and the
  relays' `hook` keep their verdict as their exit status, whoever runs them —
  a git hook, a relay, or the same hook run by hand.
- A command refuses a flag another command owns, with its own entry, instead
  of ignoring it; `help hook stop` answers with the hook's entry (D-129).
- `upgrade --apply --commit` commits its move before it prints, from any
  directory of the repository; a commit git refuses says how to finish, and a
  re-run commits a move a bare `--apply` or a refusal left written (R-177).
- `adopt` run again on a docsys/0.4 tree writes what 0.15.1 writes there,
  its rules block and git gate included, and `agents --kb` refuses a project
  tree (D-118).
- A graduation plan's destination may carry its `.md` (R-099).
- The gate of a linked worktree resolves pins at the repository's top, and
  `adopt` writes the gate there (D-100).
- The relays run from any directory and default to the tree's own root (D-099).
- A derived file name folds a Latin letter to its base (D-113).
- An unresolved `sources:` entry names the forms that resolve (R-059).
- The usage text names the real default root; the workflow for a tree at the
  repository's top passes `--root .`.
- Every `.docmeta.yml`, frontmatter and provenance value is read one way: a
  trailing comment ends every value form, so `spec: docsys/0.5   # required`
  is a 0.5 tree to the gate as to lint, and a commented `commit_policy:` or
  `profile:` is its value; writers replace a field by the lines the reader
  joined to it, the feedback draft redacts the parsed fields, and `adopt`
  stamps a `.docmeta.yml` without a spec line with the era it is read as,
  docsys/0.4 (D-002, D-118).
- The command table reads every command line: a single-dash flag, a stray
  word and a flag the entry does not name are refused, named, with the entry;
  a flag given to a group is named; `--version` and `-V` take what `version`
  takes; `help <command> <words>` answers as `<command> <words> --help`
  (D-129).
- The relays, the gates and the upgrade say each thing once and only what is
  true for the mode: a re-run once, a `git add` only when the blocked call
  ran one, the commit relay's header says what it does under
  `commit_policy: require`, the commit-msg hook says what its half stops, an
  applied upgrade prints no forecast of findings, and a docsys/0.4 tree's
  checklist names no 0.5 step (D-129).
- `agents`, `agents --kb` and `assistant` write a layer through one installer:
  `agents --kb --root .` on a project is refused, and an assistant's base
  inside a project's repository leaves the project's git gate as 0.15.1 left
  it (D-098, D-118).
- A skipped commit (`DOCSYS_SKIP=1`) under `commit_policy: require` leaves a
  debt item for each check it carried past the gate — code with no
  documentation, a lint error — and says something only when an item cannot
  be written, and then says why; a lint error the person skipped no longer
  prints a pointer to a reason nobody sees (R-209, D-093).
- `upgrade --apply` in a clone with no git gate writes the one `adopt`
  writes, hard while the tree lints clean: the step every clone takes after
  pulling a move leaves the leftover check clean (D-117).
- A `git add` in the call the commit relay sees takes untracked files too: a
  new code file staged and committed in one call is asked about (D-040).
- On docsys/0.5 the relays name the bypass as the person's —
  `DOCSYS_SKIP=1 git commit` at their terminal — since they refuse it in the
  agent's own command (R-209).
- `migrate inventory --repo .` lists the inbound references from the tree's
  own directory, `migrate apply` from below lints the tree it migrated, and
  `doctor` names a missing agent layer and its relays from the top (D-098).
- An upgrade commit holds the move's files and no other: a change staged
  beside a pending move is refused by name, a move committed by hand leaves
  no record that a later run would take for its own, and a path git ignores
  stays out; an edit of the person's to a file the move writes — staged,
  unstaged, or made after a bare `--apply` — is refused, `--force` or not,
  naming each file and writing nothing (R-177, D-117).
- The plan and the move refuse a tree whose `.docmeta.yml`, a page or a
  template holds an inline list that never closes, naming each file, field
  and line, and write nothing (D-002, D-117).
- A `.docmeta.yml` with no readable `spec:` line declares the new spec after
  the move, as the plan says (D-117, D-118).
- The upgrade says each thing once: the plan leaves to the release note how
  it is applied, the post-edit relay's reason is said on one row, and a tree
  with nothing to do is not offered `--apply` (D-129).
- A list that never closes with `]` is named by its field and its own line,
  in `.docmeta.yml` too, and never takes the next field's line as an item;
  every writer of a frontmatter that holds one refuses by file, field and
  line and writes nothing (`pin`, `pin --refresh`, `pin --gc`, `raw move`,
  and `verify` on a docsys/0.4 tree), `raw move` and `forget` refuse while a
  page's `sources:` cannot be read, and `fetch` refuses an unclosed
  `consume:` (D-002).
- On a docsys/0.4 tree an unclosed `maintainers:` list names nobody rather
  than declaring no maintainers: `verify` refuses by its line (R-208, D-002).
- `debt add` reads `.docmeta.yml` and its own file and no page, so a file it
  cannot read elsewhere no longer blocks it; a topic file or docsys/0.4
  ledger `debt add` or `question add` cannot read is refused by name and
  left as it is; `debt close` and `question close` name a topic file they
  cannot read instead of saying there is no open item, and close an item in a
  file with CRLF line endings.
- Lint's R-160 line names the file the tree could not read.
- A flag's value is never a single-dash flag either: `-h` after a value flag
  answers with the entry, `-V` or `-x` there is refused by name, and `-1` and
  `-` stay values (D-129).
- A bad invocation exits 2, as the exit table says, never 1, which is
  blocking findings alone: a missing or wrong argument, or one that names
  nothing the tree holds, to `backlinks`, `mentions`, `graph`, `debt add`,
  `debt close`, `question add`, `question close`, `ledger fix`, `journal
  add`, `page new` or `seed`; `pin` and `backlinks` name themselves once.
- `docsys hook` without an event names the word it needs and exits 2, and a
  word after the event is refused alone, never listed among the events to
  choose from (D-129).
- The version notice, `doctor` and graduation say what a tree's `spec:`
  declares: a tree that declares no spec, or a value that is no
  `docsys/0.<minor>` version, is told it is served by docsys/0.4's rules,
  never that it declares docsys/0.4 (D-118).
- `docsys help adopt` names the skills and slash commands adopt writes.
- Each fact is said once: `docsys --help` lets adopt's row say where a
  repository starts, the feedback guide leaves the environment to its
  template's line, and `/docsys-seed`'s overview step points at the evidence
  step 2 presented (D-129).
- SPEC's R-076 and R-101 cite R-151, which absorbed the withdrawn R-150.
- A shell pin's region keeps `$#`, `${#a[@]}` and `v1#x` as code: only a `#`
  where a word starts is a comment (D-119).
- `impl Trait` in a function's signature is no impl block to a symbol pin.
- `--topic` adds its tag to an item whose text starts with a wiki-link.
- `doctor` on a docsys/0.4 tree reads the gate block 0.15.1 wrote as the
  tree's own (D-118).
- `ci/pin-replay.sh` stopped early leaves no worktree in the clone.
- A commit or a merge through the git gate says each finding, the version
  notice and a pinned version's install line once, with one install attempt;
  a docsys from before pins prints one line, never its usage; a skipped commit
  whose record cannot be written says so; a retried commit says it again
  (R-171, D-116, D-118, D-120, D-125).

## [0.15.1] - 2026-09-04

### Fixed

- `ADOPTION.md` keeps the adoption's own record (D-097): a later `docsys
  adopt` no longer turns "created … merged 4 hook wire(s) … gate written"
  into "kept … already wired" — the first `## Done` block stays as written
  (`## Done — adoption (<date>)`) and the re-run reports itself under
  `## Last run — <date>`, one block, replaced each time. A report written
  before this keeps its `## Done` lines as the adoption record.

## [0.15.0] - 2026-09-04

### Added

- `docsys raw move <record> <domain>` (D-085): a note leaves `raw/inbox/` for
  `raw/<domain>/` through git, bytes untouched, and every citing page's
  `sources:` entry is rewritten in the frontmatter — the body is not touched,
  so a verified page stays verified. R-027 has its command; the ingest
  skill, the first-turn routing and the record guard name it instead of
  `git mv`.
- A JSON writer beside the hook-payload reader (`Json::render`), so the
  binary can write back a settings file it parsed.
- R-082 has its check (D-089): inside a repository, a work file HEAD holds
  as `status: graduated` whose status, body or existence changed in the
  working tree is an error at the gate — `graduated` is terminal.
- `wiki/open-questions.md` is the knowledge base's questions ledger (D-090):
  a list file under R-108's grammar (`- [ ] YYYY-MM-DD …`), so a free-form
  rewrite is an error and `status` counts what the organs recorded; the
  installed texts name the grammar and say that every file under `wiki/`
  keeps the base's declared language — only the conversation follows the
  person's.
- The agent lab, in the repository (`ci/agent-lab/`): dated, reproducible
  fixtures for the four distillation flows (a knowledge base with an inbox,
  a project with finished work files, three provider projects, a brownfield
  repository with seven years of history), a mechanical harness of exact
  expectations (`mech/run.sh`, ~200 checks, run by `ci/e2e.sh` step 14), and
  the headless-agent leg (`agent/`): one-line task texts, a rubric that names
  where the agent read each expectation, a runner with mechanical capture and
  automatic rows, the sonnet/opus matrix, the real-repository leg by
  codename, and the two reports. `run-all.sh` runs it end to end.

### Changed

- The pre-commit gate's mode follows the gate's own verdict (D-088): hard
  only when neither lint nor `refs` reports an error — a dangling `doc:` in
  code left a freshly adopted repository unable to commit its adoption.
- `adopt` and `agents --kb` merge the docsys hook wires into an existing
  `.claude/settings.json` instead of leaving it untouched (D-086): each entry
  whose command is not yet wired is appended to its event; MCP servers,
  permissions and the owner's own hooks keep their place and order; a second
  run changes nothing. A file that is not JSON is still left alone, with the
  snippet on the `ADOPTION.md` checklist.
- The installed knowledge-base layer says where sources beyond the inbox
  come from (D-087): `AGENTS.md` gains "Sources beyond the inbox" (consumed
  projects and `@namespace/id`, the git connector `inbox pull`, `status`,
  `assistant`), and the ingest skill's step 3 names `@namespace/id`
  citations. Nothing an agent needs to run a base lives in a prompt.

### Fixed

- `seed plan --target <feature>` prints its escape line (`nothing in history
  names this feature`) whenever no commit is attributed to the feature — a
  word inside a commit body was counted as a hit and returned an empty
  skeleton with `commits 0` instead; the mention is now named as a mention.
- `tests/jarvis.rs` dated its base pages with a literal day and failed the
  morning after (R-106); the pages carry the run's own date.
- The installed git pre-commit gate masked a lint failure whenever `docsys
  refs` passed after it (no `-e`; the last command decided) — every hard gate
  let a red tree commit. The block now accumulates the three statuses and
  exits on any failure in hard mode; `adopt` rewrites an older block in place
  (`git pre-commit gate: upgraded`). Found by the agent lab's mechanical
  harness.
- `--since YYYY-MM-DD` (`inbox pull`, `seed plan`, `assistant`) is the start
  of that day: git reads a bare date as "that day at this hour", so
  `--since <today>` landed nothing.
- `docsys assistant` on a directory inside another repository says so
  (`git: inside repository …`) instead of silently sharing that history.
- `/docsys-seed` §3 says what a conflicting answer becomes: a `question`
  row naming the evidence, not an `answer` row — the lab saw one model
  record the disputed claim verbatim and raise the question beside it.
- The maintainer in the session needs no second session (D-096): R-025 keeps
  a model from certifying its own page, but when the person driving the
  session is a declared maintainer and says the page is right, the agent
  records that word with `docsys verify <page>`; the installed texts say so.
- A code review's approval verifies (D-095), read from git: `docsys verify
  --range <a>...<b> --from-trailers --commit` takes the approver from a
  `Reviewed-by:`/`Approved-by:` trailer whose e-mail is a declared
  maintainer's and records every page the range touched under that identity
  — any host, no network. Where the approval lives in a host's review table,
  `--by @login` takes it from an adapter (the login on the maintainer entry:
  `handle <email> @login`); the GitHub workflow `adopt` writes is one, run
  once per declared approver when a pull request merges.
- `docsys verify <page> [--by <handle>] [--commit] [--revoke]` (D-094): a
  maintainer's record in one step — the handle from the git identity matched
  against `maintainers:`, the revision from `HEAD` once the page is committed
  as it is, refused while a source does not resolve; `--commit` lands it under
  the person's own identity, `--revoke` takes a page back to `unverified`.
- `commit_policy: require` (D-093, R-209): a commit that touches code with
  no documentation change is refused — by the agent relay and by the git hook,
  every time — until the work is recorded as a work file or a journal entry;
  the end of a turn holds the session once when code changed without its
  record, so the knowledge is captured while the session that has it exists;
  `DOCSYS_SKIP=1` still bypasses but leaves a dated debt item
  (`docsys gate --skipped` records it for the git hook). The first turn now
  carries `<docs-in-hand>` — pages, unverified count, work in flight, the
  policy — and names `improvement` among the work types. The stop relay
  passes its payload (`--stdin`); `adopt` upgrades an older gate block with
  the `docsys gate` line.
- Verification in the project profile, and maintainers (D-092, R-208): a
  permanent page may carry `verification:` and `sources:` and then answers to
  the wiki page's contract (record, body hash at `verified_rev`, sources
  resolving); `docsys page new <type> <id> --unverified` writes it.
  `.docmeta.yml` may declare `maintainers:`; then `confirmed:` and
  `verified_by:` must name one of them, and — where history exists and the
  entry carries an email — the commit that recorded it must be theirs. Anyone
  writes, a maintainer vouches; an empty list changes nothing. `status` counts
  a project's unverified pages; the routing text and the docsys skill say it.
- `/docsys-seed` §4b: the one page the seeding session may author — an
  `unverified` `explanation/<feature>-overview` from the evidence, routed,
  for a maintainer to verify — so a repository seeded with nobody present is
  readable on day one without a claim of truth.
- The gate blocks a commit that carries a seed plan file (`SEED.tsv`,
  `*.seed.tsv`) and names the two commands that replace it (D-091) — a plan
  is a draft, never documentation; two real-repository sessions had
  committed it.
- `/docsys-seed` §4 names the absent-builder case: `research`, `journal`,
  `postmortem` and `question` rows land on the person's word alone, only
  `answer` rows wait for a builder — a session on a real repository with
  nobody to ask had committed the plan file and landed nothing.

## [0.14.0] - 2026-09-02

### Added

- The assistant's character (D-083): `AGENTS.md` of a knowledge base carries
  a `## Character` placeholder; the first turn of a fresh base puts a
  `<first-run>` survey before the organ routing — name, address, tone,
  languages, never-do, defaults offered, in the person's language — and the
  answers replace the placeholder. The routing text now says: speak the
  person's language, turn by turn; pages keep the declared language.

- `docsys forget <page|record> --reason <text>` (D-084): a page to
  `_archive/` with a tombstone, its router line and compiled skill gone; a
  record to `raw/_forgotten/`, still immutable, never read and never captured
  again; the ledger `.forgotten.yml` says when and why; `status` counts it. A
  record a page cites is refused until the page is forgotten.

### Fixed

- `ci/e2e.sh` runs on the macOS leg again: no `sed -i`.

## [0.13.0] - 2026-09-02

An assistant's memory: a base that learns from the trees it consumes, a
write gate for connectors, and the digest.

### Added

- `sources:` accepts `@namespace/id` (D-078): a page distilled from consumed
  pages cites them by identifier; the citation resolves against the local
  materialization and is an error until `consume add` + `fetch` land it.
- `docsys inbox add --source <name> --id <item> [--title] [--url] [--date]
  [<file>|-]` (D-079, §20): a connector's record into `raw/inbox/` with
  provenance frontmatter; the same `(source, source_id)` lands once.
- `docsys inbox pull <repo> [--since <date>] [--as <ns>]`: the git connector,
  one record per commit, idempotent.
- `docsys status [--json]` (D-080): the digest — inbox, pages by state, open
  items, consumed namespaces, compiled skills, findings by rule. Derived,
  never stored.
- `docsys assistant [--root .] [--projects <dir>]… [--domains a,b] [--since
  30.days] [--limit 3]` (D-081): an assistant's memory in one command — the
  base, its agent layer, every docsys project one level under the given
  directories consumed and fetched, their recent commits as records, the
  digest. Idempotent. Another knowledge base found there is skipped.
- A verification is checked against its consumed sources (D-082): a
  `verified` page whose `@namespace/id` source moved since `verified_rev`
  (the provenance hash at that revision differs from the current one) is an
  R-024 error; a materialization with no committed baseline is an R-028
  error. `status` reports "sources moved" separately. This is how a base
  stays current: `fetch` brings the new hash, lint names the pages that
  rested on the old one.
- The git connector skips bookkeeping commits (no body, nothing outside
  documentation, manifests and the agent layer) unless `--all`.
- `refs` never scans `.federation/`, and in a base that is its own repository
  it scans code only — its wiki pages were reported as strays before.
- SPEC §20 Connectors (EXPERIMENTAL): the record, the boundary (no secrets,
  no timers in the tree, no outbound action by the tool), the built-in git
  connector and the design table of connector kinds.

## [0.12.0] - 2026-09-02

### Added

- `docsys compile <howto> [--dir .claude] [--force]` (R-094): the page body
  becomes `.claude/skills/<id>/SKILL.md`, byte for byte, with `docsys_source`
  and `docsys_source_hash` on the skill (R-095, D-073). Only a `howto`
  compiles; in the knowledge-base profile only a `verified` one. `lint`
  inside the repository errors when a compiled skill's page moved or is gone.

- `docsys lookup <word…> [--json]` (D-074): a question's first hop — every
  page, local and consumed (`@namespace/id`), naming every word, scored by
  where the words occur, drafts and unverified pages flagged; `raw/` never
  listed; no hit exits 1 with "not in the base".
- `docsys consume add <path|git-url>[#subdir] [--as <ns>]` and `docsys
  consume discover <dir>` (D-075): the provider list grows in this tree's
  own `.docmeta.yml`; `discover` lists candidates and writes nothing.
  `adopt` writes `namespace:` into the tree's docmeta once. No registry
  outside the repository.
- The kb-lookup skill and the docsys skill start a question with
  `docsys lookup`.
- The knowledge-base hook layer (D-076): `docsys agents --kb` installs the
  four relays, `settings.json` when absent, and the git pre-commit gate. In
  a base the PreToolUse relay blocks a `Write`/`Edit` on an existing `raw/`
  record (R-023), the first turn names the organs, `updated:` is bumped on
  wiki pages only, and the end of a turn names the notes waiting in the inbox
  and the errors the gate will stop.
- Lint checks a verification against the body it verified (D-077): a
  `verified` page whose body no longer hashes to what it held at
  `verified_rev` is an error under R-024; a revision that does not hold the
  page is an error under R-028.

### Changed

- R-095 blocks (D-070): a stale compiled skill is an error until recompiled.

## [0.11.0] - 2026-09-02

The four gaps between "kept honest by an agent" and "kept honest
mechanically", closed.

### Added

- `verifies:` pins (§11). `docsys pin <page> <path> [--symbol <s>]` writes a
  code region and its SHA-256 into the page; `docsys pin --refresh <page>`
  recomputes every pin after the author re-read the page. `lint` recomputes
  the hashes on every run inside a repository and a moved region is an error
  (R-111). SHA-256 and the canonical form are in the binary, zero-dep;
  symbols resolve as brace blocks, or `def`/`class` blocks in Python, and an
  absent or ambiguous symbol is an error rather than a guess (R-114, D-068,
  D-069). The frontmatter grammar gains the block list of flat maps.
- History-derived freshness (D-071): one `git log` walk dates every page.
  `updated:` behind the page's last commit (R-106) and a `draft`, `active` or
  `done` file untouched beyond `stale_active_days` (R-085, default 90) are
  errors. `lint --repo <dir>`; the repository is detected when omitted.
- `docsys gate --range <a>...<b>`: the code-without-docs question over a
  commit range — a pull request — failing when unanswered.
- `adopt` writes `.github/workflows/docsys.yml` when the repository has a
  `.github/` (lint and refs on every push, the range gate on a pull request),
  and the git pre-commit gate is hard when the tree lints clean inside its
  repository; a warn-mode gate is hardened in place by a later `adopt`
  (D-072).

### Changed

- R-085, R-106 and R-111 block (D-070); R-085 covers `draft` and `done`
  besides `active`.
- The agent block and the docsys skill name the pin discipline: a stale pin
  is re-read against the code, then refreshed — never refreshed blind.

## [0.10.0] - 2026-09-02

Findings of an agent lab: three sample repositories adopted, fifteen headless
agent sessions (feature · bug · research · refactor · idea), every write and
every hook observed.

### Changed

- `lint`: R-061's uniqueness domain now includes tracked work pages. A draft
  under `work/` that claims a permanent page's identifier is an error named on
  the draft (D-067); corpus case 38. Before, lint passed and `backlinks`
  silently picked the permanent page.
- `lint`: a list item without a checkbox in `debt.md` or `questions.md` —
  `- text`, before or after the first item — is an error (R-108 text
  clarified); corpus case 39. Before, a debt written that way was invisible to
  every check.
- Dates are the local day (D-066): `DOCSYS_TODAY`, then `date +%F`, then the
  UTC civil day as the floor. Before, an evening session got yesterday on
  `updated:` and today in the journal.
- Stop hook: when code and documentation moved but `work/journal.md` did not,
  the reminder asks for the session's journal line; a touched draft alone no
  longer reads as "documented". Warns, never blocks (R-150).
- Session-intent routing names `research` (→ `work/research/`, no code) and
  the in-docs link form; `rules --agents-md` states tree-wide id uniqueness
  and the `[[dir/id]]` link form.
- R-194's flowing-layer finding names the wiki-link form of a draft citation
  instead of only "distil" — the previous wording made an agent delete the
  link.
- `journal add`: a one-line text without `--title` becomes the heading alone;
  the same sentence is no longer repeated as the first bullet.

### Fixed

- README: the Commands table carries every flag the binary parses
  (`--report`, `--dir`, `--max-lines`, `--write`, `--since`, `--memory`,
  `--date`, …); Quick start opens with the one-command adoption and the
  guided tours moved under their own heading.

## [0.9.0] - 2026-08-29

### Added

- Capture commands (D-063): `debt close <n> [--note]` — the item leaves the
  ledger and a journal entry records the repayment; `journal add <text>
  [--title] [--date] [--link]` — an entry at its date; `page new
  <category|type> <id>` — a work file from `_templates/` or a permanent
  page skeleton with the opening left to the author.
- Derived navigation (D-064): `backlinks <path|id> [--repo]`, `mentions
  [<page>]` (prose naming a page without a link, with the link to add),
  `graph --format dot|json|jsoncanvas [--repo]` (links, graduation, code
  citations; JSON Canvas laid out in columns).
- `adopt --obsidian` (D-065): `.obsidian/app.json` (absolute links,
  `_archive/` and `.federation/` ignored), `templates.json` (`_templates/`),
  and a `stale-work.base` view — the docs root as a vault.

### Changed

- Same-day journal entries: the latest write goes on top.

## [0.8.0] - 2026-08-29

### Added

- Evidence locators in `sources:` — `git:<sha>`, `tag:<ref>`,
  `git:<sha>:<path>[@L<a>-L<b>]` — parsed and resolved by one module
  against the repository the tree lives in. R-059 now checks every page
  that declares sources in both profiles: an error on a knowledge base's
  permanent pages, a warning on a project's seeded work files (D-061).
- `docsys seed plan --memory <dir>` — agent memory notes as questions:
  name and description only, never the body (D-062).

### Changed

- `/docsys-sync` reads drift through `docsys seed plan --since`; `/docsys-seed`
  gains the memory step.

## [0.7.0] - 2026-08-29

### Added

- `docsys seed apply --plan <file>` — lands the rows the seeding
  conversation produced, under `work/` only, as tokens and verbatim
  quotations: `research` (a reserved, seeded research page), `answer` (the
  builder's words, attributed and dated), `journal` (a retrospective entry
  at its own date with its `git:` provenance line), `postmortem` (a
  commit's own account), `debt`, `question`. Refuses a dirty tree, a stale
  HEAD pin, a TODO row and a page it did not seed; idempotent (D-058).
- `docsys seed gaps` — the feature inventory as JSON.
- `/docsys-seed <feature>` and `/docsys-interview` — the seeding
  conversation as installed commands: research by the tool, plain questions
  by the agent, nothing written before the builder's word (D-059).

### Fixed

- `generated_preamble:` is also written as the first line inside the
  `docsys:adoption` and `docsys:rules` managed blocks, so a gate reading
  the staged diff finds it on every regeneration (D-060).

## [0.6.1] - 2026-08-29

### Added

- `generated_preamble:` in `.docmeta.yml` — verbatim line(s) written at the
  top of every markdown file docsys generates (`ADOPTION.md`, templates,
  list files, agent commands and skills), after the frontmatter when there
  is one; never into a hook, never twice, nothing when absent. A privacy
  gate that wants its marker in every generated file no longer fights the
  generator (D-056).

### Fixed

- `adopt` no longer deletes what the owner wrote in `ADOPTION.md`. The
  report lives in a `docsys:adoption:begin/end` managed block and only that
  block is regenerated; a report from before the markers is kept verbatim
  below the block with a note (R-045, D-057). Found on three trees.

## [0.6.0] - 2026-08-29

### Added

- `docsys seed plan [--target <feature>] [--since <date>]` — brownfield
  seeding, first slice. Without a target: the feature inventory (commit
  scopes, package manifests, feature directories) with span, size and
  whether a page already covers each one. With a target: one feature's
  history as evidence — commits with bodies, files by touch count and the
  other scopes they serve, birth by `--diff-filter=A`, manifests, `doc:`
  citations, the code's own comment blocks verbatim, tags in the span.
  A covered feature is refused by name; nothing is ever written (D-053).
  Hygiene by rule: merges, mega commits, delete-and-restore pairs, vendored
  and restricted paths (D-054).
- `init` and `adopt` write the R-048 templates into `_templates/` and
  `work/questions.md`, when absent (D-055).
- R-104 is checked: a journal entry below an older one is reported.

### Changed

- R-108's item table shows the opening date the lint already required.

## [0.5.2] - 2026-08-29

### Changed

- `work/debt.md` holds items only: a heading or paragraph after the first
  item is an **error** (one per run of lines) — a closed debt kept as prose,
  a lesson, or an open debt without its line. Preamble, comments, blank
  lines and indented continuations are free. Found on a pilot ledger where
  14% of the file was prose "closed" sections no check could see (D-052).
  Corpus case `35-debt-ledger-prose`.

## [0.5.1] - 2026-08-26

### Fixed

- `docsys hook stop` no longer reads stdin (it has no payload), and the
  other events skip it when stdin is a terminal — running a hook by hand
  no longer waits for EOF.

## [0.5.0] - 2026-08-26

### Changed

- The agent hooks decide in the binary. `docsys hook pre-tool-use | stop |
  post-tool-use | user-prompt-submit` reads the Claude Code payload with a
  real JSON parser and carries every rule the shell scripts used to carry
  (ask-once marker, dropped-`git add` retry, heredoc-aware command
  detection, unquoted git paths, `updated:` bump, routing text). The four
  installed scripts are one-line relays; run `docsys agents --force` to
  receive them (D-051). Behavior is unchanged; the unit-test surface is not.

## [0.4.12] - 2026-08-26

### Fixed

- The commit gate's heredoc skipping now handles `<<-` (tab-indented
  terminator) and opens the payload's `\t` escapes — found by the new
  table-driven matcher test, not in the field.

### Added

- Unit tests for the code that lived only under end-to-end tests:
  `preserved_header`, hook stamping and staleness, `scan_prefix` /
  `under_prefix`, wiki-link fragments, R-073 token rules, scannable lines,
  the gate's unquoted-path reading, and a sixteen-row payload table for the
  PreToolUse command matcher. Every hook template is parsed with `bash -n`.

## [0.4.11] - 2026-08-26

### Changed

- R-073 states its own consequence: the rule's counter-example cannot be
  written literally; prose shows the placeholder form or uses a fenced block
  or a `>` quotation. Corpus case `34-quoted-references` locks that a `>`
  quotation is not scanned (fences: `07-doc-refs`; indented code: `29`).
- README names the scope split: `lint` reads the docs root, `refs --repo .`
  reads rules files, AGENTS.md and code.

## [0.4.10] - 2026-08-26

### Fixed

- The PreToolUse commit gate no longer fires on the words `git commit`
  inside a heredoc body (a rule text, a quoted message): heredoc bodies are
  dropped before matching, the heredoc line itself still counts (D-050).

## [0.4.9] - 2026-08-26

### Fixed

- The dangling-promise finding on `work/debt.md` (D-039) now states the
  open-item grammar it expects — a tree kept its debt as a Markdown table
  and could not tell what the checker wanted (R-152).

## [0.4.8] - 2026-08-26

Version bump only — tagged before its change landed; the change is 0.4.9.

## [0.4.7] - 2026-08-26

### Fixed

- The pre-commit refusal says that the whole Bash call was blocked — a
  `git add` in it did not run — and asks for the same command from the
  start. A retry that dropped its `git add` while the tree still holds
  unstaged changes is stopped once more ("did your `git add` run?"); a
  commit that was bare from the start is never second-guessed. Found live:
  a commit whose message described all the work landed with six deletions
  as its content (D-049).
- The generated AGENTS block carries the general sentence: what landed is
  `git show HEAD:<file>`, not the tree.

## [0.4.6] - 2026-08-26

### Fixed

- A documentation page with a non-ASCII name is a documentation change. git
  quoted such paths (`"docs/g\303\274..."`) and no docs-root prefix matched,
  so `docsys gate` and the stop reminder read the change as code — a tree
  named in its own language could never answer the commit-time question.
  Git output compared against paths is read with `core.quotePath=false`; the
  hooks match the docs root with a shell pattern, not a regex (D-048).
- The PreToolUse hook read the command out of the payload up to the first
  quote, escaped or not: `printf "x" > y && git commit` was never gated.
  Executed-hook tests lock all three.

## [0.4.5] - 2026-08-26

### Fixed

- `scan_exclude` entries spelled `spec/`, `./spec`, or `spec/**` now exclude
  the directory `spec` (they silently excluded nothing); matching is on a
  path-component boundary. An entry the prefix form cannot express (glob
  syntax, `..`) is reported under R-077, naming the entry (D-045). Corpus
  case `33-scan-exclude-form`.
- `adopt` keeps the owner's leading comment block on `ADOPTION.md` — a
  privacy marker placed by hand no longer disappears on the next run (D-046).

### Added

- Hooks carry `# docsys-template: <version>`; `adopt` and `doctor` name the
  hooks behind the binary's templates and point at `docsys agents --force`
  (D-047).

## [0.4.4] - 2026-08-26

### Fixed

- The pre-commit question is asked once per (HEAD, change set) for real. The
  marker used to be consumed by any passing attempt — and an agent's
  `git add … && git commit` stages after the hook ran, so a bare `git commit`
  in between passed, committed nothing, and the next attempt was asked again
  (found live). The marker now lives under `<git-dir>/docsys-gate/` until
  HEAD moves, and keys on the working tree when nothing is staged (D-043).

## [0.4.3] - 2026-08-26

### Fixed

- A wiki-link with a `#fragment` (`[[reference/x#section]]`) no longer reads
  as a dangling link when the page exists. R-070 now states the link form: a
  link addresses a page, never a heading; the page part resolves and the
  fragment is a warning naming the alias form `[[path|Title § Section]]`.
  An unresolved page part stays the R-071 error. Corpus case `32-link-fragment`
  (D-042).

## [0.4.2] - 2026-08-26

### Fixed

- The Stop reminder (`stop-docs-reminder.sh`) now reads the commits ahead of
  the upstream (`@{u}..HEAD`) as well as the working tree. A session that
  commits as it goes left a clean tree at every turn end, and the reminder
  stayed silent through a whole session of code-only commits (found live).
  It still warns and never blocks (D-041).
- The same hook reads the NEW path of a rename; `awk '{print $2}'` read the
  old side and could count a code move as a docs change. Both are locked by
  executed-hook tests.

## [0.4.1] - 2026-08-24

### Fixed

- `doctor` and `adopt` ask git where the hooks live (`git config --get
  core.hooksPath`) instead of parsing the config file's text — a hooksPath
  set in another scope or spelled in another case sent doctor to the wrong
  directory, reporting a missing gate while the real one sat dead elsewhere
  (found live, from a field log). The regression test writes the exact
  lowercase-key config that was missed.

## [0.4.0] - 2026-08-24

### Fixed

- `adopt` writes the git gate right below the shebang, never at the end of an
  existing hook — a block appended below `exec` or `exit` is dead code that
  looks installed, found live twice in one day (D-040's check).
- A wiki-link target containing `<`, `>`, `{` or `}` is a placeholder, not a
  link (D-037) — R-073's rule for `doc:` references, applied to links. The
  tool's own knowledge-base skeleton found this: its commented router example
  failed the check it shipped with.
- An indented code block (four-plus spaces or a tab, CommonMark's other fence)
  is quoted material: reference and path scanning skips it, like a fenced
  block or a blockquote (D-035). An explanation page teaching the `doc:`
  citation form had its own indented example read as a dangling reference.

### Added

- The hook scripts are executed for real in the test suite (a payload grammar
  mistake or a wrong exit code in bash is invisible to Rust unit tests), and
  `ci/e2e.sh` runs every first-run flow from a clean box — adopt, doctor
  (including the dead-gate detection), the ask-once gate, the two-command
  knowledge base, two-git-provider federation, honest export refusal. CI runs
  it on every platform; a container reproduces it anywhere.
- `docsys doctor` — is the pipeline itself alive? Every hook checked for
  existing, executable, wired under the right event, and reachable; a git-gate
  block sitting below a top-level `exec`/`exit` is named as dead code; the
  channel semantics (what actually reaches the model) are stated so nobody
  rediscovers them (D-040). Born from a field report where five mechanisms
  had failed silently and nothing could tell.
- `docsys gate` — the commit-time question, computed by the binary: lint
  findings plus "changes outside the docs root with none inside" (staged, or
  the working tree when nothing is staged yet). The rewritten
  `pre-commit-docs.sh` relays it on the one channel that reliably reaches the
  model (PreToolUse, exit 2): lint errors block outright; the
  code-without-docs question asks ONCE — re-running the same commit proceeds.
  The settings snippet now wires it.
- The debt lifecycle (D-039): an open item carries its opening date (undated
  is reported — age must be measurable); a repaid item LEAVES the file (a
  lingering `- [x]` line is reported — the journal records the repayment); a
  page routing readers to `work/debt` while it declares no open item is
  reported (the dangling promise, found live).
- Audience modes (D-033): a page may declare `audience:`, the vocabulary is
  the tree's own (`audiences:` in `.docmeta.yml`, the domains pattern), and an
  undeclared page reads as `developer` — no migration, no guessing. `--audience`
  on `export plan/product/feature` selects by declaration: a *named* page of
  the wrong audience is refused, a `--follow`ed one becomes a named gap
  warning, and a draft for an audience nobody writes for is an error naming
  the whole-tree gap. The tool never writes and never re-audiences prose —
  the modes select, the agent authors.
- The `docsys-export` skill (installed by `agents`/`adopt`): turns "create the
  end-user doc for X" into a procedure — discover with `export plan`, compose,
  close gaps by authoring pages with approval, translate under R-122/R-123.
  Without it the prompt lives in the user's head, the hand-maintained
  knowledge D-022 exists to prevent.
- A personal knowledge base stands up in two commands (D-036):
  `docsys init --profile knowledge-base` writes the record layer, the wiki
  root and a docmeta whose `domains:` the owner fills in, and
  `docsys agents --kb` installs the base's four organs — capture, ingest,
  audit, lookup — plus an `AGENTS.md` constitution written only when absent.
  The skills carry judgment; every mechanical rule they name is the binary's.
- `docsys export manifest` — the namespace's index (D-038): what it exports,
  with content hashes, no bodies. `fetch` reads it before any page and skips
  what has not changed (bytes and fetch date preserved), so a three-hundred
  service estate refreshes only what actually moved. Line grammar, versioned
  independently of the spec; an unimplemented major version is refused by
  name. A provider without a manifest stays consumable — the tree is the index.
- `docsys fetch` — federation's first working slice (D-034): a consumer
  declares its providers as one `consume_base:` template plus a list of names
  (three hundred services, one line — or `<ns>=<location>` per service), where
  a location is a filesystem path or a git URL with `#<subdir>`; fetch
  shallow-clones git providers and materializes each provider's exported
  pages under `.federation/<ns>/` (reconstructed frontmatter,
  verbatim body, provenance sidecar; `internal: true` never crosses), and a
  `@ns/<id>` map or feature entry now composes across repositories — two
  repos sharing one feature produce one document. Foreign entries resolve
  only against the verified local state: unfetched and tampered
  materializations are refused by name, and every foreign stamp carries its
  fetch date.

## [0.3.0] - 2026-08-16

The knowledge-base profile is checked, not just recognized. Shaped by the first
real knowledge base: a personal wiki whose constitution predated the spec and
matches it — the tool met a tree that was already living the rules.

### Added

- `profile: knowledge-base` is fully linted (supersedes D-006's v0 narrowing).
  The layout (`raw/` record layer, `wiki/<domain>/<type>/` pages, `wiki/index.md`
  plus domain indexes as routers), the page contract (`domain`, `verification`,
  `sources` — R-024), undeclared domains (R-026), and the directory's type
  segment (R-029) are all checked; wiki-links resolve against the permanent
  layer root, the field convention (D-030).
- R-028's verification record has names: `verified_by:` and `verified_rev:`.
  A `verified` page without them is reported — a verification nobody can audit
  is a claim, not a record.
- R-059: every `sources:` entry must resolve; a severed evidence trail is an
  error — it is exactly the silent failure R-027 names.
- R-023: `raw/` content-immutability is checked at the gate (D-031): uncommitted
  modification or deletion of a tracked raw file is an error; relocation — the
  basename reappearing under `raw/` — is the expected flow and passes. Outside
  git the hook layer owns the promise.
- The record layer never blocks (R-194, extended to the profile): a dangling
  link or reference inside a raw note is reported, never an error, and the
  path scan skips raw notes — they are quoted source material.
- `scan_exclude` now excludes from the docs-side walk too: a template library
  inside a knowledge base is tooling, not documentation.
- `adopt` and `graduate` refuse a knowledge-base tree by name instead of
  half-running: adoption is its own release; graduation there is distillation
  (R-092), an authored rewrite no command may fake.
- `export plan` / `export product` (D-032) — the product half of the founding
  goal: a product-level document composed from the tree's permanent pages.
  `plan` drafts a product map from the tree's evidence (R-057 titles and
  summaries, grouped by type) — a proposal, never a decision; the map is plain
  markdown (H1 name, H2 sections, router-shaped entries whose targets are
  `doc:` identifiers). `product` is fully mechanical: bodies carried verbatim
  with headings shifted (prose never rewritten), a source stamp per page
  (identifier, file, drift hash, `updated`), and a hard refusal to
  half-compose — a foreign, flowing, retired or unknown identifier fails the
  run with the complete list. Foreign (`@ns/`) entries compose once federation
  consumption exists. `--lang` states the document's intended content
  language: pages whose declared language differs are warned by name, and a
  mixed composition without a stated intent is warned once — the tool reads
  declarations, determines no language, and translates nothing (R-120–R-123;
  translation is agent work).
- `export feature <id>…` — a slice of the tree with no map file: the named
  identifiers, optionally widened one hop along their wiki-links (`--follow`).
  One command exports one feature out of a large code base.
- `--out` never rewrites an unchanged document: content is compared without
  the dated header line, so a no-op regeneration leaves the file, its honest
  generation date, and every downstream consumer (watchers, builds, a
  translating agent) untouched. Regeneration itself stays stateless — a cache
  is state, and state drifts (R-002); the per-page stamps are what let
  downstream re-process only the sections whose hash moved.
- `AGENTS.md` — the repository's own ground rules: English-only content, no
  external project names, spec-first, corpus-locked.

## [0.2.3] - 2026-08-16

A formatter ran over an adopted tree and the tool lost a field. Real projects
run prettier; a checker that silently drops configuration when they do is worse
than one that never read it.

### Fixed

- `.docmeta.yml` and frontmatter follow an inline list that a formatter
  reflowed across lines, including the shape where the opening bracket lands on
  its own line. A reflowed `scan_exclude` had been silently dropped, bringing
  back four already-resolved findings.
- A trailing comma leaves punctuation, not an item: the empty slot is skipped.
  Read as an item it became an empty path prefix — "exclude everything" — and
  the scan inspected zero files. R-011's dead-scan rule caught it on the real
  tree, which is the whole reason that rule exists.

## [0.2.2] - 2026-08-16

Warnings got the same treatment errors did: on a real tree, more than half of
them were the checker misreading the repository. 311 warnings became 0, and the
tool learned six things.

### Added

- `.docmeta.yml` accepts `list_labels: [deferred=<local form>, repay when=…]`
  (R-108), the same shape `headings` already had: a tree writing its
  documentation in another language must not be forced to embed English words
  in its own prose to satisfy a checker.

### Fixed

- A token containing `<`, `>`, `{` or `}` is a metasyntactic placeholder, not a
  reference (R-073). Prose documenting the citation form — including this
  project's own generated agent text — was failing the convention it teaches.
- Closed work (`graduated`, `abandoned`) is exempt from the section template
  (R-048): the template guides open work and makes graduation mechanical, so
  demanding it of a finished page asks for structure that can do nothing.
- A root-level page's full path is its bare name, so `[[roadmap]]` is not a
  short-name link (R-070).
- `graduated_to` resolves through the shared resolver (R-056): a destination
  named through a `defines:` family is as real as a page id, and two resolvers
  over one identifier space eventually disagree.

## [0.2.1] - 2026-08-16

The same real repository, one layer deeper: 212 errors became 2, and every
step of the drop was the tool learning, not the repository changing.

### Fixed

- R-079: a family member may be written in the register's own short form —
  a page listing `(utf8-text)` under `defines: ADR-*` defines `ADR-utf8-text`,
  and demanding the prefix twice made pages fail to define what they plainly
  define. An occurrence that is itself a `doc:` citation no longer counts as
  evidence: a reference cannot prove its own target (the hole withdrawn R-064
  was meant to close).
- The historical layer never blocks. A journal entry, an archive slice, or an
  `_archive/` page citing a graduated, flowing, or missing identifier is
  reported, not an error: a dated record cannot be corrected by editing it, and
  the legitimate repairs — a tombstone, a distilled page — are what the report
  names. An error nobody may honestly clear is one people learn to bypass.
- A citation to an archived page resolves and is reported; the record is real,
  dated content, exactly as an explicit `[[_archive/…]]` link already was.
- A graduated page that records `graduated_to` is a signpost, not a husk:
  citing it is reported, not blocked. Only a graduated page with nowhere to
  point remains an error.
- Inline-code parity is counted from the start of the line, so a second
  reference on the same line parses (it used to swallow the closing backtick).

## [0.2.0] - 2026-08-16

First release shaped by a real adoption: every change below came from running
the tool against an existing repository and finding the tool, not the repo, at
fault.

### Added

- `adopt` — one-command integration: init skeleton or docmeta upgrade, agent
  assets, `settings.json` when absent, the AGENTS.md managed block, a warn-mode
  git pre-commit gate (hooksPath-aware), and an `ADOPTION.md` report whose
  checklist carries every judgment call. Idempotent; refuses an unmigrated tree.
- R-194: a `doc:` reference resolves against every page id, tracked work
  included. A citation to a graduated page is an error (its value moved on); any
  other flowing citation is reported, naming the distillation still owed.
- `.docmeta.yml` accepts `journal_entry_max_lines` (R-101, default 5) so a tree
  can state a discipline it actually keeps, and whole-line `#` comments.

### Fixed

- Repo-side scans list files through `git ls-files`, so `.gitignore` decides
  what belongs to the project (D-029). A build tree or a nested worktree used to
  turn 147 real findings into 9,171.
- A relative `--root` is anchored to `--repo`; the docs root is excluded by
  identity, not spelling (D-027) — it used to be scanned as code.
- R-073: a reference opened inside an inline-code span ends at the closing
  backtick, so a grammatical suffix glued to it (a case ending, a possessive) is
  no longer read into the identifier.
- R-100: one bracketed annotation may sit between the date and the separator —
  entry counters and channel tags are field conventions that keep the date first.
- R-075: an escaping link whose target exists is reported, not an error;
  documentation routing to a file beside the code is working, not broken.

## [0.1.0] - 2026-08-15

Initial public release.

### The specification

- `SPEC.md`: 133 normative rules across layout, identity, lifecycle,
  graduation, journal discipline, freshness, and the agent layer; survived
  six adversarial audit rounds by six independent models. Federation (§13)
  ships as EXPERIMENTAL and binds nothing until a reference implementation
  and a second real estate exist.
- `corpus/DECISIONS.md`: every implementation-defined choice registered
  with its reason (D-001..D-028).
- Conformance corpus with exact expected findings — an extra finding fails
  a case as hard as a missing one, so the checker cannot drift noisy.

### The binary (zero dependencies, `unsafe` forbidden)

- `adopt` — one-command integration: init skeleton or docmeta upgrade,
  agent assets, `settings.json` when absent, AGENTS.md managed block,
  warn-mode git pre-commit gate (hooksPath-aware), and an `ADOPTION.md`
  report whose checklist carries every judgment call. Idempotent.
- `lint` — full tree validation: frontmatter, ids, links, journal
  discipline, templates, list grammars. Warn by default; block only what
  is irreversible or silently wrong.
- `refs` — every `doc: <id>` in the code base validated against the tree.
- `migrate inventory/apply` — brownfield adoption: evidence-rich plan,
  approved mapping, mechanical move with link rewriting on both sides of
  the docs boundary.
- `graduate plan/apply` — byte-exact block movement from work files to the
  permanent layer; content is moved, never rewritten.
- `rules` / `agents` — agent-facing text generated from the embedded spec
  (no hand-maintained copy to drift) and the thin `.claude/` adapter:
  four warn-only hooks, `/docsys-sync`, skill.

### Field-proven

- Piloted against three real repositories; a 63-file flat tree migrated
  with 29 references rewritten, and a full legacy documentation system
  swapped out with zero memory loss.
