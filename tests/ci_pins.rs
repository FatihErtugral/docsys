#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// D-111, D-120, D-138: on a docsys/0.6 tree CI and every clone run one
// installer, the tree's `.docsys-install`, which holds the docsys version
// `docsys upgrade` pinned and the sha256 values the upgrade read from the
// release — fetched when the upgrade runs, reviewed with it, never fetched
// when the installer runs. A pull request that moves `.docsys-version` alone
// installs nothing. The release is a `file://` directory here: no test
// reaches the network.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const FILE: &str = ".github/workflows/docsys.yml";
const TARGETS: [&str; 4] = [
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
];
const OWN: &str = env!("CARGO_PKG_VERSION");

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-pins-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(cmd: &mut Command) -> Output {
    cmd.output().unwrap()
}

fn ok(out: &Output) -> String {
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn git(dir: &Path, args: &[&str]) -> String {
    ok(&run(Command::new("git")
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .current_dir(dir)))
    .trim()
    .to_string()
}

/// This build in `dir`, first on PATH, reading the release from `releases`.
fn docsys(dir: &Path, args: &[&str], releases: &str) -> Output {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_docsys"));
    let path = format!(
        "{}:{}",
        bin.parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    run(Command::new(&bin)
        .args(args)
        .env("PATH", path)
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .env("DOCSYS_RELEASES", releases)
        .current_dir(dir))
}

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

/// A release of `version` under `dir/v<version>/`, as the release workflow
/// publishes it: each target's archive and its `.sha256` file.
fn release(dir: &Path, version: &str) -> String {
    use std::os::unix::fs::PermissionsExt;
    let at = dir.join(format!("v{version}"));
    fs::create_dir_all(&at).unwrap();
    for target in TARGETS {
        let name = format!("docsys-v{version}-{target}");
        let pkg = at.join(&name);
        fs::create_dir_all(&pkg).unwrap();
        fs::write(
            pkg.join("docsys"),
            format!("#!/bin/sh\necho docsys {version} {target}\n"),
        )
        .unwrap();
        fs::set_permissions(pkg.join("docsys"), fs::Permissions::from_mode(0o755)).unwrap();
        ok(&run(Command::new("tar")
            .args(["czf", &format!("{name}.tar.gz"), &name])
            .current_dir(&at)));
        fs::remove_dir_all(&pkg).unwrap();
        let sum = docsys::fresh::sha256_hex(&fs::read(at.join(format!("{name}.tar.gz"))).unwrap());
        fs::write(
            at.join(format!("{name}.tar.gz.sha256")),
            format!("{sum}  {name}.tar.gz\n"),
        )
        .unwrap();
    }
    format!("file://{}", dir.display())
}

/// The sha256 a release under `dir` publishes for `target`.
fn sum_of(dir: &Path, version: &str, target: &str) -> String {
    let text = fs::read_to_string(
        dir.join(format!("v{version}"))
            .join(format!("docsys-v{version}-{target}.tar.gz.sha256")),
    )
    .unwrap();
    text.split_whitespace().next().unwrap().to_string()
}

/// A docsys/0.5 project with a `.github/` and everything committed.
fn project(name: &str) -> PathBuf {
    let repo = tmp(name);
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    fs::create_dir_all(repo.join(".github")).unwrap();
    write(&repo, "README.md", "# p\n");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "init"]);
    repo
}

/// `adopt --ci-install release` writes the tree's installer with this version
/// and its archives' sha256 values, read from the release, and a workflow
/// that runs it and holds no copy of them (D-138).
#[test]
fn adopt_pins_this_version_and_its_sha256_values() {
    let releases_dir = tmp("adopt-releases");
    let releases = release(&releases_dir, OWN);
    let repo = project("adopt");
    ok(&docsys(
        &repo,
        &["adopt", "--ci-install", "release"],
        &releases,
    ));
    let yaml = fs::read_to_string(repo.join(FILE)).unwrap();
    assert!(
        yaml.contains("      - name: docsys, the release the tree pins, by the tree's installer\n        shell: bash\n        run: bash docs/.docsys-install --print-dir >> \"$GITHUB_PATH\"\n"),
        "{yaml}"
    );
    let installer = fs::read_to_string(repo.join("docs/.docsys-install")).unwrap();
    assert!(
        installer.contains(&format!("\nversion={OWN}\n")),
        "{installer}"
    );
    for target in TARGETS {
        let sum = sum_of(&releases_dir, OWN, target);
        assert!(
            installer.contains(&format!(" target={target} sum={sum} ;;\n")),
            "{target}: {installer}"
        );
        assert!(!yaml.contains(&sum), "a second copy: {yaml}");
    }
    assert!(!yaml.contains("SHA256SUMS") && !installer.contains("SHA256SUMS"));
    let _ = fs::remove_dir_all(&repo);
    let _ = fs::remove_dir_all(&releases_dir);
}

/// The workflow's install step, run as a runner runs it, with no credential
/// and no `gh` on the PATH: the tree's installer installs the version it pins
/// and only that, its archive checked against the pinned sha256; a
/// `.docsys-version` naming another version — a pull request that moves the
/// pin alone — installs nothing. A clone with no docsys runs the same
/// installer and gets a `docsys` on its PATH (D-138).
#[test]
fn the_step_installs_only_the_pinned_version_and_archive() {
    let base = tmp("step");
    let releases_dir = base.join("releases");
    let releases = release(&releases_dir, OWN);
    release(&releases_dir, "9.9.9");
    let repo = project("step-repo");
    ok(&docsys(
        &repo,
        &["adopt", "--ci-install", "release"],
        &releases,
    ));
    let yaml = fs::read_to_string(repo.join(FILE)).unwrap();
    let step = yaml
        .lines()
        .find_map(|l| l.trim().strip_prefix("run: bash docs/.docsys-install"))
        .map(|rest| format!("bash docs/.docsys-install{rest}"))
        .unwrap_or_else(|| panic!("no installer step in:\n{yaml}"));
    // a runner or a clone with nothing but the system's tools and no token
    let attempt = |name: &str, script: &str| {
        let runner = base.join(name);
        fs::create_dir_all(runner.join("home")).unwrap();
        let path = runner.join("path");
        let mut c = Command::new("bash");
        c.args(["--noprofile", "--norc", "-eo", "pipefail", "-c", script])
            .current_dir(&repo)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", runner.join("home"))
            .env("GITHUB_PATH", &path)
            .env("DOCSYS_RELEASES", &releases);
        (
            run(&mut c),
            fs::read_to_string(path).unwrap_or_default(),
            runner,
        )
    };
    // the pinned version: installed where every docsys looks, and it runs
    let (out, added, runner) = attempt("runner-ok", &step);
    ok(&out);
    let dir = added.trim().to_string();
    assert_eq!(
        Path::new(&dir),
        runner.join(format!("home/.docsys/versions/{OWN}/bin")),
        "{added}"
    );
    let said = ok(&run(&mut Command::new(Path::new(&dir).join("docsys"))));
    assert!(said.starts_with(&format!("docsys {OWN} ")), "{said}");
    // a clone with no docsys: the same installer, and docsys on its PATH
    let (out, _, runner) = attempt("clone", "bash docs/.docsys-install");
    let told = ok(&out);
    assert!(
        told.contains(&format!("docsys {OWN} is installed")),
        "{told}"
    );
    let linked = runner.join("home/.local/bin/docsys");
    let said = ok(&run(&mut Command::new(&linked)));
    assert!(said.starts_with(&format!("docsys {OWN} ")), "{said}");
    // the pin moved alone, to a release that exists: one line, nothing installed
    write(&repo, "docs/.docsys-version", "9.9.9\n");
    let (out, added, runner) = attempt("runner-moved", &step);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        format!("docsys-install: .docsys-version names 9.9.9, and this installer holds {OWN} — run docsys upgrade --apply, which writes both\n")
    );
    assert_eq!(added, "");
    assert!(!runner.join("home/.docsys").exists());
    // no pin: the same line
    fs::remove_file(repo.join("docs/.docsys-version")).unwrap();
    let (out, added, _) = attempt("runner-unpinned", &step);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("names no version"),
        "{out:?}"
    );
    assert_eq!(added, "");
    write(&repo, "docs/.docsys-version", &format!("{OWN}\n"));
    // an archive replaced after the upgrade pinned it: refused
    for target in TARGETS {
        let p = releases_dir
            .join(format!("v{OWN}"))
            .join(format!("docsys-v{OWN}-{target}.tar.gz"));
        let mut bytes = fs::read(&p).unwrap();
        bytes.extend_from_slice(b"tampered");
        fs::write(&p, bytes).unwrap();
    }
    let (out, added, runner) = attempt("runner-tampered", &step);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("this installer pins"),
        "{out:?}"
    );
    assert_eq!(added, "");
    assert!(!runner
        .join(format!("home/.docsys/versions/{OWN}/bin/docsys"))
        .exists());
    let _ = fs::remove_dir_all(&repo);
    let _ = fs::remove_dir_all(&base);
}

/// The workflow 0.16.0 wrote for a release install on `labels`, untouched:
/// line 1 stamped, the step reading the release's SHA256SUMS when CI runs.
fn workflow_0_16_0(labels: &[&str]) -> String {
    let runs_on = match labels {
        [one] => (*one).to_string(),
        many => format!("[{}]", many.join(", ")),
    };
    let golden = include_str!("golden/workflow-release-sums.yml")
        .replace("9.9.9", "0.16.0")
        .replace(
            "runner=self-hosted,linux",
            &format!("runner={}", labels.join(",")),
        )
        .replace(
            "runs-on: [self-hosted, linux]",
            &format!("runs-on: {runs_on}"),
        )
        .replace("@RELEASES@", docsys::workflow::RELEASES);
    let hash = docsys::fresh::sha256_hex(golden.replacen("@HASH@", "", 1).as_bytes());
    golden.replacen("@HASH@", &hash, 1)
}

/// A docsys/0.5 project pinned to 0.16.0 with `workflow`, committed.
fn on_0_16_0(name: &str, workflow: &str) -> PathBuf {
    let repo = project(name);
    write(
        &repo,
        "docs/.docmeta.yml",
        "spec: docsys/0.5\nprofile: project\ndefault_content_language: en\n",
    );
    write(&repo, "docs/index.md", "# Docs\n");
    write(&repo, "docs/.docsys-version", "0.16.0\n");
    write(&repo, FILE, workflow);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "a tree 0.16.0 adopted"]);
    repo
}

/// The `ci-workflow` rows of a run's text output.
fn ci_rows(out: &str) -> Vec<&str> {
    out.lines()
        .filter(|l| l.split_whitespace().nth(1) == Some("ci-workflow"))
        .collect()
}

/// The diff a plan shows for the workflow: its changed lines.
fn changed(out: &str) -> Vec<String> {
    let diff = out
        .split_once(&format!("\n# {FILE}\n"))
        .map(|(_, d)| d)
        .unwrap_or_else(|| panic!("no diff for the workflow:\n{out}"));
    diff.lines()
        .take_while(|l| !l.starts_with("# "))
        .filter(|l| {
            (l.starts_with('-') || l.starts_with('+'))
                && !l.starts_with("---")
                && !l.starts_with("+++")
        })
        .map(str::to_string)
        .collect()
}

/// An untouched 0.16.0 workflow on a docsys/0.5 tree: the move to 0.6
/// regenerates it to run the tree's installer, which the move writes with
/// this version and each archive's sha256 read from the release — shown as a
/// diff, written by `--apply` into the upgrade commit; the runners stay, and
/// the leftover check after lists nothing for it (D-138).
#[test]
fn the_upgrade_replaces_0_16_0s_sums_step_with_the_pinned_one() {
    let releases_dir = tmp("sums-releases");
    let releases = release(&releases_dir, OWN);
    let before = workflow_0_16_0(&["self-hosted", "linux"]);
    let repo = on_0_16_0("sums", &before);
    let plan = ok(&docsys(&repo, &["upgrade"], &releases));
    let rows = ci_rows(&plan);
    assert_eq!(rows.len(), 1, "{plan}");
    assert!(
        rows.first()
            .unwrap()
            .starts_with(&format!("auto    ci-workflow        {FILE}  ")),
        "{plan}"
    );
    assert!(
        plan.contains("auto    installer          docs/.docsys-install  "),
        "{plan}"
    );
    let lines = changed(&plan);
    let x86 = sum_of(&releases_dir, OWN, "x86_64-unknown-linux-musl");
    for must in [
        "-          curl -fsSL -o SHA256SUMS \"$RELEASES/v$v/SHA256SUMS\""
            .replace("$RELEASES", docsys::workflow::RELEASES),
        "+      - name: docsys, the release the tree pins, by the tree's installer".to_string(),
        "+  merge_group:".to_string(),
    ] {
        assert!(lines.contains(&must), "`{must}` not in {lines:#?}");
    }
    assert!(lines.iter().all(|l| !l.contains("runs-on")), "{lines:#?}");
    assert_eq!(fs::read_to_string(repo.join(FILE)).unwrap(), before);
    assert!(!repo.join("docs/.docsys-install").exists());
    ok(&docsys(
        &repo,
        &["upgrade", "--apply", "--commit"],
        &releases,
    ));
    let after = fs::read_to_string(repo.join(FILE)).unwrap();
    assert!(
        !after.contains("SHA256SUMS") && !after.contains(&x86),
        "{after}"
    );
    let installer = fs::read_to_string(repo.join("docs/.docsys-install")).unwrap();
    assert!(installer.contains(&format!("sum={x86} ;;")), "{installer}");
    let head = git(&repo, &["show", "--stat", "HEAD"]);
    assert!(
        head.contains("docsys.yml") && head.contains(".docsys-install"),
        "{head}"
    );
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
    let idle = ok(&docsys(&repo, &["upgrade"], &releases));
    assert!(ci_rows(&idle).is_empty(), "{idle}");
    let _ = fs::remove_dir_all(&repo);
    let _ = fs::remove_dir_all(&releases_dir);
}

/// An owner's workflow that carries 0.16.0's SHA256SUMS step: the move to
/// 0.6 makes that step run the tree's installer, its runners and timeout as
/// the owner wrote them (D-138).
#[test]
fn an_owners_sums_step_alone_is_replaced() {
    let releases_dir = tmp("owner-releases");
    let releases = release(&releases_dir, OWN);
    let owner = workflow_0_16_0(&["self-hosted", "linux"])
        .replace("[self-hosted, linux]", "[self-hosted, linux, 'req:2cpu']")
        .replace("    steps:\n", "    timeout-minutes: 15\n    steps:\n");
    let repo = on_0_16_0("owner", &owner);
    ok(&docsys(
        &repo,
        &["upgrade", "--apply", "--commit"],
        &releases,
    ));
    let after = fs::read_to_string(repo.join(FILE)).unwrap();
    assert!(
        after.contains("      - name: docsys, the release the tree pins, by the tree's installer\n        shell: bash\n        run: bash docs/.docsys-install --print-dir >> \"$GITHUB_PATH\"\n"),
        "{after}"
    );
    assert!(
        after.contains("    runs-on: [self-hosted, linux, 'req:2cpu']\n")
            && after.contains("    timeout-minutes: 15\n"),
        "{after}"
    );
    assert!(!after.contains("SHA256SUMS"), "{after}");
    let installer = fs::read_to_string(repo.join("docs/.docsys-install")).unwrap();
    assert!(
        installer.contains(&format!(
            "sum={} ;;",
            sum_of(&releases_dir, OWN, "aarch64-apple-darwin")
        )),
        "{installer}"
    );
    let _ = fs::remove_dir_all(&repo);
    let _ = fs::remove_dir_all(&releases_dir);
}

/// When the release cannot be read — offline, no curl, a release not
/// published — the rest of the upgrade applies, the workflow stays as it was,
/// never written half or without a hash, and the command to run again is
/// named once.
#[test]
fn offline_the_workflow_stays_and_the_rerun_is_named_once() {
    let unreachable = format!("file://{}", tmp("nowhere").join("none").display());
    let before = workflow_0_16_0(&["ubuntu-latest"]);
    let repo = on_0_16_0("offline", &before);
    let out = docsys(&repo, &["upgrade", "--apply", "--commit"], &unreachable);
    let said = ok(&out);
    assert_eq!(fs::read_to_string(repo.join(FILE)).unwrap(), before);
    assert!(!repo.join("docs/.docsys-install").exists(), "{said}");
    // the move names the rerun once; the leftover check after it names what
    // the move left, this workflow among it
    let (moved, left) = said
        .split_once("— what an earlier version left")
        .unwrap_or((&said, ""));
    let rows = ci_rows(moved);
    assert_eq!(rows.len(), 1, "{said}");
    let row = rows.first().unwrap();
    assert!(
        row.starts_with(&format!("manual  ci-workflow        {FILE}  ")),
        "{row}"
    );
    assert!(row.contains("docsys upgrade --apply"), "{row}");
    assert_eq!(
        moved.matches("docsys upgrade --apply` again").count(),
        1,
        "{said}"
    );
    assert_eq!(ci_rows(left).len(), 1, "{said}");
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
    let _ = fs::remove_dir_all(&repo);
}

/// The release publishes each archive's `.sha256` file, the upgrade's source,
/// and no SHA256SUMS, which nothing reads any more.
#[test]
fn the_release_publishes_each_archives_sha256_and_no_sha256sums() {
    let release_yml = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/release.yml"),
    )
    .unwrap();
    assert!(
        release_yml.contains("shasum -a 256 \"$name.tar.gz\" > \"$name.tar.gz.sha256\""),
        "{release_yml}"
    );
    assert!(
        release_yml.contains("assets=(docsys-*.tar.gz docsys-*.tar.gz.sha256)"),
        "{release_yml}"
    );
    assert!(!release_yml.contains("SHA256SUMS"), "{release_yml}");
}
