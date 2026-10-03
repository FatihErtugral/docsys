#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// D-105, D-111: the workflow adopt writes, and what may happen to one that exists.

use docsys::workflow::{self, Ci, Existing, Install, Verify, Workflow};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-workflow-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn repo_with_github(name: &str) -> PathBuf {
    let repo = tmp(name);
    git(&repo, &["init", "-q", "-b", "main"]);
    fs::create_dir_all(repo.join(".github")).unwrap();
    repo
}

fn docsys(repo: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap()
}

const FILE: &str = ".github/workflows/docsys.yml";

const SUM_A: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
const SUM_B: &str = "2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae";

fn params(branch: &str, runner: &[&str], install: Install, verify: Verify, root: &str) -> Workflow {
    Workflow {
        version: "9.9.9".to_string(),
        branch: branch.to_string(),
        root: root.to_string(),
        ci: Ci {
            runner: runner.iter().map(|s| s.to_string()).collect(),
            install,
            verify,
        },
    }
}

/// The rendering against its golden file: the hash on line 1 is the file's
/// own, and the release address is the crate's repository.
fn matches_golden(w: &Workflow, golden: &str) {
    let text = workflow::render(w);
    let first = text.lines().next().unwrap_or_default();
    let hex = first
        .split_once("sha256:")
        .map(|(_, r)| r.split(' ').next().unwrap_or_default())
        .unwrap_or_default();
    assert_eq!(hex.len(), 64, "{first}");
    let unsigned = text.replacen(&format!("sha256:{hex}"), "sha256:", 1);
    assert_eq!(
        hex,
        docsys::fresh::sha256_hex(unsigned.as_bytes()),
        "{first}"
    );
    let shown = text
        .replacen(hex, "@HASH@", 1)
        .replace(workflow::RELEASES, "@RELEASES@");
    if shown != golden {
        panic!(
            "rendering differs from the golden file:\n{}",
            docsys::diff::unified(golden, &shown, "golden", "rendered", 2)
        );
    }
    // a rendering classifies as itself, untouched, with every parameter read back
    let current = Workflow {
        version: docsys::agents::TEMPLATE_VERSION.to_string(),
        root: if w.root.is_empty() {
            ".".to_string()
        } else {
            w.root.clone()
        },
        ..w.clone()
    };
    assert_eq!(
        workflow::classify_text(
            &text,
            &params("x", &["y"], Install::Cargo, Verify::Off, "z")
        ),
        Existing::Untouched {
            from: "9.9.9".to_string(),
            params: current
        }
    );
}

#[test]
fn the_cargo_install_on_one_runner_with_a_follow_up_pull_request() {
    matches_golden(
        &params(
            "main",
            &["ubuntu-latest"],
            Install::Cargo,
            Verify::PullRequest,
            "docs",
        ),
        include_str!("golden/workflow-cargo-pull-request.yml"),
    );
}

#[test]
fn the_release_install_on_three_runner_labels_pushing_directly() {
    matches_golden(
        &params(
            "trunk",
            &["self-hosted", "linux", "x64"],
            Install::Release(vec![
                ("x86_64-unknown-linux-musl".to_string(), SUM_A.to_string()),
                ("aarch64-apple-darwin".to_string(), SUM_B.to_string()),
            ]),
            Verify::Direct,
            "",
        ),
        include_str!("golden/workflow-release-direct.yml"),
    );
}

#[test]
fn verify_off_renders_no_verify_job() {
    let golden = include_str!("golden/workflow-cargo-off.yml");
    assert!(!golden.contains("verify-on-approval:"));
    matches_golden(
        &params(
            "main",
            &["ubuntu-24.04"],
            Install::Cargo,
            Verify::Off,
            "documentation",
        ),
        golden,
    );
}

#[test]
fn the_flags_are_read_and_a_release_install_without_sha256_is_refused() {
    let v05 = docsys::era::Era::of_spec(Some("docsys/0.5"));
    let v04 = docsys::era::Era::of_spec(Some("docsys/0.4"));
    assert_eq!(Ci::from_flags(None, None, None, None, v05), Ok(None));
    // the sha256 values a docsys/0.4 tree's release install holds, as 0.15.1
    // took them (D-118); tests/ci_install.rs holds docsys/0.5's
    let ci = Ci::from_flags(
        Some("self-hosted,linux"),
        Some("release"),
        Some(&format!("x86_64-unknown-linux-musl={SUM_A}")),
        Some("off"),
        v04,
    )
    .unwrap()
    .unwrap();
    assert_eq!(ci.runner, vec!["self-hosted", "linux"]);
    assert_eq!(
        ci.install,
        Install::Release(vec![(
            "x86_64-unknown-linux-musl".to_string(),
            SUM_A.to_string()
        )])
    );
    assert_eq!(ci.verify, Verify::Off);
    // direct is a docsys/0.4 tree's mode (D-105)
    let only_mode = Ci::from_flags(None, None, None, Some("direct"), v04)
        .unwrap()
        .unwrap();
    assert_eq!(
        only_mode,
        Ci {
            verify: Verify::Direct,
            ..Ci::default()
        }
    );
    let refused = |r: Option<&str>, i: Option<&str>, s: Option<&str>, v: Option<&str>| {
        Ci::from_flags(r, i, s, v, v04).unwrap_err()
    };
    assert!(refused(None, Some("release"), None, None).contains("--ci-sha256"));
    assert!(refused(
        None,
        None,
        Some(&format!("aarch64-apple-darwin={SUM_A}")),
        None
    )
    .contains("--ci-install release"));
    assert!(refused(
        None,
        Some("release"),
        Some("aarch64-apple-darwin=abc"),
        None
    )
    .contains("64 hex"));
    assert!(refused(
        None,
        Some("release"),
        Some(&format!("x86_64-pc-windows-msvc={SUM_A}")),
        None
    )
    .contains("aarch64-apple-darwin"));
    assert!(refused(None, Some("binary"), None, None).contains("cargo or release"));
    assert!(refused(Some("a b"), None, None, None).contains("--ci-runner"));
    assert!(refused(Some(""), None, None, None).contains("--ci-runner"));
    assert!(refused(None, None, None, Some("always")).contains("pull-request, direct or off"));

    // the binary refuses before it writes anything
    let repo = repo_with_github("refused");
    fs::create_dir_all(repo.join("docs")).unwrap();
    let docmeta = "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n";
    fs::write(repo.join("docs/.docmeta.yml"), docmeta).unwrap();
    let out = docsys(&repo, &["adopt", "--ci-install", "release"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(err.contains("--ci-sha256 <target>=<hex>"), "{err}");
    assert_eq!(
        fs::read_dir(repo.join("docs")).unwrap().count(),
        1,
        "nothing but the tree's .docmeta.yml"
    );
    assert!(!repo.join(FILE).exists());
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn adopt_writes_the_workflow_its_flags_describe_and_keeps_it_after() {
    let repo = repo_with_github("flags");
    let out = docsys(&repo, &["adopt", "--ci-runner", "self-hosted,linux"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = fs::read_to_string(repo.join(FILE)).unwrap();
    let first = text.lines().next().unwrap();
    assert!(first.starts_with(&format!(
        "# docsys-template: {} sha256:",
        docsys::agents::TEMPLATE_VERSION
    )));
    assert!(
        first.ends_with(" branch=main runner=self-hosted,linux install=cargo root=docs"),
        "{first}"
    );
    assert_eq!(text.matches("runs-on: [self-hosted, linux]\n").count(), 1);
    // CI installs the version the tree pins, which adopt wrote (D-120)
    assert!(text.contains(
        "cargo install docsys --version \"${{ steps.docsys-pin.outputs.version }}\" --locked"
    ));
    assert!(text.contains("v=$(head -n 1 \"docs/.docsys-version\" 2>/dev/null || true)"));
    assert_eq!(
        fs::read_to_string(repo.join("docs/.docsys-version")).unwrap(),
        format!("{}\n", docsys::agents::TEMPLATE_VERSION)
    );
    assert!(!text.contains("|| echo"), "{text}");
    // a re-adopt keeps the file, flags or not
    let again = docsys(&repo, &["adopt", "--ci-runner", "ubuntu-latest"]);
    let said = String::from_utf8_lossy(&again.stdout);
    assert!(
        said.contains("ci workflow (.github/workflows/docsys.yml): kept"),
        "{said}"
    );
    assert!(said.contains("shape a new workflow only"), "{said}");
    assert_eq!(fs::read_to_string(repo.join(FILE)).unwrap(), text);
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn adopt_without_flags_writes_the_defaults_and_a_tree_at_the_top_is_root_dot() {
    let repo = repo_with_github("defaults");
    docsys::adopt::run(&repo, &repo.join("docs"), "en").unwrap();
    let text = fs::read_to_string(repo.join(FILE)).unwrap();
    // a docsys/0.5 tree keeps no verification: no approval job (D-130)
    assert!(
        text.lines()
            .next()
            .unwrap()
            .ends_with(" branch=main runner=ubuntu-latest install=cargo root=docs"),
        "{text}"
    );
    assert!(!text.contains("gh pr"), "{text}");
    let report = fs::read_to_string(repo.join("ADOPTION.md")).unwrap();
    assert!(!report.contains("squash and merge"), "{report}");
    // the tree is the repository: `--root .`, never an empty one
    let top = repo_with_github("top");
    let out = docsys(&top, &["adopt", "--root", "."]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = fs::read_to_string(top.join(FILE)).unwrap();
    assert!(text.lines().next().unwrap().ends_with(" root=."), "{text}");
    assert!(text.contains("docsys lint --repo . --root .\n"), "{text}");
    assert!(!text.contains("--root  "), "{text}");
    let _ = fs::remove_dir_all(&repo);
    let _ = fs::remove_dir_all(&top);
}

#[test]
fn the_push_trigger_names_the_default_branch_of_origin() {
    let repo = repo_with_github("origin-head");
    git(
        &repo,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "base",
        ],
    );
    let head = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["update-ref", "refs/remotes/origin/trunk", &head]);
    git(
        &repo,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/trunk",
        ],
    );
    assert_eq!(workflow::default_branch(&repo), "trunk");
    git(
        &repo,
        &["symbolic-ref", "--delete", "refs/remotes/origin/HEAD"],
    );
    assert_eq!(workflow::default_branch(&repo), "main");
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn an_untouched_stamped_workflow_is_regenerable_with_what_it_records() {
    let repo = repo_with_github("untouched");
    let old = Workflow {
        version: "0.0.1".to_string(),
        ..params(
            "main",
            &["self-hosted", "linux"],
            Install::Cargo,
            Verify::Direct,
            "docs",
        )
    };
    fs::create_dir_all(repo.join(".github/workflows")).unwrap();
    fs::write(repo.join(FILE), workflow::render(&old)).unwrap();
    let Some(Existing::Untouched { from, params }) = workflow::classify(&repo, "docs") else {
        panic!("{:?}", workflow::classify(&repo, "docs"));
    };
    assert_eq!(from, "0.0.1");
    assert_eq!(
        params,
        Workflow {
            version: docsys::agents::TEMPLATE_VERSION.to_string(),
            ..old.clone()
        }
    );
    // CRLF, as a checkout on Windows may write it, is the same file
    fs::write(
        repo.join(FILE),
        workflow::render(&old).replace('\n', "\r\n"),
    )
    .unwrap();
    assert!(matches!(
        workflow::classify(&repo, "docs"),
        Some(Existing::Untouched { .. })
    ));
    // a parameter changed on line 1 alone is an edit too: the hash covers it
    let edited = workflow::render(&old).replacen("verify=direct", "verify=off", 1);
    fs::write(repo.join(FILE), &edited).unwrap();
    assert!(matches!(
        workflow::classify(&repo, "docs"),
        Some(Existing::Owned { .. })
    ));
    fs::remove_file(repo.join(FILE)).unwrap();
    assert_eq!(workflow::classify(&repo, "docs"), None);
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_byte_equal_rendering_from_before_the_stamp_is_regenerable() {
    // what 0.15.1 wrote for a tree at docs/, and 0.14.0 for one at the top (its empty --root)
    let repo = repo_with_github("legacy");
    fs::create_dir_all(repo.join(".github/workflows")).unwrap();
    for (text, root, from, verify) in [
        (
            include_str!("golden/legacy-0.15.1-docs.yml"),
            "docs",
            "0.15",
            Verify::Direct,
        ),
        (
            include_str!("golden/legacy-0.14.0-top.yml"),
            ".",
            "0.11-0.14",
            Verify::Off,
        ),
    ] {
        fs::write(repo.join(FILE), text).unwrap();
        let got = workflow::classify(&repo, root);
        assert_eq!(
            got,
            Some(Existing::Legacy {
                from,
                params: Workflow {
                    version: docsys::agents::TEMPLATE_VERSION.to_string(),
                    branch: "main".to_string(),
                    root: root.to_string(),
                    ci: Ci {
                        verify,
                        ..Ci::default()
                    }
                }
            })
        );
        // one byte off is the owner's
        fs::write(repo.join(FILE), format!("{text}\n")).unwrap();
        assert!(matches!(
            workflow::classify(&repo, root),
            Some(Existing::Owned { .. })
        ));
    }
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn a_hand_edited_workflow_is_the_owners_and_only_a_diff_is_shown() {
    // a self-hosted runner and a binary install: the shape a real repository
    // gave the generated file by hand
    let repo = repo_with_github("owned");
    fs::create_dir_all(repo.join(".github/workflows")).unwrap();
    let mine = "name: docsys\n\non:\n  pull_request:\n  push:\n    branches: [main]\n\npermissions:\n  contents: read\n\njobs:\n  docs:\n    runs-on: [self-hosted, linux]\n    steps:\n      - uses: actions/checkout@v5\n        with:\n          fetch-depth: 0\n      - run: curl -fsSL https://example.invalid/docsys.tar.gz | tar -xz -C /usr/local/bin docsys\n      - run: docsys lint --repo . --root docs\n      - run: docsys refs --repo . --root docs\n";
    fs::write(repo.join(FILE), mine).unwrap();
    let Some(Existing::Owned { diff }) = workflow::classify(&repo, "docs") else {
        panic!("{:?}", workflow::classify(&repo, "docs"));
    };
    assert_eq!(fs::read_to_string(repo.join(FILE)).unwrap(), mine);
    for line in [
        "-    runs-on: [self-hosted, linux]\n",
        "+    runs-on: ubuntu-latest\n",
        "-      - run: curl -fsSL https://example.invalid/docsys.tar.gz | tar -xz -C /usr/local/bin docsys\n",
        "+        run: cargo install docsys --version ",
    ] {
        assert!(diff.contains(line), "`{line}` not in:\n{diff}");
    }
    assert!(
        diff.starts_with("--- .github/workflows/docsys.yml"),
        "{diff}"
    );
    // a stamped file edited below line 1: the diff is to its own parameters
    let stamped = workflow::render(&params(
        "main",
        &["self-hosted", "linux"],
        Install::Cargo,
        Verify::Direct,
        "docs",
    ));
    let edited = stamped.replace("      - run: docsys refs --repo . --root docs\n", "");
    fs::write(repo.join(FILE), &edited).unwrap();
    let Some(Existing::Owned { diff }) = workflow::classify(&repo, "docs") else {
        panic!("{:?}", workflow::classify(&repo, "docs"));
    };
    assert_eq!(fs::read_to_string(repo.join(FILE)).unwrap(), edited);
    assert!(
        diff.contains("+      - run: docsys refs --repo . --root docs\n"),
        "{diff}"
    );
    assert!(!diff.contains("runs-on"), "{diff}");
    let _ = fs::remove_dir_all(&repo);
}

/// A release install holds the sha256 values of one version, and the tree
/// pins one: when they part, the job fails in one line instead of installing
/// a binary the tree does not run (D-120).
#[test]
fn a_release_install_fails_when_the_pin_is_not_its_version() {
    let text = workflow::render(&params(
        "main",
        &["ubuntu-latest"],
        Install::Release(vec![(
            "x86_64-unknown-linux-musl".to_string(),
            SUM_A.to_string(),
        )]),
        Verify::Off,
        "",
    ));
    // the install step's own lines, up to the platform match
    let script: String = text
        .lines()
        .skip_while(|l| !l.contains("the release archive checked against its sha256"))
        .skip_while(|l| l.trim() != "run: |")
        .skip(1)
        .take_while(|l| !l.contains("case \"$(uname -s)-$(uname -m)\" in"))
        .map(|l| format!("{}\n", l.trim_start()))
        .collect();
    assert!(script.contains(".docsys-version"), "{script}");
    let dir = tmp("release-pin");
    let run = |pin: &str| {
        fs::write(dir.join(".docsys-version"), format!("{pin}\n")).unwrap();
        Command::new("bash")
            .args(["-c", &script])
            .current_dir(&dir)
            .output()
            .unwrap()
    };
    let parted = run("1.2.3");
    assert_eq!(parted.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&parted.stderr),
        "docsys: the tree pins docsys 1.2.3; this workflow holds the sha256 values of 9.9.9 — write those of 1.2.3\n"
    );
    assert!(run("9.9.9").status.success());
    let _ = fs::remove_dir_all(&dir);
}

/// An approval job is a docsys/0.4 tree's: a docsys/0.5 page carries no
/// verification, so a job is refused before anything is written (D-130),
/// and a docsys/0.4 tree gets the job that writes its records (D-118).
#[test]
fn an_approval_job_is_refused_on_0_5_and_each_era_gets_its_own() {
    for mode in ["pull-request", "direct"] {
        let repo = repo_with_github(&format!("v05-{mode}"));
        let out = docsys(&repo, &["adopt", "--verify-on-approval", mode]);
        assert_eq!(out.status.code(), Some(2), "{out:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("(D-130)"),
            "{out:?}"
        );
        assert!(
            !repo.join(FILE).exists() && !repo.join("docs").exists(),
            "nothing written"
        );
        let _ = fs::remove_dir_all(&repo);
    }
    let v04 = |name: &str| {
        let repo = repo_with_github(name);
        fs::create_dir_all(repo.join("docs")).unwrap();
        fs::write(
            repo.join("docs/.docmeta.yml"),
            "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n",
        )
        .unwrap();
        repo
    };
    let repo = v04("v04-default");
    let out = docsys(&repo, &["adopt"]);
    assert!(out.status.success(), "{out:?}");
    let first = fs::read_to_string(repo.join(FILE)).unwrap();
    assert!(
        first
            .lines()
            .next()
            .unwrap()
            .contains(" verify=pull-request "),
        "a docsys/0.4 tree gets the job that writes its records: {}",
        first.lines().next().unwrap()
    );
    let _ = fs::remove_dir_all(&repo);
    let repo = v04("v04-description");
    let out = docsys(&repo, &["adopt", "--verify-on-approval", "description"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(!repo.join(FILE).exists(), "nothing written");
    let _ = fs::remove_dir_all(&repo);
}
