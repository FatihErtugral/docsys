//! The command table: one entry per command the binary dispatches, read by
//! `docsys --help` (what each is for) and `docsys <command> --help` (its flags
//! and one example). A test holds it to the dispatch in `main.rs`.

use std::fmt::Write as _;

/// One command as a person meets it.
#[derive(Debug)]
pub struct Command {
    /// the words that name it: `lint`, `graduate apply`
    pub name: &'static str,
    /// what follows the name
    pub synopsis: &'static str,
    /// the situation it is for, in one line
    pub purpose: &'static str,
    /// each flag or argument, and what it does
    pub flags: &'static [(&'static str, &'static str)],
    /// the words it takes after its name, besides its flags and their values
    pub words: Words,
    /// other names a person types for it
    pub aliases: &'static [&'static str],
    /// one invocation as a person types it
    pub example: &'static str,
}

/// The words a command takes after its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Words {
    None,
    /// at most this many
    UpTo(usize),
    /// any number
    Any,
    /// one of these
    OneOf(&'static [&'static str]),
}

/// The flags that take a value; every other flag is a switch. The parser and
/// the refusals read this one list (D-129).
pub const VALUE_FLAGS: &[&str] = &[
    "--root",
    "--repo",
    "--dir",
    "--lang",
    "--plan",
    "--out",
    "--title",
    "--audience",
    "--profile",
    "--by",
    "--confirmed",
    "--target",
    "--since",
    "--memory",
    "--note",
    "--reason",
    "--message",
    "--deferred",
    "--repay-when",
    "--answer",
    "--topic",
    "--context",
    "--date",
    "--link",
    "--format",
    "--type",
    "--rule",
    "--command",
    "--write",
    "--max-lines",
    "--range",
    "--symbol",
    "--as",
    "--source",
    "--id",
    "--url",
    "--rules-file",
    "--report-dir",
    "--ci-runner",
    "--ci-install",
    "--ci-sha256",
    "--verify-on-approval",
    "--projects",
    "--domains",
    "--limit",
];

/// Whether `flag` takes the argument after it as its value.
pub fn takes_value(flag: &str) -> bool {
    VALUE_FLAGS.contains(&flag)
}

/// A flag's form: `--…`, or `-` and a letter. A flag's value is never one;
/// `-1` and `-` alone are values (D-129).
pub fn is_flag(arg: &str) -> bool {
    arg.starts_with("--")
        || arg
            .strip_prefix('-')
            .is_some_and(|f| f.starts_with(|c: char| c.is_ascii_alphabetic()))
}

/// The events `docsys hook` relays.
pub const EVENTS: &[&str] = &[
    "pre-tool-use",
    "stop",
    "post-tool-use",
    "user-prompt-submit",
];

const JSON: (&str, &str) = (
    "--json",
    "the same answer as JSON, for a script or an agent",
);

pub const COMMANDS: &[Command] = &[
    Command {
        name: "adopt",
        synopsis: "[--repo .] [--root docs] [--lang <code>] [--rules-file <path>] [--report-dir <dir> | --no-report] [--ci-runner <label>,…] [--ci-install cargo|release] [--ci-sha256 <target>=<hex>,…] [--verify-on-approval description|pull-request|direct|off] [--obsidian]",
        purpose: "a repository starts using docsys: the tree, the agent rules, the agent's hooks, skills and slash commands, and the git gate, with ADOPTION.md listing what is left; a re-run brings them up to date",
        flags: &[
            ("--lang <code>", "the language the pages are written in"),
            ("--rules-file <path>", "the file the rules block goes to; by default where its markers are"),
            ("--report-dir <dir>", "where ADOPTION.md goes"),
            ("--no-report", "write no ADOPTION.md"),
            ("--ci-runner <label>,…", "the runner labels of the CI workflow written when .github/ exists"),
            ("--ci-install cargo|release", "how the workflow installs docsys"),
            ("--ci-sha256 <target>=<hex>,…", "the release archives' checksums, with --ci-install release on a docsys/0.4 tree; a docsys/0.5 workflow checks the archive against its release's SHA256SUMS and refuses the flag"),
            ("--verify-on-approval <mode>", "a docsys/0.4 tree's approval job: `pull-request` (a follow-up pull request, the default), `direct` (a push) or `off`"),
            ("--obsidian", "also write .obsidian settings and a stale-work view"),
        ],
        words: Words::None,
        aliases: &[],
        example: "docsys adopt --repo . --root docs",
    },
    Command {
        name: "init",
        synopsis: "[--root <dir>] [--lang <code>] [--profile project|knowledge-base]",
        purpose: "only an empty tree is wanted",
        flags: &[
            ("--lang <code>", "the language the pages are written in"),
            ("--profile project|knowledge-base", "a repository's documentation, or a knowledge base of records and wiki pages"),
        ],
        words: Words::None,
        aliases: &[],
        example: "docsys init --root docs --lang en",
    },
    Command {
        name: "upgrade",
        synopsis: "[--apply] [--commit] [--force] [--json] [--root docs] [--dir .claude]",
        purpose: "this docsys is newer than the spec the tree declares: it moves the tree, one spec at a time, then the pin",
        flags: &[
            ("--apply", "make the move; without it only the plan is printed"),
            ("--commit", "commit the move as one commit"),
            ("--force", "run on a working tree with uncommitted changes"),
            JSON,
            ("--dir <dir>", "the agent layer's directory, `.claude` by default"),
        ],
        words: Words::None,
        aliases: &[],
        example: "docsys upgrade --apply --commit",
    },
    Command {
        name: "lint",
        synopsis: "[--root <dir>] [--repo <dir>] [--json]",
        purpose: "you want to know whether the tree is right: every rule, with pins and history inside a repository",
        flags: &[JSON],
        words: Words::None,
        aliases: &[],
        example: "docsys lint",
    },
    Command {
        name: "refs",
        synopsis: "[--repo <dir>] [--root <dir>] [--json]",
        purpose: "the code side is in question: every `doc:` citation in the code resolves to a page",
        flags: &[JSON],
        words: Words::None,
        aliases: &[],
        example: "docsys refs",
    },
    Command {
        name: "gate",
        synopsis: "[--repo .] [--root docs] [--range <a>...<b>] [--skipped] | --message <file>",
        purpose: "what the git hooks and CI run: lint, and code changed without documentation",
        flags: &[
            ("--range <a>...<b>", "a pull request's commits, in CI"),
            ("--skipped", "record a bypassed gate as a debt item"),
            ("--message <file>", "the commit-msg half on docsys/0.5: the message the commit will carry"),
        ],
        words: Words::None,
        aliases: &[],
        example: "docsys gate --range origin/main...HEAD",
    },
    Command {
        name: "doctor",
        synopsis: "[--repo .] [--root docs] [--dir .claude]",
        purpose: "the documentation pipeline seems silent: are the hooks, the gate and the relays wired and alive",
        flags: &[("--dir <dir>", "the agent layer's directory")],
        words: Words::None,
        aliases: &[],
        example: "docsys doctor",
    },
    Command {
        name: "status",
        synopsis: "[--root <dir>] [--repo <dir>] [--json]",
        purpose: "the tree's state at a glance: the inbox, pages by state, open items, consumed namespaces, findings",
        flags: &[JSON],
        words: Words::None,
        aliases: &[],
        example: "docsys status",
    },
    Command {
        name: "lookup",
        synopsis: "<word…> [--root docs] [--json]",
        purpose: "a question names a few words: the pages, local and consumed, that hold them — the first hop before reading",
        flags: &[("<word…>", "the words the question names"), JSON],
        words: Words::Any,
        aliases: &[],
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
            ("--unverified", "a docsys/0.4 tree's page from evidence: `verification: unverified` and an empty `sources:`"),
        ],
        words: Words::UpTo(2),
        aliases: &[],
        example: "docsys page new reference token-ttl --title \"Token lifetime\"",
    },
    Command {
        name: "pin",
        synopsis: "<page> <path> [--symbol <s>] | --refresh <page> | --gc  [--repo .] [--root docs]",
        purpose: "lint says when the code a page describes moves away from it",
        flags: &[
            ("<page> <path>", "the page, and the code file it describes"),
            ("--symbol <s>", "the declaration in that file, instead of the whole file"),
            ("--refresh <page>", "record the region as it reads now"),
            ("--gc", "remove acknowledgements no current pin needs"),
        ],
        words: Words::UpTo(2),
        aliases: &[],
        example: "docsys pin reference/token-ttl src/auth.rs --symbol refresh_token",
    },
    Command {
        name: "verify",
        synopsis: "<page> [--by <handle|@login>] [--commit] [--revoke] | --range <a>...<b> (--by @login | --from-trailers)  [--root <dir>]",
        purpose: "a docsys/0.4 tree's page verification, as 0.15.1 kept it",
        flags: &[
            ("<page>", "the page's id or path"),
            ("--by <handle|@login>", "who verifies, when it is not the git identity"),
            ("--commit", "make the commit at once"),
            ("--revoke", "take the verification back"),
            ("--range <a>...<b>", "every page a pull request touched, under the reviewer's identity"),
            ("--from-trailers", "with --range: the identity from the `Reviewed-by:` or `Approved-by:` trailer"),
        ],
        words: Words::UpTo(1),
        aliases: &[],
        example: "docsys verify reference/token-ttl",
    },
    Command {
        name: "compile",
        synopsis: "<howto> [--root docs] [--dir .claude] [--force]",
        purpose: "a howto's steps are complete: its body becomes an agent skill, pinned to the page",
        flags: &[
            ("<howto>", "the howto page"),
            ("--dir <dir>", "the agent layer's directory"),
            ("--force", "overwrite the skill"),
        ],
        words: Words::UpTo(1),
        aliases: &[],
        example: "docsys compile howto/rotate-keys",
    },
    Command {
        name: "graduate plan",
        synopsis: "<work-file> [--root <dir>]",
        purpose: "work is done and its knowledge must reach permanent pages: the work file's blocks, for an agent to map to destinations",
        flags: &[("<work-file>", "the work file, relative to the tree")],
        words: Words::UpTo(1),
        aliases: &[],
        example: "docsys graduate plan work/features/cart-key.md > plan.tsv",
    },
    Command {
        name: "graduate apply",
        synopsis: "--plan <file> [--confirmed <who>] [--root <dir>] [--force]",
        purpose: "the person approved the plan: the blocks move byte for byte",
        flags: &[
            ("--plan <file>", "the filled plan"),
            ("--confirmed <who>", "who confirmed it: the last blocks move and the file is removed"),
            ("--force", "run on a working tree with uncommitted changes"),
        ],
        words: Words::None,
        aliases: &[],
        example: "docsys graduate apply --plan plan.tsv --confirmed \"$(git config user.name)\"",
    },
    Command {
        name: "debt add",
        synopsis: "<debt…> --deferred <reason> --repay-when <trigger> [--topic <id>] [--date <d>] [--root docs]",
        purpose: "one dated debt item, in its topic's file",
        flags: &[
            ("<debt…>", "what is deferred"),
            ("--deferred <reason>", "why it waits"),
            ("--repay-when <trigger>", "the event that makes it due"),
            ("--topic <id>", "the page or feature it concerns; `general` without one"),
            ("--date <d>", "the date, today by default"),
        ],
        words: Words::Any,
        aliases: &[],
        example: "docsys debt add \"retries are unbounded\" --deferred \"no load yet\" --repay-when \"the next outage\" --topic retry-policy",
    },
    Command {
        name: "debt close",
        synopsis: "<n|words> --note <how> [--root docs]",
        purpose: "takes a debt's line out; `Resolved:` in the commit records how",
        flags: &[
            ("<n|words>", "the item's number in the list, or words only it holds"),
            ("--note <how>", "how it was repaid"),
        ],
        words: Words::Any,
        aliases: &[],
        example: "docsys debt close \"retries are unbounded\" --note \"bounded at three\"",
    },
    Command {
        name: "question add",
        synopsis: "<question…> [--topic <id>] [--context <c>] [--date <d>] [--root docs]",
        purpose: "one dated question item",
        flags: &[
            ("<question…>", "the question"),
            ("--topic <id>", "the page or feature it concerns"),
            ("--context <c>", "where it came up"),
            ("--date <d>", "the date, today by default"),
        ],
        words: Words::Any,
        aliases: &[],
        example: "docsys question add \"who owns the cache?\" --topic cache",
    },
    Command {
        name: "question close",
        synopsis: "<n|words> --answer <line> [--root docs]",
        purpose: "a question is answered: the answer rides the commit as `Answered:`, the line goes",
        flags: &[
            ("<n|words>", "the item's number in the list, or words only it holds"),
            ("--answer <line>", "the answer"),
        ],
        words: Words::Any,
        aliases: &[],
        example: "docsys question close \"who owns the cache\" --answer \"the platform team\"",
    },
    Command {
        name: "ledger fix",
        synopsis: "[--root <dir>]",
        purpose: "a ledger or its archive slice uses em-dash field markers: rewrite them to the ASCII ones, the field text untouched",
        flags: &[],
        words: Words::None,
        aliases: &[],
        example: "docsys ledger fix",
    },
    Command {
        name: "journal",
        synopsis: "[--since <date>] [--root docs]",
        purpose: "you want what changed and why: every commit that changed the docs or carries `Docs:`, newest first",
        flags: &[("--since <date>", "only the entries from that day on")],
        words: Words::None,
        aliases: &[],
        example: "docsys journal --since 2026-09-01",
    },
    Command {
        name: "journal add",
        synopsis: "<text…> [--title <t>] [--link <path>] [--date <d>] [--root docs]",
        purpose: "on docsys/0.5, the `Docs:` line a commit message ends with, printed; nothing is written",
        flags: &[
            ("<text…>", "what changed and why"),
            ("--title <t>", "the entry's title: the whole message printed"),
            ("--link <path>", "the page it concerns: the text becomes the subject"),
            ("--date <d>", "the date, on a docsys/0.4 tree"),
        ],
        words: Words::Any,
        aliases: &[],
        example: "docsys journal add \"the run page states the timeout\" --link reference/run",
    },
    Command {
        name: "seed plan",
        synopsis: "[--target <feature>] [--since <date>] [--memory <dir>] [--repo .] [--root docs]",
        purpose: "the feature inventory a seeding starts from, as evidence",
        flags: &[
            ("--target <feature>", "one feature's evidence instead of the inventory"),
            ("--since <date>", "only history from that day on"),
            ("--memory <dir>", "an agent's memory directory, each note listed beside the evidence"),
        ],
        words: Words::None,
        aliases: &[],
        example: "docsys seed plan --target checkout",
    },
    Command {
        name: "seed gaps",
        synopsis: "[--since <date>] [--repo .] [--root docs]",
        purpose: "an interview is planned: the inventory as JSON, uncovered features first",
        flags: &[("--since <date>", "only history from that day on")],
        words: Words::None,
        aliases: &[],
        example: "docsys seed gaps",
    },
    Command {
        name: "seed apply",
        synopsis: "--plan <file> [--repo .] [--root docs] [--force]",
        purpose: "the builder approved the seeding rows: they land under work/",
        flags: &[
            ("--plan <file>", "the approved rows"),
            ("--force", "run on a working tree with uncommitted changes"),
        ],
        words: Words::None,
        aliases: &[],
        example: "docsys seed apply --plan SEED.tsv",
    },
    Command {
        name: "backlinks",
        synopsis: "<path|id|code-file> [--repo .] [--root docs]",
        purpose: "before changing a page or a code file: what points at it — pages, code, pins",
        flags: &[("<path|id|code-file>", "a page, or a code file")],
        words: Words::UpTo(1),
        aliases: &[],
        example: "docsys backlinks reference/token-ttl",
    },
    Command {
        name: "mentions",
        synopsis: "[<path|id>] [--root docs]",
        purpose: "prose names a page without linking it: where, so the link can be added",
        flags: &[("<path|id>", "one page; every page by default")],
        words: Words::UpTo(1),
        aliases: &[],
        example: "docsys mentions reference/token-ttl",
    },
    Command {
        name: "crosscheck",
        synopsis: "[<page>…] [--since <ref>] [--root <dir>] [--repo <dir>] [--json]",
        purpose: "pages are read against what they rest on, with an agent and `/docsys-crosscheck`: each page, its `sources:` and the code lines its pins resolve to now; writes nothing",
        flags: &[
            ("<page>…", "the pages, by id or path"),
            ("--since <ref>", "every permanent page changed since that revision, committed or not"),
            JSON,
        ],
        words: Words::Any,
        aliases: &[],
        example: "docsys crosscheck --since origin/main",
    },
    Command {
        name: "graph",
        synopsis: "[--format dot|json|jsoncanvas] [--repo .] [--root docs]",
        purpose: "you want to see the tree's links: the graph for a viewer",
        flags: &[("--format dot|json|jsoncanvas", "the output format")],
        words: Words::None,
        aliases: &[],
        example: "docsys graph --format dot > docs.dot",
    },
    Command {
        name: "export plan",
        synopsis: "[--root <dir>] [--audience <a>]",
        purpose: "an audience needs one document: a draft product map to edit before exporting",
        flags: &[("--audience <a>", "who reads it: end-user, developer, designer, …")],
        words: Words::None,
        aliases: &[],
        example: "docsys export plan --audience end-user > map.md",
    },
    Command {
        name: "export product",
        synopsis: "<map> [--root <dir>] [--out <file>] [--lang <code>] [--audience <a>]",
        purpose: "the product map is ready: one document composed from the pages it names",
        flags: &[
            ("<map>", "the edited product map"),
            ("--out <file>", "where the document goes; standard output by default"),
            ("--lang <code>", "the language it is wanted in"),
            ("--audience <a>", "who reads it"),
        ],
        words: Words::UpTo(1),
        aliases: &[],
        example: "docsys export product map.md --out product.md",
    },
    Command {
        name: "export feature",
        synopsis: "<id> [<id>…] [--follow] [--title <t>] [--root <dir>] [--out <file>] [--lang <code>] [--audience <a>]",
        purpose: "one feature needs its own document, made of the pages named",
        flags: &[
            ("<id>…", "the pages"),
            ("--follow", "also the pages they link"),
            ("--title <t>", "the document's title"),
            ("--out <file>", "where the document goes"),
            ("--lang <code>", "the language it is wanted in"),
            ("--audience <a>", "who reads it"),
        ],
        words: Words::Any,
        aliases: &[],
        example: "docsys export feature token-ttl --follow --out tokens.md",
    },
    Command {
        name: "export manifest",
        synopsis: "[--root <dir>] [--out <file>]",
        purpose: "another tree consumes this one: what this namespace exports",
        flags: &[("--out <file>", "where the manifest goes")],
        words: Words::None,
        aliases: &[],
        example: "docsys export manifest --out manifest.yml",
    },
    Command {
        name: "fetch",
        synopsis: "[--root <dir>]",
        purpose: "this tree consumes other namespaces: bring their pages into .federation/",
        flags: &[],
        words: Words::None,
        aliases: &[],
        example: "docsys fetch",
    },
    Command {
        name: "consume add",
        synopsis: "<path|git-url>[#subdir] [--as <ns>] [--root docs]",
        purpose: "this tree should read another tree's pages: add the provider to its consume: list",
        flags: &[
            ("<path|git-url>[#subdir]", "the provider"),
            ("--as <ns>", "the namespace it is read under"),
        ],
        words: Words::UpTo(1),
        aliases: &[],
        example: "docsys consume add ../platform#docs --as platform",
    },
    Command {
        name: "consume discover",
        synopsis: "<dir> [--root docs]",
        purpose: "you look for trees to consume: the docsys trees one level under a directory; writes nothing",
        flags: &[("<dir>", "where to look")],
        words: Words::UpTo(1),
        aliases: &[],
        example: "docsys consume discover ~/src",
    },
    Command {
        name: "inbox add",
        synopsis: "--source <name> --id <item> [--title <t>] [--url <u>] [--date <d>] [<file>|-] [--root <dir>]",
        purpose: "a connector brings a record into a knowledge base: it lands in raw/inbox/ once, with its provenance",
        flags: &[
            ("--source <name>", "where it comes from"),
            ("--id <item>", "its identifier there"),
            ("--title <t>", "its title"),
            ("--url <u>", "its address"),
            ("--date <d>", "its date"),
            ("<file>|-", "its text, from a file or standard input"),
        ],
        words: Words::UpTo(1),
        aliases: &[],
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
        ],
        words: Words::UpTo(1),
        aliases: &[],
        example: "docsys inbox pull ../api --since 2026-09-01",
    },
    Command {
        name: "raw move",
        synopsis: "<record> <domain> [--root <dir>]",
        purpose: "an inbox record is filed: it moves to raw/<domain>/ through git, bytes untouched, and every citing page follows",
        flags: &[("<record>", "the record in raw/inbox/"), ("<domain>", "where it belongs")],
        words: Words::UpTo(2),
        aliases: &[],
        example: "docsys raw move raw/inbox/2026-09-01-call.md payments",
    },
    Command {
        name: "forget",
        synopsis: "<page-id|page-path|record-path> --reason <text> [--root <dir>]",
        purpose: "a page or record must leave: a page to _archive/ with a tombstone, a record to raw/_forgotten/; the ledger says why",
        flags: &[
            ("<page-id|page-path|record-path>", "what leaves"),
            ("--reason <text>", "why"),
        ],
        words: Words::UpTo(1),
        aliases: &[],
        example: "docsys forget reference/old-flag --reason \"the flag was removed\"",
    },
    Command {
        name: "assistant",
        synopsis: "[--root .] [--projects <dir>]… [--domains a,b] [--since 30.days] [--limit 3]",
        purpose: "an assistant's memory in one step: the base, its agent layer, the projects it consumes, its pages and records",
        flags: &[
            ("--projects <dir>", "a directory of projects to consume; may repeat"),
            ("--domains a,b", "the domains the base files its records under"),
            ("--since <span>", "how far back each project's commits come into the inbox"),
            ("--limit <n>", "at most this many commits per project"),
        ],
        words: Words::None,
        aliases: &[],
        example: "docsys assistant --root ~/notes --projects ~/src",
    },
    Command {
        name: "migrate inventory",
        synopsis: "[--root <dir>] [--repo <dir>]",
        purpose: "a repository has documentation in another shape: the plan skeleton that classifies every file",
        flags: &[],
        words: Words::None,
        aliases: &[],
        example: "docsys migrate inventory > plan.tsv",
    },
    Command {
        name: "migrate apply",
        synopsis: "--plan <file> [--root <dir>] [--lang <code>] [--repo <dir>]",
        purpose: "the classification is approved: the files move into the tree",
        flags: &[
            ("--plan <file>", "the approved plan"),
            ("--lang <code>", "the language the pages are written in"),
        ],
        words: Words::None,
        aliases: &[],
        example: "docsys migrate apply --plan plan.tsv",
    },
    Command {
        name: "rules",
        synopsis: "--agents-md | --procedures [--max-lines <n>] [--write <file>]",
        purpose: "the agent rules are wanted on their own",
        flags: &[
            ("--agents-md", "the rules block AGENTS.md carries"),
            ("--procedures", "the procedures an agent follows"),
            ("--max-lines <n>", "a line budget for the text"),
            ("--write <file>", "write it there instead of printing it"),
        ],
        words: Words::None,
        aliases: &[],
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
        ],
        words: Words::None,
        aliases: &[],
        example: "docsys agents --dir .claude",
    },
    Command {
        name: "hook",
        synopsis: "pre-tool-use|stop|post-tool-use|user-prompt-submit [--repo .] [--root docs] [--stdin]",
        purpose: "what the installed agent relays call; a person does not run it",
        flags: &[
            ("<event>", "the agent event the relay forwards"),
            ("--stdin", "the event's payload, read from standard input"),
        ],
        words: Words::OneOf(EVENTS),
        aliases: &[],
        example: "docsys hook stop",
    },
    Command {
        name: "feedback",
        synopsis: "[--draft] [--type bug|false-positive|need] [--rule R-xxx] [--command \"docsys …\"] [--out <file>]",
        purpose: "a report on docsys itself: the guide, or an issue drafted with the facts filled in; it files nothing",
        flags: &[
            ("--draft", "draft the issue"),
            ("--type bug|false-positive|need", "what kind of report"),
            ("--rule R-xxx", "the rule a finding came from"),
            ("--command \"docsys …\"", "the command that misbehaved"),
            ("--out <file>", "where the draft goes"),
        ],
        words: Words::None,
        aliases: &[],
        example: "docsys feedback --draft --type false-positive --rule R-071",
    },
    Command {
        name: "version",
        synopsis: "",
        purpose: "which docsys this is, the spec it implements, and the version this tree pins",
        flags: &[],
        words: Words::None,
        aliases: &["--version", "-V"],
        example: "docsys --version",
    },
    Command {
        name: "help",
        synopsis: "[<command> [<subcommand>]]",
        purpose: "this list; with a command — or `docsys <command> --help` — its flags and an example",
        flags: &[("<command>", "the command to explain")],
        words: Words::Any,
        aliases: &["--help", "-h"],
        example: "docsys help graduate apply",
    },
];

const HEAD: &str =
    "docsys — keeps a repository's documentation true to its code: typed pages, checked
by lint, bound to the code they describe (spec: SPEC.md).

";

const TAIL: &str = "
--root names a tree, `docs` by default: from any directory inside the repository the
nearest tree above is found. The repository is the tree's own; a given --repo is read
as its top level, and a relative --dir as the agent layer there (D-098). init, adopt and
assistant create a tree where they are pointed.

$DOCSYS_HOME (~/.docsys) holds the versions pinned trees install;
DOCSYS_NO_AUTO_INSTALL=1 prints the install command instead (D-120).

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

/// The entry the words name: the command itself, or — for a command whose
/// first argument is a word of its own, as `hook stop` — that command's.
pub fn entry(words: &[&str]) -> Option<&'static Command> {
    let name = words.join(" ");
    COMMANDS
        .iter()
        .find(|c| c.name == name || c.aliases.contains(&name.as_str()))
        .or_else(|| match words {
            [first, _]
                if !COMMANDS
                    .iter()
                    .any(|c| c.name.starts_with(&format!("{first} "))) =>
            {
                COMMANDS.iter().find(|c| c.name == *first)
            }
            _ => None,
        })
}

/// The flags the entry `words` names accepts: each `--flag` its synopsis and
/// its flag lines name.
pub fn flags_of(words: &[&str]) -> Option<Vec<&'static str>> {
    let c = entry(words)?;
    let mut out = Vec::new();
    for text in std::iter::once(c.synopsis).chain(c.flags.iter().map(|(f, _)| *f)) {
        for (at, _) in text.match_indices("--") {
            let flag = text.get(at..).unwrap_or("");
            let len = 2 + flag
                .get(2..)
                .unwrap_or("")
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-')
                .count();
            if let Some(flag) = flag.get(..len).filter(|f| f.len() > 2) {
                if !out.contains(&flag) {
                    out.push(flag);
                }
            }
        }
    }
    Some(out)
}

/// `docsys <command> [<subcommand>] --help`: the entries the words name — one
/// command, or every subcommand of a command named alone.
pub fn of(words: &[&str]) -> Option<String> {
    // the command itself, then its sub-commands; a word its own command takes
    // as an argument, that command's entry
    if let (Some(c), [_, _]) = (entry(words), words) {
        if c.name != words.join(" ") {
            return of(&[c.name]);
        }
    }
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
        let mut head = format!("docsys {}", c.name);
        if !c.synopsis.is_empty() {
            head.push(' ');
            head.push_str(c.synopsis);
        }
        if !c.aliases.is_empty() {
            let _ = write!(head, " (also {})", c.aliases.join(", "));
        }
        let _ = writeln!(out, "{head}\n\n{}", c.purpose);
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

/// What a command line asks, as the table reads it (D-129).
#[derive(Debug)]
pub enum Read {
    /// run `entry` with its words; its own arguments start at `from`
    Run {
        entry: &'static Command,
        words: Vec<String>,
        from: usize,
    },
    /// print this and succeed
    Help(String),
}

/// What the table refuses: the line naming it — empty when the entries say
/// it all — then the nearest entries.
#[derive(Debug)]
pub struct Refusal {
    pub line: String,
    pub entries: String,
}

fn find(name: &str) -> Option<&'static Command> {
    COMMANDS
        .iter()
        .find(|c| c.name == name || c.aliases.contains(&name))
}

fn is_group(name: &str) -> bool {
    COMMANDS
        .iter()
        .any(|c| c.name.starts_with(&format!("{name} ")))
}

/// A command docsys does not know, named, with the nearest entries.
pub fn unknown(words: &[&str]) -> Refusal {
    Refusal {
        line: format!("docsys: `{}` is no docsys command", words.join(" ")),
        entries: words
            .first()
            .and_then(|w| of(&[w]))
            .unwrap_or_else(overview),
    }
}

/// A flag `name` does not take, and the commands that take it, where one or
/// two do.
fn not_a_flag(name: &str, flag: &str) -> String {
    let owners: Vec<&str> = COMMANDS
        .iter()
        .filter(|c| flags_of(&[c.name]).is_some_and(|f| f.contains(&flag)))
        .map(|c| c.name)
        .collect();
    let theirs = match owners.as_slice() {
        [one] => format!(" — `docsys {one} {flag}` takes it"),
        [a, b] => format!(" — `docsys {a}` and `docsys {b}` take it"),
        _ => String::new(),
    };
    format!("`{flag}` is no flag of {name}{theirs}")
}

/// Read a command line: the command it names, then each argument against
/// that command's entry — a flag it does not name, a single-dash flag and a
/// word beyond the words it takes are refused, named; `--help`, `-h` and
/// `help <command> <words>` answer with the entry.
pub fn read(args: &[String]) -> Result<Read, Refusal> {
    let Some(first) = args.first().map(String::as_str) else {
        return Err(Refusal {
            line: String::new(),
            entries: overview(),
        });
    };
    let (entry, from) = if is_group(first) {
        match args.get(1).map(String::as_str) {
            Some(w) if !w.starts_with('-') => match find(&format!("{first} {w}")) {
                Some(c) => (c, 2),
                None => return Err(unknown(&[first, w])),
            },
            _ => match find(first) {
                Some(c) => (c, 1),
                None => {
                    // a group named alone: its commands
                    let entries = of(&[first]).unwrap_or_default();
                    let rest = args.get(1..).unwrap_or_default();
                    return match rest.iter().find(|a| *a != "--help" && *a != "-h") {
                        Some(stray) => Err(Refusal {
                            line: format!(
                                "`{stray}` is no flag of {first} — name one of its commands first"
                            ),
                            entries,
                        }),
                        None if rest.is_empty() => Err(Refusal {
                            line: String::new(),
                            entries,
                        }),
                        None => Ok(Read::Help(entries)),
                    };
                }
            },
        }
    } else {
        match find(first) {
            Some(c) => (c, 1),
            None => return Err(unknown(&[first])),
        }
    };
    let (words, help) = check(entry, args.get(from..).unwrap_or_default())?;
    if entry.name == "help" {
        // `help <command> <words>` is `<command> <words> --help`
        if words.is_empty() {
            return Ok(Read::Help(if help {
                of(&[entry.name]).unwrap_or_default()
            } else {
                overview()
            }));
        }
        let mut asked = words;
        asked.push("--help".to_string());
        return read(&asked);
    }
    if help {
        return Ok(Read::Help(of(&[entry.name]).unwrap_or_default()));
    }
    Ok(Read::Run { entry, words, from })
}

/// The arguments after a command's name, against its entry: its words, and
/// whether help was asked.
fn check(entry: &'static Command, rest: &[String]) -> Result<(Vec<String>, bool), Refusal> {
    let own = flags_of(&[entry.name]).unwrap_or_default();
    let refuse = |line: String| Refusal {
        line,
        entries: of(&[entry.name]).unwrap_or_default(),
    };
    let mut words = Vec::new();
    let mut help = false;
    let mut it = rest.iter().peekable();
    while let Some(a) = it.next() {
        let a = a.as_str();
        if a == "--help" || a == "-h" {
            help = true;
        } else if a.starts_with("--") {
            if !own.contains(&a) {
                return Err(refuse(not_a_flag(entry.name, a)));
            }
            // a value is never another flag: the parser says so
            if takes_value(a) && it.peek().is_some_and(|v| !is_flag(v)) {
                it.next();
            }
        } else if a.starts_with('-') && a.len() > 1 {
            return Err(refuse(format!("`{a}` is no flag of {}", entry.name)));
        } else {
            words.push(a.to_string());
        }
    }
    let stray = match entry.words {
        Words::None => Some(0),
        Words::UpTo(n) => Some(n),
        Words::OneOf(set) => Some(usize::from(
            words.first().is_none_or(|w| set.contains(&w.as_str())),
        )),
        Words::Any => None,
    }
    .and_then(|at| words.get(at).map(|w| (at, w)));
    if let Some((at, w)) = stray {
        let line = match entry.words {
            // only the first word is chosen from the set
            Words::OneOf(set) if at == 0 => format!(
                "`{w}` is no argument of {} — one of: {}",
                entry.name,
                set.join(", ")
            ),
            _ => format!("`{w}` is no argument of {}", entry.name),
        };
        return Err(refuse(line));
    }
    Ok((words, help))
}
