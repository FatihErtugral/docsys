//! The command table: one entry per command the binary dispatches, read by
//! `docsys --help` (what each is for) and `docsys <command> --help` (its flags
//! and one example). A test holds it to the dispatch in `main.rs`.

use std::fmt::Write as _;

/// One command as a person meets it.
pub struct Command {
    /// the words that name it: `lint`, `graduate apply`
    pub name: &'static str,
    /// what follows the name
    pub synopsis: &'static str,
    /// the situation it is for, in one line
    pub purpose: &'static str,
    /// each flag or argument, and what it does
    pub flags: &'static [(&'static str, &'static str)],
    /// one invocation as a person types it
    pub example: &'static str,
}

const ROOT: (&str, &str) = (
    "--root <dir>",
    "the docs tree; `docs` by default, found from any directory inside the repository",
);
const REPO: (&str, &str) = (
    "--repo <dir>",
    "the repository; by default the one that holds the tree",
);
const JSON: (&str, &str) = (
    "--json",
    "the same answer as JSON, for a script or an agent",
);

pub const COMMANDS: &[Command] = &[
    Command {
        name: "adopt",
        synopsis: "[--repo .] [--root docs] [--lang <code>] [--rules-file <path>] [--report-dir <dir> | --no-report] [--ci-runner <label>,…] [--ci-install cargo|release] [--ci-sha256 <target>=<hex>,…] [--verify-on-approval description|pull-request|direct|off] [--obsidian]",
        purpose: "a repository starts using docsys: the tree, the agent rules, the hooks and the git gate in one step; ADOPTION.md lists what is left",
        flags: &[
            REPO,
            ROOT,
            ("--lang <code>", "the language the pages are written in"),
            ("--rules-file <path>", "the file the rules block goes to; by default where its markers are"),
            ("--report-dir <dir>", "where ADOPTION.md goes"),
            ("--no-report", "write no ADOPTION.md"),
            ("--ci-runner <label>,…", "the runner labels of the CI workflow written when .github/ exists"),
            ("--ci-install cargo|release", "how the workflow installs docsys"),
            ("--ci-sha256 <target>=<hex>,…", "the release binary's checksums, with --ci-install release"),
            ("--verify-on-approval <mode>", "how a review's approval is recorded: `description` on a docsys/0.5 tree; on a docsys/0.4 tree `pull-request` (a follow-up pull request) or `direct` (a push); the first is each tree's default, `off` records none, and a mode the tree cannot read is refused"),
            ("--obsidian", "also write .obsidian settings and a stale-work view"),
        ],
        example: "docsys adopt --repo . --root docs",
    },
    Command {
        name: "init",
        synopsis: "[--root <dir>] [--lang <code>] [--profile project|knowledge-base]",
        purpose: "only an empty tree is wanted, without the agent layer or the gate — `adopt` does both",
        flags: &[
            ("--root <dir>", "where the tree is made"),
            ("--lang <code>", "the language the pages are written in"),
            ("--profile project|knowledge-base", "a repository's documentation, or a knowledge base of records and wiki pages"),
        ],
        example: "docsys init --root docs --lang en",
    },
    Command {
        name: "upgrade",
        synopsis: "[--apply] [--commit] [--force] [--json] [--root docs] [--dir .claude]",
        purpose: "this docsys is newer than the spec the tree declares: the plan first, then the move, one commit per spec, then the pin",
        flags: &[
            ("--apply", "make the move; without it only the plan is printed"),
            ("--commit", "commit the move as one commit"),
            ("--force", "run on a working tree with uncommitted changes"),
            JSON,
            ROOT,
            ("--dir <dir>", "the agent layer's directory, `.claude` by default"),
        ],
        example: "docsys upgrade --apply --commit",
    },
    Command {
        name: "lint",
        synopsis: "[--root <dir>] [--repo <dir>] [--json]",
        purpose: "you want to know whether the tree is right: every rule, with pins and history inside a repository; exit 1 on an error",
        flags: &[ROOT, REPO, JSON],
        example: "docsys lint",
    },
    Command {
        name: "refs",
        synopsis: "[--repo <dir>] [--root <dir>] [--json]",
        purpose: "the code side is in question: every `doc:` citation in the code resolves to a page",
        flags: &[REPO, ROOT, JSON],
        example: "docsys refs",
    },
    Command {
        name: "gate",
        synopsis: "[--repo .] [--root docs] [--range <a>...<b>] [--skipped] | --message <file>",
        purpose: "what the git hooks and CI run: lint, and code changed without documentation; under `commit_policy: require` it refuses",
        flags: &[
            REPO,
            ROOT,
            ("--range <a>...<b>", "a pull request's commits, in CI"),
            ("--skipped", "record a bypassed gate as a debt item"),
            ("--message <file>", "the commit-msg half on docsys/0.5: under `require`, code with no documentation needs `Docs: <why>`"),
        ],
        example: "docsys gate --range origin/main...HEAD",
    },
    Command {
        name: "doctor",
        synopsis: "[--repo .] [--root docs] [--dir .claude]",
        purpose: "the documentation pipeline seems silent: are the hooks, the gate and the relays wired and alive",
        flags: &[REPO, ROOT, ("--dir <dir>", "the agent layer's directory")],
        example: "docsys doctor",
    },
    Command {
        name: "status",
        synopsis: "[--root <dir>] [--repo <dir>] [--json]",
        purpose: "the tree's state at a glance: the inbox, pages by state, open items, consumed namespaces, findings",
        flags: &[ROOT, REPO, JSON],
        example: "docsys status",
    },
    Command {
        name: "lookup",
        synopsis: "<word…> [--root docs] [--json]",
        purpose: "a question names a few words: the pages, local and consumed, that hold them — the first hop before reading",
        flags: &[("<word…>", "the words the question names"), ROOT, JSON],
        example: "docsys lookup token refresh",
    },
    Command {
        name: "page new",
        synopsis: "<category|type> <id> [--title <t>] [--unverified] [--root docs]",
        purpose: "a new page or work file is needed: from its template, or a permanent page's skeleton",
        flags: &[
            ("<category|type>", "a work category (feature, postmortem, research) or a page type (reference, explanation, howto, tutorial)"),
            ("<id>", "the page's identifier, which is also its file name"),
            ("--title <t>", "the page's title"),
            ("--unverified", "a page written from evidence, which a maintainer verifies later"),
            ROOT,
        ],
        example: "docsys page new reference token-ttl --title \"Token lifetime\"",
    },
    Command {
        name: "pin",
        synopsis: "<page> <path> [--symbol <s>] [--block <n>] | --refresh <page> | --gc  [--repo .] [--root docs]",
        purpose: "a page describes code: bind it to the region, so lint says when the code moves away from the page",
        flags: &[
            ("<page> <path>", "the page, and the code file it describes"),
            ("--symbol <s>", "the declaration in that file, instead of the whole file"),
            ("--block <n>", "the page block the pin backs, as `verify --show` numbers it"),
            ("--refresh <page>", "after re-reading the page against the code: record the region as it reads now"),
            ("--gc", "remove acknowledgements no current pin needs"),
            REPO,
            ROOT,
        ],
        example: "docsys pin reference/token-ttl src/auth.rs --symbol refresh_token",
    },
    Command {
        name: "verify",
        synopsis: "<page> [--by <handle|@login>] [--commit] [--revoke] | --show <page> | --range <a>...<b> (--by @login | --from-trailers) | --approval <@login>",
        purpose: "a maintainer vouches for a page: on docsys/0.5 the approval is their commit; --show lists what to read again",
        flags: &[
            ("<page>", "the page's id or path"),
            ("--by <handle|@login>", "who verifies, when it is not the git identity"),
            ("--commit", "make the commit at once"),
            ("--revoke", "take the approval back"),
            ("--show <page>", "the blocks a re-verification reads: changed, new and removed ones, stale pins, the sources"),
            ("--range <a>...<b>", "every page a pull request touched, under the reviewer's identity"),
            ("--from-trailers", "with --range: the identity from the `Reviewed-by:` or `Approved-by:` trailer"),
            ("--approval <@login>", "the `Approved-by:` line a declared maintainer's approval adds to a pull request's description"),
            ROOT,
        ],
        example: "docsys verify reference/token-ttl",
    },
    Command {
        name: "compile",
        synopsis: "<howto> [--root docs] [--dir .claude] [--force]",
        purpose: "a howto's steps are complete: its body becomes an agent skill, pinned to the page",
        flags: &[
            ("<howto>", "the howto page"),
            ROOT,
            ("--dir <dir>", "the agent layer's directory"),
            ("--force", "overwrite the skill"),
        ],
        example: "docsys compile howto/rotate-keys",
    },
    Command {
        name: "graduate plan",
        synopsis: "<work-file> [--root <dir>]",
        purpose: "work is done and its knowledge must reach permanent pages: the work file's blocks, for an agent to map to destinations",
        flags: &[("<work-file>", "the work file, relative to the tree"), ROOT],
        example: "docsys graduate plan work/features/cart-key.md > plan.tsv",
    },
    Command {
        name: "graduate apply",
        synopsis: "--plan <file> [--confirmed <who>] [--root <dir>] [--force]",
        purpose: "the person approved the plan: the blocks move byte for byte; with --confirmed on docsys/0.5 the work file leaves",
        flags: &[
            ("--plan <file>", "the filled plan"),
            ("--confirmed <who>", "the person's word that the file graduates: the last blocks move and the file is removed"),
            ROOT,
            ("--force", "run on a working tree with uncommitted changes"),
        ],
        example: "docsys graduate apply --plan plan.tsv --confirmed maintainer",
    },
    Command {
        name: "debt add",
        synopsis: "<debt…> --deferred <reason> --repay-when <trigger> [--topic <id>] [--date <d>] [--root docs]",
        purpose: "work is deferred on purpose: one dated item in its topic's file",
        flags: &[
            ("<debt…>", "what is deferred"),
            ("--deferred <reason>", "why it waits"),
            ("--repay-when <trigger>", "the event that makes it due"),
            ("--topic <id>", "the page or feature it concerns; `general` without one"),
            ("--date <d>", "the date, today by default"),
            ROOT,
        ],
        example: "docsys debt add \"retries are unbounded\" --deferred \"no load yet\" --repay-when \"the next outage\" --topic retry-policy",
    },
    Command {
        name: "debt close",
        synopsis: "<n|words> --note <how> [--root docs]",
        purpose: "a debt is repaid: the item leaves its file, and the commit carries the `Resolved:` line it prints",
        flags: &[
            ("<n|words>", "the item's number in the list, or words only it holds"),
            ("--note <how>", "how it was repaid"),
            ROOT,
        ],
        example: "docsys debt close \"retries are unbounded\" --note \"bounded at three\"",
    },
    Command {
        name: "question add",
        synopsis: "<question…> [--topic <id>] [--context <c>] [--date <d>] [--root docs]",
        purpose: "something is not known: one dated question item, never a guess on a page",
        flags: &[
            ("<question…>", "the question"),
            ("--topic <id>", "the page or feature it concerns"),
            ("--context <c>", "where it came up"),
            ("--date <d>", "the date, today by default"),
            ROOT,
        ],
        example: "docsys question add \"who owns the cache?\" --topic cache",
    },
    Command {
        name: "question close",
        synopsis: "<n|words> --answer <line> [--root docs]",
        purpose: "a question is answered: the item leaves its file, and the commit carries the `Answered:` line it prints",
        flags: &[
            ("<n|words>", "the item's number in the list, or words only it holds"),
            ("--answer <line>", "the answer"),
            ROOT,
        ],
        example: "docsys question close \"who owns the cache\" --answer \"the platform team\"",
    },
    Command {
        name: "ledger fix",
        synopsis: "[--root <dir>]",
        purpose: "a ledger or its archive slice uses em-dash field markers: rewrite them to the ASCII ones, the field text untouched",
        flags: &[ROOT],
        example: "docsys ledger fix",
    },
    Command {
        name: "journal",
        synopsis: "[--since <date>] [--root docs]",
        purpose: "you want what changed and why: every commit that changed the docs or carries `Docs:`, newest first",
        flags: &[("--since <date>", "only the entries from that day on"), ROOT],
        example: "docsys journal --since 2026-09-01",
    },
    Command {
        name: "journal add",
        synopsis: "<text…> [--title <t>] [--link <path>] [--date <d>] [--root docs]",
        purpose: "a change needs its why recorded: on docsys/0.5 it prints the commit message the entry is",
        flags: &[
            ("<text…>", "what changed and why"),
            ("--title <t>", "the entry's title"),
            ("--link <path>", "the page it concerns"),
            ("--date <d>", "the date, on a docsys/0.4 tree"),
            ROOT,
        ],
        example: "docsys journal add \"the run page states the timeout\" --link reference/run",
    },
    Command {
        name: "seed plan",
        synopsis: "[--target <feature>] [--since <date>] [--memory <dir>] [--repo .] [--root docs]",
        purpose: "an existing project has code but no pages: the feature inventory, or one feature's history as evidence",
        flags: &[
            ("--target <feature>", "one feature's evidence instead of the inventory"),
            ("--since <date>", "only history from that day on"),
            ("--memory <dir>", "an agent's memory directory to read as evidence too"),
            REPO,
            ROOT,
        ],
        example: "docsys seed plan --target checkout",
    },
    Command {
        name: "seed gaps",
        synopsis: "[--since <date>] [--repo .] [--root docs]",
        purpose: "an interview is planned: the inventory as JSON, uncovered features first",
        flags: &[("--since <date>", "only history from that day on"), REPO, ROOT],
        example: "docsys seed gaps",
    },
    Command {
        name: "seed apply",
        synopsis: "--plan <file> [--repo .] [--root docs] [--force]",
        purpose: "the builder approved the seeding rows: they land under work/",
        flags: &[
            ("--plan <file>", "the approved rows"),
            REPO,
            ROOT,
            ("--force", "run on a working tree with uncommitted changes"),
        ],
        example: "docsys seed apply --plan SEED.tsv",
    },
    Command {
        name: "backlinks",
        synopsis: "<path|id|code-file> [--repo .] [--root docs]",
        purpose: "before changing a page or a code file: what points at it — pages, code, pins",
        flags: &[("<path|id|code-file>", "a page, or a code file"), REPO, ROOT],
        example: "docsys backlinks reference/token-ttl",
    },
    Command {
        name: "mentions",
        synopsis: "[<path|id>] [--root docs]",
        purpose: "prose names a page without linking it: where, so the link can be added",
        flags: &[("<path|id>", "one page; every page by default"), ROOT],
        example: "docsys mentions reference/token-ttl",
    },
    Command {
        name: "graph",
        synopsis: "[--format dot|json|jsoncanvas] [--repo .] [--root docs]",
        purpose: "you want to see the tree's links: the graph for a viewer",
        flags: &[("--format dot|json|jsoncanvas", "the output format"), REPO, ROOT],
        example: "docsys graph --format dot > docs.dot",
    },
    Command {
        name: "export plan",
        synopsis: "[--root <dir>] [--audience <a>]",
        purpose: "an audience needs one document: a draft product map to edit before exporting",
        flags: &[ROOT, ("--audience <a>", "who reads it: end-user, developer, designer, …")],
        example: "docsys export plan --audience end-user > map.md",
    },
    Command {
        name: "export product",
        synopsis: "<map> [--root <dir>] [--out <file>] [--lang <code>] [--audience <a>]",
        purpose: "the product map is ready: one document composed from the pages it names",
        flags: &[
            ("<map>", "the edited product map"),
            ROOT,
            ("--out <file>", "where the document goes; standard output by default"),
            ("--lang <code>", "the language it is wanted in"),
            ("--audience <a>", "who reads it"),
        ],
        example: "docsys export product map.md --out product.md",
    },
    Command {
        name: "export feature",
        synopsis: "<id> [<id>…] [--follow] [--title <t>] [--root <dir>] [--out <file>] [--lang <code>] [--audience <a>]",
        purpose: "one feature needs its own document: the pages named, and with --follow the ones they link",
        flags: &[
            ("<id>…", "the pages"),
            ("--follow", "also the pages they link"),
            ("--title <t>", "the document's title"),
            ROOT,
            ("--out <file>", "where the document goes"),
            ("--lang <code>", "the language it is wanted in"),
            ("--audience <a>", "who reads it"),
        ],
        example: "docsys export feature token-ttl --follow --out tokens.md",
    },
    Command {
        name: "export manifest",
        synopsis: "[--root <dir>] [--out <file>]",
        purpose: "another tree consumes this one: what this namespace exports",
        flags: &[ROOT, ("--out <file>", "where the manifest goes")],
        example: "docsys export manifest --out manifest.yml",
    },
    Command {
        name: "fetch",
        synopsis: "[--root <dir>]",
        purpose: "this tree consumes other namespaces: bring their pages into .federation/",
        flags: &[ROOT],
        example: "docsys fetch",
    },
    Command {
        name: "consume add",
        synopsis: "<path|git-url>[#subdir] [--as <ns>] [--root docs]",
        purpose: "this tree should read another tree's pages: add the provider to its consume: list",
        flags: &[
            ("<path|git-url>[#subdir]", "the provider"),
            ("--as <ns>", "the namespace it is read under"),
            ROOT,
        ],
        example: "docsys consume add ../platform#docs --as platform",
    },
    Command {
        name: "consume discover",
        synopsis: "<dir> [--root docs]",
        purpose: "you look for trees to consume: the docsys trees one level under a directory; writes nothing",
        flags: &[("<dir>", "where to look"), ROOT],
        example: "docsys consume discover ~/src",
    },
    Command {
        name: "inbox add",
        synopsis: "--source <name> --id <item> [--title <t>] [--url <u>] [--date <d>] [<file>|-] [--root <dir>]",
        purpose: "a connector brings a record into a knowledge base: it lands in raw/inbox/ once, with its provenance",
        flags: &[
            ("--source <name>", "where it comes from"),
            ("--id <item>", "its identifier there; the same item lands once"),
            ("--title <t>", "its title"),
            ("--url <u>", "its address"),
            ("--date <d>", "its date"),
            ("<file>|-", "its text, from a file or standard input"),
            ROOT,
        ],
        example: "docsys inbox add --source chat --id C123-456 --title \"Cache decision\" note.md",
    },
    Command {
        name: "inbox pull",
        synopsis: "<repo> [--since <date>] [--limit <n>] [--as <ns>] [--all] [--root <dir>]",
        purpose: "a knowledge base follows a repository: one record per commit since a date, newest first",
        flags: &[
            ("<repo>", "the repository"),
            ("--since <date>", "the first day"),
            ("--limit <n>", "at most this many"),
            ("--as <ns>", "the source name"),
            ("--all", "bookkeeping commits too"),
            ROOT,
        ],
        example: "docsys inbox pull ../api --since 2026-09-01",
    },
    Command {
        name: "raw move",
        synopsis: "<record> <domain> [--root <dir>]",
        purpose: "an inbox record is filed: it moves to raw/<domain>/ through git, bytes untouched, and every citing page follows",
        flags: &[("<record>", "the record in raw/inbox/"), ("<domain>", "where it belongs"), ROOT],
        example: "docsys raw move raw/inbox/2026-09-01-call.md payments",
    },
    Command {
        name: "forget",
        synopsis: "<page-id|page-path|record-path> --reason <text> [--root <dir>]",
        purpose: "a page or record must leave: a page to _archive/ with a tombstone, a record to raw/_forgotten/; the ledger says why",
        flags: &[
            ("<page-id|page-path|record-path>", "what leaves"),
            ("--reason <text>", "why"),
            ROOT,
        ],
        example: "docsys forget reference/old-flag --reason \"the flag was removed\"",
    },
    Command {
        name: "assistant",
        synopsis: "[--root .] [--projects <dir>]… [--domains a,b] [--since 30.days] [--limit 3]",
        purpose: "an assistant's memory in one step: the base, its agent layer, the projects it consumes, its pages and records",
        flags: &[
            ("--root <dir>", "the base"),
            ("--projects <dir>", "a directory of projects to consume; may repeat"),
            ("--domains a,b", "the domains the base files its records under"),
            ("--since <span>", "how far back each project's commits come into the inbox"),
            ("--limit <n>", "at most this many commits per project"),
        ],
        example: "docsys assistant --root ~/notes --projects ~/src",
    },
    Command {
        name: "migrate inventory",
        synopsis: "[--root <dir>] [--repo <dir>]",
        purpose: "a repository has documentation in another shape: the plan skeleton that classifies every file",
        flags: &[ROOT, REPO],
        example: "docsys migrate inventory > plan.tsv",
    },
    Command {
        name: "migrate apply",
        synopsis: "--plan <file> [--root <dir>] [--lang <code>] [--repo <dir>]",
        purpose: "the classification is approved: the files move into the tree",
        flags: &[
            ("--plan <file>", "the approved plan"),
            ROOT,
            ("--lang <code>", "the language the pages are written in"),
            REPO,
        ],
        example: "docsys migrate apply --plan plan.tsv",
    },
    Command {
        name: "rules",
        synopsis: "--agents-md | --procedures [--max-lines <n>] [--write <file>]",
        purpose: "the agent rules are wanted on their own: the rules block, or the procedures an agent follows",
        flags: &[
            ("--agents-md", "the rules block"),
            ("--procedures", "the procedures"),
            ("--max-lines <n>", "a line budget for the text"),
            ("--write <file>", "write it there instead of printing it"),
        ],
        example: "docsys rules --agents-md --write AGENTS.md",
    },
    Command {
        name: "agents",
        synopsis: "[--dir .claude] [--force] | --kb [--root <base>] | --report",
        purpose: "the agent layer needs installing or refreshing: hooks, skills and commands",
        flags: &[
            ("--dir <dir>", "the agent layer's directory, `.claude` by default"),
            ("--force", "overwrite what is there"),
            ("--kb", "the knowledge-base layer"),
            ("--report", "what agent layer exists and which shell commands it runs; writes nothing"),
            ROOT,
        ],
        example: "docsys agents --dir .claude",
    },
    Command {
        name: "hook",
        synopsis: "pre-tool-use|stop|post-tool-use|user-prompt-submit [--repo .] [--root docs]",
        purpose: "what the installed agent relays call; a person does not run it",
        flags: &[
            ("<event>", "the agent event the relay forwards"),
            REPO,
            ROOT,
        ],
        example: "docsys hook stop",
    },
    Command {
        name: "feedback",
        synopsis: "[--draft] [--type bug|false-positive|need] [--rule R-xxx] [--command \"docsys …\"] [--out <file>]",
        purpose: "docsys is wrong or in your way: the guide, or an issue drafted with the facts filled in; it files nothing",
        flags: &[
            ("--draft", "draft the issue"),
            ("--type bug|false-positive|need", "what kind of report"),
            ("--rule R-xxx", "the rule a finding came from"),
            ("--command \"docsys …\"", "the command that misbehaved"),
            ("--out <file>", "where the draft goes"),
        ],
        example: "docsys feedback --draft --type false-positive --rule R-071",
    },
    Command {
        name: "version",
        synopsis: "(also --version, -V)",
        purpose: "which docsys this is, the spec it implements, and the version this tree pins",
        flags: &[],
        example: "docsys --version",
    },
    Command {
        name: "help",
        synopsis: "[<command> [<subcommand>]]",
        purpose: "this list; with a command, its flags and an example (also `docsys <command> --help`)",
        flags: &[("<command>", "the command to explain")],
        example: "docsys help graduate apply",
    },
];

const HEAD: &str =
    "docsys — keeps a repository's documentation true to its code: typed pages, checked
by lint, bound to the code they describe (spec: SPEC.md).
In a repository: `docsys adopt` sets it up — the tree, the agent rules, the hooks and
the git gate — and lists in ADOPTION.md what is left. `docsys <command> --help` gives
a command's flags and an example.

";

const TAIL: &str = "
--root names a tree, `docs` by default: from any directory inside the repository the
nearest tree above is found, and the repository is the tree's own (D-098). init, adopt
and assistant create a tree where they are pointed.

A pinned tree (<root>/.docsys-version) runs its own version, installed once into
$DOCSYS_HOME (~/.docsys); DOCSYS_NO_AUTO_INSTALL=1 prints the install command instead (D-120).

Exit codes (the contract scripts and CI read):
  0  ok — clean, or warnings only (warnings inform; they never block)
  1  blocking findings — the tree violates an error-level rule
  2  could not evaluate — missing docs root / .docmeta.yml, or bad invocation
";

/// `docsys --help`: every command and what it is for.
pub fn overview() -> String {
    let width = COMMANDS.iter().map(|c| c.name.len()).max().unwrap_or(0);
    let mut out = String::from(HEAD);
    out.push_str("Commands:\n");
    for c in COMMANDS {
        let _ = writeln!(out, "  docsys {:<width$}  {}", c.name, c.purpose);
    }
    out.push_str(TAIL);
    out
}

/// `docsys <command> [<subcommand>] --help`: the entries the words name — one
/// command, or every subcommand of a command named alone.
pub fn of(words: &[&str]) -> Option<String> {
    // the command itself, then its sub-commands
    let name = words.join(" ");
    let prefix = format!("{name} ");
    let hits: Vec<&Command> = COMMANDS
        .iter()
        .filter(|c| c.name == name)
        .chain(
            COMMANDS
                .iter()
                .filter(|c| !name.is_empty() && c.name.starts_with(&prefix)),
        )
        .collect();
    if hits.is_empty() {
        return None;
    }
    let mut out = String::new();
    for (i, c) in hits.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let _ = writeln!(out, "docsys {} {}\n\n{}", c.name, c.synopsis, c.purpose);
        if !c.flags.is_empty() {
            out.push('\n');
            let width = c.flags.iter().map(|(f, _)| f.len()).max().unwrap_or(0);
            for (flag, what) in c.flags {
                let _ = writeln!(out, "  {flag:<width$}  {what}");
            }
        }
        let _ = writeln!(out, "\nExample:\n  {}", c.example);
    }
    Some(out)
}
