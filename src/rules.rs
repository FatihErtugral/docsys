//! `docsys rules` — agent-facing rule text, generated from the specification
//! (R-155). The spec is embedded at build time; nothing here is a hand-written
//! copy, so the text cannot drift from the rules the way §18's prose history
//! once did. Two layers, matching progressive disclosure:
//!   --agents-md    the always-loaded block (budget-checked, R-165)
//!   --procedures   the fifteen decision procedures of §14.3, verbatim

/// The specification this binary implements, embedded verbatim.
pub const SPEC: &str = include_str!("../SPEC.md");

/// Extract the body of a section by its exact heading line, up to the next
/// heading of equal-or-higher level.
fn section(heading: &str) -> Option<String> {
    let mut out = String::new();
    let mut inside = false;
    let level = heading.chars().take_while(|c| *c == '#').count();
    for line in SPEC.lines() {
        if inside {
            let l = line.chars().take_while(|c| *c == '#').count();
            if l > 0 && l <= level && line.starts_with('#') {
                break;
            }
            out.push_str(line);
            out.push('\n');
        } else if line.trim_end() == heading {
            inside = true;
        }
    }
    if inside {
        Some(out)
    } else {
        None
    }
}

/// The fifteen authored decision procedures (§14.3), rendered verbatim —
/// transformation, not authorship (R-005/R-155). R-163 forbids inventing
/// anything absent from this text.
pub fn procedures() -> Option<String> {
    let body = section("### 14.3 The authored procedures")?;
    let mut out = String::from(
        "# Decision procedures (generated from SPEC.md — do not edit)\n\
         #\n\
         # Format per §14.2: EVIDENCE · QUESTION · OPTIONS (closed, first match\n\
         # wins) · DEFAULT · ESCAPE (always legitimate) · VERIFY · NEVER.\n",
    );
    out.push_str(&body);
    Some(out)
}

/// A rule's declaration and the first sentence of its text — `R-071 (lint ·
/// MUST): Every link target MUST resolve.` — or `None` for a number the
/// embedded spec does not declare.
pub fn rule_sentence(id: &str) -> Option<String> {
    let head = format!("**{id}** ");
    let mut lines = SPEC.lines().skip_while(|l| !l.starts_with(&head));
    let first = lines.next()?;
    let mut text = first.strip_prefix(&head)?.to_string();
    for l in lines.take_while(|l| !l.trim().is_empty()) {
        text.push(' ');
        text.push_str(l.trim());
    }
    let (tag, rest) = text.split_once(" — ")?;
    Some(format!(
        "{id} ({}): {}",
        tag.replace('`', "").trim(),
        first_sentence(rest)
    ))
}

/// First sentence of a rule's text, for the always-loaded summary. Mechanical
/// extraction (the R-057 pattern): everything up to the first `. ` boundary.
fn first_sentence(text: &str) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match joined.split_once(". ") {
        Some((s, _)) => format!("{s}."),
        None => joined,
    }
}

/// The always-loaded block for AGENTS.md. Contents are derived: every
/// `agent`-tagged rule's number and first sentence, extracted from the
/// embedded spec — plus the fixed tool-gate lines (UI text, not rule prose).
/// The spec version this binary implements: SPEC.md's own Version line.
pub fn spec_version() -> &'static str {
    SPEC.lines()
        .find_map(|l| l.strip_prefix("**Version:** "))
        .unwrap_or("?")
        .trim()
}

/// A procedure's title, QUESTION and OPTIONS lines from §14.3, verbatim and
/// dedented — what an agent needs at the moment it decides (D-114).
fn procedure_head(id: &str) -> Option<String> {
    let body = section("### 14.3 The authored procedures")?;
    let mut lines = body
        .lines()
        .skip_while(|l| !l.trim_start().starts_with(&format!("{id} — ")));
    let title = lines.next()?.trim();
    let mut out = format!("- {title}\n");
    let mut keep = false;
    for l in lines {
        let t = l.strip_prefix("    ").unwrap_or(l);
        if t.starts_with("P/R-") || t.trim().is_empty() {
            break;
        }
        if t.starts_with("QUESTION") {
            keep = true;
        } else if t.starts_with("DEFAULT") {
            break;
        } else if !t.starts_with(' ') && !t.starts_with("OPTIONS") {
            keep = t.starts_with("QUESTION");
        }
        if keep {
            out.push_str("  ");
            out.push_str(t.trim_end());
            out.push('\n');
        }
    }
    Some(out)
}

pub fn agents_md() -> String {
    let mut rules: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in SPEC.lines() {
        if let Some(rest) = line.strip_prefix("**R-") {
            if let Some((num, tail)) = rest.split_once("** ") {
                // close the previous rule
                if let Some((id, text)) = current.take() {
                    rules.push((id, first_sentence(&text)));
                }
                if tail.starts_with("`agent`") {
                    let text = tail.split_once("— ").map(|(_, t)| t).unwrap_or("");
                    current = Some((format!("R-{num}"), text.to_string()));
                }
                continue;
            }
        }
        if let Some((_, ref mut text)) = current {
            if line.trim().is_empty() || line.starts_with('#') || line.starts_with('>') {
                if let Some((id, text)) = current.take() {
                    rules.push((id, first_sentence(&text)));
                }
            } else {
                text.push(' ');
                text.push_str(line.trim());
            }
        }
    }
    if let Some((id, text)) = current.take() {
        rules.push((id, first_sentence(&text)));
    }

    let spec_version = spec_version();

    let mut out = format!(
        "## Documentation (generated by `docsys rules --agents-md`, spec {spec_version} — do not edit)\n\n\
         Layout: `reference/` (facts the code cannot state) · `howto/` (procedures) ·\n\
         `explanation/` (why; ADRs) · `work/` (flowing: journal, features, postmortems,\n\
         research, debt, questions). `index.md` routes every permanent page.\n\n\
         Mechanics are the tool's job — run them, never re-derive them:\n\
         - `docsys lint` before every commit; errors block, warnings accumulate\n\
         - `docsys refs --repo .` when code references documentation\n\
         - a page about code pins the region it promises about (`docsys pin`); the pin\n\
           is the whole binding: `docsys backlinks <code-file>` names the pages that\n\
           describe a file, so the code carries no comment for it\n\
         - inside docs a page is linked as `[[dir/id]]` — the full path from the\n\
           docs root (R-070)\n\
         - a page pinned to code (`verifies:`) that lint reports stale is re-read\n\
           against the code, then `docsys pin --refresh <page>` — never refreshed blind\n\
         - a blocked Bash call is blocked whole: `git add … && git commit` re-runs\n\
           from the `add`; what landed is `git show HEAD:<file>`, not the tree\n\
         - a pin is worth keeping when a change to its region would likely make the\n\
           page false: pin a symbol (`Class.method`), never a large file whole — every\n\
           unrelated edit to it stales the page (R-111 read with R-151)\n\
         - when docsys is wrong or in your way: `docsys feedback --draft`, then ask the\n\
           person before filing it — filing publishes\n\n"
    );
    out.push_str(VERSION_SECTION);
    out.push_str("\nJudgment stays with you, but inside these rules:\n");
    // The procedures an agent needs at the moment it writes or verifies a
    // page, their question and options verbatim from §14.3 (D-114); a rule
    // that has one is stated there, not twice
    const WRITE: [&str; 5] = ["P/R-031", "P/R-033", "P/R-045", "P/R-102", "P/R-123"];
    const VERIFY: &str = "P/R-025";
    for (id, sentence) in &rules {
        let p = format!("P/{id}");
        if WRITE.contains(&p.as_str()) || p == VERIFY {
            continue;
        }
        out.push_str(&format!("- {id}: {sentence}\n"));
    }
    out.push_str("\nWhen you write a page:\n");
    for id in WRITE {
        out.push_str(&procedure_head(id).unwrap_or_default());
    }
    out.push_str(
        "- not known → `docsys question add <question>` (R-108), never a guess left on\n\
           a page\n\
         - agent memory is a question for the person, never a source (D-062)\n\
         - work deferred on purpose → `docsys debt add <debt> --deferred <reason>\n\
           --repay-when <trigger>`; once repaid, `docsys debt close <item> --note <how>`,\n\
           and the commit carries the `Resolved:` line it prints (R-108)\n",
    );
    out.push_str("\nWhen you verify:\n");
    out.push_str(&procedure_head(VERIFY).unwrap_or_default());
    out.push_str(
        "- read every claim against `sources:` and the code it names; a maintainer's own\n\
           word in the session is recorded with `docsys verify <page>` (D-096)\n",
    );
    out.push_str(
        "\nWhen a decision procedure exists, follow it: `docsys rules --procedures`.\n\
         When no option fits, the escape is always legitimate — an honest \"I don't\n\
         know\" (an `_unsorted/` file, a question item) is cheaper than a\n\
         confident guess.\n",
    );
    out
}

/// The agent's standing instruction about the version a tree runs (D-120),
/// the same in the project block and the knowledge-base contract. It names
/// where the pin lives, never the number, so it reads the same in every
/// release.
pub const VERSION_SECTION: &str =
    "Version: a tree that pins its docsys in `.docsys-version` (beside `.docmeta.yml`)\n\
runs that version on every docsys call, installed on first use. The pin moves\n\
only with `docsys upgrade`, never by hand. After pulling an upgrade, run\n\
`docsys upgrade --apply` once in this clone. On a line naming `docsys upgrade`\n\
or an install command, run `/docsys-upgrade` and ask the person first.\n";

/// R-165: the always-loaded text must fit the budget; the floor is the
/// rendered mandatory set. Returns Err with an explanation when violated.
pub fn check_budget(max_lines: usize) -> Result<(usize, usize), String> {
    let summary_lines = agents_md().lines().count();
    let floor = summary_lines;
    if max_lines < floor {
        return Err(format!(
            "agent_rules_max_lines {max_lines} is below the floor {floor} — a budget \
             that silently drops mandatory text is R-151's \"silently wrong\""
        ));
    }
    if summary_lines > max_lines {
        return Err(format!(
            "generated summary is {summary_lines} lines, over the {max_lines} budget"
        ));
    }
    Ok((summary_lines, max_lines))
}

pub const BLOCK_BEGIN: &str = "<!-- docsys:rules:begin — generated, do not edit inside -->";
pub const BLOCK_END: &str = "<!-- docsys:rules:end -->";

/// The managed block itself, markers and the owner's preamble (D-056) included.
pub fn agents_block_with(preamble: &str) -> String {
    format!("{BLOCK_BEGIN}\n{preamble}{}{BLOCK_END}\n", agents_md())
}

/// Write/update the generated block inside a managed marker region of `path`.
/// Owner prose outside the markers is never touched; re-runs are idempotent.
pub fn write_agents_block(path: &std::path::Path) -> Result<&'static str, String> {
    write_agents_block_with(path, "")
}

/// `write_agents_block`, with the owner's generated-file preamble (D-056) as
/// the first line INSIDE the block — a gate that reads the staged diff must
/// find it in every regeneration, not only at the top of the file.
pub fn write_agents_block_with(
    path: &std::path::Path,
    preamble: &str,
) -> Result<&'static str, String> {
    let block = agents_block_with(preamble);
    let existing = std::fs::read_to_string(path).ok();
    let new_text = match existing {
        None => block.clone(),
        Some(text) => match (text.find(BLOCK_BEGIN), text.find(BLOCK_END)) {
            (Some(a), Some(b)) if b > a => {
                let after = text.get(b + BLOCK_END.len()..).unwrap_or("");
                format!(
                    "{}{block}{}",
                    text.get(..a).unwrap_or(""),
                    after.trim_start_matches('\n')
                )
            }
            _ => {
                let sep = if text.ends_with('\n') { "\n" } else { "\n\n" };
                format!("{text}{sep}{block}")
            }
        },
    };
    std::fs::write(path, new_text).map_err(|e| e.to_string())?;
    Ok("written")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn procedures_render_verbatim_from_spec() {
        let p = procedures().unwrap_or_default();
        assert!(p.contains("P/R-031"), "type-selection procedure present");
        assert!(p.contains("P/R-123"), "translation procedure present");
        assert!(p.contains("NEVER"), "format fields present");
    }

    #[test]
    fn agents_md_lists_agent_rules_and_fits_default_budget() {
        let s = agents_md();
        // a rule with an inline procedure is stated once, as the procedure
        assert!(s.contains("- P/R-031 — ") && !s.contains("- R-031:"), "{s}");
        assert!(s.contains("- P/R-025 — ") && !s.contains("- R-025:"), "{s}");
        assert!(s.contains("R-081:"), "{s}");
        assert!(s.contains("docsys backlinks <code-file>"), "{s}");
        let lines = s.lines().count();
        assert!(
            lines <= 200,
            "summary is {lines} lines — over the default budget"
        );
    }
}
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests_more {
    use super::*;

    #[test]
    fn a_spec_section_is_cut_at_the_next_heading_of_its_level() {
        let s = section("## 19. Open questions").expect("section exists in SPEC");
        assert!(s.contains("Identifier-based document links"), "{s}");
        assert!(section("## no such heading").is_none());
        let sub = section("### 14.3 The authored procedures").unwrap();
        assert!(!sub.contains("\n## "), "stops before the next H2");
    }

    #[test]
    fn first_sentence_is_cut_at_the_first_boundary() {
        assert_eq!(first_sentence("One two.  Three\nfour."), "One two.");
        assert_eq!(first_sentence("No boundary here"), "No boundary here");
        assert_eq!(first_sentence("v1.2 is fine. Next"), "v1.2 is fine.");
        assert_eq!(first_sentence("  spaced\n  out  "), "spaced out");
    }

    #[test]
    fn the_budget_has_a_floor_and_a_ceiling() {
        let (lines, max) = check_budget(200).unwrap();
        assert!(lines <= max);
        let err = check_budget(1).unwrap_err();
        assert!(err.contains("below the floor"), "{err}");
        assert!(procedures().unwrap().starts_with("# Decision procedures"));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests_teach {
    use super::*;

    /// The block teaches the procedures an agent needs while it writes and
    /// verifies (D-114) — their QUESTION lines exactly as §14.3 has them —
    /// within the budget (R-165).
    #[test]
    fn the_block_carries_the_writing_and_verifying_procedures_verbatim() {
        let block = agents_md();
        let authored = procedures().unwrap();
        for id in [
            "P/R-031", "P/R-033", "P/R-045", "P/R-102", "P/R-123", "P/R-025",
        ] {
            let question = authored
                .lines()
                .skip_while(|l| !l.trim_start().starts_with(&format!("{id} — ")))
                .find(|l| l.trim_start().starts_with("QUESTION"))
                .unwrap()
                .trim();
            assert!(block.contains(question), "{id}: `{question}` missing");
        }
        assert!(
            block.contains("`docsys question add <question>`"),
            "{block}"
        );
        assert!(
            block.contains("`docsys debt close <item> --note <how>`"),
            "{block}"
        );
        assert!(
            !block.contains("debt.md") && !block.contains("questions.md"),
            "{block}"
        );
        assert!(block.contains(
            "when docsys is wrong or in your way: `docsys feedback --draft`, then ask the\nperson before filing it — filing publishes"
        ));
        assert!(block.contains("never a large file whole"));
        assert!(
            block.lines().count() <= 200,
            "{} lines",
            block.lines().count()
        );
        // nothing past DEFAULT: the generator renders, it does not invent (R-163)
        assert!(!block.contains("DEFAULT"), "{block}");
    }

    #[test]
    fn a_rule_sentence_names_its_tag_and_first_sentence() {
        assert_eq!(
            rule_sentence("R-071").as_deref(),
            Some("R-071 (lint · MUST): Every link target MUST resolve.")
        );
        assert_eq!(rule_sentence("R-999"), None);
    }

    /// The version section reads the same in every release: it names where
    /// the pin lives, never a number (D-120).
    #[test]
    fn the_version_section_names_no_version() {
        let digits_dot_digits = VERSION_SECTION
            .as_bytes()
            .windows(3)
            .any(|w| matches!(w, [a, b'.', c] if a.is_ascii_digit() && c.is_ascii_digit()));
        assert!(!digits_dot_digits, "{VERSION_SECTION}");
        assert!(agents_md().contains(VERSION_SECTION));
        assert!(crate::agents::kb_contract().ends_with(VERSION_SECTION));
    }

    /// A newcomer learns how a page and its code bind each other from the block
    /// itself: the pin is the one binding, read from the code's side by
    /// `backlinks`; nothing asks for a citation in the code.
    #[test]
    fn the_block_says_how_a_page_and_its_code_bind() {
        let flat = agents_md().replace('\n', " ");
        assert!(
            flat.contains("a page about code pins the region it promises about (`docsys pin`); the pin is the whole binding: `docsys backlinks <code-file>` names the pages that describe a file"),
            "{flat}"
        );
        assert!(!flat.contains("// doc:"), "{flat}");
    }
}
