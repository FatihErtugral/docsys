use docsys::{lint, migrate, to_json, Outcome};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "docsys — documentation system tool (spec: SPEC.md)

Usage:
  docsys lint    [--root <dir>] [--json]
  docsys init    [--root <dir>] [--lang <code>]
  docsys migrate inventory [--root <dir>]            # plan skeleton to stdout
  docsys migrate apply --plan <file> [--root <dir>] [--lang <code>] [--repo <dir>]
  docsys refs    --repo <dir> [--root <dir>] [--json]
  docsys rules   --agents-md | --procedures [--max-lines <n>]
  docsys agents  [--dir .claude] [--force]   # install hooks + skill + /doc-sync
  docsys graduate plan <work-file>  [--root <dir>]
  docsys graduate apply --plan <file> [--root <dir>] [--force]

Exit codes: 0 clean/warnings · 1 error findings · 2 tree not operable
";

struct Opts {
    root: PathBuf,
    json: bool,
    lang: String,
    plan: Option<PathBuf>,
    repo: Option<PathBuf>,
    dir: PathBuf,
    force: bool,
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
        plan: None,
        repo: None,
        dir: PathBuf::from(".claude"),
        force: false,
        agents_md: false,
        procedures: false,
        max_lines: 200,
        positional: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--root" => o.root = PathBuf::from(it.next().ok_or("--root needs a value")?),
            "--lang" => o.lang = it.next().ok_or("--lang needs a value")?.clone(),
            "--plan" => o.plan = Some(PathBuf::from(it.next().ok_or("--plan needs a value")?)),
            "--repo" => o.repo = Some(PathBuf::from(it.next().ok_or("--repo needs a value")?)),
            "--dir" => o.dir = PathBuf::from(it.next().ok_or("--dir needs a value")?),
            "--force" => o.force = true,
            "--agents-md" => o.agents_md = true,
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
            println!("{} {} {} [{}] {}", f.severity.tag(), f.rule, f.file, f.subject, f.message);
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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, sub, rest): (&str, Option<&str>, &[String]) = match args.split_first() {
        Some((c, r)) if c == "migrate" || c == "graduate" => match r.split_first() {
            Some((s, r2)) => (c.as_str(), Some(s.as_str()), r2),
            None => (c.as_str(), None, &[]),
        },
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
        ("lint", None) => run_lint(&opts),
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
                    run_lint(&Opts { json: false, ..opts })
                }
                Err(e) => {
                    eprintln!("graduate apply: {e}");
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
                    println!("skipped {}/{f} (exists; --force to overwrite)", opts.dir.display());
                }
                println!("\n-- merge into .claude/settings.json by hand (protected file): --");
                println!("{}", docsys::agents::SETTINGS_SNIPPET);
                println!("-- and add the generated block to AGENTS.md: --");
                println!("   docsys rules --agents-md >> AGENTS.md   # review the diff first");
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
            let tree = match docsys::tree::DocTree::load(&opts.root) {
                Ok(t) if t.docmeta_present => t,
                Ok(_) => {
                    eprintln!("refs: `{}` has no .docmeta.yml", opts.root.display());
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
                    println!("{} {} {} [{}] {}", f.severity.tag(), f.rule, f.file, f.subject, f.message);
                }
                let errors = report
                    .findings
                    .iter()
                    .filter(|f| f.severity == docsys::model::Severity::Error)
                    .count();
                let units: usize = report.inspected.values().sum();
                println!("-- {errors} error(s), {} warning(s); {units} unit(s) inspected",
                    report.findings.len() - errors);
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
        ("init", None) => match migrate::init(&opts.root, &opts.lang) {
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
