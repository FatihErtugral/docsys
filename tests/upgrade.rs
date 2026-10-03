#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! `docsys upgrade` (D-117). The conformance case R-179 asks for: a tree as
//! docsys 0.15 left it, moved to docsys/0.5, compared file by file against the
//! expected tree; a second run changes nothing. Beside it, what the move
//! refuses and what it only shows.
//!
//! `DOCSYS_BLESS=1 cargo test --test upgrade` writes the expected files from
//! the current build — for a maintainer to read before they are committed.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const CASE: &str = "corpus/upgrades/0.4-to-0.5";

/// The knowledge-base contract as docsys 0.15 wrote it.
const KB_CONTRACT_0_15: &str = include_str!("golden/kb-contract-0.15.md");

/// The release's upgrade note, read from the CHANGELOG as a person reads it:
/// the lines under `### Upgrading` in the release's section.
fn changelog_note(release: &str) -> String {
    let text =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("CHANGELOG.md")).unwrap();
    let section = text
        .split(&format!("## [{release}]"))
        .nth(1)
        .unwrap()
        .split("\n## ")
        .next()
        .unwrap();
    let note = section
        .split("### Upgrading")
        .nth(1)
        .unwrap()
        .split_once('\n')
        .unwrap()
        .1;
    note.split("\n### ").next().unwrap().trim().to_string()
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-upgrade-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
}

/// PATH with this build first: the git gate a commit runs is this docsys.
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

fn docsys(dir: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .env("PATH", path())
        .current_dir(dir)
        .output()
        .unwrap()
}

/// A stored top-level `dot-claude` is the repository's `.claude`: the case
/// keeps its agent layer under another name, so an agent working in this
/// repository never loads the fixture's skills or hooks.
fn real_name(rel: &str) -> String {
    match rel.strip_prefix("dot-") {
        Some(rest) => format!(".{rest}"),
        None => rel.to_string(),
    }
}

fn stored_name(rel: &str) -> String {
    match rel.strip_prefix('.') {
        Some(rest) if !rel.starts_with(".git/") => format!("dot-{rest}"),
        _ => rel.to_string(),
    }
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
        if rel == ".git" {
            continue;
        }
        if p.is_dir() {
            walk(&p, base, out);
        } else {
            out.push(rel);
        }
    }
}

/// Every file under `dir` by its repository path, `rev` written back as the
/// placeholder.
fn snapshot(dir: &Path, rev: &str, stored: bool) -> BTreeMap<String, String> {
    let mut files = Vec::new();
    walk(dir, dir, &mut files);
    files
        .into_iter()
        .map(|rel| {
            let text = fs::read_to_string(dir.join(&rel)).unwrap();
            let key = if stored { real_name(&rel) } else { rel };
            (key, text.replace(rev, "@REV@"))
        })
        .collect()
}

fn copy_in(from: &Path, to: &Path, rev: Option<&str>) {
    let mut files = Vec::new();
    walk(from, from, &mut files);
    for rel in files {
        let target = to.join(real_name(&rel));
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let text = fs::read_to_string(from.join(&rel)).unwrap();
        fs::write(
            &target,
            rev.map_or(text.clone(), |r| text.replace("@REV@", r)),
        )
        .unwrap();
    }
}

/// The case's history: the tree committed as 0.15 left it, its pages verified
/// at that revision; then a second commit moves one page's body and the code
/// under one pin; the 0.15 gate is this clone's pre-commit hook.
fn build(name: &str) -> (PathBuf, String) {
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
    for rel in files {
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
    (repo, rev)
}

fn bless() -> bool {
    std::env::var_os("DOCSYS_BLESS").is_some()
}

fn expect_text(rel: &str, got: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(CASE).join(rel);
    if bless() {
        fs::write(&path, got).unwrap();
        return;
    }
    let want = fs::read_to_string(&path).unwrap_or_default();
    assert!(
        want == got,
        "{rel} differs\n{}",
        docsys::diff::unified(&want, got, rel, "this build", 3)
    );
}

fn findings(root: &Path, repo: &Path) -> String {
    let (r, outcome) = docsys::lint_in(root, Some(repo));
    let mut lines: Vec<String> = r
        .findings
        .iter()
        .map(|f| {
            format!(
                "{}\t{}\t{}\t{}",
                f.severity.tag(),
                f.rule,
                f.file,
                f.subject
            )
        })
        .collect();
    lines.sort();
    lines.dedup();
    let exit = match outcome {
        docsys::Outcome::Clean => 0,
        docsys::Outcome::Errors => 1,
        docsys::Outcome::Config => 2,
    };
    format!("EXIT\t{exit}\n{}", lines.join("\n") + "\n")
}

/// R-179: the 0.4 tree, moved, is the expected 0.5 tree — file for file.
#[test]
fn a_0_4_tree_moves_to_the_expected_0_5_tree_and_a_second_run_changes_nothing() {
    let (repo, rev) = build("conformance");
    let before = snapshot(&repo, &rev, false);

    // the plan writes nothing
    let out = docsys(&repo, &["upgrade"]);
    assert!(out.status.success(), "{out:?}");
    expect_text(
        "plan.txt",
        &String::from_utf8_lossy(&out.stdout).replace(&rev, "@REV@"),
    );
    assert_eq!(snapshot(&repo, &rev, false), before, "the plan wrote");
    // the same plan as data, for an agent that completes it (D-104)
    let out = docsys(&repo, &["upgrade", "--json"]);
    assert!(out.status.success(), "{out:?}");
    let json = String::from_utf8_lossy(&out.stdout).replace(&rev, "@REV@");
    assert!(docsys::hook::parse_json(json.trim()).is_some(), "{json}");
    expect_text("plan.json", &json);

    // the move is one commit, and leaves nothing behind
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains(&format!(
            "committed: docsys: upgrade the tree to docsys {}",
            env!("CARGO_PKG_VERSION")
        )),
        "{out:?}"
    );
    // what a person pulling it reads: the release's note byte for byte, and
    // the step every clone takes (D-120)
    let body = git(&repo, &["log", "-1", "--format=%B"]);
    let note = changelog_note(env!("CARGO_PKG_VERSION"));
    assert!(note.contains("before pulling this change"), "{note}");
    assert!(
        body.contains(&format!(
            "Upgrading to docsys {}:\n{note}",
            env!("CARGO_PKG_VERSION")
        )),
        "{body}"
    );
    assert!(body.ends_with(docsys::upgrade::TEAMMATES), "{body}");
    assert_eq!(
        fs::read_to_string(repo.join("docs/.docsys-version")).unwrap(),
        format!("{}\n", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(
        git(&repo, &["rev-list", "--count", &format!("{head}..HEAD")]),
        "1"
    );
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");

    let after_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CASE)
        .join("after");
    let got = snapshot(&repo, &rev, false);
    if bless() {
        let _ = fs::remove_dir_all(&after_dir);
        for (rel, text) in &got {
            let p = after_dir.join(stored_name(rel));
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        }
    } else {
        let want = snapshot(&after_dir, "@REV@", true);
        let names = |m: &BTreeMap<String, String>| m.keys().cloned().collect::<Vec<_>>();
        assert_eq!(names(&got), names(&want), "the files differ");
        for (rel, text) in &want {
            let g = got.get(rel).unwrap();
            assert!(
                g == text,
                "{rel} differs\n{}",
                docsys::diff::unified(text, g, rel, "this build", 3)
            );
        }
    }
    expect_text(
        "hooks/pre-commit.after",
        &fs::read_to_string(repo.join(".git/hooks/pre-commit")).unwrap(),
    );
    expect_text("expected.tsv", &findings(&repo.join("docs"), &repo));

    // the block record the move wrote is the one `verify` reads back whole
    let shown = docsys(&repo, &["verify", "--show", "reference/refresh"]);
    assert!(
        String::from_utf8_lossy(&shown.stdout).contains(
            "3/3 blocks as verified by maintainer at @REV@ — nothing to re-read"
                .replace("@REV@", &rev)
                .as_str()
        ),
        "{shown:?}"
    );

    // a second run: nothing automatic, nothing written, no commit
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.lines().any(|l| l.starts_with("auto ")), "{stdout}");
    assert!(!stdout.contains("committed:"), "{stdout}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(snapshot(&repo, &rev, false), got, "the second run wrote");
    let _ = fs::remove_dir_all(&repo);
}

/// R-097: the move is a commit of its own, so it does not start on top of
/// someone's unfinished work.
#[test]
fn an_upgrade_refuses_a_dirty_working_tree() {
    let (repo, rev) = build("dirty");
    let p = repo.join("docs/index.md");
    fs::write(
        &p,
        fs::read_to_string(&p).unwrap() + "\nA line in progress.\n",
    )
    .unwrap();
    let before = snapshot(&repo, &rev, false);
    let out = docsys(&repo, &["upgrade", "--apply"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("uncommitted changes"),
        "{out:?}"
    );
    assert_eq!(snapshot(&repo, &rev, false), before);
    let _ = fs::remove_dir_all(&repo);
}

/// A workflow its owner edited — a self-hosted runner, their own install —
/// is never rewritten: the upgrade shows the diff and leaves the file alone.
#[test]
fn an_owners_workflow_is_shown_as_a_diff_and_never_rewritten() {
    let (repo, rev) = build("owned-ci");
    let wf = repo.join(".github/workflows/docsys.yml");
    let owned = fs::read_to_string(&wf)
        .unwrap()
        .replace("runs-on: ubuntu-latest", "runs-on: [self-hosted, linux]")
        .replace(
            "      - run: cargo install docsys\n",
            "      - run: ./tools/install-docsys.sh v0.15.1 # the docsys the tree was authored with\n",
        );
    fs::write(&wf, &owned).unwrap();
    git(&repo, &["commit", "-qam", "ci: our runner"]);
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // the one line that must move with the tree is named
    assert!(
        stdout.contains(&format!(
            "it installs docsys 0.15.1 and the tree will pin {}",
            env!("CARGO_PKG_VERSION")
        )),
        "{stdout}"
    );
    assert!(
        stdout.contains("manual  ci-workflow        .github/workflows/docsys.yml"),
        "{stdout}"
    );
    assert!(
        stdout.contains("\n# .github/workflows/docsys.yml\n"),
        "{stdout}"
    );
    assert_eq!(fs::read_to_string(&wf).unwrap(), owned);
    assert!(!git(&repo, &["show", "--stat", "HEAD"]).contains("docsys.yml"));
    let _ = rev;
    let _ = fs::remove_dir_all(&repo);
}

/// R-171: a tree that has not moved hears it in one line, naming the command;
/// a tree that has moved, and the upgrade itself, hear nothing.
#[test]
fn a_0_4_tree_is_told_once_how_it_moves() {
    let (repo, _) = build("notice");
    let notice = |args: &[&str]| -> Vec<String> {
        let out = docsys(&repo, args);
        String::from_utf8_lossy(&out.stderr)
            .lines()
            .filter(|l| l.starts_with("docsys: this tree declares"))
            .map(str::to_string)
            .collect()
    };
    assert_eq!(
        notice(&["lint"]),
        vec![format!(
            "docsys: this tree declares docsys/0.4 and is served by its rules; `docsys upgrade` — or `/docsys-upgrade` with an agent — moves it to docsys/{} when the repository is ready",
            docsys::rules::spec_version()
        )]
    );
    assert_eq!(notice(&["upgrade"]), Vec::<String>::new());
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(notice(&["lint"]), Vec::<String>::new());
    let _ = fs::remove_dir_all(&repo);
}

/// A teammate's clone after the upgrade commit: the tracked files moved, the
/// gate under .git/hooks did not. `doctor` names the one command, and that
/// command rewrites the block and nothing else.
#[test]
fn a_clone_with_the_old_gate_is_told_to_run_the_upgrade_once() {
    let (repo, rev) = build("teammate");
    let doctor = |repo: &Path| -> Vec<String> {
        String::from_utf8_lossy(&docsys(repo, &["doctor"]).stdout)
            .lines()
            .filter(|l| l.contains("the docsys block is behind the binary"))
            .map(str::to_string)
            .collect()
    };
    // on the 0.4 tree the upgrade would move it: adopt is the command
    let behind = doctor(&repo);
    assert_eq!(behind.len(), 1, "{behind:?}");
    assert!(
        behind
            .first()
            .is_some_and(|l| l.ends_with("`docsys adopt` rewrites it")),
        "{behind:?}"
    );

    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(doctor(&repo), Vec::<String>::new());

    // the clone that pulled it still has the 0.15 block
    let case = Path::new(env!("CARGO_MANIFEST_DIR")).join(CASE);
    fs::copy(
        case.join("hooks/pre-commit"),
        repo.join(".git/hooks/pre-commit"),
    )
    .unwrap();
    let behind = doctor(&repo);
    assert_eq!(behind.len(), 1, "{behind:?}");
    assert!(
        behind
            .first()
            .is_some_and(|l| l.ends_with("`docsys upgrade --apply` rewrites it")),
        "{behind:?}"
    );
    let tracked = snapshot(&repo, &rev, false);
    let out = docsys(&repo, &["upgrade", "--apply"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(doctor(&repo), Vec::<String>::new());
    assert_eq!(
        fs::read_to_string(repo.join(".git/hooks/pre-commit")).unwrap(),
        fs::read_to_string(case.join("hooks/pre-commit.after")).unwrap()
    );
    assert_eq!(snapshot(&repo, &rev, false), tracked, "only the gate moves");
    let _ = fs::remove_dir_all(&repo);
}

/// One commit, each line once: the version notice and every finding, whether
/// the clone's gate is this version's block or still the one 0.15 wrote —
/// an old block cannot be asked to cooperate (R-171).
#[test]
fn a_commit_says_each_thing_once_under_either_gate() {
    let commit = |repo: &Path| -> String {
        let out = Command::new("git")
            .args(["-c", "commit.gpgsign=false", "commit", "-qm", "a change"])
            .env("PATH", path())
            .current_dir(repo)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
    };
    let count = |said: &str, start: &str| said.lines().filter(|l| l.starts_with(start)).count();
    for rewritten in [false, true] {
        let (repo, _) = build(&format!("once-{rewritten}"));
        if rewritten {
            // `adopt` brings the clone's gate to this version, the tree stays 0.4
            let out = docsys(&repo, &["adopt"]);
            assert!(out.status.success(), "{out:?}");
        }
        fs::write(repo.join("notes.txt"), "a change\n").unwrap();
        git(&repo, &["add", "-A"]);
        let said = commit(&repo);
        assert_eq!(
            count(&said, "docsys: this tree declares docsys/0.4"),
            1,
            "{said}"
        );
        if rewritten {
            // the stale pin of the case, once
            assert_eq!(count(&said, "ERROR R-111 reference/expiry.md"), 1, "{said}");
        }
        let _ = fs::remove_dir_all(&repo);
    }
}

/// A pinned tree whose version is not installed, with no cargo to install it:
/// the one line naming the command comes once per commit, and a skipped
/// commit says that its record was not written instead of losing it in
/// silence (R-151).
#[test]
fn an_uninstalled_pin_is_named_once_and_a_lost_skip_record_is_said() {
    let (repo, _) = build("pin-once");
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    fs::write(repo.join("docs/.docsys-version"), "0.16.9\n").unwrap();
    git(
        &repo,
        &["-c", "core.hooksPath=/dev/null", "commit", "-qam", "pin"],
    );
    // git and a shell, this build as `docsys`, and no cargo
    let tools = tmp("pin-once-tools");
    for tool in ["git", "sh", "bash", "head", "sed", "cat"] {
        let p = Command::new("sh")
            .args(["-c", &format!("command -v {tool}")])
            .output()
            .unwrap();
        let p = String::from_utf8_lossy(&p.stdout).trim().to_string();
        std::os::unix::fs::symlink(p, tools.join(tool)).unwrap();
    }
    let path = format!("{}:{}", bin().parent().unwrap().display(), tools.display());
    let home = tmp("pin-once-home");
    fs::write(repo.join("notes.txt"), "a change\n").unwrap();
    git(&repo, &["add", "-A"]);
    let run = |skip: bool| -> String {
        let mut c = Command::new("git");
        c.args(["-c", "commit.gpgsign=false", "commit", "-qm", "a change"])
            .env("PATH", &path)
            .env("DOCSYS_HOME", &home)
            .current_dir(&repo);
        if skip {
            c.env("DOCSYS_SKIP", "1");
        }
        let out = c.output().unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
    };
    let said = run(false);
    let line = "docsys: this tree pins docsys 0.16.9; install it: ";
    assert_eq!(said.matches(line).count(), 1, "{said}");
    let said = run(true);
    assert_eq!(said.matches(line).count(), 1, "{said}");
    assert!(
        said.contains("docsys: this skipped commit is not recorded"),
        "{said}"
    );
    for d in [repo, tools, home] {
        let _ = fs::remove_dir_all(d);
    }
}

/// The separators move with the tree; afterwards they are `ledger fix`'s, the
/// one command R-108's message names.
#[test]
fn after_the_move_the_separators_are_ledger_fixs() {
    let (repo, _) = build("separators");
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    // an item written by hand with dashes, after the move (D-124)
    let debt = repo.join("docs/work/debt/a-second-item.md");
    let dashed = "- [ ] 2026-09-03 A second item — deferred: later — repay when: soon\n";
    fs::write(&debt, dashed).unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "a dashed item"]);
    let out = docsys(&repo, &["upgrade", "--apply"]);
    assert!(out.status.success(), "{out:?}");
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains("ledger-separators"),
        "{out:?}"
    );
    assert_eq!(fs::read_to_string(&debt).unwrap(), dashed);
    let out = docsys(&repo, &["ledger", "fix"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        fs::read_to_string(&debt).unwrap(),
        "- [ ] 2026-09-03 A second item -- deferred: later -- repay when: soon\n"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// R-177 across a chain: a tree two specs behind moves one spec per commit,
/// each with its note, and the pin moves with the last. The second move is
/// synthetic, so the chain is tested before a second spec exists.
#[test]
fn a_tree_two_specs_behind_moves_one_commit_per_spec() {
    use docsys::upgrade::{Ctx, Item, Migration, Upgrade};
    fn to_0_6(ctx: &Ctx, u: &mut Upgrade, apply: bool) -> Result<(), String> {
        let file = format!("{}work/next-spec.md", ctx.prefix);
        u.items.push(Item {
            strategy: "auto",
            step: "next-spec",
            file: file.clone(),
            what: "the next spec's page".to_string(),
            command: None,
        });
        if apply {
            fs::write(ctx.root.join("work/next-spec.md"), "# Next\n").map_err(|e| e.to_string())?;
            u.written.push(file);
        }
        Ok(())
    }
    let real = docsys::upgrade::MIGRATIONS.first().unwrap();
    let chain = [
        Migration {
            from: real.from,
            to: real.to,
            release: real.release,
            steps: real.steps,
            retired: real.retired,
            apply: real.apply,
        },
        Migration {
            from: 5,
            to: 6,
            release: "9.9.9",
            steps: "spec-line\tauto\ttracked\t-\n",
            retired: "",
            apply: to_0_6,
        },
    ];
    let (repo, _) = build("chain");
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let (root, claude) = (repo.join("docs"), repo.join(".claude"));
    let mut messages = Vec::new();
    loop {
        let u = docsys::upgrade::run_with(&chain, 6, &repo, &root, &claude, true).unwrap();
        docsys::upgrade::commit(&repo, &u).unwrap();
        messages.push(docsys::upgrade::message(&u));
        if u.last {
            break;
        }
    }
    let [first, last] = messages.as_slice() else {
        panic!("{messages:?}");
    };
    assert!(
        first
            .starts_with("docsys: upgrade the tree to docsys/0.5\n\nUpgrading to docsys 0.16.0:\n"),
        "{first}"
    );
    assert!(!first.contains(docsys::upgrade::TEAMMATES));
    assert_eq!(
        *last,
        format!(
            "docsys: upgrade the tree to docsys {}\n\n{}",
            env!("CARGO_PKG_VERSION"),
            docsys::upgrade::TEAMMATES
        )
    );
    assert_eq!(
        git(&repo, &["rev-list", "--count", &format!("{head}..HEAD")]),
        "2"
    );
    // the pin moves with the last commit only
    assert_eq!(
        git(&repo, &["diff", "--name-only", "HEAD~1", "HEAD"]),
        "docs/.docmeta.yml\ndocs/.docsys-version\ndocs/work/next-spec.md"
    );
    assert!(fs::read_to_string(root.join(".docmeta.yml"))
        .unwrap()
        .starts_with("spec: docsys/0.6\n"));
    let _ = fs::remove_dir_all(&repo);
}

/// A knowledge base's contract is the owner's file once written: the upgrade
/// refreshes it only while it is the text 0.15 wrote, and shows its owner
/// the diff otherwise.
#[test]
fn a_knowledge_base_contract_is_refreshed_only_while_untouched() {
    for edited in [false, true] {
        let kb = tmp(&format!("kb-contract-{edited}"));
        git(&kb, &["init", "-q", "-b", "main"]);
        git(&kb, &["config", "user.email", "t@example.invalid"]);
        git(&kb, &["config", "user.name", "t"]);
        let out = docsys(&kb, &["init", "--profile", "knowledge-base", "--root", "."]);
        assert!(out.status.success(), "{out:?}");
        // the base as 0.15 left it: spec 0.4, no pin, the 0.15 contract
        let dm = kb.join(".docmeta.yml");
        let text = fs::read_to_string(&dm).unwrap().replace(
            &format!("spec: docsys/{}", docsys::rules::spec_version()),
            "spec: docsys/0.4",
        );
        fs::write(&dm, text).unwrap();
        fs::remove_file(kb.join(".docsys-version")).unwrap();
        let contract = if edited {
            KB_CONTRACT_0_15.replace("Never invent.", "Never invent; cite.")
        } else {
            KB_CONTRACT_0_15.to_string()
        };
        fs::write(kb.join("AGENTS.md"), &contract).unwrap();
        git(&kb, &["add", "-A"]);
        git(&kb, &["commit", "-qm", "the base as 0.15 left it"]);
        let out = docsys(&kb, &["upgrade", "--apply", "--commit", "--root", "."]);
        assert!(out.status.success(), "{out:?}");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let now = fs::read_to_string(kb.join("AGENTS.md")).unwrap();
        if edited {
            assert_eq!(now, contract);
            assert!(
                stdout.contains("manual  kb-contract        AGENTS.md"),
                "{stdout}"
            );
            assert!(stdout.contains("\n# AGENTS.md\n"), "{stdout}");
        } else {
            assert_eq!(now, docsys::agents::kb_contract());
            assert!(
                stdout.contains("auto    kb-contract        AGENTS.md"),
                "{stdout}"
            );
        }
        let _ = fs::remove_dir_all(&kb);
    }
}

/// A repository that tracks its hooks in `.githooks/`: a fresh clone has no
/// `core.hooksPath` yet. The gate the upgrade rewrites is the tracked one,
/// and it goes into the upgrade commit, so a second run finds nothing to do.
#[test]
fn a_tracked_githooks_gate_is_part_of_the_upgrade_commit() {
    let (repo, _) = build("githooks");
    let case = Path::new(env!("CARGO_MANIFEST_DIR")).join(CASE);
    use std::os::unix::fs::PermissionsExt;
    let hooks = repo.join(".githooks");
    fs::create_dir_all(&hooks).unwrap();
    fs::copy(case.join("hooks/pre-commit"), hooks.join("pre-commit")).unwrap();
    fs::set_permissions(hooks.join("pre-commit"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::remove_file(repo.join(".git/hooks/pre-commit")).unwrap();
    git(&repo, &["add", ".githooks/pre-commit"]);
    git(&repo, &["commit", "-qm", "the project's own hooks"]);

    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stdout)
            .contains("auto    git-gate           .githooks/pre-commit  the docsys block rewritten for this version, its mode kept\n"),
        "{out:?}"
    );
    assert!(git(&repo, &["show", "--stat", "HEAD"]).contains(".githooks/pre-commit"));
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    let _ = fs::remove_dir_all(&repo);
}

/// A docsys asset nobody edited since an older release wrote it is that
/// release's text, not its owner's: refreshed, not shown as a diff.
#[test]
fn an_asset_an_older_release_wrote_is_refreshed() {
    let (repo, _) = build("old-asset");
    let skill = repo.join(".claude/skills/docsys/SKILL.md");
    fs::write(&skill, include_str!("golden/docsys-skill-0.1.0.md")).unwrap();
    git(&repo, &["commit", "-qam", "the skill as 0.1.0 wrote it"]);
    let out = docsys(&repo, &["upgrade"]);
    assert!(
        String::from_utf8_lossy(&out.stdout).contains(
            "auto    assets             .claude/skills/docsys/SKILL.md  refreshed: a text docsys 0.1.0 wrote, untouched"
        ),
        "{out:?}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// A relay its owner touched — a comment line is a touch — is the owner's:
/// shown as a diff, never rewritten (D-117).
#[test]
fn a_relay_with_an_owners_comment_is_never_rewritten() {
    let (repo, _) = build("relay-comment");
    let relay = repo.join(".claude/hooks/post-edit-updated.sh");
    let owned = fs::read_to_string(&relay).unwrap() + "# owner: keep this relay quiet in CI\n";
    fs::write(&relay, &owned).unwrap();
    git(&repo, &["commit", "-qam", "the owner edits a relay"]);
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains(
            "manual  hook-scripts       .claude/hooks/post-edit-updated.sh  edited by its owner"
        ),
        "{out:?}"
    );
    assert_eq!(fs::read_to_string(&relay).unwrap(), owned);
    let _ = fs::remove_dir_all(&repo);
}

/// A teammate's fresh clone of a repository that tracks its gate in
/// `.githooks/`: git does not run it until `core.hooksPath` says so. `doctor`
/// names the one command, and that command — the per-clone step the upgrade
/// commit names — sets it.
#[test]
fn a_fresh_clone_of_a_githooks_repository_gets_its_gate_from_the_per_clone_step() {
    use std::os::unix::fs::PermissionsExt;
    let (repo, _) = build("githooks-clone");
    let case = Path::new(env!("CARGO_MANIFEST_DIR")).join(CASE);
    fs::create_dir_all(repo.join(".githooks")).unwrap();
    fs::copy(
        case.join("hooks/pre-commit"),
        repo.join(".githooks/pre-commit"),
    )
    .unwrap();
    fs::set_permissions(
        repo.join(".githooks/pre-commit"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    git(&repo, &["add", ".githooks/pre-commit"]);
    git(&repo, &["commit", "-qm", "the project's own hooks"]);
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");

    let clone = tmp("githooks-clone-2");
    fs::remove_dir_all(&clone).unwrap();
    let ok = Command::new("git")
        .args(["clone", "-q"])
        .arg(&repo)
        .arg(&clone)
        .status()
        .unwrap();
    assert!(ok.success());
    git(&clone, &["config", "user.email", "t@example.invalid"]);
    git(&clone, &["config", "user.name", "t"]);
    let doctor = String::from_utf8_lossy(&docsys(&clone, &["doctor"]).stdout).into_owned();
    assert!(
        doctor.contains("`docsys upgrade --apply` points core.hooksPath at it"),
        "{doctor}"
    );
    let out = docsys(&clone, &["upgrade", "--apply"]);
    assert!(out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("core.hooksPath"),
        "{out:?}"
    );
    assert_eq!(
        git(&clone, &["config", "--get", "core.hooksPath"]),
        ".githooks"
    );
    let doctor = String::from_utf8_lossy(&docsys(&clone, &["doctor"]).stdout).into_owned();
    assert!(
        doctor.contains(".githooks/pre-commit gate reachable"),
        "{doctor}"
    );
    for d in [repo, clone] {
        let _ = fs::remove_dir_all(d);
    }
}

/// In a tree whose root is not `docs`, a skill an older release wrote with
/// `docs` in it is refreshed to the text that names the tree's own root.
#[test]
fn a_refreshed_asset_names_the_trees_own_root() {
    let repo = tmp("asset-root");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    fs::write(repo.join("README.md"), "# x\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "init"]);
    let out = docsys(&repo, &["adopt", "--root", "documentation"]);
    assert!(out.status.success(), "{out:?}");
    let skill = repo.join(".claude/skills/docsys/SKILL.md");
    fs::write(&skill, include_str!("golden/docsys-skill-0.1.0.md")).unwrap();
    git(&repo, &["add", "-A"]);
    git(
        &repo,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-qm",
            "adopt, an old skill",
        ],
    );
    let out = docsys(&repo, &["upgrade", "--apply", "--root", "documentation"]);
    assert!(out.status.success(), "{out:?}");
    let text = fs::read_to_string(&skill).unwrap();
    assert!(
        !text.contains("--root docs") && !text.contains(" docs/"),
        "{text}"
    );
    assert!(text.contains("documentation/.docmeta.yml"), "{text}");
    let _ = fs::remove_dir_all(&repo);
}

/// A file docsys owns that holds exactly the text this version writes is
/// docsys's own, tracked or not: an untracked one goes into the upgrade
/// commit instead of staying behind as `??`.
#[test]
fn an_untracked_asset_holding_this_versions_text_goes_into_the_commit() {
    let (repo, _) = build("untracked-asset");
    // `docsys agents` writes the command the 0.4 tree lacks, before the upgrade
    let out = docsys(&repo, &["agents"]);
    assert!(out.status.success(), "{out:?}");
    assert!(
        git(&repo, &["status", "--porcelain"]).contains("?? .claude/commands/docsys-upgrade.md")
    );
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    assert!(
        git(&repo, &["show", "--stat", "HEAD"]).contains(".claude/commands/docsys-upgrade.md"),
        "{out:?}"
    );
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
    let _ = fs::remove_dir_all(&repo);
}

/// The preview lists what the tree loses as well as what it gains, `refs`
/// included: a false citation 0.4 read mid-comment is gone under 0.5 (D-117).
#[test]
fn the_preview_lists_the_findings_the_move_takes_away() {
    let (repo, _) = build("preview-gone");
    fs::write(
        repo.join("src/limits.ts"),
        "// see the provider doc: it caps the rate\nexport const limit = 10;\n",
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    git(
        &repo,
        &["-c", "core.hooksPath=/dev/null", "commit", "-qm", "a limit"],
    );
    let out = docsys(&repo, &["upgrade"]);
    let plan = String::from_utf8_lossy(&out.stdout);
    assert!(
        plan.lines()
            .any(|l| l.starts_with("- ERROR R-076 src/limits.ts [it]")),
        "{plan}"
    );
    assert!(!plan.contains(", 0 gone"), "{plan}");
    let _ = fs::remove_dir_all(&repo);
}

/// N4: a branch opened before the move still writes `updated:`; after it
/// merges, lint names the upgrade, and a re-run takes out exactly that line
/// (D-122).
#[test]
fn a_late_branchs_date_line_is_absorbed_by_a_re_run() {
    let (repo, _) = build("late-date");
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    let page = repo.join("docs/reference/expiry.md");
    let moved = fs::read_to_string(&page).unwrap();
    assert!(!moved.contains("\nupdated:"), "{moved}");
    let late = moved.replacen(
        "type: reference\n",
        "type: reference\nupdated: 2026-09-03\n",
        1,
    );
    fs::write(&page, &late).unwrap();
    git(&repo, &["commit", "-qam", "a branch from before the move"]);
    let lint = docsys(&repo, &["lint"]);
    assert!(
        String::from_utf8_lossy(&lint.stdout).contains("`docsys upgrade --apply` removes the line"),
        "{lint:?}"
    );
    let out = docsys(&repo, &["upgrade", "--apply"]);
    assert!(out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("dates"),
        "{out:?}"
    );
    assert_eq!(fs::read_to_string(&page).unwrap(), moved);
    let again = docsys(&repo, &["upgrade", "--apply"]);
    assert!(
        !String::from_utf8_lossy(&again.stdout).contains("dates"),
        "{again:?}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// Merge `branch`, and where git stops, do what a person does with two
/// lists: keep both sides, each line once.
fn merge_keeping_both(repo: &Path, branch: &str) -> bool {
    let merged = Command::new("git")
        .args(["merge", "-q", "--no-edit", branch])
        .current_dir(repo)
        .output()
        .unwrap();
    if merged.status.success() {
        return true;
    }
    let conflicted = git(repo, &["diff", "--name-only", "--diff-filter=U"]);
    for f in conflicted.lines() {
        let p = repo.join(f);
        match fs::read_to_string(&p) {
            Ok(text) => {
                let mut seen = std::collections::BTreeSet::new();
                let both: String = text
                    .lines()
                    .filter(|l| {
                        !l.starts_with("<<<<<<<")
                            && !l.starts_with("=======")
                            && !l.starts_with(">>>>>>>")
                    })
                    .filter(|l| !l.starts_with("- [") || seen.insert(l.to_string()))
                    .map(|l| format!("{l}\n"))
                    .collect();
                fs::write(&p, both).unwrap();
            }
            Err(_) => {
                git(repo, &["checkout", "--theirs", "--", f]);
            }
        }
    }
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "--no-edit"]);
    false
}

/// N4: a branch opened before the move appends to the old ledgers. Run on
/// the branch, the same upgrade makes the same files; where both sides now
/// hold one topic's file, it merges as any shared file does. Merged without
/// the upgrade, git carries the line into the frozen slice, and after the
/// person keeps both sides a re-run moves exactly that item into its topic's
/// file. Every closed item and line of prose stays as written (D-124).
#[test]
fn a_late_branchs_ledger_lines_are_absorbed_and_the_frozen_slice_keeps_its_bytes() {
    let (repo, _) = build("late-ledger");
    let debt = repo.join("docs/work/debt.md");
    let kept = "# Debt\n\nItems the team chose to defer.\n\n- [x] 2026-08-20 a closed one -- deferred: a -- repay when: b -- resolved: done\n";
    let before = "- [ ] 2026-09-01 open before -- deferred: c -- repay when: d\n";
    fs::write(&debt, format!("{kept}{before}")).unwrap();
    git(&repo, &["commit", "-qam", "the ledger"]);
    let late = "- [ ] 2026-09-04 added on the branch -- deferred: e -- repay when: f\n";
    let other = "- [ ] 2026-09-05 added on another branch -- deferred: g -- repay when: h\n";
    for (branch, line) in [("upgraded", late), ("plain", other)] {
        git(&repo, &["checkout", "-qb", branch]);
        fs::write(&debt, fs::read_to_string(&debt).unwrap() + line).unwrap();
        git(&repo, &["commit", "-qam", "a branch adds a debt"]);
        git(&repo, &["checkout", "-q", "main"]);
    }
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    let slice = repo.join("docs/_archive/work/debt.md");
    assert_eq!(
        fs::read_to_string(&slice).unwrap(),
        kept,
        "the rest, as written"
    );
    // an untagged item's topic is `general` (D-124)
    let item = repo.join("docs/work/debt/general.md");
    assert_eq!(fs::read_to_string(&item).unwrap(), before);

    // the branch runs the same upgrade before it merges: no conflict
    git(&repo, &["checkout", "-q", "upgraded"]);
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    git(&repo, &["checkout", "-q", "main"]);
    merge_keeping_both(&repo, "upgraded");
    assert_eq!(
        fs::read_to_string(&item).unwrap(),
        format!("{before}{late}")
    );
    let closed = docsys(
        &repo,
        &["debt", "close", "added on the branch", "--note", "done"],
    );
    assert!(closed.status.success(), "{closed:?}");
    git(&repo, &["commit", "-qam", "repaid", "-m", "Resolved: done"]);

    // the other merges as it is: git carries its line wherever it follows
    // the ledger
    merge_keeping_both(&repo, "plain");
    let lint = docsys(&repo, &["lint"]);
    assert!(
        String::from_utf8_lossy(&lint.stdout).contains("`docsys upgrade --apply` moves"),
        "{lint:?}"
    );
    let out = docsys(&repo, &["upgrade", "--apply"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        fs::read_to_string(&item).unwrap(),
        format!("{before}{other}"),
        "the branch's item, once; the one closed on main stays closed; none twice"
    );
    assert_eq!(
        fs::read_to_string(&slice).unwrap(),
        kept,
        "the slice is back to its bytes"
    );
    assert!(!debt.exists());
    let again = docsys(&repo, &["upgrade", "--apply"]);
    assert!(
        !String::from_utf8_lossy(&again.stdout).contains("ledgers"),
        "{again:?}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// N4, seen on real clones: a ledger that held only its title and untagged
/// items becomes `general.md`, and git takes that for a rename — a late
/// branch's tagged item follows it there. Lint names the upgrade, and a
/// re-run moves the line, verbatim, into the file its tag names (D-124).
#[test]
fn a_tagged_item_git_carried_into_another_topic_moves_to_its_own() {
    let (repo, _) = build("late-topic");
    let debt = repo.join("docs/work/debt.md");
    let before = "- [ ] 2026-09-01 open before -- deferred: c -- repay when: d\n";
    fs::write(&debt, format!("# Debt\n\n{before}")).unwrap();
    git(&repo, &["commit", "-qam", "the ledger"]);
    let late = "- [ ] 2026-09-04 [cache] added on the branch -- deferred: e -- repay when: f\n";
    git(&repo, &["checkout", "-qb", "plain"]);
    fs::write(&debt, fs::read_to_string(&debt).unwrap() + late).unwrap();
    git(&repo, &["commit", "-qam", "a branch adds a debt"]);
    git(&repo, &["checkout", "-q", "main"]);
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    let general = repo.join("docs/work/debt/general.md");
    assert_eq!(fs::read_to_string(&general).unwrap(), before);
    merge_keeping_both(&repo, "plain");
    assert_eq!(
        fs::read_to_string(&general).unwrap(),
        format!("{before}{late}"),
        "git followed the rename"
    );
    let lint = docsys(&repo, &["lint"]);
    assert!(
        String::from_utf8_lossy(&lint.stdout).contains("`docsys upgrade --apply` moves it there"),
        "{lint:?}"
    );
    let out = docsys(&repo, &["upgrade", "--apply"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(fs::read_to_string(&general).unwrap(), before);
    assert_eq!(
        fs::read_to_string(repo.join("docs/work/debt/cache.md")).unwrap(),
        late
    );
    let lint = docsys(&repo, &["lint"]);
    assert!(
        !String::from_utf8_lossy(&lint.stdout).contains("R-108"),
        "{lint:?}"
    );
    let again = docsys(&repo, &["upgrade", "--apply"]);
    assert!(
        !String::from_utf8_lossy(&again.stdout).contains("ledgers"),
        "{again:?}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// D-125: the journal moves under `_archive/journal/` byte for byte, and a
/// page that linked it links it where it is now (R-172).
#[test]
fn the_journal_moves_as_written_and_its_links_follow() {
    let (repo, _) = build("journal-links");
    let journal = repo.join("docs/work/journal.md");
    let before = fs::read_to_string(&journal).unwrap();
    let page = repo.join("docs/reference/refresh.md");
    let text = fs::read_to_string(&page).unwrap();
    fs::write(
        &page,
        format!("{text}\nWhy it moved: [[work/journal|the journal]].\n"),
    )
    .unwrap();
    git(&repo, &["commit", "-qam", "a link to the journal"]);
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    assert!(!journal.exists());
    assert_eq!(
        fs::read_to_string(repo.join("docs/_archive/journal/journal.md")).unwrap(),
        before,
        "as written"
    );
    let text = fs::read_to_string(&page).unwrap();
    assert!(
        text.contains("[[_archive/journal/journal|the journal]]"),
        "{text}"
    );
    let lint = docsys(&repo, &["lint"]);
    assert!(
        !String::from_utf8_lossy(&lint.stdout).contains("R-071"),
        "{lint:?}"
    );
    let shown = docsys(&repo, &["journal"]);
    assert!(
        String::from_utf8_lossy(&shown.stdout)
            .contains("<!-- frozen: _archive/journal/journal.md -->"),
        "{shown:?}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// Agent-facing text docsys did not write that still names a concept the move
/// retires is listed for a person, line by line, with what replaces it, and
/// never edited; the rules block and docsys's own assets are docsys's.
#[test]
fn text_docsys_did_not_write_is_listed_where_it_names_a_retired_concept() {
    let (repo, _) = build("retired");
    let write = |rel: &str, text: &str| {
        let p = repo.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    };
    let claude_md = format!(
        "# Team\n\n{}\nRecord debt in docs/work/debt.md.\n{}\n\nAfter each task, add a journal line to docs/work/journal.md.\n",
        docsys::rules::BLOCK_BEGIN,
        docsys::rules::BLOCK_END
    );
    write("CLAUDE.md", &claude_md);
    write(
        ".claude/rules/docs.md",
        "Bump the `updated:` field whenever you edit a page.\n",
    );
    write(
        ".claude/commands/ship.md",
        "---\ndescription: ship a change\n---\nRecord deferred work in docs/work/debt.md before you push.\n",
    );
    git(&repo, &["add", "-A"]);
    git(
        &repo,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-qm",
            "the team's own text",
        ],
    );
    let out = docsys(&repo, &["upgrade"]);
    assert!(out.status.success(), "{out:?}");
    let plan = String::from_utf8_lossy(&out.stdout);
    let listed: Vec<&str> = plan
        .lines()
        .filter(|l| l.contains(" retired-concepts "))
        .filter(|l| [" CLAUDE.md:", " .claude/"].iter().any(|f| l.contains(f)))
        .collect();
    let at = |place: &str, literal: &str| {
        listed
            .iter()
            .any(|l| l.starts_with("manual ") && l.contains(place) && l.contains(literal))
    };
    assert!(at(" CLAUDE.md:7 ", "`work/journal`"), "{plan}");
    assert!(at(" CLAUDE.md:7 ", "`journal line`"), "{plan}");
    assert!(at(" .claude/rules/docs.md:1 ", "``updated:``"), "{plan}");
    assert!(
        at(" .claude/commands/ship.md:4 ", "`work/debt.md`"),
        "{plan}"
    );
    assert!(
        listed.iter().all(|l| !l.contains(" CLAUDE.md:4 ")),
        "inside the rules block: {plan}"
    );
    assert!(
        listed.iter().all(|l| !l.contains("docsys-")),
        "docsys's own assets: {plan}"
    );
    assert_eq!(listed.len(), 4, "{plan}");
    // the move edits none of it
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        fs::read_to_string(repo.join("CLAUDE.md")).unwrap(),
        claude_md
    );
    let _ = fs::remove_dir_all(&repo);
}

/// Every gate reads the pin — the commit-msg half a `git merge` runs alone
/// included — and a commit or a merge says the install once: one line for a
/// docsys from before pins, never its usage text; one attempt and its outcome
/// for a version this machine lacks (D-120, R-151).
#[test]
fn every_gate_names_the_pin_once_and_a_merge_is_a_gate_too() {
    let (repo, _) = build("pin-gates");
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    assert!(repo.join(".git/hooks/commit-msg").is_file());
    let hard = !fs::read_to_string(repo.join(".git/hooks/commit-msg"))
        .unwrap()
        .contains("[ \"0\" -eq 0 ]");
    fs::write(repo.join("docs/.docsys-version"), "0.16.9\n").unwrap();
    let quiet_git = |args: &[&str]| {
        let mut all = vec!["-c", "core.hooksPath=/dev/null"];
        all.extend_from_slice(args);
        git(&repo, &all);
    };
    quiet_git(&["commit", "-qam", "pin"]);
    quiet_git(&["checkout", "-qb", "side"]);
    fs::write(repo.join("side.txt"), "side\n").unwrap();
    quiet_git(&["add", "side.txt"]);
    quiet_git(&["commit", "-qm", "side"]);
    quiet_git(&["checkout", "-q", "main"]);
    let base = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    // a shell and git, then the `docsys` and `cargo` each case puts first
    let tools = tmp("pin-gates-tools");
    for tool in ["git", "sh", "bash", "head", "sed", "cat", "mkdir", "chmod"] {
        let p = Command::new("sh")
            .args(["-c", &format!("command -v {tool}")])
            .output()
            .unwrap();
        let p = String::from_utf8_lossy(&p.stdout).trim().to_string();
        std::os::unix::fs::symlink(p, tools.join(tool)).unwrap();
    }
    let script = |dir: &Path, name: &str, text: &str| {
        use std::os::unix::fs::PermissionsExt;
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(name), text).unwrap();
        fs::set_permissions(dir.join(name), fs::Permissions::from_mode(0o755)).unwrap();
    };
    // a docsys from before pins: no `--version`, its usage for anything
    let old = tmp("pin-gates-old");
    script(
        &old,
        "docsys",
        "#!/bin/sh\necho 'OLD USAGE line one' >&2\necho 'OLD USAGE line two' >&2\nexit 2\n",
    );
    // a cargo whose install fails, logging each attempt
    let failing = tmp("pin-gates-cargo");
    let log = failing.join("attempts.log");
    script(
        &failing,
        "cargo",
        &format!(
            "#!/bin/sh\n[ \"$1\" = --version ] && {{ echo 'cargo 1.0.0'; exit 0; }}\necho \"$*\" >> '{}'\necho 'error: could not compile docsys' >&2\nexit 101\n",
            log.display()
        ),
    );
    let this = bin().parent().unwrap().to_path_buf();
    let home = tmp("pin-gates-home");
    let run = |first: &[&Path], args: &[&str]| -> (bool, String) {
        let path = first
            .iter()
            .map(|p| p.display().to_string())
            .chain([tools.display().to_string()])
            .collect::<Vec<_>>()
            .join(":");
        let out = Command::new("git")
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .env("PATH", path)
            .env("DOCSYS_HOME", &home)
            .env_remove("DOCSYS_DISPATCHED")
            .current_dir(&repo)
            .output()
            .unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).into_owned()
                + &String::from_utf8_lossy(&out.stderr),
        )
    };
    let reset = || {
        let _ = Command::new("git")
            .args(["merge", "--abort"])
            .current_dir(&repo)
            .output();
        quiet_git(&["reset", "-q", "--hard", &base]);
    };
    let old_line = "docsys: this tree pins docsys 0.16.9; install: cargo install docsys --version 0.16.9 --locked";
    let missing = "docsys: this tree pins docsys 0.16.9; install it: ";
    // a docsys from before pins: a commit, then a merge
    fs::write(repo.join("notes.txt"), "a change\n").unwrap();
    git(&repo, &["add", "notes.txt"]);
    let (_, said) = run(&[&old], &["commit", "-qm", "a change"]);
    assert_eq!(said.matches(old_line).count(), 1, "commit: {said}");
    assert!(!said.contains("OLD USAGE"), "commit: {said}");
    reset();
    let (ok, said) = run(&[&old], &["merge", "--no-ff", "-m", "merge side", "side"]);
    assert_eq!(said.matches(old_line).count(), 1, "merge: {said}");
    assert!(!said.contains("OLD USAGE"), "merge: {said}");
    assert_eq!(ok, !hard, "merge: {said}");
    reset();
    // this docsys, the pinned version missing, no cargo: the merge names it
    let (ok, said) = run(&[&this], &["merge", "--no-ff", "-m", "merge side", "side"]);
    assert_eq!(said.matches(missing).count(), 1, "merge, no cargo: {said}");
    assert_eq!(ok, !hard, "merge, no cargo: {said}");
    reset();
    // a cargo whose install fails: one attempt, and its outcome is the last word
    fs::write(repo.join("notes.txt"), "a change\n").unwrap();
    git(&repo, &["add", "notes.txt"]);
    let (_, said) = run(&[&failing, &this], &["commit", "-qm", "a change"]);
    let attempts = fs::read_to_string(&log).unwrap_or_default().lines().count();
    assert_eq!(attempts, 1, "{said}");
    assert_eq!(said.matches("installing it once").count(), 1, "{said}");
    assert!(
        said.lines()
            .rfind(|l| l.starts_with("docsys:"))
            .is_some_and(|l| l.contains("could not be installed")),
        "{said}"
    );
    for d in [repo, tools, old, failing, home] {
        let _ = fs::remove_dir_all(d);
    }
}
