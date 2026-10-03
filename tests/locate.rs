#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// Where docsys runs (D-098): a command or a relay started in a subdirectory,
// a git hook in a linked worktree (git exports GIT_DIR there), a monorepo
// with a tree per package, and a stray tree above a repository. Unix-only:
// the relays and the git hook are bash.
#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-locate-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
}

fn path_with_bin() -> String {
    format!(
        "{}:{}",
        bin().parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

fn git(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("PATH", path_with_bin())
        .env("DOCSYS_TODAY", "2026-10-02")
        // history on the day docsys is told it is, so `updated:` and the last
        // change agree whatever day the suite runs (R-106)
        .env("GIT_AUTHOR_DATE", "2026-10-02T12:00:00+00:00")
        .env("GIT_COMMITTER_DATE", "2026-10-02T12:00:00+00:00")
        .output()
        .unwrap()
}

fn ok(dir: &Path, args: &[&str]) {
    let out = git(dir, args);
    assert!(
        out.status.success(),
        "git {args:?}: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn docsys(dir: &Path, args: &[&str]) -> (i32, String) {
    let out = Command::new(bin())
        .args(args)
        .current_dir(dir)
        .env("DOCSYS_TODAY", "2026-10-02")
        .env_remove("CLAUDE_PROJECT_DIR")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

const AUTH_RS: &str =
    "pub fn other() -> u32 {\n    1\n}\n\npub fn refresh_token(ttl: u32) -> u32 {\n    ttl / 2\n}\n";

/// An adopted repository with one routed page pinned to `src/auth.rs`, and a
/// package directory to stand in.
fn project(name: &str) -> PathBuf {
    let repo = tmp(name);
    ok(&repo, &["init", "-q"]);
    ok(&repo, &["config", "user.email", "t@example.invalid"]);
    ok(&repo, &["config", "user.name", "t"]);
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/auth.rs"), AUTH_RS).unwrap();
    fs::create_dir_all(repo.join("apps/x/src")).unwrap();
    fs::write(repo.join("apps/x/src/main.rs"), "fn main() {}\n").unwrap();
    let (code, out) = docsys(&repo, &["adopt"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("git pre-commit gate: written (hard)"), "{out}");
    fs::create_dir_all(repo.join("docs/reference")).unwrap();
    fs::write(
        repo.join("docs/reference/token-ttl.md"),
        "---\nid: token-ttl\ntype: reference\n---\n\n# Token TTL\n\nThis page says how long a token lives; read it before changing the refresh.\n",
    )
    .unwrap();
    let index = fs::read_to_string(repo.join("docs/index.md")).unwrap();
    fs::write(
        repo.join("docs/index.md"),
        format!("{index}- [[reference/token-ttl|Token TTL]] -- How long a token lives.\n"),
    )
    .unwrap();
    let (code, out) = docsys(
        &repo,
        &[
            "pin",
            "reference/token-ttl",
            "src/auth.rs",
            "--symbol",
            "refresh_token",
        ],
    );
    assert_eq!(code, 0, "{out}");
    ok(&repo, &["add", "-A"]);
    ok(
        &repo,
        &["commit", "-qm", "adopt docsys, pin the token page"],
    );
    repo
}

#[test]
fn a_command_in_a_subdirectory_answers_as_it_does_at_the_top() {
    let repo = project("sub");
    let deep = repo.join("apps/x/src");
    for args in [
        vec!["lint"],
        vec!["status"],
        vec!["lookup", "token"],
        vec!["gate"],
        vec!["backlinks", "token-ttl"],
    ] {
        let top = docsys(&repo, &args);
        let below = docsys(&deep, &args);
        assert_eq!(
            top, below,
            "{args:?} differs between the top and a subdirectory"
        );
    }
    // the pin resolves from below too, and a refresh lands on the same page
    let (code, out) = docsys(&deep, &["pin", "--refresh", "reference/token-ttl"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        git(&repo, &["status", "--porcelain"]).stdout.is_empty() || {
            // `pin --refresh` bumps `updated:` only when the day moved; nothing else may change
            let diff =
                String::from_utf8_lossy(&git(&repo, &["diff", "--stat"]).stdout).into_owned();
            diff.contains("docs/reference/token-ttl.md") && !diff.contains("apps/")
        }
    );
    let _ = fs::remove_dir_all(&repo);
}

fn relay(repo: &Path, script: &str, cwd: &Path, payload: &str) -> (i32, String, String) {
    let markers = repo.join(".git/test-tmp");
    let _ = fs::create_dir_all(&markers);
    let mut child = Command::new("bash")
        .arg(repo.join(".claude/hooks").join(script))
        .current_dir(cwd)
        .env("PATH", path_with_bin())
        .env("TMPDIR", &markers)
        .env("DOCSYS_TODAY", "2026-10-02")
        .env("CLAUDE_PROJECT_DIR", repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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

/// The measurement from a real repository, as a test: every relay run from a
/// package directory behaves as it does from the top.
#[test]
fn every_relay_works_from_a_subdirectory() {
    let repo = project("relays");
    let deep = repo.join("apps/x/src");
    let page = repo.join("docs/reference/token-ttl.md");
    let cwd = |d: &Path| d.display().to_string();

    // the commit gate: nothing staged but a page, so it passes — from below too
    fs::write(
        &page,
        fs::read_to_string(&page).unwrap() + "\nOne more sentence.\n",
    )
    .unwrap();
    ok(&repo, &["add", "docs"]);
    let commit = |d: &Path| {
        format!(
            r#"{{"cwd":"{}","tool_name":"Bash","tool_input":{{"command":"git commit -m x"}}}}"#,
            cwd(d)
        )
    };
    let top = relay(&repo, "pre-commit-docs.sh", &repo, &commit(&repo));
    let below = relay(&repo, "pre-commit-docs.sh", &deep, &commit(&deep));
    assert_eq!(top.0, 0, "{}", top.2);
    assert_eq!(
        below.0, 0,
        "the gate blocked from a subdirectory: {}",
        below.2
    );
    ok(&repo, &["commit", "-qm", "page"]);

    // a docsys/0.5 tree runs no post-edit relay: its date and its
    // verification are history's (D-126)
    assert!(!repo.join(".claude/hooks/post-edit-updated.sh").exists());

    // the first-turn digest counts the tree's pages
    let prompt = format!(
        r#"{{"cwd":"{}","session_id":"s-{}","prompt":"hi"}}"#,
        cwd(&deep),
        std::process::id()
    );
    let (code, out, err) = relay(&repo, "session-intent.sh", &deep, &prompt);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("1 permanent page(s)"),
        "digest from below: {out}"
    );

    // the end-of-turn reminder sees the page change as documentation
    fs::write(repo.join("src/auth.rs"), AUTH_RS.replace("1\n", "2\n")).unwrap();
    fs::write(
        &page,
        fs::read_to_string(&page).unwrap() + "\nA second sentence.\n",
    )
    .unwrap();
    let stop = format!(r#"{{"cwd":"{}","stop_hook_active":false}}"#, cwd(&deep));
    let (_, _, below_msg) = relay(&repo, "stop-docs-reminder.sh", &deep, &stop);
    let (_, _, top_msg) = relay(&repo, "stop-docs-reminder.sh", &repo, &stop);
    assert_eq!(below_msg, top_msg);
    assert!(
        !below_msg.contains("no documentation"),
        "a page change read as none: {below_msg}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// git exports GIT_DIR to the hooks of a linked worktree. The pre-commit gate
/// must still find the repository's top, resolve the pins there, pass a clean
/// commit — and stop the commit whose pinned code moved.
#[test]
fn the_git_gate_in_a_worktree_resolves_pins_at_the_top() {
    let repo = project("worktree");
    let wt = repo.with_file_name(format!(
        "{}-wt",
        repo.file_name().unwrap().to_string_lossy()
    ));
    let _ = fs::remove_dir_all(&wt);
    ok(&repo, &["worktree", "add", "-q", wt.to_str().unwrap()]);

    // a clean commit from a package directory of the worktree: the gate runs
    // with GIT_DIR exported and passes
    fs::write(wt.join("apps/x/src/main.rs"), "fn main() { let _ = 1; }\n").unwrap();
    fs::write(
        wt.join("docs/reference/main.md"),
        "---\nid: main\ntype: reference\n---\nThis page states what main does; read it before changing it.\n",
    )
    .unwrap();
    ok(&wt.join("apps/x"), &["add", "-A", "."]);
    ok(&wt.join("apps/x"), &["add", "../../docs"]);
    let out = git(
        &wt.join("apps/x"),
        &["commit", "-m", "touch main with a journal line"],
    );
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "a clean commit was stopped: {all}");
    assert!(
        !all.contains("R-111"),
        "pins read as moved under GIT_DIR: {all}"
    );

    // the check seen failing: the pinned region moves, the gate stops it
    fs::write(
        wt.join("src/auth.rs"),
        AUTH_RS.replace("ttl / 2", "ttl / 3"),
    )
    .unwrap();
    ok(&wt, &["add", "src/auth.rs"]);
    let out = git(&wt, &["commit", "-m", "move the pinned region"]);
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!out.status.success(), "a moved pin passed the gate: {all}");
    assert!(
        all.contains("ERROR R-111 reference/token-ttl.md [src/auth.rs#refresh_token]"),
        "{all}"
    );
    let _ = fs::remove_dir_all(&wt);
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn in_a_monorepo_the_nearest_tree_answers() {
    let repo = project("mono");
    fs::create_dir_all(repo.join("apps/y/src")).unwrap();
    let (code, out) = docsys(&repo.join("apps/y"), &["init", "--root", "docs"]);
    assert_eq!(code, 0, "{out}");
    fs::create_dir_all(repo.join("apps/y/docs/reference")).unwrap();
    fs::write(
        repo.join("apps/y/docs/reference/only-here.md"),
        "---\nid: only-here\ntype: reference\nupdated: 2026-10-02\n---\n\n# Only here\n\nSee [[reference/ghost]].\n",
    )
    .unwrap();
    let (_, below) = docsys(&repo.join("apps/y/src"), &["lint"]);
    assert!(below.contains("reference/only-here.md"), "{below}");
    let (_, top) = docsys(&repo, &["lint"]);
    assert!(!top.contains("only-here"), "{top}");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_tree_above_the_repository_is_never_adopted() {
    let outer = tmp("stray");
    fs::create_dir_all(outer.join("docs")).unwrap();
    fs::write(
        outer.join("docs/.docmeta.yml"),
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n",
    )
    .unwrap();
    let inner = outer.join("inner");
    fs::create_dir_all(inner.join("src")).unwrap();
    ok(&inner, &["init", "-q"]);
    let (code, out) = docsys(&inner.join("src"), &["lint"]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("R-160"), "{out}");
    let _ = fs::remove_dir_all(&outer);
}

fn docsys_all(dir: &Path, args: &[&str]) -> (i32, String) {
    let out = Command::new(bin())
        .args(args)
        .current_dir(dir)
        .env("DOCSYS_TODAY", "2026-10-02")
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .env_remove("CLAUDE_PROJECT_DIR")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr),
    )
}

/// A repository whose one tree is not `docs/`: from its top, a command with
/// no `--root` works on that tree, and `docsys agents` writes relays for it.
/// Where no tree exists, or several and none above, nothing is guessed.
#[test]
fn a_repository_with_one_tree_is_that_trees_from_its_top() {
    let repo = tmp("one-tree");
    ok(&repo, &["init", "-q"]);
    ok(&repo, &["config", "user.email", "t@example.invalid"]);
    ok(&repo, &["config", "user.name", "t"]);
    let (code, out) = docsys_all(&repo, &["adopt", "--root", "documentation"]);
    assert_eq!(code, 0, "{out}");
    let (code, out) = docsys_all(&repo, &["lint"]);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("R-160"), "{out}");
    let (code, out) = docsys_all(
        &repo,
        &["question", "add", "who owns it?", "--topic", "cart"],
    );
    assert_eq!(code, 0, "{out}");
    assert!(
        repo.join("documentation/work/questions/cart.md").is_file(),
        "{out}"
    );
    assert!(!repo.join("docs").exists(), "no second tree: {out}");
    // the relays `agents` writes name that tree
    fs::remove_dir_all(repo.join(".claude/hooks")).unwrap();
    let (code, out) = docsys_all(&repo, &["agents"]);
    assert_eq!(code, 0, "{out}");
    let relay = fs::read_to_string(repo.join(".claude/hooks/pre-commit-docs.sh")).unwrap();
    assert!(relay.contains("DOCS_ROOT:-documentation}"), "{relay}");
    assert!(!relay.contains("DOCS_ROOT:-docs}"), "{relay}");
    let _ = fs::remove_dir_all(&repo);
}

/// A command that writes into a tree refuses where there is none (R-160):
/// it never makes a second, half tree.
#[test]
fn a_write_command_outside_a_tree_refuses_with_r160() {
    let repo = tmp("no-tree");
    ok(&repo, &["init", "-q"]);
    for args in [
        &["question", "add", "who?"][..],
        &["debt", "add", "x", "--deferred", "y", "--repay-when", "z"],
        &["page", "new", "reference", "cart"],
        &["journal", "add", "a line"],
        &["debt", "close", "1", "--note", "n"],
        &["question", "close", "1", "--answer", "x"],
    ] {
        let (code, out) = docsys_all(&repo, args);
        assert_eq!(code, 2, "{args:?}: {out}");
        assert!(out.contains("R-160"), "{args:?}: {out}");
    }
    assert!(!repo.join("docs").exists(), "nothing written");
    // two trees and none above: no guess
    for t in ["a/docs", "b/docs"] {
        let (code, out) = docsys_all(&repo, &["init", "--root", t]);
        assert_eq!(code, 0, "{out}");
    }
    let (code, out) = docsys_all(&repo, &["lint"]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("R-160"), "{out}");
    let _ = fs::remove_dir_all(&repo);
}

/// `agents` from a subdirectory installs into the agent layer `adopt` wrote at
/// the repository's top — never a second one where it stands — and only the
/// relays the tree's era runs (D-098, D-126).
#[test]
fn agents_from_a_subdirectory_writes_the_repositorys_own_layer() {
    let repo = project("agents-sub");
    let deep = repo.join("apps/x/src");
    for args in [&["agents"][..], &["agents", "--force"]] {
        let (code, out) = docsys(&deep, args);
        assert_eq!(code, 0, "{args:?}: {out}");
        assert!(!deep.join(".claude").exists(), "{args:?}: {out}");
        assert!(!out.contains("post-edit"), "{args:?}: {out}");
    }
    assert!(!repo.join(".claude/hooks/post-edit-updated.sh").exists());
    let _ = fs::remove_dir_all(&repo);
}

/// Every command that reads the repository reads all of it from a
/// subdirectory (D-098): a given `--repo` is its top level, and the agent
/// layer `--dir` names is the one at the top. The same command from the top
/// and from below says the same and writes the same place.
#[test]
fn a_command_that_reads_the_repository_sees_it_all_from_below() {
    let repo = project("repo-from-below");
    let deep = repo.join("apps/x/src");
    // a code citation outside the subdirectory: only a walk from the top sees it
    fs::write(
        repo.join("src/cite.rs"),
        "// doc: token-ttl\npub fn cite() {}\n",
    )
    .unwrap();
    fs::create_dir_all(repo.join("docs/howto")).unwrap();
    fs::write(
        repo.join("docs/howto/release.md"),
        "---\nid: release\ntype: howto\n---\n# Release\n\nThis page lists the release steps; read it before tagging.\n\n1. Tag the version.\n2. Push the tag.\n",
    )
    .unwrap();
    ok(&repo, &["add", "-A"]);
    ok(&repo, &["commit", "-qm", "a citation and a howto"]);
    // bare, the repository is the tree's own, as help says
    for args in [
        &["backlinks", "token-ttl", "--repo", "."][..],
        &["backlinks", "token-ttl"],
        &["graph", "--repo", "."],
        &["graph"],
        &["mentions", "token-ttl"],
    ] {
        let top = docsys_all(&repo, args);
        assert_eq!(top, docsys_all(&deep, args), "{args:?}");
        if args.first() != Some(&"mentions") {
            assert!(top.1.contains("src/cite.rs"), "{args:?}: {}", top.1);
        }
    }
    // the rules block carries the tree's own preamble, written from anywhere
    let meta = repo.join("docs/.docmeta.yml");
    let text = fs::read_to_string(&meta).unwrap();
    fs::write(
        &meta,
        format!("{text}generated_preamble: \"Owned by the platform team.\"\n"),
    )
    .unwrap();
    let agents_md = repo.join("AGENTS.md");
    let target = agents_md.to_str().unwrap();
    let block = |dir: &Path| {
        let _ = fs::remove_file(&agents_md);
        let (code, out) = docsys_all(dir, &["rules", "--agents-md", "--write", target]);
        assert_eq!(code, 0, "{out}");
        fs::read_to_string(&agents_md).unwrap()
    };
    let top = block(&repo);
    assert!(top.contains("Owned by the platform team."), "{top}");
    assert_eq!(block(&deep), top);
    // a compiled skill lands in the agent layer at the top
    let (code, out) = docsys_all(&deep, &["compile", "howto/release"]);
    assert_eq!(code, 0, "{out}");
    assert!(!deep.join(".claude").exists(), "{out}");
    assert!(
        repo.join(".claude/skills/release/SKILL.md").is_file(),
        "{out}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// `migrate inventory --repo .` reports every inbound reference of the
/// repository, from wherever it runs (D-098).
#[test]
fn a_migration_sees_the_repositorys_references_from_below() {
    let repo = tmp("migrate-below");
    ok(&repo, &["init", "-q"]);
    fs::create_dir_all(repo.join("pkg/legacy")).unwrap();
    fs::write(repo.join("pkg/legacy/setup.md"), "# Setup\n\nInstall it.\n").unwrap();
    fs::write(repo.join("README.md"), "See pkg/legacy/setup.md.\n").unwrap();
    fs::write(repo.join("pkg/NOTES.md"), "See legacy/setup.md.\n").unwrap();
    let inbound = |dir: &Path, root: &str| -> Vec<String> {
        let (code, out) = docsys_all(
            dir,
            &["migrate", "inventory", "--root", root, "--repo", "."],
        );
        assert_eq!(code, 0, "{out}");
        out.lines()
            .filter(|l| l.starts_with("# inbound:"))
            .map(str::to_string)
            .collect()
    };
    // `--root` names a directory from the repository's top, from anywhere
    let top = inbound(&repo, "pkg/legacy");
    assert!(top.iter().any(|l| l.contains("README.md")), "{top:?}");
    assert_eq!(inbound(&repo.join("pkg"), "pkg/legacy"), top);
    let _ = fs::remove_dir_all(&repo);
}

/// `migrate inventory` of an existing tree from a subdirectory reads the tree
/// it reads from the top (D-098).
#[test]
fn a_migration_inventory_finds_the_tree_from_below() {
    let repo = project("migrate-tree-below");
    let deep = repo.join("apps/x/src");
    for args in [
        &["migrate", "inventory"][..],
        &["migrate", "inventory", "--repo", ".", "--root", "docs"],
    ] {
        let top = docsys_all(&repo, args);
        assert_eq!(top.0, 0, "{args:?}: {}", top.1);
        assert_eq!(docsys_all(&deep, args), top, "{args:?}");
    }
    let _ = fs::remove_dir_all(&repo);
}

/// `agents --kb` from a subdirectory of a knowledge base writes the relays
/// `agents --kb` writes at its top: the base is the repository's one tree.
#[test]
fn agents_for_a_knowledge_base_from_below_writes_what_it_writes_at_the_top() {
    let repo = tmp("kb-agents-below");
    ok(&repo, &["init", "-q"]);
    let (code, out) = docsys_all(
        &repo,
        &["init", "--profile", "knowledge-base", "--root", "."],
    );
    assert_eq!(code, 0, "{out}");
    let (code, out) = docsys_all(&repo, &["agents", "--kb"]);
    assert_eq!(code, 0, "{out}");
    let relays = |r: &Path| -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = fs::read_dir(r.join(".claude/hooks"))
            .unwrap()
            .flatten()
            .map(|e| {
                (
                    e.file_name().to_string_lossy().into_owned(),
                    fs::read_to_string(e.path()).unwrap(),
                )
            })
            .collect();
        v.sort();
        v
    };
    let top = relays(&repo);
    let below = repo.join("wiki/ops");
    fs::create_dir_all(&below).unwrap();
    let (code, out) = docsys_all(&below, &["agents", "--kb", "--force"]);
    assert_eq!(code, 0, "{out}");
    assert!(!below.join(".claude").exists(), "{out}");
    assert_eq!(relays(&repo), top, "{out}");
    // the README's form names the base with `--root .`: the tree from the top
    let agents_md = fs::read_to_string(repo.join("AGENTS.md")).unwrap_or_default();
    for args in [
        &["agents", "--kb", "--root", ".", "--force"][..],
        &[
            "agents", "--kb", "--root", ".", "--force", "--dir", ".claude",
        ],
        &["status", "--root", "."],
    ] {
        let at_top = docsys_all(&repo, args);
        assert_eq!(docsys_all(&below, args), at_top, "{args:?}");
        assert_eq!(relays(&repo), top, "{args:?}");
        assert!(!below.join(".claude").exists(), "{args:?}");
        assert!(!below.join("AGENTS.md").exists(), "{args:?}");
    }
    assert_eq!(
        fs::read_to_string(repo.join("AGENTS.md")).unwrap_or_default(),
        agents_md
    );
    let _ = fs::remove_dir_all(&repo);
}

/// The agent layer is the repository's, at its top (D-098): `agents` in every
/// form — a given relative `--dir` too — and `compile` write there from a
/// subdirectory, read there for `--report`, never write a retired relay on a
/// docsys/0.5 tree, and name what they wrote from the top, so the same
/// command prints the same from anywhere.
#[test]
fn the_agent_layer_is_written_and_named_from_the_top() {
    let repo = project("layer-from-below");
    let deep = repo.join("apps/x/src");
    fs::write(repo.join(".claude/commands/own.md"), "own\n").unwrap();
    fs::create_dir_all(repo.join("docs/howto")).unwrap();
    fs::write(
        repo.join("docs/howto/release.md"),
        "---\nid: release\ntype: howto\n---\n# Release\n\nThis page lists the release steps; read it before tagging.\n\n1. Tag the version.\n2. Push the tag.\n",
    )
    .unwrap();
    for args in [
        &["agents"][..],
        &["agents", "--dir", ".claude"],
        &["agents", "--force"],
        &["agents", "--force", "--dir", ".claude"],
        &["agents", "--report"],
        &["agents", "--report", "--dir", ".claude"],
        &["compile", "howto/release", "--force"],
        &["compile", "howto/release", "--force", "--dir", ".claude"],
    ] {
        let top = docsys_all(&repo, args);
        assert_eq!(top.0, 0, "{args:?}: {}", top.1);
        assert_eq!(docsys_all(&deep, args), top, "{args:?}");
        assert!(!deep.join(".claude").exists(), "{args:?}");
        assert!(
            !repo.join(".claude/hooks/post-edit-updated.sh").exists(),
            "{args:?}"
        );
    }
    let _ = fs::remove_dir_all(&repo);
}
