use docsys::{lint, to_json, Outcome};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "docsys — documentation system tool (spec: SPEC.md)

Usage:
  docsys lint [--root <dir>] [--json]

Exit codes: 0 clean/warnings · 1 error findings · 2 tree not operable
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut it = args.iter();
    match it.next().map(String::as_str) {
        Some("lint") => {}
        _ => {
            eprint!("{USAGE}");
            return ExitCode::from(2);
        }
    }
    let mut root = PathBuf::from("docs");
    let mut json = false;
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--root" => {
                let Some(v) = it.next() else {
                    eprintln!("--root needs a value");
                    return ExitCode::from(2);
                };
                root = PathBuf::from(v);
            }
            "--json" => json = true,
            other => {
                eprintln!("unknown argument `{other}`");
                eprint!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }

    let (report, outcome) = lint(&root);

    if json {
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
