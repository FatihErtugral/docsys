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

Exit codes: 0 clean/warnings · 1 error findings · 2 tree not operable
";

struct Opts {
    root: PathBuf,
    json: bool,
    lang: String,
    plan: Option<PathBuf>,
    repo: Option<PathBuf>,
}

fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        root: PathBuf::from("docs"),
        json: false,
        lang: "en".to_string(),
        plan: None,
        repo: None,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--root" => o.root = PathBuf::from(it.next().ok_or("--root needs a value")?),
            "--lang" => o.lang = it.next().ok_or("--lang needs a value")?.clone(),
            "--plan" => o.plan = Some(PathBuf::from(it.next().ok_or("--plan needs a value")?)),
            "--repo" => o.repo = Some(PathBuf::from(it.next().ok_or("--repo needs a value")?)),
            "--json" => o.json = true,
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
        Some((c, r)) if c == "migrate" => match r.split_first() {
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
                        root: opts.root.clone(),
                        json: false,
                        lang: opts.lang.clone(),
                        plan: None,
                        repo: None,
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
