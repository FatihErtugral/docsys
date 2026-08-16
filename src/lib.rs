//! docsys — reference implementation of the docsys specification (SPEC.md).
//! v0 scope: `lint` over both profiles (`project`, `knowledge-base`). Every
//! implementation-defined choice is registered in corpus/DECISIONS.md (R-193).

pub mod adopt;
pub mod agents;
pub mod checks;
pub mod export;
pub mod fm;
pub mod graduate;
pub mod migrate;
pub mod model;
pub mod refs;
pub mod rules;
pub mod tree;

use model::Severity;
use std::path::Path;

/// Exit codes (D-005): 0 clean or warnings only · 1 error findings · 2 the
/// tree is not operable (missing root or missing .docmeta.yml).
pub enum Outcome {
    Clean,
    Errors,
    Config,
}

pub fn lint(root: &Path) -> (checks::Report, Outcome) {
    if !root.is_dir() {
        let mut r = checks::Report {
            findings: Vec::new(),
            inspected: std::collections::BTreeMap::new(),
        };
        r.findings.push(model::Finding::err(
            model::RuleId("R-160"),
            "-",
            "root",
            format!("documentation root `{}` does not exist", root.display()),
        ));
        return (r, Outcome::Config);
    }
    let tree = match tree::DocTree::load(root) {
        Ok(t) => t,
        Err(e) => {
            let mut r = checks::Report {
                findings: Vec::new(),
                inspected: std::collections::BTreeMap::new(),
            };
            r.findings.push(model::Finding::err(
                model::RuleId("R-160"),
                "-",
                "io",
                format!("cannot read the tree: {e}"),
            ));
            return (r, Outcome::Config);
        }
    };
    let report = checks::run(&tree);
    let outcome = if !tree.docmeta_present {
        Outcome::Config
    } else if report
        .findings
        .iter()
        .any(|f| f.severity == Severity::Error)
    {
        Outcome::Errors
    } else {
        Outcome::Clean
    };
    (report, outcome)
}

/// Minimal JSON emission — the shape is part of the corpus contract.
pub fn to_json(report: &checks::Report) -> String {
    fn esc(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 2);
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out
    }
    let mut s = String::from("{\n  \"findings\": [\n");
    for (i, f) in report.findings.iter().enumerate() {
        let comma = if i + 1 == report.findings.len() {
            ""
        } else {
            ","
        };
        s.push_str(&format!(
            "    {{\"severity\":\"{}\",\"rule\":\"{}\",\"file\":\"{}\",\"subject\":\"{}\",\"message\":\"{}\"}}{comma}\n",
            f.severity.tag(),
            f.rule,
            esc(&f.file),
            esc(&f.subject),
            esc(&f.message)
        ));
    }
    s.push_str("  ],\n  \"inspected\": {");
    let mut first = true;
    for (k, v) in &report.inspected {
        if !first {
            s.push(',');
        }
        first = false;
        s.push_str(&format!("\"{k}\":{v}"));
    }
    s.push_str("}\n}\n");
    s
}
