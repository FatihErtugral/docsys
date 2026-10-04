#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// D-111, D-120: on a docsys/0.5 tree the CI workflow installs the docsys
// version `docsys upgrade` pinned in it, and checks the archive against the
// sha256 values the upgrade wrote there from the release — fetched when the
// upgrade runs, reviewed with it, never fetched when CI runs. A pull request
// that moves `.docsys-version` alone installs nothing. The release is a
// `file://` directory here: no test reaches the network.

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

/// The `run:` script of the install step in `yaml`.
fn install_script(yaml: &str) -> String {
    let at = yaml
        .find("the release the tree pins")
        .unwrap_or_else(|| panic!("no release install in:\n{yaml}"));
    let block = yaml[at..]
        .split_once("        run: |\n")
        .map(|(_, b)| b)
        .unwrap();
    block
        .lines()
        .take_while(|l| l.starts_with("          ") || l.is_empty())
        .map(|l| format!("{}\n", l.get(10..).unwrap_or("")))
        .collect()
}

/// `script` as a runner's `shell: bash` step runs it.
fn bash(script: &str, dir: &Path, envs: &[(&str, &str)]) -> Output {
    let mut c = Command::new("bash");
    c.args(["--noprofile", "--norc", "-eo", "pipefail", "-c", script])
        .current_dir(dir);
    for (k, v) in envs {
        c.env(k, v);
    }
    run(&mut c)
}

/// `adopt --ci-install release` pins this version and its archives' sha256
/// values, read from the release, into the install step; nothing is read at
/// CI time but the archive itself.
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
        yaml.contains(&format!(
            "      - name: docsys {OWN}, the release the tree pins, checked against the sha256 its upgrade wrote\n"
        )),
        "{yaml}"
    );
    for target in TARGETS {
        let sum = sum_of(&releases_dir, OWN, target);
        assert!(
            yaml.contains(&format!(" target={target} sum={sum} ;;\n")),
            "{target}: {yaml}"
        );
    }
    assert!(!yaml.contains("SHA256SUMS"), "{yaml}");
    let _ = fs::remove_dir_all(&repo);
    let _ = fs::remove_dir_all(&releases_dir);
}

/// The install step installs the version it pins and only that: its archive
/// checked against the pinned sha256; a `.docsys-version` naming another
/// version — a pull request that moves the pin alone — installs nothing.
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
    let script = install_script(&yaml).replace(docsys::workflow::RELEASES, &releases);
    let attempt = |name: &str| {
        let runner = base.join(name);
        fs::create_dir_all(&runner).unwrap();
        let path = runner.join("path");
        let out = bash(
            &script,
            &repo,
            &[
                ("RUNNER_TEMP", &runner.display().to_string()),
                ("GITHUB_PATH", &path.display().to_string()),
                ("PATH", &std::env::var("PATH").unwrap_or_default()),
            ],
        );
        (out, fs::read_to_string(path).unwrap_or_default())
    };
    // the pinned version: installed, and it runs
    let (out, added) = attempt("runner-ok");
    ok(&out);
    let dir = added.trim().to_string();
    assert!(dir.contains(&format!("docsys-v{OWN}-")), "{added}");
    let said = ok(&run(&mut Command::new(Path::new(&dir).join("docsys"))));
    assert!(said.starts_with(&format!("docsys {OWN} ")), "{said}");
    // the pin moved alone, to a release that exists: one line, nothing installed
    write(&repo, "docs/.docsys-version", "9.9.9\n");
    let (out, added) = attempt("runner-moved");
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        format!("docsys: docs/.docsys-version names 9.9.9, and this workflow installs {OWN} — run docsys upgrade --apply, which pins both\n")
    );
    assert_eq!(added, "");
    // no pin: the same line
    fs::remove_file(repo.join("docs/.docsys-version")).unwrap();
    let (out, added) = attempt("runner-unpinned");
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
    let (out, added) = attempt("runner-tampered");
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("this workflow pins"),
        "{out:?}"
    );
    assert_eq!(added, "");
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

/// An untouched 0.16.0 workflow: its SHA256SUMS step becomes this version's
/// pinned one — the version and each archive's sha256 read from the release —
/// shown as a diff, written by `--apply` into the upgrade commit; nothing else
/// of the file moves, and the leftover check after lists nothing for it.
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
    let lines = changed(&plan);
    let x86 = sum_of(&releases_dir, OWN, "x86_64-unknown-linux-musl");
    for must in [
        "-          curl -fsSL -o SHA256SUMS \"$RELEASES/v$v/SHA256SUMS\""
            .replace("$RELEASES", docsys::workflow::RELEASES),
        format!("+            Linux-x86_64) target=x86_64-unknown-linux-musl sum={x86} ;;"),
    ] {
        assert!(lines.contains(&must), "`{must}` not in {lines:#?}");
    }
    assert!(
        lines
            .iter()
            .all(|l| !l.contains("runs-on") && !l.contains("docsys lint")),
        "{lines:#?}"
    );
    assert_eq!(fs::read_to_string(repo.join(FILE)).unwrap(), before);
    ok(&docsys(
        &repo,
        &["upgrade", "--apply", "--commit"],
        &releases,
    ));
    let after = fs::read_to_string(repo.join(FILE)).unwrap();
    assert!(after.contains(&format!("sum={x86} ;;")), "{after}");
    assert!(!after.contains("SHA256SUMS"), "{after}");
    assert!(git(&repo, &["show", "--stat", "HEAD"]).contains("docsys.yml"));
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
    let idle = ok(&docsys(&repo, &["upgrade"], &releases));
    assert!(ci_rows(&idle).is_empty(), "{idle}");
    let _ = fs::remove_dir_all(&repo);
    let _ = fs::remove_dir_all(&releases_dir);
}

/// An owner's workflow that carries 0.16.0's SHA256SUMS step: that step
/// alone becomes the pinned one, every other line as the owner wrote it.
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
    let step = |t: &str| {
        let from = t.find("      - name: docsys").unwrap();
        let to = t[from..].find("      - run: docsys lint").unwrap() + from;
        (t[..from].to_string(), t[to..].to_string())
    };
    assert_eq!(step(&after), step(&owner), "{after}");
    assert!(
        after.contains(&format!(
            "sum={} ;;",
            sum_of(&releases_dir, OWN, "aarch64-apple-darwin")
        )),
        "{after}"
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
    let rows = ci_rows(&said);
    assert_eq!(rows.len(), 1, "{said}");
    let row = rows.first().unwrap();
    assert!(
        row.starts_with(&format!("manual  ci-workflow        {FILE}  ")),
        "{row}"
    );
    assert!(row.contains("docsys upgrade --apply"), "{row}");
    assert_eq!(
        said.matches("docsys upgrade --apply` again").count(),
        1,
        "{said}"
    );
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
