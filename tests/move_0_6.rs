#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! The docsys/0.5 → docsys/0.6 move and the clones it reaches (D-136, D-138):
//! the workflow it writes, the hook it leaves on every branch, the installer
//! it names, and a re-run that has nothing to read. Every repository is a
//! temporary one, driven through the binary and its git hook.

use docsys::workflow::{self, Ci, Reinstall, Workflow};
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
    let dir = std::env::temp_dir().join(format!("docsys-review-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn release(name: &str) -> String {
    let dir = tmp(&format!("release-{name}"));
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

fn real_name(rel: &str) -> String {
    match rel.strip_prefix("dot-") {
        Some(rest) => format!(".{rest}"),
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

/// The 0.5-to-0.6 case's `before/`, committed, its tracked gate configured;
/// `edit` changes files before the commit.
fn build(name: &str, edit: &dyn Fn(&Path)) -> PathBuf {
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
    edit(&repo);
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

/// A `docsys` on the PATH that answers `--version` as the pinned version and
/// fails every check:
/// what the git block does with a failing gate, read as shell.
fn failing_docsys(name: &str) -> PathBuf {
    let dir = tmp(&format!("stub-{name}"));
    let stub = dir.join("docsys");
    fs::write(
        &stub,
        format!(
            "#!/bin/sh\ncase \"$1\" in --version) echo \"docsys {} (spec docsys/0.6), a stub\"; exit 0 ;; esac\necho \"stub docsys $*: an error\" >&2\nexit 1\n",
            env!("CARGO_PKG_VERSION")
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

fn adopted_0_6(name: &str) -> PathBuf {
    let repo = tmp(name);
    git(&repo, &["init", "-q", "-b", "main"]);
    fs::create_dir_all(repo.join(".github")).unwrap();
    docsys::adopt::run(&repo, &repo.join("docs"), "en").unwrap();
    repo
}

fn set_spec(repo: &Path, spec: Option<&str>) {
    let dm = repo.join("docs/.docmeta.yml");
    let text = fs::read_to_string(&dm).unwrap();
    let kept: Vec<&str> = text.lines().filter(|l| !l.starts_with("spec:")).collect();
    let mut out = String::new();
    if let Some(s) = spec {
        out.push_str(&format!("spec: {s}\n"));
    }
    out.push_str(&kept.join("\n"));
    out.push('\n');
    fs::write(dm, out).unwrap();
}

// ---------------------------------------------------------------------------
// 1. The default (cargo) install on a docsys/0.6 tree: README and D-136 say
//    the pull request is checked by `gate --range` alone, push runs `lint`
//    and `refs`, and a merge queue is checked.

#[test]
fn adopt_on_a_0_6_tree_with_the_default_install_writes_the_0_6_workflow() {
    let repo = adopted_0_6("adopt-cargo");
    let dm = fs::read_to_string(repo.join("docs/.docmeta.yml")).unwrap();
    assert!(dm.contains("spec: docsys/0.6"), "{dm}");
    let wf = fs::read_to_string(repo.join(".github/workflows/docsys.yml")).unwrap();
    assert!(wf.contains("install=cargo"), "{wf}");
    for line in [
        "  merge_group:\n",
        "      - if: github.event_name == 'push'\n        run: docsys lint --repo . --root docs\n",
        "      - if: github.event_name == 'push'\n        run: docsys refs --repo . --root docs\n",
    ] {
        assert!(wf.contains(line), "`{line}` not in the 0.6 workflow:\n{wf}");
    }
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn an_untouched_cargo_workflow_moves_to_the_0_6_shape() {
    let releases = release("untouched-cargo");
    let stamped = workflow::render(&Workflow {
        version: "0.17.0".to_string(),
        branch: "main".to_string(),
        root: "docs".to_string(),
        ci: Ci::default(),
    });
    let repo = build("untouched-cargo", &|repo| {
        fs::write(repo.join(".github/workflows/docsys.yml"), &stamped).unwrap();
    });
    let out = run(
        &repo,
        &bin(),
        &["upgrade", "--apply", "--commit"],
        &path(),
        &releases,
    );
    assert!(out.status.success(), "{}", text(&out));
    let dm = fs::read_to_string(repo.join("docs/.docmeta.yml")).unwrap();
    assert!(dm.starts_with("spec: docsys/0.6\n"), "{dm}");
    let wf = fs::read_to_string(repo.join(".github/workflows/docsys.yml")).unwrap();
    for line in [
        "  merge_group:\n",
        "      - if: github.event_name == 'push'\n        run: docsys lint --repo . --root docs\n",
    ] {
        assert!(
            wf.contains(line),
            "`{line}` not in the moved workflow:\n{wf}\n--- upgrade said:\n{}",
            text(&out)
        );
    }
    let _ = fs::remove_dir_all(&repo);
}

// ---------------------------------------------------------------------------
// 2. D-138: "of an owner's workflow the move replaces a release install with
//    the step that runs the installer". A cargo install is the owner's
//    choice (a runner with a toolchain, no release archive for its target):
//    it stays a cargo install, as an untouched cargo workflow does.

const OWNER_CARGO: &str = "name: docs\n\non:\n  push:\n    branches: [main]\n  pull_request:\n\njobs:\n  docs:\n    runs-on: [self-hosted, linux, s390x]\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          fetch-depth: 0\n      - name: docsys\n        run: cargo install docsys --version 0.17.0 --locked\n      - if: github.event_name == 'push'\n        run: docsys lint --repo . --root docs\n      - if: github.event_name == 'pull_request'\n        run: docsys gate --repo . --root docs --range \"origin/${{ github.base_ref }}...HEAD\"\n";

#[test]
fn an_owners_cargo_install_is_not_replaced_by_the_tree_installer() {
    let own = env!("CARGO_PKG_VERSION");
    match workflow::reinstall_tree(OWNER_CARGO, "docs", own) {
        Reinstall::Rewritten { text, what } => {
            assert!(
                !text.contains(".docsys-install") && text.contains("cargo install docsys"),
                "the owner's cargo install became a release download:\n{what}\n{}",
                docsys::diff::unified(OWNER_CARGO, &text, "owner", "moved", 2)
            );
        }
        other => panic!("{other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 3. scope_owner: a block `on:` written as `"on":` (yamllint's `truthy` rule
//    asks for it) is a block, not "triggers written on one line".

const OWNER_QUOTED_ON: &str = "name: docs\n\n\"on\":\n  push:\n  pull_request:\n\njobs:\n  docs:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: docsys lint --repo . --root docs\n      - if: github.event_name == 'pull_request'\n        run: docsys gate --repo . --root docs --range \"origin/${{ github.base_ref }}...HEAD\"\n";

#[test]
fn a_quoted_on_block_is_scoped_not_called_one_line() {
    match workflow::scope_owner(OWNER_QUOTED_ON, "docs", "main") {
        Ok(Some((new, _))) => {
            assert!(new.contains("  merge_group:\n"), "{new}");
            assert!(new.contains("  push:\n    branches: [main]\n"), "{new}");
        }
        other => panic!("a block `\"on\":` was not scoped: {other:?}"),
    }
}

// 4. scope_owner: triggers written as a block sequence (`- push`) must not
//    get a mapping key appended inside the sequence — that is no YAML.

const OWNER_SEQUENCE_ON: &str = "name: docs\n\non:\n  - push\n  - pull_request\n\njobs:\n  docs:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - if: github.event_name == 'push'\n        run: docsys lint --repo . --root docs\n      - if: github.event_name == 'pull_request'\n        run: docsys gate --repo . --root docs --range \"origin/${{ github.base_ref }}...HEAD\"\n";

#[test]
fn triggers_written_as_a_list_are_named_and_not_written() {
    let said = workflow::scope_owner(OWNER_SEQUENCE_ON, "docs", "main");
    assert!(
        matches!(&said, Err((3, why)) if why.contains("written as a list")),
        "{said:?}"
    );
}

// ---------------------------------------------------------------------------
// 5. A second run of a moved tree changes nothing (R-176, "A second run
//    changes nothing"): with the release unreachable — offline, no curl —
//    the installer that already holds this version needs nothing read.

#[test]
fn a_moved_tree_rerun_offline_has_no_leftover() {
    let releases = release("offline");
    let repo = build("offline", &|_| {});
    let out = run(
        &repo,
        &bin(),
        &["upgrade", "--apply", "--commit"],
        &path(),
        &releases,
    );
    assert!(out.status.success(), "{}", text(&out));
    assert!(repo.join("docs/.docsys-install").is_file());
    let online = run(&repo, &bin(), &["upgrade"], &path(), &releases);
    let said_online = String::from_utf8_lossy(&online.stdout).to_string();
    assert!(said_online.contains("no leftover"), "{said_online}");
    let gone = format!("file://{}/no-release-here", tmp("offline-none").display());
    let offline = run(&repo, &bin(), &["upgrade"], &path(), &gone);
    let said = String::from_utf8_lossy(&offline.stdout).to_string();
    assert!(
        said.contains("no leftover"),
        "online: {said_online}\noffline: {said}"
    );
    let _ = fs::remove_dir_all(&repo);
}

// ---------------------------------------------------------------------------
// 6. D-136 / CHANGELOG 0.18.0: "On a branch whose tree still declares an
//    older spec it [the docsys/0.6 git gate] warns." The clone's hook serves
//    every branch. Read as shell, with a docsys whose checks fail.

#[test]
fn the_0_6_commit_msg_half_warns_on_an_older_spec_branch() {
    let repo = adopted_0_6("msg-old-branch");
    set_spec(&repo, Some("docsys/0.5"));
    let stub = failing_docsys("msg");
    let p = format!("{}:/usr/bin:/bin", stub.display());
    let pre = run(&repo, Path::new("bash"), &[".git/hooks/pre-commit"], &p, "");
    assert_eq!(pre.status.code(), Some(0), "pre-commit: {}", text(&pre));
    fs::write(repo.join(".git/COMMIT_EDITMSG"), "add code\n").unwrap();
    let msg = run(
        &repo,
        Path::new("bash"),
        &[".git/hooks/commit-msg", ".git/COMMIT_EDITMSG"],
        &p,
        "",
    );
    assert_eq!(
        msg.status.code(),
        Some(0),
        "the pre-commit half warns on a docsys/0.5 branch, the commit-msg half blocks: {}",
        text(&msg)
    );
    let _ = fs::remove_dir_all(&repo);
}

// 7. A tree whose spec the binary reads as older than docsys/0.6 is not
//    scoped (D-118: no spec is docsys/0.4; a trailing ` # comment` is no
//    part of a value, D-002), so the docsys/0.6 block warns there too.

#[test]
fn the_0_6_pre_commit_half_warns_on_every_tree_read_below_0_6() {
    for (case, spec) in [
        (
            "commented",
            Some("docsys/0.5  # moves to 0.6 with the team"),
        ),
        ("no-spec", None),
    ] {
        let repo = adopted_0_6(case);
        set_spec(&repo, spec);
        let era = docsys::era::Era::at(&repo.join("docs"));
        assert!(!era.scoped_gate(), "{case}: {era:?}");
        let stub = failing_docsys(case);
        let p = format!("{}:/usr/bin:/bin", stub.display());
        let pre = run(&repo, Path::new("bash"), &[".git/hooks/pre-commit"], &p, "");
        assert_eq!(
            pre.status.code(),
            Some(0),
            "{case}: the tree is served by docsys/0.{} (not scoped), and the docsys/0.6 block blocks there: {}",
            era.0,
            text(&pre)
        );
        let _ = fs::remove_dir_all(&repo);
    }
}

// 8. The docsys/0.6 block runs `gate` alone because on a docsys/0.6 tree
//    `gate` runs `refs` (D-136). On a branch at an older spec the block
//    "warns" — and the 0.5 block it replaced reported `refs` there. A commit
//    that adds a dangling citation on such a branch must at least be told.

#[test]
fn the_0_6_block_reports_refs_on_an_older_spec_branch() {
    let repo = adopted_0_6("refs-old-branch");
    set_spec(&repo, Some("docsys/0.5"));
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(
        repo.join("src/c.ts"),
        "export const c = 3; // doc: phantom\n",
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    let refs = run(
        &repo,
        &bin(),
        &["refs", "--repo", ".", "--root", "docs"],
        &path(),
        "",
    );
    assert!(
        text(&refs).contains("phantom"),
        "refs itself: {}",
        text(&refs)
    );
    let pre = run(
        &repo,
        Path::new("bash"),
        &[".git/hooks/pre-commit"],
        &path(),
        "",
    );
    assert_eq!(pre.status.code(), Some(0), "{}", text(&pre));
    assert!(
        text(&pre).contains("phantom"),
        "the dangling `doc: phantom` the commit adds is not even reported on a docsys/0.5 branch:\n{}",
        text(&pre)
    );
    let _ = fs::remove_dir_all(&repo);
}

// ---------------------------------------------------------------------------
// 9. D-137 turns inline code into a citation: a Markdown file outside the
//    tree that showed `doc: <id>` as an example now cites it. A citation of
//    no page is an error the move's commit would add: the plan lists it for a
//    person, `--apply` writes nothing until it is fixed, and then the move is
//    one commit.

#[test]
fn a_citation_of_no_page_the_move_creates_waits_for_a_person() {
    let releases = release("new-citation");
    let repo = build("new-citation", &|repo| {
        fs::write(
            repo.join("README.md"),
            "# team service\n\nA page is cited from code as `doc: some-page`.\n",
        )
        .unwrap();
    });
    let plan = run(&repo, &bin(), &["upgrade"], &path(), &releases);
    let plan_text = String::from_utf8_lossy(&plan.stdout).to_string();
    assert!(
        plan_text.lines().any(|l| l.starts_with("manual")
            && l.contains("README.md:3")
            && l.contains("some-page")),
        "{plan_text}"
    );
    assert!(
        plan_text.contains("`--apply` writes nothing until each is fixed"),
        "{plan_text}"
    );
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let out = run(
        &repo,
        &bin(),
        &["upgrade", "--apply", "--commit"],
        &path(),
        &releases,
    );
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(
        text(&out).contains("README.md:3: `doc: some-page`"),
        "{}",
        text(&out)
    );
    assert_eq!(
        git(&repo, &["status", "--porcelain"]),
        "",
        "the refused move wrote"
    );
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    // fixed and committed, the move is one commit
    fs::write(
        repo.join("README.md"),
        "# team service\n\nA page is cited from code.\n",
    )
    .unwrap();
    git(
        &repo,
        &["-c", "core.hooksPath=/dev/null", "commit", "-qam", "readme"],
    );
    let fixed = git(&repo, &["rev-parse", "HEAD"]);
    let out = run(
        &repo,
        &bin(),
        &["upgrade", "--apply", "--commit"],
        &path(),
        &releases,
    );
    assert!(out.status.success(), "{}", text(&out));
    assert_eq!(
        git(&repo, &["rev-list", "--count", &format!("{fixed}..HEAD")]),
        "1"
    );
    let _ = fs::remove_dir_all(&repo);
}

// 10. The retired-concepts row names the version that retired the concept:
//     warn-mode is retired by docsys/0.6 (migrations/0.5-0.6-retired.tsv).

#[test]
fn a_concept_the_0_6_move_retires_is_named_as_0_6s() {
    let releases = release("retired");
    let repo = build("retired", &|repo| {
        fs::write(
            repo.join("CLAUDE.md"),
            "# Notes\n\nOur git gate runs in warn-mode until the debt is paid.\n",
        )
        .unwrap();
    });
    let plan = run(&repo, &bin(), &["upgrade"], &path(), &releases);
    let said = String::from_utf8_lossy(&plan.stdout).to_string();
    let row = said
        .lines()
        .find(|l| l.contains("retired-concepts") && l.contains("CLAUDE.md:3"))
        .unwrap_or_else(|| panic!("no row:\n{said}"))
        .to_string();
    assert!(
        row.contains("docsys/0.6 retired"),
        "the 0.6 move's concept is said to be retired by docsys/0.5: {row}"
    );
    let _ = fs::remove_dir_all(&repo);
}

// 11. An owner's hook that starts `set -e` gets the block right below it
//     (ensure_block). On a branch whose tree is as 0.15 left it — docsys/0.4,
//     no `.docsys-version`, no `.docsys-install` — a machine with no docsys
//     is to be told how to install it and the commit goes on (D-138).

#[test]
fn the_0_6_block_under_set_e_lets_a_commit_go_on_with_no_docsys() {
    let repo = tmp("set-e");
    git(&repo, &["init", "-q", "-b", "main"]);
    fs::create_dir_all(repo.join(".git/hooks")).unwrap();
    fs::write(
        repo.join(".git/hooks/pre-commit"),
        "#!/bin/sh\nset -e\necho \"the owner's own check\"\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(
        repo.join(".git/hooks/pre-commit"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    docsys::adopt::run(&repo, &repo.join("docs"), "en").unwrap();
    let hook = fs::read_to_string(repo.join(".git/hooks/pre-commit")).unwrap();
    assert!(
        hook.starts_with("#!/bin/sh\nset -e\n\n# --- docsys documentation gate"),
        "{hook}"
    );
    set_spec(&repo, Some("docsys/0.4"));
    fs::remove_file(repo.join("docs/.docsys-version")).unwrap();
    for sh in ["/bin/sh", "/bin/dash", "/bin/bash"] {
        if !Path::new(sh).exists() {
            continue;
        }
        let out = run(
            &repo,
            Path::new(sh),
            &[".git/hooks/pre-commit"],
            "/usr/bin:/bin",
            "",
        );
        assert_eq!(
            out.status.code(),
            Some(0),
            "{sh}: the commit is stopped, and nothing says why: {:?}",
            text(&out)
        );
    }
    let _ = fs::remove_dir_all(&repo);
}

// 5b. The same for a workflow `adopt --ci-install release` wrote on a
//     docsys/0.6 tree, untouched: nothing to change, nothing to read.

#[test]
fn an_untouched_tree_install_rerun_offline_has_no_leftover() {
    let releases = release("offline-untouched");
    let repo = tmp("offline-untouched");
    git(&repo, &["init", "-q", "-b", "main"]);
    fs::create_dir_all(repo.join(".github")).unwrap();
    let adopt = run(
        &repo,
        &bin(),
        &["adopt", "--ci-install", "release"],
        &path(),
        &releases,
    );
    assert!(adopt.status.success(), "{}", text(&adopt));
    let wf = fs::read_to_string(repo.join(".github/workflows/docsys.yml")).unwrap();
    assert!(wf.contains("install=tree"), "{wf}");
    assert!(repo.join("docs/.docsys-install").is_file());
    git(&repo, &["add", "-A"]);
    git(
        &repo,
        &["-c", "core.hooksPath=/dev/null", "commit", "-qm", "adopt"],
    );
    let online = run(&repo, &bin(), &["upgrade"], &path(), &releases);
    let said_online = String::from_utf8_lossy(&online.stdout).to_string();
    assert!(said_online.contains("no leftover"), "{said_online}");
    let gone = format!(
        "file://{}/no-release-here",
        tmp("offline-untouched-none").display()
    );
    let offline = run(&repo, &bin(), &["upgrade"], &path(), &gone);
    let said = String::from_utf8_lossy(&offline.stdout).to_string();
    assert!(said.contains("no leftover"), "offline: {said}");
    let _ = fs::remove_dir_all(&repo);
}

// 12. D-120 as 0.18.0 amends it: "A docsys/0.6 tree names its installer,
//     `.docsys-install`, as the install command (D-138)." A pinned version
//     this machine lacks is named by dispatch — on a machine the installer
//     set up there is no cargo, and the tree holds the installer of its pin.

#[test]
fn dispatch_names_the_trees_installer_for_a_missing_pin() {
    let repo = adopted_0_6("dispatch-installer");
    let sums: Vec<(String, String)> = TARGETS
        .iter()
        .map(|t| (t.to_string(), "a".repeat(64)))
        .collect();
    fs::write(repo.join("docs/.docsys-version"), "9.9.9\n").unwrap();
    fs::write(
        repo.join("docs/.docsys-install"),
        workflow::installer_text("9.9.9", &sums),
    )
    .unwrap();
    let home = tmp("dispatch-installer-home");
    let out = Command::new(bin())
        .args(["lint"])
        .current_dir(&repo)
        .env("DOCSYS_HOME", &home)
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .env_remove("DOCSYS_DISPATCHED")
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(1), "{said}");
    assert!(
        said.contains("docs/.docsys-install"),
        "a docsys/0.6 tree with its installer is told to install with cargo: {said}"
    );
    let _ = fs::remove_dir_all(&repo);
}

// ---------------------------------------------------------------------------
// 13. D-136: a clone's hook serves every branch. On a branch whose tree still
//     declares docsys/0.5 the gate is not scoped, and it keeps the mode this
//     clone's block had before the move: a hard block stays hard there, a
//     warn-mode one warns.

#[test]
fn a_branch_at_an_older_spec_keeps_the_mode_the_clone_had_before_the_move() {
    for hard in [true, false] {
        let name = if hard { "older-hard" } else { "older-warn" };
        let releases = release(name);
        let repo = build(name, &|repo| {
            if hard {
                for (hook, from, to) in [
                    ("pre-commit", "docsys_gate_exit=0", "docsys_gate_exit=1"),
                    ("commit-msg", "[ \"0\" -eq 0 ]", "[ \"1\" -eq 0 ]"),
                ] {
                    let p = repo.join(".githooks").join(hook);
                    let t = fs::read_to_string(&p).unwrap();
                    assert!(t.contains(from), "{hook}: {t}");
                    fs::write(&p, t.replace(from, to)).unwrap();
                }
            }
        });
        let out = run(
            &repo,
            &bin(),
            &["upgrade", "--apply", "--commit"],
            &path(),
            &releases,
        );
        assert!(out.status.success(), "{}", text(&out));
        let older = if hard { "1" } else { "0" };
        for hook in ["pre-commit", "commit-msg"] {
            let t = fs::read_to_string(repo.join(".githooks").join(hook)).unwrap();
            assert!(
                t.contains(&format!("*) docsys_gate_exit={older}")),
                "{hook}: {t}"
            );
        }
        set_spec(&repo, Some("docsys/0.5"));
        let stub = failing_docsys(name);
        let p = format!("{}:/usr/bin:/bin", stub.display());
        let pre = run(&repo, Path::new("bash"), &[".githooks/pre-commit"], &p, "");
        assert_eq!(
            pre.status.code(),
            Some(if hard { 1 } else { 0 }),
            "{name}: {}",
            text(&pre)
        );
        fs::write(repo.join(".git/COMMIT_EDITMSG"), "a change\n").unwrap();
        let msg = run(
            &repo,
            Path::new("bash"),
            &[".githooks/commit-msg", ".git/COMMIT_EDITMSG"],
            &p,
            "",
        );
        assert_eq!(
            msg.status.code(),
            Some(if hard { 1 } else { 0 }),
            "{name}: {}",
            text(&msg)
        );
        let _ = fs::remove_dir_all(&repo);
    }
}

// 14. D-138: a `push:` the owner filtered that still runs on every branch
//     keeps its filter; `lint` and `refs` then run on a push to the default
//     branch only, so an error that branch carries fails no other push.

#[test]
fn a_push_that_runs_on_every_branch_checks_the_whole_tree_on_the_default_branch_only() {
    let owner = "on:\n  push:\n    branches-ignore: [automated-staging]\n  pull_request:\njobs:\n  docs:\n    steps:\n      - run: docsys lint --root docs --repo .\n      - name: refs\n        run: docsys refs --repo . --root docs\n      - if: github.event_name == 'pull_request'\n        run: docsys gate --repo . --root docs --range \"origin/${{ github.base_ref }}...HEAD\"\n";
    let on_develop = "github.event_name == 'push' && github.ref_name == 'develop'";
    for (filter, every) in [
        ("branches-ignore: [automated-staging]", true),
        ("paths: ['src/**']", true),
        ("branches: [develop, main]", false),
        ("tags: ['v*']", false),
    ] {
        let text = owner.replace("branches-ignore: [automated-staging]", filter);
        let (new, what) = workflow::scope_owner(&text, "docs", "develop")
            .unwrap()
            .unwrap();
        assert!(
            new.contains(&format!("  push:\n    {filter}\n  pull_request:\n")),
            "{new}"
        );
        let cond = if every {
            on_develop
        } else {
            "github.event_name == 'push'"
        };
        assert!(
            new.contains(&format!(
                "      - if: {cond}\n        run: docsys lint --root docs --repo .\n"
            )),
            "{filter}: {new}"
        );
        assert!(
            new.contains(&format!("      - name: refs\n        if: {cond}\n")),
            "{filter}: {new}"
        );
        assert_eq!(
            what.iter().any(|w| w.contains("on a push to develop only")),
            every,
            "{what:?}"
        );
        assert_eq!(workflow::scope_owner(&new, "docs", "develop"), Ok(None));
    }
    let inline = owner.replace(
        "  push:\n    branches-ignore: [automated-staging]\n",
        "  push: { branches-ignore: [wip] }\n",
    );
    let (new, _) = workflow::scope_owner(&inline, "docs", "develop")
        .unwrap()
        .unwrap();
    assert!(new.contains(on_develop), "{new}");
}

// 15. D-138: the hook and the relays find the pinned version where every
//     docsys looks for it, so a machine whose PATH has no docsys needs no
//     profile edit; and behind a docsys that cannot install the pin (no
//     cargo) they name the tree's installer, as no other line here can.

#[test]
fn the_hook_runs_the_installed_pin_and_names_the_installer_when_nothing_installs_it() {
    let releases = release("installed-pin");
    let repo = tmp("installed-pin");
    git(&repo, &["init", "-q", "-b", "main"]);
    fs::create_dir_all(repo.join(".github")).unwrap();
    let adopt = run(
        &repo,
        &bin(),
        &["adopt", "--ci-install", "release"],
        &path(),
        &releases,
    );
    assert!(adopt.status.success(), "{}", text(&adopt));
    assert!(repo.join("docs/.docsys-install").is_file());
    let own = env!("CARGO_PKG_VERSION");
    let home = tmp("installed-pin-home");
    let tools = "/usr/bin:/bin";
    let hook = |path: &str| {
        Command::new("bash")
            .arg(".git/hooks/pre-commit")
            .current_dir(&repo)
            .env("PATH", path)
            .env("HOME", &home)
            .env_remove("DOCSYS_HOME")
            .env_remove("DOCSYS_DISPATCHED")
            .output()
            .unwrap()
    };
    // no docsys on the PATH, the pin installed where every docsys looks
    let at = home.join(".docsys/versions").join(own).join("bin");
    fs::create_dir_all(&at).unwrap();
    fs::copy(bin(), at.join("docsys")).unwrap();
    let out = hook(tools);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert!(text(&out).contains("-- 0 error(s)"), "{}", text(&out));
    assert!(!text(&out).contains("not installed"), "{}", text(&out));
    let relay = Command::new("bash")
        .arg(".claude/hooks/session-intent.sh")
        .current_dir(&repo)
        .env("PATH", tools)
        .env("HOME", &home)
        .env("CLAUDE_PROJECT_DIR", &repo)
        .env_remove("DOCSYS_HOME")
        .output()
        .unwrap();
    assert!(
        !text(&relay).contains("is not installed"),
        "{}",
        text(&relay)
    );
    // a docsys that pins, an older one, no cargo, the pin not installed
    fs::remove_dir_all(home.join(".docsys")).unwrap();
    let old = tmp("installed-pin-old");
    fs::write(
        old.join("docsys"),
        "#!/bin/sh\ncase \"$1\" in --version) echo 'docsys 0.17.0 (spec docsys/0.5)'; exit 0 ;; esac\necho 'docsys: this tree pins docsys X; install it: cargo install docsys' >&2\nexit 1\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(old.join("docsys"), fs::Permissions::from_mode(0o755)).unwrap();
    let out = hook(&format!("{}:{tools}", old.display()));
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(
        text(&out).contains(&format!(
            "this tree pins docsys {own}; install: bash docs/.docsys-install"
        )),
        "{}",
        text(&out)
    );
    assert!(!text(&out).contains("cargo install"), "{}", text(&out));
    for d in [repo, home, old] {
        let _ = fs::remove_dir_all(d);
    }
}

// 16. After the pull, a clone runs `docsys upgrade --apply` once: on a moved
//     tree it rewrites only the clone's own git hooks, so an edit of the
//     person's is in no commit's way (R-177); and until it runs, the commit
//     gate says so, naming a command that runs this version.

#[test]
fn a_clone_behind_the_move_is_told_at_commit_and_its_upgrade_runs_beside_an_edit() {
    let releases = release("clone-behind");
    let repo = tmp("clone-behind");
    git(&repo, &["init", "-q", "-b", "main"]);
    docsys::adopt::run(&repo, &repo.join("docs"), "en").unwrap();
    git(&repo, &["add", "-A"]);
    git(
        &repo,
        &["-c", "core.hooksPath=/dev/null", "commit", "-qm", "adopt"],
    );
    // the hook a clone holds from before the move
    let case = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CASE)
        .join("before/dot-githooks");
    for hook in ["pre-commit", "commit-msg"] {
        fs::copy(case.join(hook), repo.join(".git/hooks").join(hook)).unwrap();
    }
    fs::write(
        repo.join("docs/index.md"),
        fs::read_to_string(repo.join("docs/index.md")).unwrap() + "\nA sentence.\n",
    )
    .unwrap();
    git(&repo, &["add", "-A"]);
    let gate = run(&repo, &bin(), &["gate"], &path(), &releases);
    assert!(
        text(&gate).contains("this clone's git hook is older than the tree's docsys")
            && text(&gate).contains("upgrade --apply` once in this clone"),
        "{}",
        text(&gate)
    );
    // the person's edit stays staged; the upgrade rewrites the hooks alone
    let out = run(&repo, &bin(), &["upgrade", "--apply"], &path(), &releases);
    assert!(out.status.success(), "{}", text(&out));
    let hook = fs::read_to_string(repo.join(".git/hooks/pre-commit")).unwrap();
    assert!(
        hook.contains(&format!("# docsys-template: {}", env!("CARGO_PKG_VERSION"))),
        "{hook}"
    );
    assert_eq!(
        git(&repo, &["diff", "--cached", "--name-only"]),
        "docs/index.md"
    );
    let gate = run(&repo, &bin(), &["gate"], &path(), &releases);
    assert!(
        !text(&gate).contains("older than the tree's docsys"),
        "{}",
        text(&gate)
    );
    let _ = fs::remove_dir_all(&repo);
}
