#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! R-179: the docsys/0.5 → docsys/0.6 conformance case — a team's repository
//! as docsys 0.17.0 adopted it, with debt, a tracked `.githooks/` gate in
//! warn mode and the owner's own workflow, moved by `docsys upgrade --apply
//! --commit` and compared file by file with the expected repository. The move
//! needs no hand edit: the workflow runs the tree's installer, the gate is
//! hard and blocks only on what a commit adds, and a machine with no docsys
//! is told how to install it (D-136, D-138). `DOCSYS_BLESS=1 cargo test
//! --test team_move` writes the expected files from the current build.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const CASE: &str = "corpus/upgrades/0.5-to-0.6";
const TARGETS: [&str; 4] = [
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
];

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-team-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// This version's release as a `file://` directory: each archive's
/// `.sha256` file, one fixed value a target, so the installer the move
/// writes is the same bytes on every run.
fn release() -> String {
    let dir = tmp("release");
    let v = env!("CARGO_PKG_VERSION");
    let at = dir.join(format!("v{v}"));
    fs::create_dir_all(&at).unwrap();
    for (target, digit) in TARGETS.iter().zip(["1", "2", "3", "4"]) {
        let name = format!("docsys-v{v}-{target}.tar.gz");
        fs::write(
            at.join(format!("{name}.sha256")),
            format!("{}  {name}\n", digit.repeat(64)),
        )
        .unwrap();
    }
    format!("file://{}", dir.display())
}

fn run(dir: &Path, program: &Path, args: &[&str], path: &str, releases: &str) -> Output {
    Command::new(program)
        .args(args)
        .current_dir(dir)
        .env("PATH", path)
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .env("DOCSYS_RELEASES", releases)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
        .env("GIT_AUTHOR_DATE", "2026-10-01T12:00:00+00:00")
        .env("GIT_COMMITTER_DATE", "2026-10-01T12:00:00+00:00")
        .output()
        .unwrap()
}

/// PATH with this build first: the gate a commit runs is this docsys.
fn path() -> String {
    format!(
        "{}:{}",
        bin().parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

fn git(dir: &Path, args: &[&str]) -> String {
    let mut all = vec!["-c", "commit.gpgsign=false"];
    all.extend_from_slice(args);
    let out = run(dir, Path::new("git"), &all, &path(), "");
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// A stored top-level `dot-<name>` is the repository's `.<name>`.
fn real_name(rel: &str) -> String {
    match rel.strip_prefix("dot-") {
        Some(rest) => format!(".{rest}"),
        None => rel.to_string(),
    }
}

fn stored_name(rel: &str) -> String {
    match rel.strip_prefix('.') {
        Some(rest) => format!("dot-{rest}"),
        None => rel.to_string(),
    }
}

fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
    let mut entries: Vec<_> = fs::read_dir(dir).unwrap().flatten().collect();
    entries.sort_by_key(|e| e.path());
    for e in entries {
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

fn snapshot(dir: &Path, stored: bool) -> BTreeMap<String, String> {
    let mut files = Vec::new();
    walk(dir, dir, &mut files);
    files
        .into_iter()
        .map(|rel| {
            let body = fs::read_to_string(dir.join(&rel)).unwrap();
            (if stored { real_name(&rel) } else { rel }, body)
        })
        .collect()
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

/// The team's repository: `before/` committed, its tracked gate configured as
/// the team's own setup step configures it.
fn build(name: &str) -> PathBuf {
    let case = Path::new(env!("CARGO_MANIFEST_DIR")).join(CASE);
    let repo = tmp(name);
    git(&repo, &["init", "-q", "-b", "main"]);
    let mut files = Vec::new();
    let before = case.join("before");
    walk(&before, &before, &mut files);
    for rel in files {
        let target = repo.join(real_name(&rel));
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(before.join(&rel), &target).unwrap();
    }
    use std::os::unix::fs::PermissionsExt;
    for hook in ["pre-commit", "commit-msg"] {
        let p = repo.join(".githooks").join(hook);
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    }
    git(&repo, &["config", "core.hooksPath", ".githooks"]);
    git(&repo, &["add", "-A"]);
    git(
        &repo,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-qm",
            "the team's tree as docsys 0.17.0 adopted it",
        ],
    );
    repo
}

/// The move, file for file, and what the moved repository does: its gate
/// hard and scoped, its workflow and installer, and a clone with no docsys.
#[test]
fn a_teams_0_5_tree_moves_to_0_6_with_no_hand_edit() {
    let releases = release();
    let repo = build("move");
    let docsys = |args: &[&str]| run(&repo, &bin(), args, &path(), &releases);

    // the plan writes nothing
    let before = snapshot(&repo, false);
    let plan = docsys(&["upgrade"]);
    assert!(plan.status.success(), "{plan:?}");
    expect_text("plan.txt", &String::from_utf8_lossy(&plan.stdout));
    assert_eq!(snapshot(&repo, false), before, "the plan wrote");

    // the move is one commit, and leaves nothing behind
    let out = docsys(&["upgrade", "--apply", "--commit"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(git(&repo, &["rev-list", "--count", "HEAD~1..HEAD"]), "1");
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
    let after_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CASE)
        .join("after");
    let got = snapshot(&repo, false);
    if bless() {
        let _ = fs::remove_dir_all(&after_dir);
        for (rel, body) in &got {
            let p = after_dir.join(stored_name(rel));
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, body).unwrap();
        }
    } else {
        let want = snapshot(&after_dir, true);
        let names = |m: &BTreeMap<String, String>| m.keys().cloned().collect::<Vec<_>>();
        assert_eq!(names(&got), names(&want), "the files differ");
        for (rel, body) in &want {
            let g = got.get(rel).unwrap();
            assert!(
                g == body,
                "{rel} differs\n{}",
                docsys::diff::unified(body, g, rel, "this build", 3)
            );
        }
    }
    let lint = docsys(&["lint"]);
    expect_text("lint.txt", &String::from_utf8_lossy(&lint.stdout));

    // what the move wrote, by the items it serves
    let workflow = fs::read_to_string(repo.join(".github/workflows/docsys.yml")).unwrap();
    for line in [
        "  push:\n    branches: [main]\n",
        "  merge_group:\n",
        "        run: bash docs/.docsys-install --print-dir >> \"$GITHUB_PATH\"\n",
        "      - if: github.event_name == 'push'\n        run: docsys lint --root docs --repo .\n",
        "      - if: github.event_name == 'merge_group'\n",
    ] {
        assert!(workflow.contains(line), "`{line}` not in:\n{workflow}");
    }
    assert!(!workflow.contains("0.16.1"), "{workflow}");
    let installer = fs::read_to_string(repo.join("docs/.docsys-install")).unwrap();
    assert!(
        installer.contains(&format!("\nversion={}\n", env!("CARGO_PKG_VERSION"))),
        "{installer}"
    );
    let gate = fs::read_to_string(repo.join(".githooks/pre-commit")).unwrap();
    // hard on the moved tree; a branch at docsys/0.5 keeps the warn mode
    // this clone had, and runs refs there (D-136)
    assert!(
        gate.contains("\ndocsys_gate_exit=1\n")
            && gate.contains("*) docsys_gate_exit=0; docsys_scoped=0 ;;")
            && gate.contains("[ \"$docsys_scoped\" -eq 1 ] || docsys refs"),
        "{gate}"
    );

    // the gate the move hardened: the debt is inherited, a new error stops
    // the commit (D-136)
    let commit = |message: &str| {
        git(&repo, &["add", "-A"]);
        let out = run(
            &repo,
            Path::new("git"),
            &["-c", "commit.gpgsign=false", "commit", "-q", "-m", message],
            &path(),
            &releases,
        );
        (out.status.code(), text(&out))
    };
    fs::write(repo.join("src/b.ts"), "export const b = 2;\n").unwrap();
    fs::write(
        repo.join("docs/reference/b.md"),
        "---\nid: b\ntype: reference\n---\n# B\n\nThis page describes b.\n",
    )
    .unwrap();
    let (code, said) = commit("add b");
    assert_eq!(code, Some(0), "{said}");
    assert!(said.contains("inherited from "), "{said}");
    fs::write(
        repo.join("src/c.ts"),
        "export const c = 3; // doc: phantom\n",
    )
    .unwrap();
    fs::write(
        repo.join("docs/reference/c.md"),
        "---\nid: c\ntype: reference\n---\n# C\n\nThis page describes c.\n",
    )
    .unwrap();
    let (code, said) = commit("add c");
    assert_eq!(code, Some(1), "{said}");
    assert!(said.contains("ERROR R-076 src/c.ts [phantom]"), "{said}");
    git(&repo, &["reset", "-q", "--hard"]);
    let _ = fs::remove_file(repo.join("src/c.ts"));
    let _ = fs::remove_file(repo.join("docs/reference/c.md"));

    // a clone with no docsys: the git gate warns and the commit goes on; the
    // prompt relay tells the agent to install the pinned version; the rules
    // block says the same (D-138)
    let bare = "/usr/bin:/bin";
    let hook = run(
        &repo,
        Path::new("bash"),
        &[".githooks/pre-commit"],
        bare,
        "",
    );
    assert_eq!(hook.status.code(), Some(0), "{hook:?}");
    assert_eq!(
        String::from_utf8_lossy(&hook.stderr),
        "docsys: not installed, so this commit is not checked; install the version this tree pins: bash docs/.docsys-install\n"
    );
    let mut relay = Command::new("bash");
    relay
        .arg(".claude/hooks/session-intent.sh")
        .current_dir(&repo)
        .env_clear()
        .env("PATH", bare)
        .env("CLAUDE_PROJECT_DIR", &repo)
        .stdin(std::process::Stdio::null());
    let relay = relay.output().unwrap();
    assert_eq!(relay.status.code(), Some(0), "{relay:?}");
    assert!(
        String::from_utf8_lossy(&relay.stdout).contains(
            "Install the version this tree pins now, without asking the person first: `bash docs/.docsys-install`"
        ),
        "{relay:?}"
    );
    let rules = fs::read_to_string(repo.join("AGENTS.md")).unwrap();
    assert!(
        rules.contains("install the pinned version yourself, without asking first")
            && !rules
                .contains("or an install command, run `/docsys-upgrade` and ask the person first"),
        "{rules}"
    );

    // a second run: nothing automatic, no commit
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let again = docsys(&["upgrade", "--apply", "--commit"]);
    assert!(again.status.success(), "{again:?}");
    let said = String::from_utf8_lossy(&again.stdout);
    assert!(!said.lines().any(|l| l.starts_with("auto ")), "{said}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    let _ = fs::remove_dir_all(&repo);
}
