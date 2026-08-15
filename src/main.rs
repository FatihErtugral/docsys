use docsys::{lint, migrate, to_json, Outcome};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "docsys — documentation system tool (spec: SPEC.md)

Usage:
  docsys lint    [--root <dir>] [--json]
  docsys init    [--root <dir>] [--lang <code>]
  docsys migrate inventory [--root <dir>]            # plan skeleton to stdout
  docsys migrate apply --plan <file> [--root <dir>] [--lang <code>]

Exit codes: 0 clean/warnings · 1 error findings · 2 tree not operable
";

struct Opts {
    root: PathBuf,
    json: bool,
    lang: String,
    plan: Option<PathBuf>,
}

fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        root: PathBuf::from("docs"),
        json: false,
        lang: "en".to_string(),
        plan: None,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--root" => o.root = PathBuf::from(it.next().ok_or("--root needs a value")?),
            "--lang" => o.lang = it.next().ok_or("--lang needs a value")?.clone(),
            "--plan" => o.plan = Some(PathBuf::from(it.next().ok_or("--plan needs a value")?)),
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
            match migrate::apply(&opts.root, &plan, &opts.lang) {
                Ok(done) => {
                    println!(
                        "moved {} · kept {} · archived {} · links rewritten {}",
                        done.moved, done.kept, done.archived, done.links_rewritten
                    );
                    println!("-- running lint on the migrated tree --");
                    run_lint(&Opts { json: false, ..Opts {
                        root: opts.root.clone(), json: false, lang: opts.lang.clone(), plan: None } })
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
