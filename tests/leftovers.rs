#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! The leftover check (D-117): `docsys upgrade` on a tree already at this
//! version lists what an earlier version left, each with its file and its
//! fix, and a tree with none says so in one line.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const CASE: &str = "corpus/upgrades/0.4-to-0.5";

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-leftovers-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
}

fn path() -> String {
    format!(
        "{}:{}",
        bin().parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

fn git_at(dir: &Path, day: Option<&str>, args: &[&str]) -> String {
    let mut c = Command::new("git");
    c.args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("PATH", path())
        .current_dir(dir);
    if let Some(day) = day {
        let at = format!("{day}T12:00:00+00:00");
        c.env("GIT_AUTHOR_DATE", &at).env("GIT_COMMITTER_DATE", &at);
    }
    let out = c.output().unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn git(dir: &Path, args: &[&str]) -> String {
    git_at(dir, None, args)
}

/// A commit the git gate does not judge: the test sets the tree up.
fn commit_all(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(
        dir,
        &["-c", "core.hooksPath=/dev/null", "commit", "-qm", message],
    );
}

fn docsys(dir: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .env("PATH", path())
        .current_dir(dir)
        .output()
        .unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let rel = p
            .strip_prefix(base)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if p.is_dir() {
            walk(&p, base, out);
        } else {
            out.push(rel);
        }
    }
}

/// A stored top-level `dot-<name>` is the repository's `.<name>`.
fn copy_in(from: &Path, to: &Path, rev: Option<&str>) {
    let mut files = Vec::new();
    walk(from, from, &mut files);
    for rel in files {
        let real = match rel.strip_prefix("dot-") {
            Some(rest) => format!(".{rest}"),
            None => rel.clone(),
        };
        let target = to.join(real);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let text = fs::read_to_string(from.join(&rel)).unwrap();
        fs::write(
            &target,
            rev.map_or(text.clone(), |r| text.replace("@REV@", r)),
        )
        .unwrap();
    }
}

/// The conformance case's 0.4 repository (tests/upgrade.rs builds the same):
/// the tree as 0.15 left it, a second commit, the 0.15 gate in this clone.
fn build(name: &str) -> PathBuf {
    let case = Path::new(env!("CARGO_MANIFEST_DIR")).join(CASE);
    let repo = tmp(name);
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    copy_in(&case.join("before"), &repo, None);
    git(&repo, &["add", "-A"]);
    git_at(
        &repo,
        Some("2026-09-01"),
        &["commit", "-qm", "the tree as 0.15 left it"],
    );
    let rev = git(&repo, &["rev-parse", "--short=7", "HEAD"]);
    let mut files = Vec::new();
    walk(&repo, &repo, &mut files);
    for rel in files.into_iter().filter(|f| !f.starts_with(".git/")) {
        let p = repo.join(&rel);
        let text = fs::read_to_string(&p).unwrap();
        if text.contains("@REV@") {
            fs::write(&p, text.replace("@REV@", &rev)).unwrap();
        }
    }
    copy_in(&case.join("edits"), &repo, Some(&rev));
    git(&repo, &["add", "-A"]);
    git_at(
        &repo,
        Some("2026-09-02"),
        &["commit", "-qm", "expiry: revocation"],
    );
    let hook = repo.join(".git/hooks/pre-commit");
    fs::copy(case.join("hooks/pre-commit"), &hook).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    repo
}

/// The case moved to docsys/0.5 in its one commit.
fn moved(name: &str) -> PathBuf {
    let repo = build(name);
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(git(&repo, &["status", "--porcelain"]), "", "{out:?}");
    repo
}

/// The idle run's output: `docsys upgrade` on the moved tree.
fn idle(repo: &Path) -> String {
    let out = docsys(repo, &["upgrade"]);
    assert!(out.status.success(), "{out:?}");
    stdout(&out)
}

/// The row of `out` for one step and file, as the text form prints it.
fn row<'a>(out: &'a str, strategy: &str, step: &str, file: &str) -> Option<&'a str> {
    let head = format!("{strategy:<7} {step:<18} {file}  ");
    out.lines().find(|l| l.starts_with(&head))
}

fn write(repo: &Path, rel: &str, text: &str) {
    let p = repo.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

/// The owner-edited relay and command the case carries, back to this
/// version's text: a moved tree with nothing of the earlier version left.
fn without_owner_edits(repo: &Path) {
    let root = repo.join("docs");
    write(
        repo,
        ".claude/hooks/stop-docs-reminder.sh",
        &docsys::agents::relay_for("hooks/stop-docs-reminder.sh", "docs").unwrap(),
    );
    let (_, sync, _) = docsys::agents::owned_assets(false)
        .into_iter()
        .find(|(a, _, _)| *a == "commands/docsys-sync.md")
        .unwrap();
    write(
        repo,
        ".claude/commands/docsys-sync.md",
        &docsys::migrate::with_preamble(
            &docsys::agents::render_root(sync, "docs"),
            &docsys::migrate::generated_preamble(&root),
        ),
    );
    commit_all(repo, "the owner takes this version's relay and command");
}

/// A tree with no leftover says so in one line, and offers nothing to apply;
/// a pin that moved is lint's (R-111), never the leftover check's.
#[test]
fn a_moved_tree_with_no_leftover_says_so_in_one_line() {
    let repo = moved("clean");
    without_owner_edits(&repo);
    let lint = stdout(&docsys(&repo, &["lint"]));
    assert!(
        lint.contains("ERROR R-111 reference/expiry.md [src/auth.rs#expire]"),
        "{lint}"
    );
    let one = "docsys upgrade: docsys/0.5 — no leftover of an earlier version\n";
    assert_eq!(idle(&repo), one);
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(stdout(&out), one);
    let _ = fs::remove_dir_all(&repo);
}

/// The owner's workflow the field report met: its own push branch, its own
/// runners, a release install of a version written in an `env:` block.
fn owned_workflow(version: &str) -> String {
    format!(
        "name: docs\non:\n  pull_request:\n  push:\n    branches: [develop]\njobs:\n  docs:\n    runs-on: [self-hosted, linux, docs-runner]\n    env:\n      DOCSYS_VERSION: v{version}\n      DOCSYS_SHA256_LINUX_X64: {}\n      DOCSYS_SHA256_LINUX_ARM64: {}\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          fetch-depth: 0\n      - name: install the docsys release archive\n        run: |\n          curl -fsSL -o docsys.tar.gz \"https://releases.example.invalid/docsys/${{DOCSYS_VERSION}}/docsys-${{DOCSYS_VERSION}}-linux-x64.tar.gz\"\n          echo \"${{DOCSYS_SHA256_LINUX_X64}}  docsys.tar.gz\" | sha256sum -c -\n          tar -xzf docsys.tar.gz\n      - run: ./docsys lint --repo . --root docs\n",
        "1".repeat(64),
        "2".repeat(64)
    )
}

/// What an owner's release-install workflow must change with the tree, and
/// nothing else: the version it installs, on its line, and the sha256 values
/// the person copies from the release page — never a template default, never
/// a value docsys made up.
fn assert_only_what_moves(out: &str) {
    let own = env!("CARGO_PKG_VERSION");
    let line = row(
        out,
        "manual",
        "ci-workflow",
        ".github/workflows/docsys.yml:10",
    )
    .unwrap_or_else(|| panic!("{out}"));
    assert!(
        line.contains("docsys 0.15.1") && line.contains(own),
        "{line}"
    );
    assert!(
        line.contains("sha256") && line.contains("release page"),
        "{line}"
    );
    for default in [
        "\n# .github/workflows/docsys.yml\n",
        "ubuntu-latest",
        "branches: [main]",
        "install=cargo",
        &"1".repeat(64),
    ] {
        assert!(!out.contains(default), "`{default}` in {out}");
    }
}

/// An owner's workflow, in the move's plan: only what moves with the tree.
#[test]
fn an_owners_workflow_hears_only_what_moves_with_the_tree() {
    let repo = build("owned-plan");
    let wf = ".github/workflows/docsys.yml";
    write(&repo, wf, &owned_workflow("0.15.1"));
    commit_all(&repo, "ci: our runners, our release install");
    assert_only_what_moves(&stdout(&docsys(&repo, &["upgrade"])));
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        fs::read_to_string(repo.join(wf)).unwrap(),
        owned_workflow("0.15.1")
    );
    let _ = fs::remove_dir_all(&repo);
}

/// Leftover 1: a workflow that installs a version other than the tree's pin,
/// in every shape the classification knows — untouched (its install reading
/// the pin, or a release install holding another version's sha256 values),
/// a rendering from before the stamp, and the owner's own.
#[test]
fn a_workflow_that_installs_another_version_is_a_leftover() {
    use docsys::workflow::{render, Ci, Install, Verify, Workflow};
    let own = env!("CARGO_PKG_VERSION");
    let repo = moved("ci");
    without_owner_edits(&repo);
    let wf = ".github/workflows/docsys.yml";
    let stamped = |install: Install| Workflow {
        version: "0.15.9".to_string(),
        branch: "main".to_string(),
        root: "docs".to_string(),
        ci: Ci {
            runner: vec!["ubuntu-latest".to_string()],
            install,
            verify: Verify::Description,
        },
    };
    // untouched, its install reading the pin: regenerated
    write(&repo, wf, &render(&stamped(Install::Cargo)));
    commit_all(&repo, "a workflow an earlier build stamped");
    let out = idle(&repo);
    assert!(
        row(&out, "auto", "ci-workflow", wf).is_some_and(|l| l.contains("regenerated from 0.15.9")),
        "{out}"
    );
    // untouched, a release install: another version's sha256 values
    let sum = "3".repeat(64);
    write(
        &repo,
        wf,
        &render(&stamped(Install::Release(vec![(
            "x86_64-unknown-linux-musl".to_string(),
            sum.clone(),
        )]))),
    );
    commit_all(&repo, "a release install an earlier build stamped");
    let out = idle(&repo);
    let line = row(&out, "manual", "ci-workflow", wf).unwrap_or_else(|| panic!("{out}"));
    assert!(
        line.contains("0.15.9") && line.contains(own) && line.contains("release page"),
        "{line}"
    );
    assert!(!out.contains(&sum), "never a value made up: {out}");
    assert!(!out.contains("\n# .github/workflows/docsys.yml\n"), "{out}");
    // a rendering from before the stamp: regenerated
    let legacy = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations/workflow-0.15.yml"),
    )
    .unwrap()
    .replace("@ROOT@", "docs");
    write(&repo, wf, &legacy);
    commit_all(&repo, "the 0.15 workflow came back");
    let out = idle(&repo);
    assert!(
        row(&out, "auto", "ci-workflow", wf).is_some_and(|l| l.contains("the 0.15 workflow")),
        "{out}"
    );
    // the owner's: the version on its line, the sha256 values to copy
    write(&repo, wf, &owned_workflow("0.15.1"));
    commit_all(&repo, "ci: our runners");
    assert_only_what_moves(&idle(&repo));
    // the owner's, installing the version the tree pins: nothing left
    write(&repo, wf, &owned_workflow(own));
    commit_all(&repo, "ci: our runners, the pinned version");
    assert_eq!(
        idle(&repo),
        "docsys upgrade: docsys/0.5 — no leftover of an earlier version\n"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// Leftover 2: the type-directory route lines an earlier 0.16 build wrote.
#[test]
fn the_route_lines_an_earlier_build_wrote_are_a_leftover() {
    let repo = moved("routes");
    let index = repo.join("docs/index.md");
    let mut text = fs::read_to_string(&index).unwrap();
    for line in docsys::migrate::DIRECTORY_ROUTES {
        text.push_str(line);
        text.push('\n');
    }
    fs::write(&index, &text).unwrap();
    commit_all(&repo, "an earlier build's routes");
    let out = idle(&repo);
    assert!(
        row(&out, "auto", "routes", "docs/index.md")
            .is_some_and(|l| l.contains("4 type-directory route line(s)")),
        "{out}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// Leftover 3: the post-edit relay and its wire, the reason said once.
#[test]
fn the_post_edit_relay_and_its_wire_are_a_leftover() {
    let repo = moved("post-edit");
    let before = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CASE)
        .join("before/dot-claude");
    for rel in ["hooks/post-edit-updated.sh", "settings.json"] {
        write(
            &repo,
            &format!(".claude/{rel}"),
            &fs::read_to_string(before.join(rel)).unwrap(),
        );
    }
    commit_all(&repo, "a branch brought the relay back");
    let out = idle(&repo);
    assert!(
        row(&out, "auto", "hook-wires", ".claude/settings.json")
            .is_some_and(|l| l.contains("post-edit wire(s) taken out")),
        "{out}"
    );
    assert!(
        row(
            &out,
            "auto",
            "hook-scripts",
            ".claude/hooks/post-edit-updated.sh"
        )
        .is_some(),
        "{out}"
    );
    assert_eq!(
        out.matches("the relay has nothing left to do").count(),
        1,
        "one reason, said once: {out}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// Leftover 4: `updated:` lines, and every verification field a page still
/// holds. One that still holds leaves the page with its approval carried
/// into the commit, so the page reads verified from history.
#[test]
fn date_lines_and_record_fields_in_pages_are_leftovers() {
    let repo = moved("fields");
    let moved_at = git(&repo, &["rev-parse", "--short=7", "HEAD"]);
    let refresh = repo.join("docs/reference/refresh.md");
    let text = fs::read_to_string(&refresh).unwrap();
    fs::write(
        &refresh,
        text.replacen(
            "type: reference\n",
            &format!("type: reference\nupdated: 2026-09-03\nverification: verified\nverified_by: maintainer\nverified_rev: {moved_at}\n"),
            1,
        ),
    )
    .unwrap();
    let expiry = repo.join("docs/reference/expiry.md");
    let text = fs::read_to_string(&expiry).unwrap();
    fs::write(
        &expiry,
        text.replacen(
            "type: reference\n",
            "type: reference\nverification: unverified\n",
            1,
        ),
    )
    .unwrap();
    commit_all(&repo, "a branch from before the move");
    let out = idle(&repo);
    assert!(
        row(&out, "auto", "dates", "docs").is_some_and(|l| l.contains("from 1 page(s)")),
        "{out}"
    );
    assert!(
        row(&out, "auto", "records", "docs/reference/expiry.md").is_some(),
        "{out}"
    );
    assert!(
        row(&out, "auto", "records", "docs/reference/refresh.md")
            .is_some_and(|l| l.contains("Approved-by:")),
        "a record that holds is a leftover too: {out}"
    );
    let done = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(done.status.success(), "{done:?}");
    for page in [&refresh, &expiry] {
        let text = fs::read_to_string(page).unwrap();
        for field in ["updated:", "verification:", "verified_by:", "verified_rev:"] {
            assert!(!text.contains(&format!("\n{field}")), "{text}");
        }
    }
    let shown = stdout(&docsys(&repo, &["verify", "--show", "reference/refresh"]));
    assert!(
        shown.starts_with("reference/refresh.md (verified)"),
        "{shown}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// Leftover 5: the journal and the ledgers a branch from before the move
/// brought back.
#[test]
fn a_journal_and_ledgers_a_branch_brought_back_are_leftovers() {
    let repo = moved("ledgers");
    write(
        &repo,
        "docs/work/journal.md",
        "# Journal\n\n## 2026-09-04 - a late entry\n- written on an old branch\n",
    );
    write(
        &repo,
        "docs/work/debt.md",
        "# Debt\n\n- [ ] 2026-09-04 a late debt -- deferred: a -- repay when: b\n",
    );
    write(
        &repo,
        "docs/work/questions.md",
        "# Questions\n\n- [ ] 2026-09-04 a late question\n",
    );
    commit_all(&repo, "a branch from before the move");
    let out = idle(&repo);
    assert!(
        row(&out, "auto", "journal", "docs/work/journal.md").is_some(),
        "{out}"
    );
    assert!(
        row(&out, "auto", "ledgers", "docs/work/debt.md").is_some(),
        "{out}"
    );
    assert!(
        row(&out, "auto", "ledgers", "docs/work/questions.md").is_some(),
        "{out}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// Leftover 6: an agent layer below the repository's top that holds
/// docsys's own assets — the layer lives at the top (D-098). A knowledge
/// base's own layer at its root, and a `.claude/` holding only a person's
/// files, are not stray.
#[test]
fn a_stray_agent_layer_below_the_top_is_a_leftover() {
    let repo = moved("stray");
    let sync = fs::read_to_string(repo.join(".claude/commands/docsys-sync.md")).unwrap();
    let relay = fs::read_to_string(repo.join(".claude/hooks/pre-commit-docs.sh")).unwrap();
    write(&repo, "pkg/api/.claude/commands/docsys-sync.md", &sync);
    write(&repo, "pkg/api/.claude/hooks/pre-commit-docs.sh", &relay);
    write(&repo, "tools/.claude/settings.local.json", "{}\n");
    write(
        &repo,
        "kb/.docmeta.yml",
        "spec: docsys/0.5\nprofile: knowledge-base\n",
    );
    write(
        &repo,
        "kb/.claude/skills/kb-capture/SKILL.md",
        "---\nname: kb-capture\n---\n",
    );
    commit_all(&repo, "layers below the top");
    let out = idle(&repo);
    let line =
        row(&out, "manual", "agent-layer", "pkg/api/.claude/").unwrap_or_else(|| panic!("{out}"));
    assert!(line.contains("D-098"), "{line}");
    assert!(!out.contains("tools/.claude"), "{out}");
    assert!(!out.contains("kb/.claude"), "{out}");
    let _ = fs::remove_dir_all(&repo);
}

/// Leftover 7: agent-facing text the tree's people wrote that names a
/// retired concept.
#[test]
fn text_naming_a_retired_concept_is_a_leftover() {
    let repo = moved("retired");
    write(
        &repo,
        "CLAUDE.md",
        "# Team\n\nAfter each task, add a journal line.\n",
    );
    commit_all(&repo, "the team's own text");
    let out = idle(&repo);
    assert!(
        row(&out, "manual", "retired-concepts", "CLAUDE.md:3")
            .is_some_and(|l| l.contains("`journal line`")),
        "{out}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// Leftover 8: the record a bare `--apply` leaves in the git directory, once
/// its move was committed by hand or discarded: named, and taken out.
#[test]
fn the_upgrades_own_record_left_behind_is_named_and_taken_out() {
    let record = |repo: &Path| {
        ["message", "files", "head"]
            .iter()
            .filter(|n| repo.join(format!(".git/docsys-upgrade-{n}")).exists())
            .count()
    };
    let named = |out: &str| row(out, "auto", "upgrade-record", ".git/docsys-upgrade-*").is_some();
    // committed by hand, as the bare `--apply` says
    let repo = build("record-committed");
    assert!(docsys(&repo, &["upgrade", "--apply"]).status.success());
    git(&repo, &["add", "-A"]);
    git(
        &repo,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-q",
            "-F",
            ".git/docsys-upgrade-message",
        ],
    );
    assert_eq!(record(&repo), 3);
    let out = idle(&repo);
    assert!(named(&out), "{out}");
    assert_eq!(record(&repo), 0);
    assert!(!named(&idle(&repo)), "said once");
    let _ = fs::remove_dir_all(&repo);
    // discarded: the working tree back to HEAD
    let repo = build("record-discarded");
    assert!(docsys(&repo, &["upgrade", "--apply"]).status.success());
    git(&repo, &["checkout", "-q", "--", "."]);
    git(&repo, &["clean", "-fdq"]);
    assert_eq!(record(&repo), 3);
    let out = idle(&repo);
    assert!(named(&out), "{out}");
    assert_eq!(record(&repo), 0);
    let _ = fs::remove_dir_all(&repo);
}

/// `/docsys-upgrade` ends with the audit: the leftover check, then lint, each
/// item walked through with the person; what only the person decides is
/// named once, in one place.
#[test]
fn the_upgrade_command_ends_with_the_audit_and_names_the_persons_part_once() {
    let (_, text, _) = docsys::agents::owned_assets(false)
        .into_iter()
        .find(|(a, _, _)| *a == "commands/docsys-upgrade.md")
        .unwrap();
    let last = text
        .rsplit("\n\n")
        .find(|p| p.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .and_then(|p| {
            p.lines()
                .rfind(|l| l.chars().next().is_some_and(|c| c.is_ascii_digit()))
        })
        .unwrap_or_else(|| panic!("{text}"));
    let last = &text[text.rfind(last).unwrap()..];
    // the move printed the leftover check last: the step walks through it and
    // runs no second one
    let check = last
        .find("leftover")
        .unwrap_or_else(|| panic!("the last step walks the leftover list: {last}"));
    let lint = last
        .find("`docsys lint`")
        .unwrap_or_else(|| panic!("then lint: {last}"));
    assert!(check < lint, "{last}");
    assert!(!last.contains("run `docsys upgrade`"), "{last}");
    let consent = ["their word", "say yes", "person's word", "said yes"]
        .iter()
        .map(|p| text.matches(p).count())
        .sum::<usize>();
    assert_eq!(consent, 1, "the person's part, named once: {text}");
}

/// The move ends with the leftover check, whichever door it is taken by: the
/// last lines `upgrade --apply` prints, with `--commit` or without, are what
/// an earlier version left on the moved tree, or the one line that says
/// nothing is (D-117).
#[test]
fn the_move_ends_with_the_leftover_check() {
    for commit in [true, false] {
        let repo = build(if commit { "tail-commit" } else { "tail-apply" });
        let args: &[&str] = if commit {
            &["upgrade", "--apply", "--commit"]
        } else {
            &["upgrade", "--apply"]
        };
        let out = docsys(&repo, args);
        assert!(out.status.success(), "{out:?}");
        let text = stdout(&out);
        let at = text
            .rfind("docsys upgrade: docsys/0.5 — ")
            .unwrap_or_else(|| panic!("commit: {commit}\n{text}"));
        let tail = text.get(at..).unwrap_or_default();
        assert!(
            tail.contains(" for information\n")
                || tail == "docsys upgrade: docsys/0.5 — no leftover of an earlier version\n",
            "commit: {commit}\n{tail}"
        );
        if commit {
            assert_eq!(tail, idle(&repo), "the check the idle run makes");
        }
        let _ = fs::remove_dir_all(&repo);
    }
}
