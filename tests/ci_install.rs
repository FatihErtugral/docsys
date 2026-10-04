#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// D-111, D-120: on a docsys/0.5 tree the CI workflow installs the version
// `docsys upgrade` pinned in it, the archive checked against the sha256
// values the upgrade read from the release: an owner's install becomes that
// one, with no hand edit to CI. The release is a `file://` directory here;
// tests/ci_pins.rs runs the step itself.

use docsys::workflow::{self, Ci, Existing, Install, Reinstall, Verify, Workflow};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const FILE: &str = ".github/workflows/docsys.yml";
const OWN: &str = env!("CARGO_PKG_VERSION");

/// The sha256 values the stand-in release publishes for this version.
fn stub_sums() -> Vec<(String, String)> {
    TARGETS
        .iter()
        .zip(["a", "b", "c", "d"])
        .map(|(t, hex)| (t.to_string(), hex.repeat(64)))
        .collect()
}

/// This version's release as a `file://` directory: each archive's
/// `.sha256` file, the values `stub_sums` gives.
fn stub_release() -> &'static str {
    static AT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    AT.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("docsys-ci-release-{}", std::process::id()));
        let at = dir.join(format!("v{OWN}"));
        fs::create_dir_all(&at).unwrap();
        for (target, hex) in stub_sums() {
            let name = format!("docsys-v{OWN}-{target}.tar.gz");
            fs::write(
                at.join(format!("{name}.sha256")),
                format!("{hex}  {name}\n"),
            )
            .unwrap();
        }
        format!("file://{}", dir.display())
    })
}
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

/// Runs this build in `dir`, first on PATH: a git hook the move writes runs
/// this build too.
fn docsys(dir: &Path, args: &[&str]) -> Output {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_docsys"));
    let path = format!(
        "{}:{}",
        bin.parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    run(Command::new(&bin)
        .args(args)
        .env("PATH", path)
        .env("DOCSYS_RELEASES", stub_release())
        .current_dir(dir))
}

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
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

/// A docsys/0.5 release install: the version and each archive's sha256 the
/// upgrade read from the release, in the step — line 1's stamp and hash work
/// as for every rendering, and the file reads back as itself (D-111).
#[test]
fn a_05_release_install_pins_its_version_and_sha256_values() {
    let w = params(Install::ReleasePinned(stub_sums()));
    let text = workflow::render(&w);
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
    let golden_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/workflow-release-pinned.yml");
    if std::env::var_os("DOCSYS_BLESS").is_some() {
        fs::write(&golden_path, &shown).unwrap();
    }
    let golden = fs::read_to_string(&golden_path).unwrap();
    assert!(
        shown == golden,
        "the 0.5 release install differs from the golden file:\n{}",
        docsys::diff::unified(&golden, &shown, "golden", "rendered", 2)
    );
    for (target, sum) in stub_sums() {
        assert!(
            text.contains(&format!(" target={target} sum={sum} ;;\n")),
            "{text}"
        );
    }
    assert!(!text.contains("SHA256SUMS"), "{text}");
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
        Ok(Some(Install::ReleasePinned(Vec::new()))),
        "a 0.5 release install takes no sha256: the upgrade reads them"
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
            err.starts_with("--ci-sha256") && err.contains("from the release"),
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
    assert!(
        text.contains("the release the tree pins, checked against the sha256 its upgrade wrote"),
        "{text}"
    );
    for (target, sum) in stub_sums() {
        assert!(
            text.contains(&format!(" target={target} sum={sum} ;;\n")),
            "{text}"
        );
    }
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
        String::from_utf8_lossy(&out.stderr).contains("--ci-sha256"),
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
    let text = workflow::render(&Workflow {
        version: OWN.to_string(),
        ..params(Install::ReleasePinned(stub_sums()))
    });
    let mut out = String::new();
    let mut on = false;
    for l in text.lines() {
        if l.starts_with("      - name: docsys ") && l.contains("the release the tree pins") {
            on = true;
        } else if on && l.starts_with("      - ") {
            break;
        }
        if on {
            out.push_str(l);
            out.push('\n');
        }
    }
    assert!(out.contains("the release the tree pins"), "{text}");
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
        let Reinstall::Rewritten { text, what } =
            workflow::reinstall(&owner, "docs", OWN, Some(&stub_sums()))
        else {
            panic!(
                "{:?}",
                workflow::reinstall(&owner, "docs", OWN, Some(&stub_sums()))
            );
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
        assert!(
            what.contains(&format!("installs docsys {OWN} and checks its archive against the sha256 values this upgrade read from the release")),
            "{what}"
        );
        // the rewritten file is left alone by the next run
        assert_eq!(
            workflow::reinstall(&text, "docs", OWN, None),
            Reinstall::Pinned
        );
        // and a release install still to be written asks for the values
        assert_eq!(
            workflow::reinstall(&owner, "docs", OWN, None),
            Reinstall::NeedsRelease
        );
    }
    // an `env:` that holds more than docsys keeps the rest, a value on more
    // lines than one included
    let more = OWNER.replace(
        "  DOCSYS_SHA256_AARCH64: 2222222222222222222222222222222222222222222222222222222222222222\n",
        "  DOCSYS_SHA256_AARCH64: 2222222222222222222222222222222222222222222222222222222222222222\n  RELEASE_NOTES: |\n    kept as the owner wrote it\n",
    );
    let Reinstall::Rewritten { text, .. } =
        workflow::reinstall(&more, "docs", OWN, Some(&stub_sums()))
    else {
        panic!(
            "{:?}",
            workflow::reinstall(&more, "docs", OWN, Some(&stub_sums()))
        );
    };
    assert!(
        text.contains("\nenv:\n  RELEASE_NOTES: |\n    kept as the owner wrote it\n\njobs:\n"),
        "{text}"
    );
    assert!(!text.contains("DOCSYS_"), "{text}");
    // a file with CRLF line ends keeps them
    let crlf = OWNER.replace('\n', "\r\n");
    let Reinstall::Rewritten { text, .. } =
        workflow::reinstall(&crlf, "docs", OWN, Some(&stub_sums()))
    else {
        panic!(
            "{:?}",
            workflow::reinstall(&crlf, "docs", OWN, Some(&stub_sums()))
        );
    };
    assert_eq!(text, owner_moved(OWNER).replace('\n', "\r\n"));
    // what this version renders for this version is current already
    for install in [Install::Cargo, Install::ReleasePinned(stub_sums())] {
        let rendered = workflow::render(&Workflow {
            version: OWN.to_string(),
            ..params(install)
        });
        assert_eq!(
            workflow::reinstall(&rendered, "docs", OWN, None),
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
    let Reinstall::Rewritten { text, .. } =
        workflow::reinstall(&old, "docs", OWN, Some(&stub_sums()))
    else {
        panic!(
            "{:?}",
            workflow::reinstall(&old, "docs", OWN, Some(&stub_sums()))
        );
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
    let Reinstall::Rewritten { text, .. } = workflow::reinstall(&cargo, "docs", OWN, None) else {
        panic!("{:?}", workflow::reinstall(&cargo, "docs", OWN, None));
    };
    let rendered = workflow::render(&Workflow {
        version: OWN.to_string(),
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
    let unclear = |text: &str| match workflow::reinstall(text, "docs", OWN, None) {
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
        what.contains(&format!(
            "it installs docsys 0.15.1, and docs/.docsys-version pins {OWN}"
        )),
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
        &format!("+      - name: docsys {OWN}, the release the tree pins, checked against the sha256 its upgrade wrote\n"),
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
            && rows.first().unwrap().contains(
                "pins this version and its archives' sha256 values, read from the release"
            ),
        "{out}"
    );
    let text = fs::read_to_string(repo.join(FILE)).unwrap();
    assert_eq!(
        text,
        workflow::render(&Workflow {
            version: docsys::agents::TEMPLATE_VERSION.to_string(),
            ci: Ci {
                install: Install::ReleasePinned(stub_sums()),
                ..old.ci.clone()
            },
            ..old.clone()
        })
    );
    let idle = ok(&docsys(&repo, &["upgrade"]));
    assert!(ci_rows(&idle).is_empty(), "{idle}");
    let _ = fs::remove_dir_all(&repo);
}
