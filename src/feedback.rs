//! `docsys feedback` — the way a person or an agent tells the project that
//! docsys is wrong or in the way (D-116). Offline and deterministic: the tool
//! prints the guide, drafts an issue body with the facts it can know, and
//! names the new-issue address; it never files anything. Filing publishes,
//! and that is the person's act. The issue format lives here once; the
//! repository's `.github/ISSUE_TEMPLATE/` files are its rendering, held equal
//! by a test.

use std::path::Path;

/// The three kinds of issue, each a template on the project's tracker.
pub const TYPES: [&str; 3] = ["bug", "false-positive", "need"];

/// The issue body every kind shares (the owner's format: the problem or need
/// in one sentence, a concrete example, the rule, the impact, a proposal with
/// its trade-off, the environment).
pub const FORMAT: &str = "### Problem or need
<one sentence: what is wrong, or what is missing>

### Example
<the smallest tree, or the exact command; what happened; what was expected>

### Rule or command
<R-xxx / D-xxx / `docsys …`>

### Impact
<who it hits, and how often>

### Proposal (optional)
<the change, and what it costs or risks>

### Environment
<filled by `docsys feedback --draft`: version, OS, profile, spec>
";

/// The front matter of each kind's tracker template.
fn front(kind: &str) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
    match kind {
        "bug" => Some((
            "Bug",
            "docsys does the wrong thing: a crash, a wrong write, a wrong exit code",
            "[bug] ",
            "bug",
        )),
        "false-positive" => Some((
            "False positive",
            "a finding docsys reports that the rule's text does not support",
            "[false positive] ",
            "false-positive",
        )),
        "need" => Some((
            "Need",
            "something docsys does not do yet, or does in a way that is in the way",
            "[need] ",
            "enhancement",
        )),
        _ => None,
    }
}

/// A kind's tracker template: its front matter, then the shared format.
pub fn template(kind: &str) -> Option<String> {
    let (name, about, title, label) = front(kind)?;
    Some(format!(
        "---\nname: {name}\nabout: {about}\ntitle: \"{title}\"\nlabels: {label}\n---\n\n{FORMAT}"
    ))
}

/// Where a kind's issue is opened: the repository this binary was built
/// from, so a fork's binary reports to the fork.
pub fn new_issue_url(kind: &str) -> String {
    format!(
        "{}/issues/new?template={kind}.md",
        env!("CARGO_PKG_REPOSITORY").trim_end_matches('/')
    )
}

/// The rules whose checks read free text — the findings most often wrong —
/// under which `lint`, `refs` and `gate` name this command.
pub const DISPUTED: [&str; 9] = [
    "R-035", "R-070", "R-071", "R-073", "R-075", "R-076", "R-108", "R-111", "R-114",
];

/// One line per disputed rule among the findings, in rule order.
pub fn pointers<'a>(rules: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut seen: Vec<&str> = rules.filter(|r| DISPUTED.contains(r)).collect();
    seen.sort_unstable();
    seen.dedup();
    seen.iter()
        .map(|r| format!("Finding wrong? `docsys feedback --rule {r}`"))
        .collect()
}

pub fn guide() -> String {
    let mut out = String::from(
        "docsys feedback — when docsys is wrong or in your way\n\n\
         An issue explains the problem with a concrete example, states the need or the\n\
         problem in one sentence, names the rule, and proposes a fix where one can be\n\
         given. Draft it here; nothing is filed by this command:\n\n\
         \x20 docsys feedback --draft [--type bug|false-positive|need] [--rule R-xxx]\n\
         \x20                         [--command \"docsys …\"] [--out <file>]\n\n\
         The draft fills what the tool knows — version, OS, the tree's profile and spec,\n\
         the rule's text, the command's exit code and output, a redacted .docmeta.yml and\n\
         the files the rule's findings name — and leaves TODO where only you can write.\n\
         Read it for private content before it leaves your machine. An agent drafts the\n\
         issue and asks the person before filing: filing publishes.\n\n\
         The format:\n\n",
    );
    out.push_str(FORMAT);
    out.push_str("\nWhere it is filed:\n");
    for kind in TYPES {
        out.push_str(&format!("  {kind:<15} {}\n", new_issue_url(kind)));
    }
    out
}

/// What the draft is built from.
pub struct Draft<'a> {
    pub kind: &'a str,
    pub rule: Option<&'a str>,
    pub command: Option<&'a str>,
    /// the tree the draft describes, when one is found
    pub root: Option<&'a Path>,
}

/// Values in `.docmeta.yml` that name people, places or projects.
const PRIVATE_KEYS: [&str; 6] = [
    "maintainers",
    "consume",
    "consume_base",
    "namespace",
    "manifest_url",
    "content_url",
];

/// The `.docmeta.yml` with what names people, places or projects removed:
/// the private keys' values, and every `<…@…>` address.
pub fn redact_docmeta(text: &str) -> String {
    let mut out = String::new();
    let mut in_private_block = false;
    for line in text.lines() {
        if in_private_block && line.starts_with(' ') {
            continue;
        }
        in_private_block = false;
        // a comment is prose the owner or adopt wrote: it can name anything
        if line.trim_start().starts_with('#') {
            continue;
        }
        let key = line.split_once(':').map(|(k, _)| k.trim());
        if let Some(k) = key.filter(|k| PRIVATE_KEYS.contains(k) && !line.starts_with(' ')) {
            out.push_str(&format!("{k}: <redacted>\n"));
            in_private_block = true;
            continue;
        }
        out.push_str(&redact_emails(line));
        out.push('\n');
    }
    out
}

fn redact_emails(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('<') {
        let Some(close) = rest.get(open..).and_then(|s| s.find('>')).map(|i| open + i) else {
            break;
        };
        let inside = rest.get(open + 1..close).unwrap_or("");
        out.push_str(rest.get(..open).unwrap_or(""));
        if inside.contains('@') && !inside.contains(' ') {
            out.push_str("<email>");
        } else {
            out.push_str(rest.get(open..=close).unwrap_or(""));
        }
        rest = rest.get(close + 1..).unwrap_or("");
    }
    out.push_str(rest);
    out
}

/// The home directory written as `~`, so a draft does not carry the
/// reporter's account name.
fn redact_home(text: &str) -> String {
    match std::env::var("HOME").ok().filter(|h| h.len() > 1) {
        Some(home) => text.replace(&home, "~"),
        None => text.to_string(),
    }
}

/// Run a docsys command for the draft — this binary, never a shell, and
/// nothing that is not docsys. Returns its exit code and its first 100 lines.
pub fn run_command(command: &str) -> Result<(i32, String), String> {
    let mut words = command.split_whitespace();
    if words.next() != Some("docsys") {
        return Err(format!(
            "`{command}` is not a docsys command — the draft runs docsys only, never a shell"
        ));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let out = std::process::Command::new(exe)
        .args(words)
        .output()
        .map_err(|e| e.to_string())?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let shown: Vec<&str> = text.lines().take(100).collect();
    Ok((out.status.code().unwrap_or(-1), shown.join("\n")))
}

/// The issue body: the format with the machine facts filled in and TODO
/// where only a person or an agent can write.
pub fn draft(d: &Draft) -> Result<String, String> {
    let (_, _, title, _) =
        front(d.kind).ok_or_else(|| format!("`{}` is not one of: {}", d.kind, TYPES.join(", ")))?;
    let rule_line = match d.rule {
        Some(r) => crate::rules::rule_sentence(r)
            .ok_or_else(|| format!("`{r}` is no rule of the embedded spec"))?,
        None => "TODO: the rule (R-xxx) or decision (D-xxx) involved, if any".to_string(),
    };
    let mut example = String::from(
        "TODO: the smallest tree, or the exact command; what happened; what was expected\n",
    );
    if let Some(c) = d.command {
        let (code, output) = run_command(c)?;
        example.push_str(&format!(
            "\n`{c}` exited {code}:\n\n```\n{}\n```\n",
            redact_home(&output)
        ));
    }
    let mut env = format!(
        "- docsys {} (spec docsys/{})\n- {} {}\n",
        env!("CARGO_PKG_VERSION"),
        crate::rules::spec_version(),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    if let Some(root) = d.root {
        let docmeta = std::fs::read_to_string(root.join(".docmeta.yml")).unwrap_or_default();
        let field = |k: &str| {
            docmeta
                .lines()
                .find_map(|l| l.strip_prefix(k))
                .and_then(|v| v.strip_prefix(':'))
                .map(|v| v.trim().to_string())
                .unwrap_or_else(|| "?".to_string())
        };
        env.push_str(&format!(
            "- tree: profile {}, {}\n",
            field("profile"),
            field("spec")
        ));
        example.push_str(&format!(
            "\nThe tree's `.docmeta.yml`, redacted:\n\n```yaml\n{}```\n",
            redact_docmeta(&docmeta)
        ));
        if let Some(r) = d.rule {
            let (report, _) = crate::lint(root);
            let mut files: Vec<&str> = report
                .findings
                .iter()
                .filter(|f| f.rule.0 == r)
                .map(|f| f.file.as_str())
                .collect();
            files.sort_unstable();
            files.dedup();
            if !files.is_empty() {
                example.push_str(&format!(
                    "\nFiles {r} names — cut to the smallest tree that still shows it:\n\n{}\n",
                    files
                        .iter()
                        .map(|f| format!("- {f}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                ));
            }
        }
    }
    Ok(format!(
        "{title}TODO: a title that states the problem\n\n\
         ### Problem or need\nTODO: one sentence — what is wrong, or what is missing\n\n\
         ### Example\n{example}\n\
         ### Rule or command\n{rule_line}{}\n\n\
         ### Impact\nTODO: who it hits, and how often\n\n\
         ### Proposal (optional)\nTODO: the change, and what it costs or risks — or delete this section\n\n\
         ### Environment\n{env}",
        d.command.map(|c| format!("\n`{c}`")).unwrap_or_default()
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn the_docmeta_loses_names_and_addresses() {
        let text = "spec: docsys/0.5\n# The name a consumer uses for this tree — `consume: [shop]`.\nnamespace: shop\nmaintainers:\n  - ayse <ayse@example.com> @ayse\n  - bora\nconsume: [auth=/home/x/auth#docs]\nprofile: project\nheadings: [Context=<x>]\n";
        let out = redact_docmeta(text);
        assert!(!out.contains("ayse") && !out.contains("bora") && !out.contains("/home/x"));
        assert!(!out.contains("shop"));
        assert!(out.contains("maintainers: <redacted>\nconsume: <redacted>\nprofile: project\n"));
        assert!(out.contains("headings: [Context=<x>]"), "{out}");
        assert_eq!(redact_emails("a <x@y.z> b <c>"), "a <email> b <c>");
    }

    #[test]
    fn only_disputed_rules_get_a_pointer_once_each() {
        let p = pointers(["R-071", "R-050", "R-071", "R-111"].into_iter());
        assert_eq!(
            p,
            vec![
                "Finding wrong? `docsys feedback --rule R-071`",
                "Finding wrong? `docsys feedback --rule R-111`"
            ]
        );
    }

    #[test]
    fn every_kind_has_a_template_and_an_address() {
        for kind in TYPES {
            assert!(template(kind).unwrap().ends_with(FORMAT));
            assert!(new_issue_url(kind).ends_with(&format!("/issues/new?template={kind}.md")));
        }
        assert!(template("other").is_none());
    }

    #[test]
    fn a_command_that_is_not_docsys_is_refused() {
        assert!(run_command("rm -rf x")
            .unwrap_err()
            .contains("not a docsys command"));
        assert!(run_command("sh -c x").is_err());
    }
}
