use docsys::{migrate, to_json, Outcome};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "docsys — documentation system tool (spec: SPEC.md)

Usage:
  docsys lint    [--root <dir>] [--repo <dir>] [--json]   # inside a git repository: pins and history too
  docsys pin     <page> <path> [--symbol <s>] [--repo .] [--root docs]   # pin a page to a code region (verifies:, §11)
  docsys pin     --refresh <page> [--repo .] [--root docs]              # recompute its pins after re-reading the page
  docsys compile <howto> [--root docs] [--dir .claude] [--force]        # a howto's body as an executable skill, pinned to its source hash (R-094, R-095)
  docsys lookup  <word…> [--root docs] [--json]   # a question's first hop: pages, local and consumed (@ns/id), naming every word
  docsys consume add <path|git-url>[#subdir] [--as <ns>] [--root docs]   # one provider into this tree's consume: list
  docsys consume discover <dir> [--root docs]     # the docsys trees one level under a directory, as candidates; writes nothing
  docsys init    [--root <dir>] [--lang <code>] [--profile project|knowledge-base]
  docsys migrate inventory [--root <dir>] [--repo <dir>]   # plan skeleton to stdout
  docsys migrate apply --plan <file> [--root <dir>] [--lang <code>] [--repo <dir>]
  docsys refs    --repo <dir> [--root <dir>] [--json]
  docsys rules   --agents-md | --procedures [--max-lines <n>] [--write <file>]
  docsys agents  --report [--dir .claude]    # existing layer + its shell calls
  docsys adopt   [--repo .] [--root docs] [--lang <code>]  # one-command adoption
  docsys agents  [--dir .claude] [--force]   # install hooks + skill + /doc-sync
  docsys agents  --kb [--root <base>] [--dir .claude] [--force]  # knowledge-base layer
  docsys graduate plan <work-file>  [--root <dir>]
  docsys graduate apply --plan <file> [--root <dir>] [--force]
  docsys export plan    [--root <dir>] [--audience <a>]   # draft product map to stdout
  docsys export product <map> [--root <dir>] [--out <file>] [--lang <code>] [--audience <a>]
  docsys export feature <id> [<id>...] [--follow] [--title <t>] [--root <dir>] [--out <file>] [--lang <code>] [--audience <a>]
  docsys export manifest [--root <dir>] [--out <file>]   # what this namespace exports
  docsys fetch   [--root <dir>]              # materialize consumed namespaces into .federation/
  docsys gate    [--repo .] [--root docs] [--range <a>...<b>]   # commit-time question: lint + code-without-docs; --range: a pull request, in CI
  docsys doctor  [--repo .] [--root docs] [--dir .claude]   # is the pipeline itself alive?
  docsys seed    plan [--target <feature>] [--since <date>] [--memory <dir>] [--repo .] [--root docs]
                                             # brownfield: feature inventory, or one feature's history as evidence
  docsys seed    gaps [--since <date>] [--repo .] [--root docs]      # the inventory as JSON, for /docsys-interview
  docsys seed    apply --plan <file> [--repo .] [--root docs] [--force]  # land the approved rows under work/
  docsys debt    close <n> [--note <line>] [--root docs]   # repaid: item leaves the ledger, journal records it
  docsys journal add <text…> [--title <t>] [--date <d>] [--link <path>] [--root docs]
  docsys page    new <category|type> <id> [--title <t>] [--root docs]   # from _templates/, or a permanent skeleton
  docsys backlinks <path|id> [--repo .] [--root docs]      # pages (and code) pointing at a page
  docsys mentions [<path|id>] [--root docs]                 # prose naming a page without a link
  docsys graph   [--format dot|json|jsoncanvas] [--repo .] [--root docs]
  docsys adopt   --obsidian …                # + .obsidian settings and a stale-work .base view
  docsys hook    pre-tool-use|stop|post-tool-use|user-prompt-submit [--repo .] [--root docs]
                                             # agent-hook logic; the installed scripts relay to it

Exit codes (the contract scripts and CI read):
  0  ok — clean, or warnings only (warnings inform; they never block)
  1  blocking findings — the tree violates an error-level rule
  2  could not evaluate — missing docs root / .docmeta.yml, or bad invocation
";

struct Opts {
    root: PathBuf,
    json: bool,
    lang: String,
    lang_explicit: bool,
    title: Option<String>,
    follow: bool,
    audience: Option<String>,
    profile: Option<String>,
    kb: bool,
    plan: Option<PathBuf>,
    out: Option<PathBuf>,
    repo: Option<PathBuf>,
    dir: PathBuf,
    force: bool,
    target: Option<String>,
    since: Option<String>,
    memory: Option<PathBuf>,
    note: Option<String>,
    date: Option<String>,
    link: Option<String>,
    format: Option<String>,
    obsidian: bool,
    agents_md: bool,
    procedures: bool,
    max_lines: usize,
    range: Option<String>,
    refresh: bool,
    symbol: Option<String>,
    as_ns: Option<String>,
    positional: Vec<String>,
}

fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        root: PathBuf::from("docs"),
        json: false,
        lang: "en".to_string(),
        lang_explicit: false,
        title: None,
        follow: false,
        audience: None,
        profile: None,
        kb: false,
        plan: None,
        out: None,
        repo: None,
        dir: PathBuf::from(".claude"),
        force: false,
        target: None,
        since: None,
        memory: None,
        note: None,
        date: None,
        link: None,
        format: None,
        obsidian: false,
        agents_md: false,
        procedures: false,
        max_lines: 200,
        range: None,
        refresh: false,
        symbol: None,
        as_ns: None,
        positional: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--root" => o.root = PathBuf::from(it.next().ok_or("--root needs a value")?),
            "--lang" => {
                o.lang = it.next().ok_or("--lang needs a value")?.clone();
                o.lang_explicit = true;
            }
            "--plan" => o.plan = Some(PathBuf::from(it.next().ok_or("--plan needs a value")?)),
            "--out" => o.out = Some(PathBuf::from(it.next().ok_or("--out needs a value")?)),
            "--title" => o.title = Some(it.next().ok_or("--title needs a value")?.clone()),
            "--follow" => o.follow = true,
            "--audience" => o.audience = Some(it.next().ok_or("--audience needs a value")?.clone()),
            "--profile" => o.profile = Some(it.next().ok_or("--profile needs a value")?.clone()),
            "--kb" => o.kb = true,
            "--repo" => o.repo = Some(PathBuf::from(it.next().ok_or("--repo needs a value")?)),
            "--dir" => o.dir = PathBuf::from(it.next().ok_or("--dir needs a value")?),
            "--force" => o.force = true,
            "--target" => o.target = Some(it.next().ok_or("--target needs a value")?.clone()),
            "--since" => o.since = Some(it.next().ok_or("--since needs a value")?.clone()),
            "--memory" => {
                o.memory = Some(PathBuf::from(it.next().ok_or("--memory needs a value")?))
            }
            "--note" => o.note = Some(it.next().ok_or("--note needs a value")?.clone()),
            "--date" => o.date = Some(it.next().ok_or("--date needs a value")?.clone()),
            "--link" => o.link = Some(it.next().ok_or("--link needs a value")?.clone()),
            "--format" => o.format = Some(it.next().ok_or("--format needs a value")?.clone()),
            "--obsidian" => o.obsidian = true,
            "--agents-md" => o.agents_md = true,
            "--write" => o.plan = Some(PathBuf::from(it.next().ok_or("--write needs a value")?)),
            "--report" => o.procedures = true, // reuse: agents --report
            "--procedures" => o.procedures = true,
            "--max-lines" => {
                o.max_lines = it
                    .next()
                    .ok_or("--max-lines needs a value")?
                    .parse()
                    .map_err(|_| "--max-lines needs a number".to_string())?;
            }
            "--json" => o.json = true,
            "--range" => o.range = Some(it.next().ok_or("--range needs a value")?.clone()),
            "--refresh" => o.refresh = true,
            "--symbol" => o.symbol = Some(it.next().ok_or("--symbol needs a value")?.clone()),
            "--as" => o.as_ns = Some(it.next().ok_or("--as needs a value")?.clone()),
            other if !other.starts_with("--") => o.positional.push(other.to_string()),
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(o)
}

fn run_lint(o: &Opts) -> ExitCode {
    // The repository is where pins resolve and history lives: given, or the
    // one the root sits in. Outside any repository the tree is linted alone.
    let repo = o.repo.clone().or_else(|| docsys::repo_of(&o.root));
    let (report, outcome) = docsys::lint_in(&o.root, repo.as_deref());
    if o.json {
        print!("{}", to_json(&report));
    } else {
        for f in &report.findings {
            println!(
                "{} {} {} [{}] {}",
                f.severity.tag(),
                f.rule,
                f.file,
                f.subject,
                f.message
            );
        }
        let errors = report
            .findings
            .iter()
            .filter(|f| f.severity == docsys::model::Severity::Error)
            .count();
        let warns = report.findings.len() - errors;
        let units: usize = report.inspected.values().sum();
        println!("-- {errors} error(s), {warns} warning(s); {units} unit(s) inspected");
    }
    match outcome {
        Outcome::Clean => ExitCode::SUCCESS,
        Outcome::Errors => ExitCode::from(1),
        Outcome::Config => ExitCode::from(2),
    }
}

/// Print/write a composed export. An unchanged `--out` file is left
/// untouched so nothing downstream re-triggers on a no-op.
fn emit_export(done: &docsys::export::ProductOutcome, out: Option<&std::path::Path>) -> ExitCode {
    for w in &done.warnings {
        eprintln!("WARN {w}");
    }
    match out {
        Some(path) => match docsys::export::write_if_changed(path, &done.output) {
            Ok(true) => {
                eprintln!("composed {} page(s) -> {}", done.pages, path.display());
                ExitCode::SUCCESS
            }
            Ok(false) => {
                eprintln!("unchanged — {} left untouched", path.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("export: {e}");
                ExitCode::from(2)
            }
        },
        None => {
            print!("{}", done.output);
            ExitCode::SUCCESS
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, sub, rest): (&str, Option<&str>, &[String]) = match args.split_first() {
        Some((c, r))
            if c == "migrate"
                || c == "graduate"
                || c == "export"
                || c == "hook"
                || c == "seed"
                || c == "debt"
                || c == "journal"
                || c == "page"
                || c == "consume" =>
        {
            match r.split_first() {
                Some((s, r2)) => (c.as_str(), Some(s.as_str()), r2),
                None => (c.as_str(), None, &[]),
            }
        }
        Some((c, r)) => (c.as_str(), None, r),
        None => ("", None, &[]),
    };
    let opts = match parse_opts(rest) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            eprint!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match (cmd, sub) {
        ("help", _) | ("--help", _) | ("-h", _) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        ("lint", None) => run_lint(&opts),
        ("adopt", None) => {
            let repo = opts.repo.clone().unwrap_or_else(|| PathBuf::from("."));
            let root = if opts.root.is_absolute() {
                opts.root.clone()
            } else {
                repo.join(&opts.root)
            };
            if opts.obsidian {
                match docsys::adopt::obsidian(&root) {
                    Ok(w) if w.is_empty() => println!("obsidian: already configured"),
                    Ok(w) => println!("obsidian: {} written", w.join(", ")),
                    Err(e) => {
                        eprintln!("adopt --obsidian: {e}");
                        return ExitCode::from(2);
                    }
                }
            }
            match docsys::adopt::run(&repo, &root, &opts.lang) {
                Ok(done) => {
                    for s in &done.summary {
                        println!("{s}");
                    }
                    println!("\nreport + judgment checklist: {}", done.report_path);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("adopt: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("rules", None) => {
            if opts.procedures {
                match docsys::rules::procedures() {
                    Some(p) => {
                        print!("{p}");
                        ExitCode::SUCCESS
                    }
                    None => {
                        eprintln!("rules: section 14.3 not found in the embedded spec");
                        ExitCode::from(2)
                    }
                }
            } else if opts.agents_md {
                match docsys::rules::check_budget(opts.max_lines) {
                    Ok(_) => {
                        if let Some(target) = &opts.plan {
                            match docsys::rules::write_agents_block_with(
                                target,
                                &docsys::migrate::generated_preamble(&opts.root),
                            ) {
                                Ok(_) => {
                                    println!(
                                        "managed block written to {} (idempotent)",
                                        target.display()
                                    );
                                    return ExitCode::SUCCESS;
                                }
                                Err(e) => {
                                    eprintln!("rules --write: {e}");
                                    return ExitCode::from(2);
                                }
                            }
                        }
                        print!("{}", docsys::rules::agents_md());
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("rules: {e}");
                        ExitCode::from(1)
                    }
                }
            } else {
                eprintln!("rules needs --agents-md or --procedures");
                ExitCode::from(2)
            }
        }
        ("seed", Some("plan")) => {
            let repo = opts.repo.clone().unwrap_or_else(|| PathBuf::from("."));
            let root = if opts.root.is_absolute() {
                opts.root.clone()
            } else {
                repo.join(&opts.root)
            };
            let o = docsys::seed::Options {
                target: opts.target.clone(),
                since: opts.since.clone(),
                memory: opts.memory.clone(),
            };
            match docsys::seed::plan(&repo, &root, &o) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("seed: {e}");
                    ExitCode::from(1)
                }
            }
        }
        ("debt", Some("close")) => {
            let Some(n) = opts
                .positional
                .first()
                .and_then(|p| p.parse::<usize>().ok())
            else {
                eprintln!("debt close needs the item number: `docsys debt close <n>`");
                return ExitCode::from(2);
            };
            match docsys::capture::debt_close(&opts.root, n, opts.note.as_deref()) {
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("debt close: {e}");
                    ExitCode::from(1)
                }
            }
        }
        ("journal", Some("add")) => {
            let text = opts.positional.join(" ");
            match docsys::capture::journal_add(
                &opts.root,
                &text,
                opts.title.as_deref(),
                opts.date.as_deref(),
                opts.link.as_deref(),
            ) {
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("journal add: {e}");
                    ExitCode::from(1)
                }
            }
        }
        ("page", Some("new")) => {
            let (Some(kind), Some(id)) = (opts.positional.first(), opts.positional.get(1)) else {
                eprintln!("page new needs a kind and an id: `docsys page new <feature|postmortem|research|reference|howto|explanation|tutorial> <id>`");
                return ExitCode::from(2);
            };
            match docsys::capture::page_new(&opts.root, kind, id, opts.title.as_deref()) {
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("page new: {e}");
                    ExitCode::from(1)
                }
            }
        }
        ("backlinks", None) | ("mentions", None) | ("graph", None) => {
            let tree = match docsys::tree::DocTree::load(&opts.root) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("{cmd}: {e}");
                    return ExitCode::from(2);
                }
            };
            let repo = opts.repo.as_deref();
            let result = match cmd {
                "backlinks" => match opts.positional.first() {
                    Some(w) => docsys::graph::backlinks(&tree, repo, w),
                    None => Err("backlinks needs a page path or id".to_string()),
                },
                "mentions" => {
                    docsys::graph::mentions(&tree, opts.positional.first().map(String::as_str))
                }
                _ => docsys::graph::render(&tree, repo, opts.format.as_deref().unwrap_or("dot")),
            };
            match result {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{cmd}: {e}");
                    ExitCode::from(1)
                }
            }
        }
        ("seed", Some("gaps")) => {
            let repo = opts.repo.clone().unwrap_or_else(|| PathBuf::from("."));
            let root = if opts.root.is_absolute() {
                opts.root.clone()
            } else {
                repo.join(&opts.root)
            };
            let o = docsys::seed::Options {
                target: None,
                since: opts.since.clone(),
                memory: None,
            };
            match docsys::seed::gaps_json(&repo, &root, &o) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("seed: {e}");
                    ExitCode::from(1)
                }
            }
        }
        ("seed", Some("apply")) => {
            let repo = opts.repo.clone().unwrap_or_else(|| PathBuf::from("."));
            let root = if opts.root.is_absolute() {
                opts.root.clone()
            } else {
                repo.join(&opts.root)
            };
            let Some(plan) = opts.plan.clone() else {
                eprintln!("seed apply needs --plan <file>");
                return ExitCode::from(2);
            };
            match docsys::seed::apply(&repo, &root, &plan, opts.force) {
                Ok(done) => {
                    for d in done {
                        println!("{d}");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("seed apply: {e}");
                    ExitCode::from(1)
                }
            }
        }
        ("hook", Some(event)) => {
            let repo = opts.repo.clone().unwrap_or_else(|| PathBuf::from("."));
            let root = if opts.root.is_absolute() {
                opts.root.clone()
            } else {
                repo.join(&opts.root)
            };
            // The payload comes on stdin from the agent harness. `stop` needs
            // none, and a human at a terminal must not be left waiting for EOF.
            let mut payload = String::new();
            if event != "stop" && !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut payload);
            }
            let reply = match event {
                "pre-tool-use" => docsys::hook::pre_tool_use(
                    &repo,
                    &root,
                    &payload,
                    std::env::var_os("DOCSYS_SKIP").is_some_and(|v| !v.is_empty()),
                ),
                "stop" => docsys::hook::stop(&repo, &root),
                "post-tool-use" => {
                    docsys::hook::post_tool_use(&repo, &root, &payload, &migrate::today())
                }
                "user-prompt-submit" => docsys::hook::user_prompt_submit(&payload),
                other => {
                    eprintln!("hook: unknown event `{other}` (pre-tool-use | stop | post-tool-use | user-prompt-submit)");
                    return ExitCode::from(2);
                }
            };
            print!("{}", reply.stdout);
            eprint!("{}", reply.stderr);
            ExitCode::from(reply.code)
        }
        ("lookup", None) => {
            if opts.positional.is_empty() {
                eprintln!("lookup needs at least one word");
                return ExitCode::from(2);
            }
            match docsys::lookup::lookup(&opts.root, &opts.positional) {
                Ok(hits) => {
                    if opts.json {
                        print!("{}", docsys::lookup::render_json(&hits));
                    } else {
                        print!("{}", docsys::lookup::render(&hits, &opts.positional));
                    }
                    if hits.is_empty() {
                        ExitCode::from(1)
                    } else {
                        ExitCode::SUCCESS
                    }
                }
                Err(e) => {
                    eprintln!("lookup: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("consume", Some("add")) => match opts.positional.first() {
            Some(target) => match docsys::consume::add(&opts.root, target, opts.as_ns.as_deref()) {
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("consume add: {e}");
                    ExitCode::from(2)
                }
            },
            None => {
                eprintln!("consume add needs <path|git-url>[#subdir]");
                ExitCode::from(2)
            }
        },
        ("consume", Some("discover")) => {
            let dir = opts
                .positional
                .first()
                .map_or_else(|| PathBuf::from("."), PathBuf::from);
            match docsys::consume::discover(&opts.root, &dir) {
                Ok(found) if found.is_empty() => {
                    println!("no docsys tree one level under {}", dir.display());
                    ExitCode::SUCCESS
                }
                Ok(found) => {
                    for c in &found {
                        let action = if c.already {
                            "already consumed".to_string()
                        } else {
                            format!(
                                "docsys consume add {} --root {}",
                                c.path.display(),
                                opts.root.display()
                            )
                        };
                        println!("{}\t{}\t{}\t{action}", c.ns, c.profile, c.path.display());
                    }
                    println!("-- {} candidate(s); nothing written", found.len());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("consume discover: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("consume", _) => {
            eprintln!("consume needs `add` or `discover`");
            ExitCode::from(2)
        }
        ("compile", None) => match opts.positional.first() {
            Some(page) => match docsys::compile::compile(&opts.root, &opts.dir, page, opts.force) {
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("compile: {e}");
                    ExitCode::from(2)
                }
            },
            None => {
                eprintln!("compile needs <howto> (an id or a root-relative path)");
                ExitCode::from(2)
            }
        },
        ("pin", None) => {
            let repo = opts.repo.clone().unwrap_or_else(|| PathBuf::from("."));
            let root = if opts.root.is_absolute() {
                opts.root.clone()
            } else {
                repo.join(&opts.root)
            };
            let result = if opts.refresh {
                match opts.positional.first() {
                    Some(page) => docsys::fresh::refresh(&root, &repo, page),
                    None => Err("pin --refresh needs <page>".to_string()),
                }
            } else {
                match (opts.positional.first(), opts.positional.get(1)) {
                    (Some(page), Some(path)) => {
                        docsys::fresh::pin(&root, &repo, page, path, opts.symbol.as_deref())
                    }
                    _ => Err(
                        "pin needs <page> <path> [--symbol <s>], or --refresh <page>".to_string(),
                    ),
                }
            };
            match result {
                Ok(msg) => {
                    println!("{msg}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("pin: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("gate", None) => {
            let repo = opts.repo.clone().unwrap_or_else(|| PathBuf::from("."));
            let root = if opts.root.is_absolute() {
                opts.root.clone()
            } else {
                repo.join(&opts.root)
            };
            let result = match &opts.range {
                Some(r) => docsys::gate::run_range(&repo, &root, r),
                None => docsys::gate::run(&repo, &root),
            };
            match result {
                Ok((g, report)) => {
                    for f in &report.findings {
                        println!(
                            "{} {} {} [{}] {}",
                            f.severity.tag(),
                            f.rule,
                            f.file,
                            f.subject,
                            f.message
                        );
                    }
                    if !g.code.is_empty() && g.docs == 0 {
                        let head: Vec<&str> = g.code.iter().take(5).map(String::as_str).collect();
                        let more = g.code.len().saturating_sub(head.len());
                        let tail = if more > 0 {
                            format!(" (+{more} more)")
                        } else {
                            String::new()
                        };
                        println!(
                            "GATE {} changes with no docs change: {}{tail}",
                            g.scope,
                            head.join(", ")
                        );
                    }
                    println!(
                        "-- {} error(s), {} warning(s)",
                        g.lint_errors, g.lint_warnings
                    );
                    // Over a range there is nobody to ask once: code without
                    // documentation fails the check, as CI must.
                    let unanswered = opts.range.is_some() && !g.code.is_empty() && g.docs == 0;
                    if g.lint_errors > 0 || unanswered {
                        ExitCode::from(1)
                    } else {
                        ExitCode::SUCCESS
                    }
                }
                Err(e) => {
                    eprintln!("gate: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("doctor", None) => {
            let repo = opts.repo.clone().unwrap_or_else(|| PathBuf::from("."));
            let root = if opts.root.is_absolute() {
                opts.root.clone()
            } else {
                repo.join(&opts.root)
            };
            let d = docsys::doctor::run(&repo, &root, &opts.dir);
            for l in &d.lines {
                println!("{l}");
            }
            if d.failed > 0 {
                println!(
                    "-- {} check(s) FAILED — the pipeline is not alive",
                    d.failed
                );
                ExitCode::from(1)
            } else {
                println!("-- pipeline alive");
                ExitCode::SUCCESS
            }
        }
        ("fetch", None) => match docsys::export::fetch(&opts.root) {
            Ok(summary) => {
                for s in &summary {
                    println!("{s}");
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("fetch: {e}");
                ExitCode::from(2)
            }
        },
        ("export", Some("manifest")) => match docsys::export::manifest(&opts.root) {
            Ok(text) => match &opts.out {
                Some(path) => match docsys::export::write_if_changed(path, &text) {
                    Ok(true) => {
                        eprintln!("manifest -> {}", path.display());
                        ExitCode::SUCCESS
                    }
                    Ok(false) => {
                        eprintln!("unchanged — {} left untouched", path.display());
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("export manifest: {e}");
                        ExitCode::from(2)
                    }
                },
                None => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
            },
            Err(e) => {
                eprintln!("export manifest: {e}");
                ExitCode::from(2)
            }
        },
        ("export", Some("plan")) => {
            match docsys::export::plan(&opts.root, opts.audience.as_deref()) {
                Ok(p) => {
                    print!("{p}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("export plan: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("export", Some("product")) => {
            let Some(map) = opts.positional.first() else {
                eprintln!("export product needs a <map> argument");
                return ExitCode::from(2);
            };
            let want_lang = opts.lang_explicit.then_some(opts.lang.as_str());
            match docsys::export::product(
                &opts.root,
                std::path::Path::new(map),
                want_lang,
                opts.audience.as_deref(),
            ) {
                Ok(done) => emit_export(&done, opts.out.as_deref()),
                Err(e) => {
                    eprintln!("export product: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("export", Some("feature")) => {
            if opts.positional.is_empty() {
                eprintln!("export feature needs at least one <id>");
                return ExitCode::from(2);
            }
            let want_lang = opts.lang_explicit.then_some(opts.lang.as_str());
            match docsys::export::feature(
                &opts.root,
                &opts.positional,
                opts.follow,
                opts.title.clone(),
                want_lang,
                opts.audience.as_deref(),
            ) {
                Ok(done) => emit_export(&done, opts.out.as_deref()),
                Err(e) => {
                    eprintln!("export feature: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("graduate", Some("plan")) => {
            let Some(src) = opts.positional.first() else {
                eprintln!("graduate plan needs a <work-file> argument");
                return ExitCode::from(2);
            };
            match docsys::graduate::plan(&opts.root, src) {
                Ok(p) => {
                    print!("{p}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("graduate plan: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("graduate", Some("apply")) => {
            let Some(plan_path) = &opts.plan else {
                eprintln!("graduate apply needs --plan <file>");
                return ExitCode::from(2);
            };
            let plan = match std::fs::read_to_string(plan_path) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("plan: {e}");
                    return ExitCode::from(2);
                }
            };
            match docsys::graduate::apply(&opts.root, &plan, opts.force) {
                Ok(done) => {
                    println!(
                        "moved {} block(s) · linked {} · destinations touched: {}",
                        done.moved,
                        done.linked,
                        done.dest_files.join(", ")
                    );
                    println!("-- running lint --");
                    run_lint(&Opts {
                        json: false,
                        ..opts
                    })
                }
                Err(e) => {
                    eprintln!("graduate apply: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("agents", None) if opts.procedures => {
            // --report: mechanical inventory of the existing layer (D-026).
            println!("existing agent layer under {}:", opts.dir.display());
            for line in docsys::agents::adoption_report(&opts.dir) {
                println!("  {line}");
            }
            println!("\ndelegation is judgment: keep the owner's prose, repoint the");
            println!("mechanical calls to docsys where they duplicate a command.");
            ExitCode::SUCCESS
        }
        ("agents", None) if opts.kb => {
            // The base is the docs root's parent when the root is the base
            // itself (a knowledge base is usually its own repository).
            let base = if opts.root.as_path() == std::path::Path::new("docs") {
                PathBuf::from(".")
            } else {
                opts.root.clone()
            };
            match docsys::agents::install_kb(&opts.dir, &base, opts.force) {
                Ok(done) => {
                    for f in &done.written {
                        println!("wrote   {f}");
                    }
                    for f in &done.skipped {
                        println!("skipped {f} (exists; --force to overwrite)");
                    }
                    println!("\nthe base's four organs: capture · ingest · audit · lookup.");
                    println!(
                        "declare your subjects in .docmeta.yml `domains:` and start capturing."
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("agents --kb: {e}");
                    ExitCode::from(2)
                }
            }
        }
        ("agents", None) => match docsys::agents::install(&opts.dir, opts.force) {
            Ok(done) => {
                for f in &done.written {
                    println!("wrote   {}/{f}", opts.dir.display());
                }
                for f in &done.skipped {
                    println!(
                        "skipped {}/{f} (exists; --force to overwrite)",
                        opts.dir.display()
                    );
                }
                println!("\n-- merge into .claude/settings.json by hand (protected file): --");
                println!("{}", docsys::agents::SETTINGS_SNIPPET);
                println!("-- and add the generated block to AGENTS.md: --");
                println!("   docsys rules --agents-md >> AGENTS.md   # review the diff first");
                println!("\ntip: `docsys adopt` does all of this in one pass — assets,");
                println!("settings.json (when absent), AGENTS.md block, git gate, report.");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("agents: {e}");
                ExitCode::from(2)
            }
        },
        ("refs", None) => {
            let Some(repo) = &opts.repo else {
                eprintln!("refs needs --repo <dir>");
                return ExitCode::from(2);
            };
            // The root must be anchored to the repo: a bare `docs` next to a
            // walk that yields `./docs/...` fails the inside-the-tree prefix
            // test and the docs tree gets scanned as if it were code.
            let root = if opts.root.is_absolute() {
                opts.root.clone()
            } else {
                repo.join(&opts.root)
            };
            let tree = match docsys::tree::DocTree::load(&root) {
                Ok(t) if t.docmeta_present => t,
                Ok(_) => {
                    eprintln!("refs: `{}` has no .docmeta.yml", root.display());
                    return ExitCode::from(2);
                }
                Err(e) => {
                    eprintln!("refs: {e}");
                    return ExitCode::from(2);
                }
            };
            let report = docsys::refs::run(repo, &tree);
            if opts.json {
                print!("{}", to_json(&report));
            } else {
                for f in &report.findings {
                    println!(
                        "{} {} {} [{}] {}",
                        f.severity.tag(),
                        f.rule,
                        f.file,
                        f.subject,
                        f.message
                    );
                }
                let errors = report
                    .findings
                    .iter()
                    .filter(|f| f.severity == docsys::model::Severity::Error)
                    .count();
                let units: usize = report.inspected.values().sum();
                println!(
                    "-- {errors} error(s), {} warning(s); {units} unit(s) inspected",
                    report.findings.len() - errors
                );
            }
            if report
                .findings
                .iter()
                .any(|f| f.severity == docsys::model::Severity::Error)
            {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        ("init", None) => match migrate::init_profile(
            &opts.root,
            &opts.lang,
            opts.profile.as_deref().unwrap_or("project"),
        ) {
            Ok(()) => {
                println!("initialized `{}`", opts.root.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("init: {e}");
                ExitCode::from(2)
            }
        },
        ("migrate", Some("inventory")) => match migrate::inventory(&opts.root) {
            Ok(plan) => {
                print!("{plan}");
                if let Some(repo) = &opts.repo {
                    println!("# -- inbound references from the repo (will need rewriting) --");
                    for (file, hits) in migrate::inbound_report(repo, &opts.root) {
                        println!("# inbound: {file} · {hits} reference(s)");
                    }
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("inventory: {e}");
                ExitCode::from(2)
            }
        },
        ("migrate", Some("apply")) => {
            let Some(plan_path) = &opts.plan else {
                eprintln!("migrate apply needs --plan <file>");
                return ExitCode::from(2);
            };
            let plan = match std::fs::read_to_string(plan_path) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("plan: {e}");
                    return ExitCode::from(2);
                }
            };
            match migrate::apply(&opts.root, &plan, &opts.lang, opts.repo.as_deref()) {
                Ok(done) => {
                    println!(
                        "moved {} · kept {} · archived {} · links rewritten {}",
                        done.moved, done.kept, done.archived, done.links_rewritten
                    );
                    for (file, n) in &done.repo_rewrites {
                        println!("repo rewrite: {file} · {n} reference(s)");
                    }
                    for risk in &done.repo_risks {
                        println!("RISK unmapped inbound: {risk}");
                    }
                    println!("-- running lint on the migrated tree --");
                    run_lint(&Opts {
                        json: false,
                        ..opts
                    })
                }
                Err(e) => {
                    eprintln!("apply: {e}");
                    ExitCode::from(2)
                }
            }
        }
        _ => {
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
