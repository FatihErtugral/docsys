use docsys::{lint, migrate, to_json, Outcome};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "docsys — documentation system tool (spec: SPEC.md)

Usage:
  docsys lint    [--root <dir>] [--json]
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
  docsys gate    [--repo .] [--root docs]    # commit-time question: lint + code-without-docs
  docsys doctor  [--repo .] [--root docs] [--dir .claude]   # is the pipeline itself alive?
  docsys seed    plan [--target <feature>] [--since <date>] [--repo .] [--root docs]
                                             # brownfield: feature inventory, or one feature's history as evidence
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
    agents_md: bool,
    procedures: bool,
    max_lines: usize,
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
        agents_md: false,
        procedures: false,
        max_lines: 200,
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
            other if !other.starts_with("--") => o.positional.push(other.to_string()),
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(o)
}

fn run_lint(o: &Opts) -> ExitCode {
    let (report, outcome) = lint(&o.root);
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
            if c == "migrate" || c == "graduate" || c == "export" || c == "hook" || c == "seed" =>
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
                            match docsys::rules::write_agents_block(target) {
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
        ("gate", None) => {
            let repo = opts.repo.clone().unwrap_or_else(|| PathBuf::from("."));
            let root = if opts.root.is_absolute() {
                opts.root.clone()
            } else {
                repo.join(&opts.root)
            };
            match docsys::gate::run(&repo, &root) {
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
                    if g.lint_errors > 0 {
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
