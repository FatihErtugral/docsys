#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! The journal is history on a docsys/0.5 tree (§10, D-125): `docsys journal`
//! reads it, the commit-msg gate holds what `commit_policy: require` asks of a
//! message, and a range in CI is answered by a `Docs:` line.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-journal-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
}

/// git with this build first on PATH, so the gate the hooks run is this one.
fn git(dir: &Path, args: &[&str]) -> Output {
    let path = format!(
        "{}:{}",
        bin().parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("PATH", path)
        .env_remove("DOCSYS_DISPATCHED")
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .output()
        .unwrap()
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let out = git(dir, args);
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn docsys(dir: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .current_dir(dir)
        .env_remove("DOCSYS_DISPATCHED")
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .output()
        .unwrap()
}

/// An adopted repository, its gate hard, committed.
fn adopted(name: &str, policy: &str) -> PathBuf {
    let repo = tmp(name);
    ok(&repo, &["init", "-q", "-b", "main"]);
    ok(&repo, &["config", "user.email", "t@example.invalid"]);
    ok(&repo, &["config", "user.name", "t"]);
    ok(&repo, &["config", "commit.gpgsign", "false"]);
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    let out = docsys(&repo, &["adopt"]);
    assert!(out.status.success(), "{out:?}");
    let meta = repo.join("docs/.docmeta.yml");
    let text = fs::read_to_string(&meta).unwrap();
    fs::write(
        &meta,
        text.replace("commit_policy: ask", &format!("commit_policy: {policy}")),
    )
    .unwrap();
    ok(&repo, &["add", "-A"]);
    ok(
        &repo,
        &[
            "commit",
            "-qm",
            "adopt docsys",
            "-m",
            "Docs: the tree itself",
        ],
    );
    repo
}

#[test]
fn the_journal_is_read_from_history_newest_first_then_the_frozen_files() {
    let repo = adopted("render", "ask");
    // a code change that needed no page, said in its message
    fs::write(repo.join("main.rs"), "fn main() { run() }\n").unwrap();
    ok(
        &repo,
        &[
            "commit",
            "-qam",
            "run on start",
            "-m",
            "Docs: the entry point only calls run",
        ],
    );
    // a code change that says nothing: no entry
    fs::write(repo.join("main.rs"), "fn main() { run(); }\n").unwrap();
    ok(&repo, &["commit", "-qam", "format"]);
    // a docs change: an entry
    fs::create_dir_all(repo.join("docs/reference")).unwrap();
    fs::write(
        repo.join("docs/reference/run.md"),
        "---\nid: run\ntype: reference\n---\nThis page states what run does; read it before changing it.\n",
    )
    .unwrap();
    ok(&repo, &["add", "-A"]);
    ok(
        &repo,
        &[
            "commit",
            "-qm",
            "the run page",
            "-m",
            "what run does, for the next reader",
        ],
    );
    // a 0.4 tree's journal, frozen by the move
    fs::create_dir_all(repo.join("docs/_archive/journal")).unwrap();
    fs::write(
        repo.join("docs/_archive/journal/journal.md"),
        "# Journal\n\n## 2026-08-01 - before the move\n- written by hand\n",
    )
    .unwrap();
    let out = docsys(&repo, &["journal"]);
    assert!(out.status.success(), "{out:?}");
    let text = String::from_utf8_lossy(&out.stdout);
    let heads: Vec<&str> = text.lines().filter(|l| l.starts_with("## ")).collect();
    let titles: Vec<&str> = heads
        .iter()
        .map(|h| h.split_once(" - ").map_or("", |(_, t)| t))
        .collect();
    assert_eq!(
        titles,
        [
            "the run page",
            "run on start",
            "adopt docsys",
            "before the move"
        ],
        "{text}"
    );
    assert!(text.contains("## 2026-08-01 - before the move"), "{text}");
    assert!(
        !text.contains(" - format"),
        "a silent code change is no entry: {text}"
    );
    assert!(
        text.contains("Docs: the entry point only calls run"),
        "{text}"
    );
    assert!(
        text.contains("<!-- frozen: _archive/journal/journal.md -->"),
        "{text}"
    );
    // since: history's entries from that day, the frozen files left out
    let today = docsys::migrate::today();
    let out = docsys(&repo, &["journal", "--since", &today]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!text.contains("before the move"), "{text}");
    // `journal add` writes nothing: it hands back the message
    let out = docsys(
        &repo,
        &["journal", "add", "Run settled", "--link", "reference/run"],
    );
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "Run settled\n\nDocs: reference/run\n"
    );
    assert_eq!(ok(&repo, &["status", "--porcelain"]), "?? docs/_archive/\n");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn outside_history_the_journal_is_the_frozen_files_and_says_so() {
    let root = tmp("no-repo").join("docs");
    docsys::migrate::init_profile(&root, "en", "project").unwrap();
    let text = docsys::journal::render(None, &root, None);
    assert!(text.contains("history: unknown"), "{text}");
}

#[test]
fn under_require_the_message_gate_wants_why_and_a_closed_item_wants_its_trailer() {
    let repo = adopted("require", "require");
    assert!(
        fs::read_to_string(repo.join(".git/hooks/commit-msg"))
            .unwrap()
            .contains("docsys gate --repo . --root docs --message"),
        "adopt writes the commit-msg half"
    );
    // code alone, and a message that says nothing: refused
    fs::write(repo.join("main.rs"), "fn main() { run() }\n").unwrap();
    ok(&repo, &["add", "main.rs"]);
    let out = git(&repo, &["commit", "-qm", "run on start"]);
    assert!(!out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("add a `Docs: <why>` line"),
        "{out:?}"
    );
    // the same commit saying why lands
    ok(
        &repo,
        &[
            "commit",
            "-qm",
            "run on start",
            "-m",
            "Docs: the entry point only calls run",
        ],
    );
    // a debt closed without its trailer lands, and is said
    let added = docsys(
        &repo,
        &[
            "debt",
            "add",
            "run has no timeout",
            "--deferred",
            "no load",
            "--repay-when",
            "first slow start",
        ],
    );
    assert!(added.status.success(), "{added:?}");
    ok(&repo, &["add", "-A"]);
    ok(&repo, &["commit", "-qm", "a debt"]);
    let closed = docsys(&repo, &["debt", "close", "1", "--note", "bounded"]);
    assert!(closed.status.success(), "{closed:?}");
    ok(&repo, &["add", "-A"]);
    let out = git(&repo, &["commit", "-qm", "repaid"]);
    assert!(out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("carries no `Resolved:` line"),
        "{out:?}"
    );
    // a team's long body is its own convention; a long `Docs:` entry is said
    fs::write(repo.join("main.rs"), "fn main() { run(); }\n").unwrap();
    ok(&repo, &["add", "main.rs"]);
    let out = git(
        &repo,
        &[
            "commit",
            "-qm",
            "long body",
            "-m",
            "1\n2\n3\n4\n5\n6",
            "-m",
            "Docs: why",
        ],
    );
    assert!(out.status.success(), "{out:?}");
    assert!(
        !String::from_utf8_lossy(&out.stderr).contains("GATE"),
        "{out:?}"
    );
    fs::write(repo.join("main.rs"), "fn main() { run(); run(); }\n").unwrap();
    ok(&repo, &["add", "main.rs"]);
    let out = git(
        &repo,
        &[
            "commit",
            "-qm",
            "long entry",
            "-m",
            "Docs: one\n two\n three\n four\n five\n six",
        ],
    );
    assert!(out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("the `Docs:` entry is 6 lines"),
        "{out:?}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// What `journal add` prints is a message as it stands (D-125): a message
/// that is only its `Docs:` line says why — to the commit-msg gate, to a range
/// in CI, and to the journal history makes.
#[test]
fn the_line_journal_add_prints_is_a_message_the_gate_takes() {
    let repo = adopted("docs-only", "require");
    let base = ok(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    let msg = tmp("docs-only-msg").join("msg");
    for (i, why) in ["the entry point only calls run", "line 1\nline 2"]
        .iter()
        .enumerate()
    {
        fs::write(repo.join("main.rs"), format!("fn main() {{ run({i}) }}\n")).unwrap();
        ok(&repo, &["add", "main.rs"]);
        let printed = docsys(&repo, &["journal", "add", why]);
        assert!(printed.status.success(), "{printed:?}");
        fs::write(&msg, &printed.stdout).unwrap();
        let out = git(&repo, &["commit", "-q", "-F", msg.to_str().unwrap()]);
        assert!(out.status.success(), "{why}: {out:?}");
    }
    let range = format!("{base}..HEAD");
    let out = docsys(&repo, &["gate", "--range", &range]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let out = docsys(&repo, &["journal"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("Docs: the entry point only calls run"),
        "{text}"
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_range_is_answered_by_a_docs_line_in_any_of_its_commits() {
    let repo = adopted("range", "ask");
    let base = ok(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    fs::write(repo.join("main.rs"), "fn main() { run() }\n").unwrap();
    ok(&repo, &["commit", "-qam", "run on start"]);
    let range = format!("{base}..HEAD");
    let out = docsys(&repo, &["gate", "--range", &range]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    ok(
        &repo,
        &[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "why run",
            "-m",
            "Docs: the entry point only calls run",
        ],
    );
    let out = docsys(&repo, &["gate", "--range", &range]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let _ = fs::remove_dir_all(&repo);
}

/// Under `commit_policy: ask` the commit-msg block says nothing: no refusal,
/// no report — the message is the team's own.
#[test]
fn under_ask_the_message_gate_is_silent() {
    let repo = adopted("ask-silent", "ask");
    let added = docsys(
        &repo,
        &[
            "debt",
            "add",
            "run has no timeout",
            "--deferred",
            "no load",
            "--repay-when",
            "first slow start",
        ],
    );
    assert!(added.status.success(), "{added:?}");
    ok(&repo, &["add", "-A"]);
    ok(&repo, &["commit", "-qm", "a debt"]);
    let closed = docsys(&repo, &["debt", "close", "1", "--note", "bounded"]);
    assert!(closed.status.success(), "{closed:?}");
    fs::write(repo.join("main.rs"), "fn main() { run() }\n").unwrap();
    ok(&repo, &["add", "-A"]);
    let out = git(
        &repo,
        &[
            "commit",
            "-qm",
            "repaid",
            "-m",
            "Docs: one\n two\n three\n four\n five\n six",
        ],
    );
    assert!(out.status.success(), "{out:?}");
    assert!(
        !String::from_utf8_lossy(&out.stderr).contains("GATE"),
        "{out:?}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// Commits of one second keep history's order: a code commit carrying
/// `Docs:` sits between the docs commits around it, not after them.
#[test]
fn entries_of_one_second_keep_the_order_history_gives_them() {
    let repo = adopted("one-second", "ask");
    let at = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&repo)
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    bin().parent().unwrap().display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("GIT_COMMITTER_DATE", "2030-01-01T12:00:00Z")
            .env("GIT_AUTHOR_DATE", "2030-01-01T12:00:00Z")
            .env("DOCSYS_NO_AUTO_INSTALL", "1")
            .env_remove("DOCSYS_DISPATCHED")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
    };
    fs::create_dir_all(repo.join("docs/reference")).unwrap();
    let page = |n: &str| {
        fs::write(
            repo.join(format!("docs/reference/{n}.md")),
            format!("---\nid: {n}\ntype: reference\n---\nThis page states what {n} does; read it before changing it.\n"),
        )
        .unwrap();
    };
    page("first");
    at(&["add", "-A"]);
    at(&["commit", "-qm", "first page"]);
    fs::write(repo.join("main.rs"), "fn main() { run() }\n").unwrap();
    at(&[
        "commit",
        "-qam",
        "code between",
        "-m",
        "Docs: no page needed",
    ]);
    page("second");
    at(&["add", "-A"]);
    at(&["commit", "-qm", "second page"]);
    let entries = docsys::journal::entries(&repo, &repo.join("docs"), None).unwrap();
    let titles: Vec<&str> = entries.iter().map(|e| e.title.as_str()).take(3).collect();
    assert_eq!(titles, ["second page", "code between", "first page"]);
    let _ = fs::remove_dir_all(&repo);
}

/// The record of a closed item is its commit's trailer, whether its file
/// keeps other items or goes with it, counted in items; a line that moves
/// from one topic file to another closes nothing (R-108, D-124).
#[test]
fn every_closed_item_is_counted_whether_its_file_stays_or_goes() {
    let repo = adopted("closed-items", "require");
    for w in ["one", "two", "three"] {
        let added = docsys(
            &repo,
            &["debt", "add", w, "--deferred", "x", "--repay-when", "y"],
        );
        assert!(added.status.success(), "{added:?}");
    }
    ok(&repo, &["add", "-A"]);
    ok(
        &repo,
        &["commit", "-qm", "items", "-m", "Docs: three debts"],
    );
    let said = |out: Output| String::from_utf8_lossy(&out.stderr).into_owned();
    // one closed, two stay in the file
    let closed = docsys(&repo, &["debt", "close", "one", "--note", "done"]);
    assert!(closed.status.success(), "{closed:?}");
    ok(&repo, &["add", "-A"]);
    let out = said(git(&repo, &["commit", "-qm", "close one"]));
    assert!(out.contains("removes 1 item(s) from work/debt"), "{out}");
    // the file goes with both of its items
    ok(&repo, &["rm", "-q", "docs/work/debt/general.md"]);
    let out = said(git(&repo, &["commit", "-qm", "drop the rest"]));
    assert!(out.contains("removes 2 item(s) from work/debt"), "{out}");
    // a line that moves to another topic closes nothing
    let added = docsys(
        &repo,
        &[
            "debt",
            "add",
            "four",
            "--deferred",
            "x",
            "--repay-when",
            "y",
        ],
    );
    assert!(added.status.success(), "{added:?}");
    ok(&repo, &["add", "-A"]);
    ok(&repo, &["commit", "-qm", "four", "-m", "Docs: a debt"]);
    // its topic is its tag: the move adds one, and a later move changes it
    let line = fs::read_to_string(repo.join("docs/work/debt/general.md")).unwrap();
    let tagged = line.replacen(" four --", " [cache] four --", 1);
    assert_ne!(tagged, line);
    fs::remove_file(repo.join("docs/work/debt/general.md")).unwrap();
    fs::write(repo.join("docs/work/debt/cache.md"), &tagged).unwrap();
    ok(&repo, &["add", "-A"]);
    let out = said(git(&repo, &["commit", "-qm", "four is about the cache"]));
    assert!(!out.contains("item(s)"), "{out}");
    fs::remove_file(repo.join("docs/work/debt/cache.md")).unwrap();
    fs::write(
        repo.join("docs/work/debt/api.md"),
        tagged.replacen("[cache]", "[api]", 1),
    )
    .unwrap();
    ok(&repo, &["add", "-A"]);
    let out = said(git(&repo, &["commit", "-qm", "four is about the api"]));
    assert!(!out.contains("item(s)"), "{out}");
    let _ = fs::remove_dir_all(&repo);
}

/// On a docsys/0.4 tree the journal is its own file (D-118): `docsys journal`
/// shows `work/journal.md` as the tree keeps it, never commit subjects.
#[test]
fn on_a_0_4_tree_the_journal_is_its_own_file() {
    let repo = tmp("v04-journal");
    ok(&repo, &["init", "-q", "-b", "main"]);
    ok(&repo, &["config", "user.email", "t@example.invalid"]);
    ok(&repo, &["config", "user.name", "t"]);
    ok(&repo, &["config", "commit.gpgsign", "false"]);
    fs::create_dir_all(repo.join("docs/work")).unwrap();
    fs::write(
        repo.join("docs/.docmeta.yml"),
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n",
    )
    .unwrap();
    fs::write(
        repo.join("docs/work/journal.md"),
        "# Journal\n\n## 2026-10-02 - a note the tree keeps\n- written by hand\n\n## 2026-09-01 - initialized\n",
    )
    .unwrap();
    ok(&repo, &["add", "-A"]);
    ok(
        &repo,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-qm",
            "a commit subject",
        ],
    );
    let out = docsys(&repo, &["journal"]);
    assert!(out.status.success(), "{out:?}");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("## 2026-10-02 - a note the tree keeps"),
        "{text}"
    );
    assert!(text.contains("## 2026-09-01 - initialized"), "{text}");
    assert!(!text.contains("a commit subject"), "{text}");
    let out = docsys(&repo, &["journal", "--since", "2026-10-01"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("a note the tree keeps") && !text.contains("initialized"),
        "{text}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// `journal add` says each thing once: the why is the `Docs:` line, a title
/// the person gives is the subject, a page linked is the trailer's value
/// (D-125).
#[test]
fn journal_add_says_the_why_once() {
    let repo = adopted("add-once", "ask");
    let said = |args: &[&str]| {
        let out = docsys(&repo, args);
        assert!(out.status.success(), "{out:?}");
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    assert_eq!(
        said(&["journal", "add", "the entry point only calls run"]),
        "Docs: the entry point only calls run\n"
    );
    assert_eq!(
        said(&[
            "journal",
            "add",
            "the entry point only calls run",
            "--title",
            "Run on start"
        ]),
        "Run on start\n\nDocs: the entry point only calls run\n"
    );
    assert_eq!(
        said(&["journal", "add", "Run settled", "--link", "reference/run"]),
        "Run settled\n\nDocs: reference/run\n"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// What a command says names what it wrote: a skipped commit's debt lands in
/// its topic file and the gate names that file; a guess marker is answered
/// with the command that records the question (D-124).
#[test]
fn the_messages_name_the_files_and_commands_of_a_0_5_tree() {
    let repo = adopted("names", "require");
    fs::write(repo.join("main.rs"), "fn main() { run() }\n").unwrap();
    ok(&repo, &["add", "main.rs"]);
    let out = docsys(
        &repo,
        &["gate", "--repo", ".", "--root", "docs", "--skipped"],
    );
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(
        said.contains("records it in work/debt/general.md"),
        "{said}"
    );
    // the tree declares its own uncertainty marker (R-210)
    let meta = repo.join("docs/.docmeta.yml");
    let text = fs::read_to_string(&meta).unwrap();
    fs::write(&meta, format!("{text}uncertainty_markers: [\"(guess)\"]\n")).unwrap();
    fs::create_dir_all(repo.join("docs/reference")).unwrap();
    fs::write(
        repo.join("docs/reference/run.md"),
        "---\nid: run\ntype: reference\n---\nThis page states what run does; read it first.\n\nRun retries three times (guess).\n",
    )
    .unwrap();
    let out = docsys(&repo, &["lint"]);
    let said = String::from_utf8_lossy(&out.stdout);
    let line = said
        .lines()
        .find(|l| l.contains("R-210"))
        .unwrap_or_default();
    assert!(
        line.contains("docsys question add") && !line.contains("questions.md"),
        "{said}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// One commit points at each disputed rule once, though two of its gate's
/// calls report it — a page's citation and the code's (D-116); a commit
/// tried again, nothing restaged, points at it again.
#[test]
fn a_commit_points_at_each_rule_once() {
    let repo = adopted("pointers", "ask");
    fs::create_dir_all(repo.join("docs/reference")).unwrap();
    fs::write(
        repo.join("docs/reference/run.md"),
        "---\nid: run\ntype: reference\n---\nThis page states what run does; read it first.\n\nSee `doc: no-such-id`.\n",
    )
    .unwrap();
    fs::write(repo.join("main.rs"), "// doc: no-such-page\nfn main() {}\n").unwrap();
    ok(&repo, &["add", "-A"]);
    for attempt in 1..=3 {
        let out = git(&repo, &["commit", "-qm", "two dangling citations"]);
        assert!(!out.status.success(), "{out:?}");
        let said = String::from_utf8_lossy(&out.stdout).into_owned()
            + &String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            said.matches("Finding wrong? `docsys feedback --rule R-076`")
                .count(),
            1,
            "attempt {attempt}: {said}"
        );
    }
    let _ = fs::remove_dir_all(&repo);
}

/// `status` on a docsys/0.5 tree counts what the tree still has: no counter
/// of `updated:` lines nothing writes, and no verified page "whose body
/// moved" — a page carries no verification (D-122, D-130).
#[test]
fn status_on_a_0_5_tree_names_no_retired_counter() {
    let repo = adopted("status-counters", "ask");
    let out = docsys(&repo, &["status"]);
    assert!(out.status.success(), "{out:?}");
    let said = String::from_utf8_lossy(&out.stdout);
    let line = said
        .lines()
        .find(|l| l.starts_with("freshness:"))
        .unwrap_or_default();
    assert!(line.contains("stale pin(s)"), "{said}");
    assert!(
        !line.contains("updated behind history") && !line.contains("whose body moved"),
        "{said}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// The commit-msg half says what it stops — the message, not lint, which the
/// pre-commit half runs (D-125).
#[test]
fn the_message_hook_says_what_its_half_stops() {
    let repo = adopted("message-mode", "require");
    let pre = fs::read_to_string(repo.join(".git/hooks/pre-commit")).unwrap();
    let msg = fs::read_to_string(repo.join(".git/hooks/commit-msg")).unwrap();
    assert!(pre.contains("lint errors"), "{pre}");
    assert!(!msg.contains("lint errors"), "{msg}");
    assert!(msg.contains("message"), "{msg}");
    let _ = fs::remove_dir_all(&repo);
}

/// Inside a git hook only the findings decide: a reader that closed its end
/// of the commit's output never turns a warnings-only gate into a refusal.
#[test]
fn a_closed_reader_never_refuses_a_commit_the_gate_lets_through() {
    let repo = adopted("closed-reader", "ask");
    fs::create_dir_all(repo.join("docs/reference")).unwrap();
    fs::write(
        repo.join("docs/reference/x.md"),
        "---\nid: x\ntype: reference\nupdated: 2026-10-01\n---\n# X\n\nThis page states x.\n",
    )
    .unwrap();
    ok(&repo, &["add", "-A"]);
    ok(
        &repo,
        &["commit", "-qm", "x", "-m", "Docs: x", "--no-verify"],
    );
    let lint = docsys(&repo, &["lint"]);
    let said = String::from_utf8_lossy(&lint.stdout);
    assert!(
        said.contains("WARN") && said.contains("0 error(s)"),
        "{said}"
    );
    fs::write(repo.join("main.rs"), "fn main() { run() }\n").unwrap();
    ok(&repo, &["add", "main.rs"]);
    let head = ok(&repo, &["rev-parse", "HEAD"]);
    let (r1, w1) = std::io::pipe().unwrap();
    let (r2, w2) = std::io::pipe().unwrap();
    drop(r1);
    drop(r2);
    let path = format!(
        "{}:{}",
        bin().parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let status = Command::new("git")
        .args(["commit", "-q", "-m", "plain", "-m", "Docs: y"])
        .current_dir(&repo)
        .env("PATH", path)
        .env_remove("DOCSYS_DISPATCHED")
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .stdout(w1)
        .stderr(w2)
        .status()
        .unwrap();
    assert!(status.success(), "{status:?}");
    assert_ne!(ok(&repo, &["rev-parse", "HEAD"]), head);
    let _ = fs::remove_dir_all(&repo);
}
