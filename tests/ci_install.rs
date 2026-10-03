#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// D-111, D-120: on a docsys/0.5 tree the CI workflow names no docsys version
// and no sha256 — it installs the version the pin names and checks the
// archive against the SHA256SUMS the release publishes, so an upgrade never
// needs a hand edit to CI.

use docsys::workflow::{self, Ci, Existing, Install, Reinstall, Verify, Workflow};
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

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-ci-{name}-{}", std::process::id()));
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

fn docsys(dir: &Path, args: &[&str]) -> Output {
    run(Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args(args)
        .current_dir(dir))
}

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

/// The lines of the `run: |` block of the first step whose script holds
/// `marker`, de-indented: the shell a runner executes.
fn script_of(yaml: &str, marker: &str) -> String {
    let lines: Vec<&str> = yaml.lines().collect();
    for (i, l) in lines.iter().enumerate() {
        if l.trim() != "run: |" {
            continue;
        }
        let key = l.len() - l.trim_start().len();
        let body: Vec<&str> = lines
            .get(i + 1..)
            .unwrap_or_default()
            .iter()
            .take_while(|b| b.trim().is_empty() || b.len() - b.trim_start().len() > key)
            .copied()
            .collect();
        let cut = body
            .iter()
            .filter(|b| !b.trim().is_empty())
            .map(|b| b.len() - b.trim_start().len())
            .min()
            .unwrap_or(0);
        let text: String = body
            .iter()
            .map(|b| format!("{}\n", b.get(cut..).unwrap_or("")))
            .collect();
        if text.contains(marker) {
            return text;
        }
    }
    panic!("no `run: |` block holds `{marker}` in:\n{yaml}");
}

fn params(install: Install) -> Workflow {
    Workflow {
        version: "9.9.9".to_string(),
        branch: "main".to_string(),
        root: "docs".to_string(),
        ci: Ci {
            runner: vec!["self-hosted".to_string(), "linux".to_string()],
            install,
            verify: Verify::Off,
        },
    }
}

/// A docsys/0.5 release install: the version comes from the pin at run time,
/// the check from the release's SHA256SUMS — line 1's stamp and hash work as
/// for every rendering, and the file reads back as itself (D-111).
#[test]
fn a_05_release_install_names_no_version_and_no_sha256() {
    let w = params(Install::ReleaseSums);
    let text = workflow::render(&w);
    let golden = include_str!("golden/workflow-release-sums.yml");
    let first = text.lines().next().unwrap();
    let hex = first
        .split_once("sha256:")
        .map(|(_, r)| r.split(' ').next().unwrap())
        .unwrap();
    assert_eq!(
        hex,
        docsys::fresh::sha256_hex(text.replacen(hex, "", 1).as_bytes()),
        "{first}"
    );
    let shown = text
        .replacen(hex, "@HASH@", 1)
        .replace(workflow::RELEASES, "@RELEASES@");
    assert!(
        shown == golden,
        "the 0.5 release install differs from the golden file:\n{}",
        docsys::diff::unified(golden, &shown, "golden", "rendered", 2)
    );
    // below line 1 nothing names a version or a sha256 value
    for line in text.lines().skip(1) {
        assert!(!line.contains("9.9.9"), "a version in `{line}`");
        assert!(
            !line
                .split(|c: char| !c.is_ascii_hexdigit())
                .any(|t| t.len() == 64),
            "a sha256 in `{line}`"
        );
    }
    assert_eq!(
        workflow::classify_text(&text, &params(Install::Cargo)),
        Existing::Untouched {
            from: "9.9.9".to_string(),
            params: Workflow {
                version: docsys::agents::TEMPLATE_VERSION.to_string(),
                ..w
            }
        }
    );
}

/// The release archives, as the release workflow's build legs upload them.
fn archives(dir: &Path, version: &str) {
    fs::create_dir_all(dir).unwrap();
    for target in TARGETS {
        let name = format!("docsys-v{version}-{target}");
        let pkg = dir.join(&name);
        fs::create_dir_all(&pkg).unwrap();
        fs::write(
            pkg.join("docsys"),
            format!("#!/bin/sh\necho docsys {version} {target}\n"),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(pkg.join("docsys"), fs::Permissions::from_mode(0o755)).unwrap();
        ok(&run(Command::new("tar")
            .args(["czf", &format!("{name}.tar.gz"), &name])
            .current_dir(dir)));
        fs::remove_dir_all(&pkg).unwrap();
        fs::write(
            dir.join(format!("{name}.tar.gz.sha256")),
            "not what SHA256SUMS lists\n",
        )
        .unwrap();
    }
}

/// A `gh` that serves `release download` from `$FAKE_RELEASE` and records
/// `release upload` into `$FAKE_UPLOADED`.
fn fake_gh(bin: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(bin).unwrap();
    let gh = bin.join("gh");
    fs::write(
        &gh,
        r#"#!/bin/sh
set -e
verb="$1 $2"
shift 2
tag="$1"
shift
case "$verb" in
  "release download")
    dir=. pat='*'
    while [ $# -gt 0 ]; do
      case "$1" in
        --dir) dir="$2"; shift 2 ;;
        --pattern) pat="$2"; shift 2 ;;
        *) shift ;;
      esac
    done
    mkdir -p "$dir"
    for f in "$FAKE_RELEASE"/$pat; do cp "$f" "$dir/"; done ;;
  "release upload")
    echo "$tag" > "$FAKE_UPLOADED/tag"
    for f in "$@"; do
      case "$f" in --*) ;; *) cp "$f" "$FAKE_UPLOADED/" ;; esac
    done ;;
  *) echo "gh: not faked: $verb" >&2; exit 1 ;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)).unwrap();
}

/// Where `tool` is on this machine's PATH.
fn which(tool: &str) -> Option<String> {
    let found = run(Command::new("sh").args(["-c", &format!("command -v {tool}")]));
    let path = String::from_utf8_lossy(&found.stdout).trim().to_string();
    (found.status.success() && path.starts_with('/')).then_some(path)
}

/// `script` as a runner's `shell: bash` step runs it.
fn bash(script: &str, dir: &Path, envs: &[(&str, &str)]) -> Output {
    let mut c = Command::new(which("bash").unwrap());
    c.args(["--noprofile", "--norc", "-eo", "pipefail", "-c", script])
        .current_dir(dir);
    for (k, v) in envs {
        c.env(k, v);
    }
    run(&mut c)
}

/// A directory of links to the tools the install step runs, `sha256sum`
/// left out: a runner where only `shasum` checks.
fn without_sha256sum(dir: &Path) -> String {
    fs::create_dir_all(dir).unwrap();
    for tool in [
        "head", "tr", "uname", "curl", "awk", "tar", "gzip", "shasum", "cat",
    ] {
        if let Some(path) = which(tool) {
            std::os::unix::fs::symlink(path, dir.join(tool)).unwrap();
        }
    }
    assert!(dir.join("shasum").exists(), "the fallback needs shasum");
    dir.display().to_string()
}

/// The release publishes SHA256SUMS beside its archives with no hand step,
/// and the 0.5 install step takes the archive of the version the tree pins
/// only when SHA256SUMS lists its hash — `sha256sum -c`, or `shasum -a 256 -c`
/// on a runner without it.
#[test]
fn the_release_publishes_sha256sums_and_the_install_checks_the_pinned_archive_against_it() {
    let base = tmp("sums");
    // the release workflow's SHA256SUMS step, run against a release that
    // holds the four archives
    let release_yml = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/release.yml"),
    )
    .unwrap();
    assert!(
        release_yml.contains("SHA256SUMS"),
        "the release workflow publishes no SHA256SUMS:\n{release_yml}"
    );
    let sums_step = script_of(&release_yml, "SHA256SUMS");
    let published = base.join("published");
    archives(&published, "1.2.3");
    let uploaded = base.join("uploaded");
    fs::create_dir_all(&uploaded).unwrap();
    let bin = base.join("bin");
    fake_gh(&bin);
    let work = base.join("work");
    fs::create_dir_all(&work).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    ok(&bash(
        &sums_step,
        &work,
        &[
            ("PATH", &path),
            ("GITHUB_REF_NAME", "v1.2.3"),
            ("FAKE_RELEASE", &published.display().to_string()),
            ("FAKE_UPLOADED", &uploaded.display().to_string()),
            ("GH_TOKEN", "unused"),
        ],
    ));
    assert_eq!(
        fs::read_to_string(uploaded.join("tag")).unwrap().trim(),
        "v1.2.3"
    );
    let sums = fs::read_to_string(uploaded.join("SHA256SUMS")).unwrap();
    let mut want: Vec<String> = TARGETS
        .iter()
        .map(|t| {
            let name = format!("docsys-v1.2.3-{t}.tar.gz");
            let bytes = fs::read(published.join(&name)).unwrap();
            format!("{}  {name}", docsys::fresh::sha256_hex(&bytes))
        })
        .collect();
    want.sort();
    let mut got: Vec<String> = sums.lines().map(str::to_string).collect();
    got.sort();
    assert_eq!(got, want, "one sha256sum line per archive:\n{sums}");

    // the release as a runner downloads it
    let releases = base.join("releases");
    let at = releases.join("v1.2.3");
    fs::create_dir_all(&at).unwrap();
    for t in TARGETS {
        let name = format!("docsys-v1.2.3-{t}.tar.gz");
        fs::copy(published.join(&name), at.join(&name)).unwrap();
    }
    fs::write(at.join("SHA256SUMS"), &sums).unwrap();
    let install = script_of(
        &workflow::render(&params(Install::ReleaseSums)),
        "SHA256SUMS",
    )
    .replace(
        workflow::RELEASES,
        &format!("file://{}", releases.display()),
    );
    let repo = base.join("repo");
    write(&repo, "docs/.docsys-version", "1.2.3\n");
    let attempt = |name: &str, path: &str| {
        let runner = base.join(name);
        fs::create_dir_all(&runner).unwrap();
        let gh_path = runner.join("path");
        let out = bash(
            &install,
            &repo,
            &[
                ("PATH", path),
                ("RUNNER_TEMP", &runner.display().to_string()),
                ("GITHUB_PATH", &gh_path.display().to_string()),
            ],
        );
        (out, fs::read_to_string(gh_path).unwrap_or_default())
    };
    let path = std::env::var("PATH").unwrap_or_default();
    let (out, added) = attempt("runner-ok", &path);
    ok(&out);
    let dir = added.trim().to_string();
    assert!(dir.contains("docsys-v1.2.3-"), "{added}");
    let said = ok(&run(&mut Command::new(Path::new(&dir).join("docsys"))));
    assert!(said.starts_with("docsys 1.2.3 "), "{said}");
    // a runner without sha256sum checks with shasum
    let (out, added) = attempt(
        "runner-shasum",
        &without_sha256sum(&base.join("no-sha256sum")),
    );
    ok(&out);
    assert!(added.contains("docsys-v1.2.3-"), "{added}");
    // no pin: one line, nothing installed
    fs::remove_file(repo.join("docs/.docsys-version")).unwrap();
    let (out, added) = attempt("runner-unpinned", &path);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "docsys: docs/.docsys-version names no docsys version to install\n"
    );
    assert_eq!(added, "");
    write(&repo, "docs/.docsys-version", "1.2.3\n");
    // an archive SHA256SUMS does not list: refused
    fs::write(at.join("SHA256SUMS"), "").unwrap();
    let (out, added) = attempt("runner-unlisted", &path);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .contains("the SHA256SUMS of docsys 1.2.3 lists no docsys-v1.2.3-"),
        "{out:?}"
    );
    assert_eq!(added, "");
    // an archive replaced after SHA256SUMS was published: refused
    fs::write(at.join("SHA256SUMS"), &sums).unwrap();
    archives(&base.join("other"), "1.2.3");
    for t in TARGETS {
        let name = format!("docsys-v1.2.3-{t}.tar.gz");
        let mut bytes = fs::read(base.join("other").join(&name)).unwrap();
        bytes.extend_from_slice(b"tampered");
        fs::write(at.join(&name), bytes).unwrap();
    }
    let (out, added) = attempt("runner-tampered", &path);
    assert!(!out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains(": FAILED"),
        "the check refused it: {out:?}"
    );
    assert_eq!(added, "");
    let _ = fs::remove_dir_all(&base);
}

/// `--ci-install release` on a docsys/0.5 tree needs no `--ci-sha256` and
/// refuses one by name; a docsys/0.4 tree keeps 0.15.1's flags and refusals
/// (D-118).
#[test]
fn a_05_tree_takes_a_release_install_without_sha256_and_refuses_the_flag() {
    let v05 = docsys::era::Era::of_spec(Some("docsys/0.5"));
    let v04 = docsys::era::Era::of_spec(Some("docsys/0.4"));
    let ci = Ci::from_flags(None, Some("release"), None, None, v05);
    assert_eq!(
        ci.map(|c| c.map(|c| c.install)),
        Ok(Some(Install::ReleaseSums)),
        "a 0.5 release install takes no sha256"
    );
    let sum = "a".repeat(64);
    for install in [Some("release"), None, Some("cargo")] {
        let err = Ci::from_flags(
            None,
            install,
            Some(&format!("x86_64-unknown-linux-musl={sum}")),
            None,
            v05,
        )
        .unwrap_err();
        assert!(
            err.starts_with("--ci-sha256") && err.contains("SHA256SUMS"),
            "{err}"
        );
    }
    // 0.4: as 0.15.1
    assert!(Ci::from_flags(None, Some("release"), None, None, v04)
        .unwrap_err()
        .contains("--ci-sha256 <target>=<hex>"));
    assert_eq!(
        Ci::from_flags(
            None,
            Some("release"),
            Some(&format!("x86_64-unknown-linux-musl={sum}")),
            None,
            v04
        )
        .map(|c| c.map(|c| c.install)),
        Ok(Some(Install::Release(vec![(
            "x86_64-unknown-linux-musl".to_string(),
            sum.clone()
        )])))
    );
    // the binary: a new tree is docsys/0.5
    let repo = tmp("adopt-release");
    git(&repo, &["init", "-q", "-b", "main"]);
    fs::create_dir_all(repo.join(".github")).unwrap();
    let out = docsys(&repo, &["adopt", "--ci-install", "release"]);
    ok(&out);
    let text = fs::read_to_string(repo.join(FILE)).unwrap();
    assert!(text.lines().next().unwrap().contains(" install=release "));
    assert!(text.contains("checked against its SHA256SUMS"), "{text}");
    let refused = tmp("adopt-release-sha");
    git(&refused, &["init", "-q", "-b", "main"]);
    fs::create_dir_all(refused.join(".github")).unwrap();
    let out = docsys(
        &refused,
        &[
            "adopt",
            "--ci-install",
            "release",
            "--ci-sha256",
            &format!("x86_64-unknown-linux-musl={sum}"),
        ],
    );
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("SHA256SUMS"),
        "{out:?}"
    );
    assert!(!refused.join(FILE).exists() && !refused.join("docs").exists());
    let _ = fs::remove_dir_all(&repo);
    let _ = fs::remove_dir_all(&refused);
}

/// The shape a real repository gave its workflow: its own runners and
/// timeout, an `env:` block with the version and the archives' sha256
/// values, and an install step that reads them.
const OWNER: &str = r#"# docs checks, adapted by hand from what `docsys adopt` wrote.
name: docsys

on:
  pull_request:
  push:
    branches: [develop]

permissions:
  contents: read

concurrency:
  group: docsys-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true

env:
  # The docsys the tree is authored against: bump it with the local install,
  # since a newer binary can carry rules this tree was never checked against.
  DOCSYS_VERSION: v0.15.1
  # The release archives' sha256, so a replaced asset fails the install.
  DOCSYS_SHA256_X86_64: 1111111111111111111111111111111111111111111111111111111111111111
  DOCSYS_SHA256_AARCH64: 2222222222222222222222222222222222222222222222222222222222222222

jobs:
  docs:
    # Only these runners reach the network the checkout needs.
    runs-on: [self-hosted, linux, 'req:2cpu', 'req:4gb']
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install docsys
        # The prebuilt static binary: this runner image has no Rust toolchain.
        working-directory: ${{ runner.temp }}
        run: |
          set -euo pipefail
          case "$(uname -m)" in
            x86_64)        target=x86_64-unknown-linux-musl;  sha256="${DOCSYS_SHA256_X86_64}" ;;
            aarch64|arm64) target=aarch64-unknown-linux-musl; sha256="${DOCSYS_SHA256_AARCH64}" ;;
            *) echo "no docsys release for $(uname -m)" >&2; exit 1 ;;
          esac
          dir="docsys-${DOCSYS_VERSION}-${target}"
          base="https://releases.example.invalid/docsys/releases/download/${DOCSYS_VERSION}"
          curl -fsSL --retry 3 -O "${base}/${dir}.tar.gz" || {
            echo "docsys ${DOCSYS_VERSION}: ${dir}.tar.gz could not be fetched" >&2
            exit 1
          }
          echo "${sha256}  ${dir}.tar.gz" | sha256sum -c -
          tar -xzf "${dir}.tar.gz"
          mkdir -p bin
          install -m 0755 "${dir}/docsys" bin/docsys
          echo "${RUNNER_TEMP}/bin" >> "$GITHUB_PATH"

      - run: docsys lint --root docs --repo .
      - run: docsys refs --repo . --root docs
      - if: github.event_name == 'pull_request'
        run: docsys gate --repo . --root docs --range "origin/${{ github.base_ref }}...HEAD"
"#;

/// The same install under a runner list one expression builds.
fn owner_listed_runners() -> String {
    OWNER.replace(
        "    runs-on: [self-hosted, linux, 'req:2cpu', 'req:4gb']\n",
        "    runs-on:\n      - \"${{ format('self-hosted@{0}-{1}', github.repository_id, github.run_id) }}\"\n      - \"linux\"\n      - \"req:1cpu\"\n      - \"lim:2gb\"\n",
    )
}

/// This version's 0.5 release install step for a tree at `docs`, as `adopt`
/// renders it.
fn release_step() -> String {
    let text = workflow::render(&params(Install::ReleaseSums));
    let mut out = String::new();
    let mut on = false;
    for l in text.lines() {
        if l.starts_with("      - name: docsys, the release the tree pins") {
            on = true;
        } else if on && l.starts_with("      - ") {
            break;
        }
        if on {
            out.push_str(l);
            out.push('\n');
        }
    }
    assert!(out.contains("SHA256SUMS"), "{text}");
    out
}

/// The owner's install step in `owner`, through its last line.
fn install_step(owner: &str) -> String {
    owner
        .split_once("      - name: Install docsys\n")
        .map(|(_, rest)| {
            format!(
                "      - name: Install docsys\n{}\n",
                rest.split_once("\n\n").unwrap().0
            )
        })
        .unwrap()
}

/// `owner` as the move leaves it: the version and sha256 lines gone with the
/// comments that explain them, the install step `step`, every other line as
/// the owner wrote it.
fn owner_moved_to(owner: &str, step: &str) -> String {
    let env_block = owner
        .split_once("\nenv:\n")
        .map(|(_, rest)| format!("env:\n{}\n", rest.split_once("\n\n").unwrap().0))
        .unwrap();
    owner
        .replacen(&format!("{env_block}\n"), "", 1)
        .replacen(&install_step(owner), step, 1)
}

fn owner_moved(owner: &str) -> String {
    owner_moved_to(owner, &release_step())
}

#[test]
fn an_owners_install_is_replaced_and_every_other_line_kept() {
    for owner in [OWNER.to_string(), owner_listed_runners()] {
        let Reinstall::Rewritten { text, what } = workflow::reinstall(&owner, "docs", "0.16.0")
        else {
            panic!("{:?}", workflow::reinstall(&owner, "docs", "0.16.0"));
        };
        let want = owner_moved(&owner);
        assert!(
            text == want,
            "{}",
            docsys::diff::unified(&want, &text, "expected", "rewritten", 2)
        );
        assert!(
            !text.contains("DOCSYS_") && !text.contains("0.15.1"),
            "{text}"
        );
        assert!(what.contains("SHA256SUMS"), "{what}");
        // the rewritten file is left alone by the next run
        assert_eq!(
            workflow::reinstall(&text, "docs", "0.16.0"),
            Reinstall::Pinned
        );
    }
    // an `env:` that holds more than docsys keeps the rest, a value on more
    // lines than one included
    let more = OWNER.replace(
        "  DOCSYS_SHA256_AARCH64: 2222222222222222222222222222222222222222222222222222222222222222\n",
        "  DOCSYS_SHA256_AARCH64: 2222222222222222222222222222222222222222222222222222222222222222\n  RELEASE_NOTES: |\n    kept as the owner wrote it\n",
    );
    let Reinstall::Rewritten { text, .. } = workflow::reinstall(&more, "docs", "0.16.0") else {
        panic!("{:?}", workflow::reinstall(&more, "docs", "0.16.0"));
    };
    assert!(
        text.contains("\nenv:\n  RELEASE_NOTES: |\n    kept as the owner wrote it\n\njobs:\n"),
        "{text}"
    );
    assert!(!text.contains("DOCSYS_"), "{text}");
    // a file with CRLF line ends keeps them
    let crlf = OWNER.replace('\n', "\r\n");
    let Reinstall::Rewritten { text, .. } = workflow::reinstall(&crlf, "docs", "0.16.0") else {
        panic!("{:?}", workflow::reinstall(&crlf, "docs", "0.16.0"));
    };
    assert_eq!(text, owner_moved(OWNER).replace('\n', "\r\n"));
    // what this version renders installs from the pin already
    for install in [Install::Cargo, Install::ReleaseSums] {
        assert_eq!(
            workflow::reinstall(&workflow::render(&params(install)), "docs", "0.16.0"),
            Reinstall::Pinned
        );
    }
    // a 0.4 release install edited by its owner: its version and sha256
    // values go with its step
    let old = workflow::render(&params(Install::Release(vec![(
        "x86_64-unknown-linux-musl".to_string(),
        "3".repeat(64),
    )])))
    .replace(
        "runs-on: [self-hosted, linux]",
        "runs-on: [self-hosted, linux, x64]",
    );
    let Reinstall::Rewritten { text, .. } = workflow::reinstall(&old, "docs", "0.16.0") else {
        panic!("{:?}", workflow::reinstall(&old, "docs", "0.16.0"));
    };
    assert!(text.contains(&release_step()), "{text}");
    assert!(
        text.contains("runs-on: [self-hosted, linux, x64]"),
        "{text}"
    );
    assert!(
        !text.contains(&"3".repeat(64)) && !text.contains("docsys 9.9.9"),
        "{text}"
    );
    // a cargo install that names its version becomes this version's cargo
    // install, which reads the pin; the sha256 values nothing reads go too
    let cargo = OWNER.replacen(
        &install_step(OWNER),
        "      - name: Install docsys\n        run: cargo install docsys --version \"${DOCSYS_VERSION#v}\" --locked\n",
        1,
    );
    let Reinstall::Rewritten { text, .. } = workflow::reinstall(&cargo, "docs", "0.16.0") else {
        panic!("{:?}", workflow::reinstall(&cargo, "docs", "0.16.0"));
    };
    let rendered = workflow::render(&Workflow {
        version: "0.16.0".to_string(),
        ..params(Install::Cargo)
    });
    let cargo_steps = rendered
        .split_once("          fetch-depth: 0\n")
        .and_then(|(_, rest)| rest.split_once("      - run: docsys lint"))
        .map(|(steps, _)| steps.to_string())
        .unwrap();
    assert!(
        cargo_steps.contains("steps.docsys-pin.outputs.version"),
        "{cargo_steps}"
    );
    assert_eq!(text, owner_moved_to(&cargo, &cargo_steps));
}

/// What installs docsys, found ambiguously: nothing is rewritten, and the
/// one line to change is named with what to change there.
#[test]
fn an_install_that_cannot_be_told_apart_is_named_never_rewritten() {
    let line_of = |text: &str, needle: &str| {
        text.lines()
            .position(|l| l.contains(needle))
            .map(|i| i + 1)
            .unwrap()
    };
    let unclear = |text: &str| match workflow::reinstall(text, "docs", "0.16.0") {
        Reinstall::Unclear { line, what } => (line, what),
        other => panic!("{other:?} for:\n{text}"),
    };
    // a script of the owner's that installs a version docsys cannot replace
    let legacy = include_str!("../migrations/workflow-0.15.yml").replace("@ROOT@", "docs");
    let script = legacy.replacen(
        "      - run: cargo install docsys\n",
        "      - run: ./tools/install-docsys.sh v0.15.1 # the docsys the tree was authored with\n",
        1,
    );
    let (line, what) = unclear(&script);
    assert_eq!(line, line_of(&script, "install-docsys.sh"), "{what}");
    assert!(
        what.contains("it installs docsys 0.15.1, and the tree pins 0.16.0"),
        "{what}"
    );
    assert!(what.contains("docs/.docsys-version"), "{what}");
    // the binary run from where the owner's step left it
    let path = OWNER.replace(
        "      - run: docsys lint --root docs --repo .",
        "      - run: \"$RUNNER_TEMP/bin/docsys\" lint --root docs --repo .",
    );
    let (line, what) = unclear(&path);
    assert_eq!(line, line_of(&path, "/bin/docsys\" lint"), "{what}");
    assert!(what.contains("PATH"), "{what}");
    // the version read outside the install step too
    let shared = OWNER.replace(
        "      - run: docsys refs --repo . --root docs\n",
        "      - run: docsys refs --repo . --root docs\n      - run: echo \"checked with docsys $DOCSYS_VERSION\"\n",
    );
    let (line, what) = unclear(&shared);
    assert_eq!(line, line_of(&shared, "checked with docsys"), "{what}");
    assert!(what.contains("DOCSYS_VERSION"), "{what}");
    // an install step that runs a check of its own
    let busy = OWNER.replace(
        "          echo \"${RUNNER_TEMP}/bin\" >> \"$GITHUB_PATH\"\n",
        "          echo \"${RUNNER_TEMP}/bin\" >> \"$GITHUB_PATH\"\n          bin/docsys lint --root ../docs\n",
    );
    let (line, what) = unclear(&busy);
    assert_eq!(line, line_of(&busy, "- name: Install docsys"), "{what}");
    assert!(what.contains("lint"), "{what}");
    // an action that installs docsys
    let action = legacy.replacen(
        "      - run: cargo install docsys\n",
        "      - uses: example/setup-docsys@v1\n        with:\n          version: 0.15.1\n",
        1,
    );
    let (line, what) = unclear(&action);
    assert_eq!(line, line_of(&action, "setup-docsys"), "{what}");
    assert!(what.contains("docs/.docsys-version"), "{what}");
}

fn tree_04(name: &str, workflow_text: &str) -> PathBuf {
    let repo = tmp(name);
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    write(
        &repo,
        "docs/.docmeta.yml",
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n",
    );
    write(
        &repo,
        "docs/index.md",
        "---\ntitle: Docs\ntype: index\nupdated: 2026-09-01\n---\n\n# Docs\n",
    );
    write(&repo, FILE, workflow_text);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "a tree 0.15 adopted"]);
    repo
}

/// The `ci-workflow` rows of a run's text output.
fn ci_rows(out: &str) -> Vec<&str> {
    out.lines()
        .filter(|l| l.split_whitespace().nth(1) == Some("ci-workflow"))
        .collect()
}

/// The move, on an owner's workflow: the plan shows the replacement as an
/// `auto` row with its diff and writes nothing; `--apply` writes it into the
/// move's commit; the leftover check after lists nothing for the workflow.
#[test]
fn the_move_replaces_an_owners_install_and_leaves_nothing_for_ci() {
    let own = env!("CARGO_PKG_VERSION");
    let repo = tree_04("owner-move", OWNER);
    let plan = ok(&docsys(&repo, &["upgrade"]));
    let rows = ci_rows(&plan);
    assert_eq!(rows.len(), 1, "{plan}");
    assert!(
        rows.first()
            .unwrap()
            .starts_with(&format!("auto    ci-workflow        {FILE}  ")),
        "{plan}"
    );
    let diff = plan
        .split_once(&format!("\n# {FILE}\n"))
        .map(|(_, d)| d)
        .unwrap_or_else(|| panic!("no diff for the workflow:\n{plan}"));
    for line in [
        "-  DOCSYS_VERSION: v0.15.1\n",
        "-      - name: Install docsys\n",
        "+      - name: docsys, the release the tree pins, checked against its SHA256SUMS\n",
    ] {
        assert!(diff.contains(line), "`{line}` not in:\n{diff}");
    }
    assert!(!diff.contains("runs-on"), "{diff}");
    assert_eq!(fs::read_to_string(repo.join(FILE)).unwrap(), OWNER);
    let json = ok(&docsys(&repo, &["upgrade", "--json"]));
    assert!(json.contains("\"step\": \"ci-workflow\""), "{json}");
    ok(&docsys(&repo, &["upgrade", "--apply", "--commit"]));
    assert_eq!(
        fs::read_to_string(repo.join(FILE)).unwrap(),
        owner_moved(OWNER)
    );
    assert_eq!(
        fs::read_to_string(repo.join("docs/.docsys-version")).unwrap(),
        format!("{own}\n")
    );
    assert!(git(&repo, &["show", "--stat", "HEAD"]).contains("docsys.yml"));
    let idle = ok(&docsys(&repo, &["upgrade"]));
    assert!(ci_rows(&idle).is_empty(), "{idle}");
    let _ = fs::remove_dir_all(&repo);
}

/// An install the move cannot tell apart: one `manual` row at its line, the
/// file as the owner wrote it.
#[test]
fn the_move_names_an_unclear_install_and_writes_nothing_there() {
    let owner = OWNER.replace(
        "      - run: docsys lint --root docs --repo .",
        "      - run: \"$RUNNER_TEMP/bin/docsys\" lint --root docs --repo .",
    );
    let line = owner
        .lines()
        .position(|l| l.contains("/bin/docsys\" lint"))
        .unwrap()
        + 1;
    let repo = tree_04("owner-unclear", &owner);
    let out = ok(&docsys(&repo, &["upgrade"]));
    let rows = ci_rows(&out);
    assert_eq!(rows.len(), 1, "{out}");
    assert!(
        rows.first()
            .unwrap()
            .starts_with(&format!("manual  ci-workflow        {FILE}:{line}  ")),
        "{out}"
    );
    assert!(!out.contains(&format!("\n# {FILE}\n")), "{out}");
    ok(&docsys(&repo, &["upgrade", "--apply", "--commit"]));
    assert_eq!(fs::read_to_string(repo.join(FILE)).unwrap(), owner);
    assert!(!git(&repo, &["show", "--stat", "HEAD"]).contains("docsys.yml"));
    let _ = fs::remove_dir_all(&repo);
}

/// An untouched 0.4 release install: regenerated as the 0.5 one, its sha256
/// values gone, and nothing left for the leftover check.
#[test]
fn an_untouched_04_release_install_is_regenerated_without_sha256() {
    let old = Workflow {
        version: "0.15.9".to_string(),
        ci: Ci {
            verify: Verify::Off,
            ..params(Install::Release(vec![(
                "x86_64-unknown-linux-musl".to_string(),
                "4".repeat(64),
            )]))
            .ci
        },
        ..params(Install::Cargo)
    };
    let repo = tree_04("untouched-release", &workflow::render(&old));
    let out = ok(&docsys(&repo, &["upgrade", "--apply", "--commit"]));
    let rows = ci_rows(&out);
    assert_eq!(rows.len(), 1, "{out}");
    assert!(
        rows.first()
            .unwrap()
            .starts_with(&format!("auto    ci-workflow        {FILE}  "))
            && rows.first().unwrap().contains("SHA256SUMS"),
        "{out}"
    );
    let text = fs::read_to_string(repo.join(FILE)).unwrap();
    assert_eq!(
        text,
        workflow::render(&Workflow {
            version: docsys::agents::TEMPLATE_VERSION.to_string(),
            ci: Ci {
                install: Install::ReleaseSums,
                ..old.ci.clone()
            },
            ..old.clone()
        })
    );
    let idle = ok(&docsys(&repo, &["upgrade"]));
    assert!(ci_rows(&idle).is_empty(), "{idle}");
    let _ = fs::remove_dir_all(&repo);
}
