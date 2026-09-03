#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// The knowledge-base hook layer, executed for real: the record guard, the
// organ routing, the `updated:` bump that never touches raw/, the end-of-turn
// inbox nudge, the doctor's verdict, and lint's check that a verified page
// still holds the body that was verified. Unix-only: the relays are bash.
#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-kbhooks-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn git(dir: &Path, args: &[&str]) {
    assert!(Command::new("git")
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap()
        .success());
}

fn write(base: &Path, rel: &str, text: &str) {
    let p = base.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

/// A base that is its own repository, with the agent layer installed, one
/// note in the inbox, one wiki page.
fn build_base(name: &str) -> PathBuf {
    let base = tmp(name);
    git(&base, &["init", "-q"]);
    git(&base, &["config", "user.email", "t@example.invalid"]);
    git(&base, &["config", "user.name", "t"]);
    docsys::migrate::init_profile(&base, "en", "knowledge-base").unwrap();
    let dm = base.join(".docmeta.yml");
    let text = fs::read_to_string(&dm)
        .unwrap()
        .replace("domains: []", "domains: [ops]");
    fs::write(&dm, text).unwrap();
    write(
        &base,
        "raw/inbox/2026-09-02-note.md",
        "Rotate keys monthly.\n",
    );
    // `updated:` is today: the page is committed today, and history must
    // agree with the field (R-106)
    write(
        &base,
        "wiki/ops/reference/rotation.md",
        &format!(
            "---\nid: rotation\ntype: reference\ndomain: ops\nverification: unverified\nupdated: {}\nsources: [raw/inbox/2026-09-02-note.md]\n---\n# Rotation\n\nThis page states the rotation cadence; read it before rotating.\n\nMonthly.\n",
            docsys::migrate::today()
        ),
    );
    write(
        &base,
        "wiki/ops/index.md",
        "# ops\n\n- [[ops/reference/rotation|Rotation]] -- the cadence.\n",
    );
    write(
        &base,
        "wiki/index.md",
        "# Knowledge base\n\n- [[ops/index|Ops]] -- operations.\n",
    );
    let done = docsys::agents::install_kb(&base.join(".claude"), &base, false).unwrap();
    assert!(
        done.written.iter().any(|w| w == "hooks/pre-commit-docs.sh"),
        "{done:?}"
    );
    assert!(
        done.written.iter().any(|w| w == "settings.json"),
        "{done:?}"
    );
    assert!(
        done.notes.iter().any(|n| n.contains("git pre-commit gate")),
        "{:?}",
        done.notes
    );
    git(&base, &["add", "-A"]);
    git(&base, &["commit", "-q", "-m", "base"]);
    base
}

/// Run one installed relay with a payload; `docsys` on PATH is the test
/// binary; TMPDIR isolated so once-per-session markers cannot leak.
fn run_relay(base: &Path, script: &str, payload: &str, session: &str) -> (i32, String, String) {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_docsys"));
    let path = format!(
        "{}:{}",
        bin.parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let markers = base.join(".markers");
    let _ = fs::create_dir_all(&markers);
    let payload = payload.replace("SESSION", session);
    let mut child = Command::new("bash")
        .arg(base.join(".claude/hooks").join(script))
        .current_dir(base)
        .env("PATH", path)
        .env("TMPDIR", &markers)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write as _;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn write_payload(path: &Path) -> String {
    format!(
        r#"{{"session_id":"SESSION","tool_name":"Write","tool_input":{{"file_path":"{}","content":"x"}}}}"#,
        path.display()
    )
}

#[test]
fn an_existing_record_is_guarded_and_a_new_note_passes() {
    let base = build_base("guard");
    let existing = base.join("raw/inbox/2026-09-02-note.md");
    let (code, _, err) = run_relay(&base, "pre-commit-docs.sh", &write_payload(&existing), "s1");
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("R-023"), "{err}");
    assert!(err.contains("raw/inbox/2026-09-02-note.md"), "{err}");
    // the record is untouched — the hook decided before the write
    assert_eq!(
        fs::read_to_string(&existing).unwrap(),
        "Rotate keys monthly.\n"
    );
    let fresh = base.join("raw/inbox/2026-09-02-another.md");
    let (code, _, err) = run_relay(&base, "pre-commit-docs.sh", &write_payload(&fresh), "s1");
    assert_eq!(code, 0, "{err}");
    let page = base.join("wiki/ops/reference/rotation.md");
    let (code, _, err) = run_relay(&base, "pre-commit-docs.sh", &write_payload(&page), "s1");
    assert_eq!(code, 0, "a wiki page is authored, not a record: {err}");
    // and a commit still runs the gate: the base lints clean, so it passes
    let (code, _, err) = run_relay(
        &base,
        "pre-commit-docs.sh",
        r#"{"session_id":"SESSION","tool_name":"Bash","tool_input":{"command":"git commit -m x"}}"#,
        "s1",
    );
    assert_eq!(code, 0, "{err}");
}

#[test]
fn a_wiki_edit_bumps_updated_and_a_record_never_changes() {
    let base = build_base("bump");
    let page = base.join("wiki/ops/reference/rotation.md");
    // an edit that skipped the tooling left the field behind
    let stale = fs::read_to_string(&page)
        .unwrap()
        .replace(&docsys::migrate::today(), "2026-01-01");
    fs::write(&page, stale).unwrap();
    let payload = format!(
        r#"{{"session_id":"SESSION","tool_name":"Edit","tool_input":{{"file_path":"{}","old_string":"Monthly.","new_string":"Weekly."}}}}"#,
        page.display()
    );
    let (code, _, err) = run_relay(&base, "post-edit-updated.sh", &payload, "s2");
    assert_eq!(code, 0, "{err}");
    let text = fs::read_to_string(&page).unwrap();
    assert!(
        text.contains(&format!("updated: {}", docsys::migrate::today())),
        "{text}"
    );
    let record = base.join("raw/inbox/2026-09-02-note.md");
    fs::write(&record, "updated: 2026-01-01\nRotate keys monthly.\n").unwrap();
    let payload = format!(
        r#"{{"session_id":"SESSION","tool_name":"Write","tool_input":{{"file_path":"{}","content":"x"}}}}"#,
        record.display()
    );
    let (code, _, _) = run_relay(&base, "post-edit-updated.sh", &payload, "s2");
    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(&record).unwrap(),
        "updated: 2026-01-01\nRotate keys monthly.\n",
        "no tooling edits a record"
    );
}

#[test]
fn the_first_turn_names_the_organs_once() {
    let base = build_base("routing");
    let payload =
        r#"{"session_id":"SESSION","hook_event_name":"UserPromptSubmit","prompt":"note this"}"#;
    let (code, out, _) = run_relay(&base, "session-intent.sh", payload, "s3");
    assert_eq!(code, 0);
    // a fresh base: the character survey comes first, then the organs
    assert!(out.starts_with("<first-run>"), "{out}");
    assert!(out.contains("Name — what they call the assistant"), "{out}");
    assert!(
        out.contains("in the language the person just wrote"),
        "{out}"
    );
    assert!(out.contains("knowledge base"), "{out}");
    assert!(out.contains("capture"), "{out}");
    assert!(out.contains("not in the base"), "{out}");
    assert!(out.contains("Speak the person's language"), "{out}");
    assert!(!out.contains("work/features"), "{out}");
    let (_, again, _) = run_relay(&base, "session-intent.sh", payload, "s3");
    assert!(again.is_empty(), "{again}");
    // the character is set: the survey never returns, the routing stays
    let agents = base.join("AGENTS.md");
    let text = fs::read_to_string(&agents).unwrap();
    let start = text.find("<!-- character: unset").unwrap();
    let end = start + text[start..].find("-->").unwrap() + 3;
    let set = format!(
        "{}- Name: Jarvis\n- Address: by first name, informal\n- Tone: plain and brief{}",
        &text[..start],
        &text[end..]
    );
    fs::write(&agents, set).unwrap();
    let (_, later, _) = run_relay(&base, "session-intent.sh", payload, "s3b");
    assert!(!later.contains("<first-run>"), "{later}");
    assert!(later.contains("knowledge base"), "{later}");
}

#[test]
fn the_end_of_a_turn_names_the_inbox_and_the_gate() {
    let base = build_base("stop");
    let (code, _, err) = run_relay(&base, "stop-docs-reminder.sh", "", "s4");
    assert_eq!(code, 0, "warns, never blocks");
    assert!(err.contains("1 note(s) waiting in raw/inbox"), "{err}");
    fs::remove_file(base.join("raw/inbox/2026-09-02-note.md")).unwrap();
    // a deleted record is a lint error (R-023): the nudge says the gate will stop
    let (_, _, err) = run_relay(&base, "stop-docs-reminder.sh", "", "s4");
    assert!(err.contains("error(s)"), "{err}");
    assert!(!err.contains("waiting"), "{err}");
}

#[test]
fn the_doctor_finds_the_layer_alive() {
    let base = build_base("doctor");
    let d = docsys::doctor::run(&base, &base, &base.join(".claude"));
    assert_eq!(d.failed, 0, "{:?}", d.lines);
}

#[test]
fn a_verified_page_must_still_hold_the_verified_body() {
    let base = build_base("verified");
    let page = base.join("wiki/ops/reference/rotation.md");
    let head = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(&base)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    let errors = |base: &Path| -> Vec<String> {
        let (r, _) = docsys::lint_in(base, Some(base));
        r.findings
            .iter()
            .filter(|f| f.severity == docsys::model::Severity::Error)
            .map(|f| format!("{} {} {}", f.rule, f.file, f.subject))
            .collect()
    };
    // the audit records the revision it read: bookkeeping only, body intact
    let text = fs::read_to_string(&page).unwrap().replace(
        "verification: unverified",
        &format!("verification: verified\nverified_by: other-session\nverified_rev: {head}"),
    );
    fs::write(&page, &text).unwrap();
    assert!(errors(&base).is_empty(), "{:?}", errors(&base));
    // an `updated:` bump is bookkeeping too
    fs::write(
        &page,
        text.replace("updated: 2026-01-01", "updated: 2026-09-02"),
    )
    .unwrap();
    assert!(errors(&base).is_empty(), "{:?}", errors(&base));
    // the body moves: the verification describes content that is gone
    fs::write(&page, text.replace("Monthly.", "Weekly.")).unwrap();
    let errs = errors(&base);
    assert!(
        errs.contains(&"R-024 wiki/ops/reference/rotation.md verification".to_string()),
        "{errs:?}"
    );
    // a revision nobody can audit
    fs::write(&page, text.replace(&head, "0000000")).unwrap();
    let errs = errors(&base);
    assert!(
        errs.contains(&"R-028 wiki/ops/reference/rotation.md verified_rev".to_string()),
        "{errs:?}"
    );
}

#[test]
fn an_existing_settings_file_is_merged_into_and_never_clobbered() {
    let base = tmp("settings-merge");
    git(&base, &["init", "-q"]);
    docsys::migrate::init_profile(&base, "en", "knowledge-base").unwrap();
    let claude = base.join(".claude");
    fs::create_dir_all(&claude).unwrap();
    let mine = "{\n  \"permissions\": { \"allow\": [\"Bash(ls:*)\"], \"deny\": [] },\n  \"hooks\": {\n    \"PreToolUse\": [\n      { \"matcher\": \"Bash\", \"hooks\": [ { \"type\": \"command\", \"command\": \"./mine.sh\" } ] }\n    ]\n  },\n  \"mcpServers\": { \"notes\": { \"command\": \"notes-mcp\" } }\n}\n";
    fs::write(claude.join("settings.json"), mine).unwrap();

    let done = docsys::agents::install_kb(&claude, &base, false).unwrap();
    assert!(
        done.written.iter().any(|w| w == "settings.json"),
        "{done:?}"
    );
    assert!(
        done.notes
            .iter()
            .any(|n| n.contains("merged 4 docsys hook wire(s)")),
        "{:?}",
        done.notes
    );
    let text = fs::read_to_string(claude.join("settings.json")).unwrap();
    let json = docsys::hook::parse_json(&text).expect("still JSON");
    assert_eq!(
        json.string_at(&["mcpServers", "notes", "command"]),
        Some("notes-mcp"),
        "{text}"
    );
    assert!(
        text.contains("Bash(ls:*)") && text.contains("./mine.sh"),
        "the person's permissions and hook stay:\n{text}"
    );
    for hook in [
        "session-intent.sh",
        "pre-commit-docs.sh",
        "post-edit-updated.sh",
        "stop-docs-reminder.sh",
    ] {
        assert!(text.contains(hook), "{hook} missing:\n{text}");
    }
    let (p, h, m) = (
        text.find("\"permissions\"").unwrap(),
        text.find("\"hooks\"").unwrap(),
        text.find("\"mcpServers\"").unwrap(),
    );
    assert!(p < h && h < m, "key order is the person's:\n{text}");

    // idempotent: nothing to add, nothing written
    let again = docsys::agents::install_kb(&claude, &base, false).unwrap();
    assert!(
        again.skipped.iter().any(|s| s == "settings.json"),
        "{again:?}"
    );
    assert_eq!(
        fs::read_to_string(claude.join("settings.json")).unwrap(),
        text
    );

    // a file that is not JSON is never touched
    fs::write(claude.join("settings.json"), "{ not json\n").unwrap();
    let broken = docsys::agents::install_kb(&claude, &base, true).unwrap();
    assert!(
        broken.notes.iter().any(|n| n.contains("not valid JSON")),
        "{:?}",
        broken.notes
    );
    assert_eq!(
        fs::read_to_string(claude.join("settings.json")).unwrap(),
        "{ not json\n"
    );
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn the_installed_layer_names_the_sources_beyond_the_inbox() {
    // D-087: what an agent needs is in the installed layer, never in a prompt
    let base = build_base("layer-text");
    let agents = fs::read_to_string(base.join("AGENTS.md")).unwrap();
    for needle in [
        "## Sources beyond the inbox",
        "docsys consume add",
        "docsys fetch",
        "@namespace/id",
        "docsys inbox pull",
        "docsys status",
        "docsys assistant",
        "docsys raw move",
        "every file under `wiki/`",
        "questions ledger",
    ] {
        assert!(agents.contains(needle), "AGENTS.md lacks `{needle}`");
    }
    let ingest = fs::read_to_string(base.join(".claude/skills/kb-ingest/SKILL.md")).unwrap();
    for needle in [
        "docsys raw move",
        "@namespace/id",
        "R-027",
        "(noise) stays too, with one dated line",
        "- [ ] YYYY-MM-DD",
        "unless the person you are working with is a declared maintainer",
    ] {
        assert!(ingest.contains(needle), "kb-ingest lacks `{needle}`");
    }
    assert!(
        !ingest.contains("move the note from"),
        "the hand move is no longer taught"
    );
    let _ = fs::remove_dir_all(&base);
}
