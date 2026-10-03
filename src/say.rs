//! What docsys says at a moment — a relay's reply, a gate's line, an
//! upgrade's row — in one place. Each text says one thing once and is true
//! for the mode it runs in: a `git add` is named only when the blocked call
//! ran one, the re-run is said once, and a docsys/0.4 tree hears what 0.15.1
//! said (D-118, D-129).

use std::path::Path;

/// How a refused Bash call runs again: the commit alone, or — when the call
/// staged files too — the whole call, its `git add` with it.
fn rerun(adds: bool) -> &'static str {
    if adds {
        "run the SAME command again, its `git add` with it (this whole Bash call was blocked)"
    } else {
        "run the SAME commit again"
    }
}

/// The question under `commit_policy: ask`, below the GATE line that names
/// what moved (D-040).
pub fn ask(adds: bool) -> String {
    format!(
        "If a contract moved, update the page (or say why in the commit message: `Docs: <why>`) and commit; if nothing user-visible moved, {} — this gate asks once.\n",
        rerun(adds)
    )
}

/// The refusal under `commit_policy: require`, every time (R-209).
pub fn require(adds: bool) -> String {
    format!(
        "commit_policy: require — name the work, record it — a work file under work/<category>/ or, at minimum, a `Docs: <why>` trailer in this commit's message — stage it, and {}. {BYPASS}, and it leaves a debt item.\n",
        rerun(adds)
    )
}

/// The question and the refusal as 0.15.1 said them, which a docsys/0.4 tree
/// still hears (D-118).
pub const ASK_015: &str = "code moves with no documentation change. If a contract moved, update the page (or say why in the commit message: `Docs: <why>`) and commit; if nothing user-visible moved, run the same commit again — this gate asks once.\nThis whole Bash call was blocked — a `git add` in it did not run either. Re-run the SAME command from the start, `add` included.\n";
pub const REQUIRE_015: &str = "commit_policy: require — nothing lands without its documentation. Name the work (feature | bug | improvement | research), record it — a work file under work/<category>/ or, at minimum, a `Docs: <why>` trailer in this commit's message — stage it, and run the SAME commit again, `git add` included. DOCSYS_SKIP=1 bypasses once and leaves a debt item.\nThis whole Bash call was blocked — a `git add` in it did not run either.\n";

/// What a blocked call takes with it, as 0.15.1 said it on every refusal.
const BLOCKED_015: &str = " This whole Bash call was blocked, any `git add` in it included.";

/// What a blocked call takes with it: on docsys/0.5 its `git add`, named only
/// when it ran one.
fn blocked(v05: bool, adds: bool) -> &'static str {
    match (v05, adds) {
        (false, _) => BLOCKED_015,
        (true, true) => " This whole Bash call was blocked, its `git add` too.",
        (true, false) => "",
    }
}

/// The bypass a relay names: the agent it speaks to cannot take it, since
/// the relay refuses `DOCSYS_SKIP=1` in the agent's own command (R-209).
const BYPASS: &str =
    "Only the person bypasses this, once: `DOCSYS_SKIP=1 git commit` at their terminal";

/// Lint errors stop the commit the relay saw.
pub fn lint_block(v05: bool, adds: bool) -> String {
    if !v05 {
        return format!(
            "docsys gate: lint errors block this commit — fix them first (DOCSYS_SKIP=1 to bypass once).{}\n",
            blocked(v05, adds)
        );
    }
    format!(
        "docsys gate: lint errors block this commit — fix them first. {BYPASS}.{}\n",
        blocked(v05, adds)
    )
}

/// A seed plan in the change set stops the commit the relay saw (D-091).
pub fn seed_plan_block(v05: bool, adds: bool, plans: &[String]) -> String {
    format!(
        "docsys gate: a seed plan is in this commit ({}) — a plan is a conversation's draft, \
         never documentation (R-003, D-091). `git reset -- {}`, land its rows with \
         `docsys seed apply --plan {} --repo . --root <docs>`, and commit the docs root only.{}\n",
        plans.join(", "),
        plans.join(" "),
        plans.first().map(String::as_str).unwrap_or("SEED.tsv"),
        blocked(v05, adds)
    )
}

/// The retry after a blocked call that ran `git add` runs none (D-049).
pub const DROPPED_ADD: &str = "docsys gate: the blocked call ran `git add`; this retry does not, and the working tree still has unstaged changes — did your `git add` run? Re-run the original command from the start, or stage explicitly. (asked once)\n";

/// A record of a knowledge base is never edited through the agent's tools
/// (R-023).
pub fn raw_record(rel: &str) -> String {
    format!(
        "docsys: raw/ is the record and content-immutable (R-023): `{rel}` already \
         exists and is never edited or overwritten. New knowledge is a NEW file under \
         raw/inbox/; a processed note moves with `docsys raw move <record> <domain>` — \
         bytes untouched, every citing page's sources: rewritten (R-027).\n"
    )
}

/// The end of a turn that changed code and no documentation, on docsys/0.5.
pub fn stop_undocumented(where_: &str) -> String {
    format!(
        "docs: {where_} changed code but no documentation — if a contract\nmoved, the page moves in the SAME session; at minimum the commit says why:\n`Docs: <why>`.\n"
    )
}

/// The turn held under `require`: on 0.5 the reason above names the way out.
pub const HELD: &str = "commit_policy: require — this turn holds until the work is named and recorded as said above.\n";
pub const HELD_015: &str = "commit_policy: require — before this session ends, name the work (feature | bug | improvement | research) and record it: a work file under work/<category>/, or a commit whose message says why with `Docs: <why>`. Then stop.\n";

/// A knowledge base's end of turn: what waits.
pub fn stop_kb(notes: usize, errors: usize) -> String {
    let mut msg = String::new();
    if notes > 0 {
        msg.push_str(&format!(
            "base: {notes} note(s) waiting in raw/inbox — `process my inbox` distils them when you are ready.\n"
        ));
    }
    if errors > 0 {
        msg.push_str(&format!(
            "base: docsys lint reports {errors} error(s) — the gate stops the next commit until they are fixed.\n"
        ));
    }
    msg
}

/// The git gate's line naming a change set with code and no documentation.
pub fn gate_undocumented(scope: &str, code: &[String]) -> String {
    let head: Vec<&str> = code.iter().take(5).map(String::as_str).collect();
    let more = code.len().saturating_sub(head.len());
    let tail = if more > 0 {
        format!(" (+{more} more)")
    } else {
        String::new()
    };
    format!(
        "GATE {scope} changes with no docs change: {}{tail}",
        head.join(", ")
    )
}

/// The git gate under `require` on a docsys/0.4 tree, as 0.15.1 said it.
pub const GATE_REQUIRE_015: &str = "GATE commit_policy: require — name the work (feature | bug | improvement | research), record it (a work file, or `Docs: <why>` in the commit message), stage it, commit again. DOCSYS_SKIP=1 bypasses once and leaves a debt item.";

/// The git gate's line for a seed plan in the change set (D-091).
pub fn gate_seed_plan(plans: &[String]) -> String {
    format!(
        "GATE a seed plan is in the change set: {} — a plan is a draft, never documentation (D-091); `git reset -- {}` and land it with `docsys seed apply`",
        plans.join(", "),
        plans.join(" ")
    )
}

/// A bypassed gate under `require` leaves a debt item (D-093).
pub fn gate_bypassed(file: &str) -> String {
    format!("gate: bypassed under commit_policy: require — a debt item records it in {file}")
}

/// The commit-msg gate's refusal: code, no documentation, no why (R-209).
pub fn message_refusal(code: &[&str]) -> String {
    format!(
        "GATE commit_policy: require — this commit changes {} and no documentation, and its message says nothing of why: add a `Docs: <why>` line, or the page or work file the change needs (R-209)",
        code.join(", ")
    )
}

/// A closed item's record is its commit's trailer (R-108).
pub fn message_trailer(removed: usize, dir: &str, trailer: &str) -> String {
    format!(
        "GATE this commit removes {removed} item(s) from {dir} and carries no `{trailer}:` line — that line is the record of what closed them (R-108)"
    )
}

/// A `Docs:` entry keeps its budget (R-101).
pub fn message_budget(entry: usize, max: usize) -> String {
    format!(
        "GATE the `Docs:` entry is {entry} lines; a journal entry keeps {max} — link the page, do not narrate (R-101)"
    )
}

/// The mode line of the pre-commit block: what stops the commit.
pub const GATE_HARD: &str = "# Hard gate: lint errors and dangling references stop the commit.";
pub const GATE_WARN: &str =
    "# Warn-mode until the adoption debt is triaged; `docsys adopt` hardens it once lint is clean.";
/// The mode line of the commit-msg block: what this half stops, which is the
/// message's, not lint's (D-125).
pub const MESSAGE_HARD: &str =
    "# Hard gate: a message the gate refuses stops the commit (commit_policy: require).";
pub const MESSAGE_WARN: &str =
    "# Warn-mode with the pre-commit half: the gate's lines inform until `docsys adopt` hardens it.";

/// The findings an upgrade adds and removes: a forecast before the move.
pub fn upgrade_preview(to: u32, added: usize, removed: usize) -> String {
    format!(
        "judged by docsys/0.{to} as the tree is now: {added} new finding(s), {removed} gone — the steps above clear their part"
    )
}

/// Each phrase of the agent text that names the journal, as a docsys/0.5
/// tree hears it and as a 0.4 tree heard it from 0.15.1 (D-118).
const ERA_PHRASES: [(&str, &str); 15] = [
    (
        "improvement (refactor, performance, cleanup), research. If the",
        "improvement (refactor, performance, cleanup), research, idea-note. If the",
    ),
    (
        "update the page (or say why in the commit message: `Docs: <why>`) and commit",
        "update the page (or add the journal line) and commit",
    ),
    (
        "a work file under work/<category>/ or, at minimum, a `Docs: <why>` trailer in this commit's message",
        "a work file under work/<category>/ or, at minimum, a journal entry linking these files and saying why",
    ),
    (
        "a work file under work/<category>/, or a commit whose message says why with `Docs: <why>`",
        "a work file under work/<category>/ or a journal entry linking the files and saying why",
    ),
    (
        "wrong line = the fix, wrong assumption = invariant\nin reference/ or a postmortem (test: can it recur?); research",
        "wrong line = journal line, wrong assumption = invariant\nin reference/ or a postmortem (test: can it recur?); improvement touching a\npublic surface → reference/ updated, and always record WHY; research",
    ),
    (
        "idea → a question item or a roadmap line",
        "idea → journal or roadmap line",
    ),
    (
        "An id is unique across the whole tree, drafts included.\n</session-doc-routing>",
        "An id is unique across the whole tree, drafts included.\nEnd of session: journal line (≤5 lines, links not content). Gate: docsys lint.\nJudgment calls follow the procedures: docsys rules --procedures.\n</session-doc-routing>",
    ),
    (
        "the end of a turn holds until the work is recorded.\n",
        "the end of a turn holds until the work is recorded (feature | bug | improvement | research → work file or journal entry).\n",
    ),
    (
        "record it (a work file, or `Docs: <why>` in the commit message)",
        "record it (a work file or a journal entry linking these files)",
    ),
    (
        "Contract-surface changes update their documentation in the SAME session.\n",
        "Contract-surface changes update their documentation in the SAME session.\nA permanent page you write from evidence, or change in substance, carries\n`verification: unverified` (+ sources); a maintainer verifies it in another\nsession — `verified_by:` and `confirmed:` name someone in .docmeta.yml\n`maintainers:` (R-208). Nothing you write is the truth yet; say so in the page.\n",
    ),
    (
        "ingest → one wiki page per note (id, type, domain,\nsources),",
        "ingest → one wiki page per note (id, type, domain, verification: unverified,\nsources),",
    ),
    // a docsys/0.5 base keeps no verification (D-130); a 0.4 one hears 0.15.1
    (
        "capture,\ningest or lookup;",
        "capture,\ningest, audit or lookup;",
    ),
    (
        "rewritten by the tool). lookup → `docsys",
        "rewritten by the tool). audit → only in a session that did not write the\npage; `verified` records verified_by and verified_rev. lookup → `docsys",
    ),
    (
        "the hook blocks the attempt. Gate: docsys lint",
        "the hook blocks the attempt. A wiki page whose body changes is unverified\nagain. Gate: docsys lint",
    ),
    (
        "edit a record).",
        "edit a record, verify its own page).",
    ),
];

/// The agent text is written for docsys/0.5, whose journal is history; a
/// docsys/0.4 tree hears its journal named as 0.15.1 named it (D-118).
pub fn era_text(root: &Path, text: &str) -> String {
    if crate::era::Era::at(root).journal_from_history() {
        return text.to_string();
    }
    ERA_PHRASES
        .iter()
        .fold(text.to_string(), |t, (now, before)| t.replace(now, before))
}

/// What a docsys/0.5 tree has instead of page verification (D-130).
pub const NO_VERIFICATION: &str = "a docsys/0.5 page carries no verification — `/docsys-crosscheck` has an agent check pages against their sources and code, and fix what is wrong";

/// Every text above as a docsys/0.5 tree hears it, in each mode: what the
/// one-home check reads (D-129).
pub fn catalog() -> Vec<String> {
    let code = vec!["src/x.rs".to_string()];
    let plans = vec!["SEED.tsv".to_string()];
    let mut out = Vec::new();
    for adds in [false, true] {
        out.push(format!(
            "{}\n{}",
            gate_undocumented("staged", &code),
            ask(adds)
        ));
        out.push(format!(
            "{}\n{}",
            gate_undocumented("staged", &code),
            require(adds)
        ));
        out.push(lint_block(true, adds));
        out.push(seed_plan_block(true, adds, &plans));
    }
    out.push(DROPPED_ADD.to_string());
    out.push(raw_record("raw/inbox/a.md"));
    out.push(format!("{}{HELD}", stop_undocumented("this session")));
    out.push(stop_kb(2, 1));
    out.push(gate_seed_plan(&plans));
    out.push(gate_bypassed("work/debt/general.md"));
    out.push(message_refusal(&["src/x.rs"]));
    out.push(message_trailer(1, "work/debt", "Resolved"));
    out.push(message_budget(7, 5));
    out.extend([GATE_HARD, GATE_WARN, MESSAGE_HARD, MESSAGE_WARN].map(String::from));
    out.push(upgrade_preview(5, 3, 0));
    out.push(NO_VERIFICATION.to_string());
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    /// No text of the catalog says six words in a row twice, in any mode.
    #[test]
    fn each_text_says_each_thing_once() {
        for text in super::catalog() {
            let words: Vec<String> = text
                .split_whitespace()
                .map(|w| {
                    w.chars()
                        .filter(|c| c.is_alphanumeric())
                        .collect::<String>()
                        .to_lowercase()
                })
                .filter(|w| !w.is_empty())
                .collect();
            let mut seen = std::collections::BTreeSet::new();
            for p in words.windows(6).map(|w| w.join(" ")) {
                assert!(seen.insert(p.clone()), "said twice: `{p}` in {text}");
            }
        }
    }
}
