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
            "docsys: this tree declares docsys/0.4 and is served by its rules; `docsys upgrade` moves it to docsys/{} when the repository is ready",
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

/// R-171 once per run: a commit through the gate runs lint, refs and gate,
/// and a 0.4 tree hears the notice once, from lint.
#[test]
fn a_commit_in_a_0_4_tree_prints_the_notice_once() {
    let (repo, _) = build("notice-once");
    // `adopt` brings the clone's gate to this version and leaves the tree at 0.4
    let out = docsys(&repo, &["adopt"]);
    assert!(out.status.success(), "{out:?}");
    git(&repo, &["add", "-A"]);
    let out = Command::new("git")
        .args(["-c", "commit.gpgsign=false", "commit", "-qm", "adopt again"])
        .env("PATH", path())
        .current_dir(&repo)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        stderr
            .lines()
            .filter(|l| l.starts_with("docsys: this tree declares docsys/0.4"))
            .count(),
        1,
        "{stderr}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// The separators move with the tree; afterwards they are `ledger fix`'s, the
/// one command R-108's message names.
#[test]
fn after_the_move_the_separators_are_ledger_fixs() {
    let (repo, _) = build("separators");
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    let debt = repo.join("docs/work/debt.md");
    let dashed = "- [ ] 2026-09-03 A second item — deferred: later — repay when: soon\n";
    fs::write(&debt, fs::read_to_string(&debt).unwrap() + dashed).unwrap();
    git(&repo, &["commit", "-qam", "a dashed item"]);
    let out = docsys(&repo, &["upgrade", "--apply"]);
    assert!(out.status.success(), "{out:?}");
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains("ledger-separators"),
        "{out:?}"
    );
    assert!(fs::read_to_string(&debt).unwrap().ends_with(dashed));
    let out = docsys(&repo, &["ledger", "fix"]);
    assert!(out.status.success(), "{out:?}");
    assert!(fs::read_to_string(&debt)
        .unwrap()
        .ends_with("- [ ] 2026-09-03 A second item -- deferred: later -- repay when: soon\n"));
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
            apply: real.apply,
        },
        Migration {
            from: 5,
            to: 6,
            release: "9.9.9",
            steps: "spec-line\tauto\ttracked\t-\n",
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
