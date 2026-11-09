//! The GitHub workflow `adopt` writes (D-111), and what may happen to one
//! that exists. Every parameter of a rendering is on its line 1 with a hash
//! of the rest of the file, so a later binary can tell a file nobody edited —
//! regenerable with the same parameters — from one that is its owner's, which
//! is never written, only shown as a diff.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Where a release's archives are downloaded from.
pub const RELEASES: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/releases/download");

/// Each target's archive sha256 for `version`, read from the `.sha256` file
/// the release publishes beside each archive (`DOCSYS_RELEASES` names another
/// place than `RELEASES`, a mirror or a `file://` directory), in `TARGETS`
/// order. Every target or nothing: a value never read is never written.
pub fn release_sums_of(version: &str) -> Result<Vec<(String, String)>, String> {
    let base = std::env::var("DOCSYS_RELEASES")
        .ok()
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| RELEASES.to_string());
    let mut out = Vec::new();
    for (target, _) in TARGETS {
        let archive = format!("docsys-v{version}-{target}.tar.gz");
        let url = format!("{base}/v{version}/{archive}.sha256");
        let got = std::process::Command::new("curl")
            .args(["-fsSL", "--connect-timeout", "10", "--max-time", "60", &url])
            .output()
            .map_err(|e| format!("curl could not run ({e})"))?;
        if !got.status.success() {
            let why = String::from_utf8_lossy(&got.stderr);
            return Err(format!("{url} could not be read: {}", why.trim()));
        }
        let text = String::from_utf8_lossy(&got.stdout);
        let mut words = text.split_whitespace();
        match (words.next(), words.next()) {
            (Some(hex), Some(name))
                if hex.len() == 64
                    && hex.bytes().all(|b| b.is_ascii_hexdigit())
                    && name.trim_start_matches('*') == archive =>
            {
                out.push((target.to_string(), hex.to_ascii_lowercase()));
            }
            _ => return Err(format!("{url} holds no sha256 line for {archive}")),
        }
    }
    Ok(out)
}

/// How the runners get docsys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Install {
    /// `cargo install docsys --version <v> --locked`, cached by version
    Cargo,
    /// the release archive for the runner's target, checked against the
    /// sha256 the person supplied: `(target, hex)` — a docsys/0.4 tree's
    Release(Vec<(String, String)>),
    /// docsys/0.5: the release archive of the version `docsys upgrade` pinned,
    /// checked against the sha256 values it wrote from that release:
    /// `(target, hex)`, empty while they are still to be read
    ReleasePinned(Vec<(String, String)>),
    /// docsys/0.6: the tree's own installer, `<root>/.docsys-install`, which
    /// holds the pinned release's sha256 values — the one installer CI and
    /// every clone run (D-138)
    Tree,
}

/// What a docsys/0.4 tree's verify-on-approval job does with the records
/// (D-105); a docsys/0.5 tree keeps no verification and has no such job
/// (D-130).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verify {
    PullRequest,
    Direct,
    Off,
}

impl Verify {
    pub fn name(self) -> &'static str {
        match self {
            Verify::PullRequest => "pull-request",
            Verify::Direct => "direct",
            Verify::Off => "off",
        }
    }

    /// The mode a tree takes: a docsys/0.4 tree's job writes the records it
    /// reads (D-105); a docsys/0.5 tree has none (D-130).
    pub fn of_era(era: crate::era::Era) -> Verify {
        if era.page_verification() {
            Verify::PullRequest
        } else {
            Verify::Off
        }
    }

    /// A job a docsys/0.5 tree would never read is refused.
    pub fn readable_by(self, era: crate::era::Era) -> Result<(), String> {
        match (era.page_verification(), self) {
            (false, Verify::PullRequest | Verify::Direct) => Err(format!(
                "--verify-on-approval {}: a docsys/0.5 page carries no verification, so the workflow has no approval job (D-130)",
                self.name()
            )),
            _ => Ok(()),
        }
    }

    fn named(s: &str) -> Option<Verify> {
        [Verify::PullRequest, Verify::Direct, Verify::Off]
            .into_iter()
            .find(|v| v.name() == s)
    }
}

/// The adopt flags' part of a workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ci {
    pub runner: Vec<String>,
    pub install: Install,
    pub verify: Verify,
}

impl Default for Ci {
    fn default() -> Ci {
        Ci {
            runner: vec!["ubuntu-latest".to_string()],
            install: Install::Cargo,
            verify: Verify::Off,
        }
    }
}

/// The release archives a workflow can install, by the `uname -s`-`uname -m`
/// of the runner that takes each.
const TARGETS: [(&str, &str); 4] = [
    ("x86_64-unknown-linux-musl", "Linux-x86_64"),
    ("aarch64-unknown-linux-musl", "Linux-aarch64"),
    ("x86_64-apple-darwin", "Darwin-x86_64"),
    ("aarch64-apple-darwin", "Darwin-arm64"),
];

fn is_label(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

impl Ci {
    /// `--ci-runner`, `--ci-install`, `--ci-sha256`, `--verify-on-approval`;
    /// `None` when none is given.
    pub fn from_flags(
        runner: Option<&str>,
        install: Option<&str>,
        sha256: Option<&str>,
        verify: Option<&str>,
        era: crate::era::Era,
    ) -> Result<Option<Ci>, String> {
        if runner.is_none() && install.is_none() && sha256.is_none() && verify.is_none() {
            return Ok(None);
        }
        let mut ci = Ci {
            verify: Verify::of_era(era),
            ..Ci::default()
        };
        if let Some(r) = runner {
            let labels: Vec<String> = r.split(',').map(|l| l.trim().to_string()).collect();
            if let Some(bad) = labels.iter().find(|l| !is_label(l)) {
                return Err(format!(
                    "--ci-runner: `{bad}` is not a runner label — letters, digits, `.`, `_` and `-`, several separated by commas"
                ));
            }
            ci.runner = labels;
        }
        if let Some(v) = verify {
            ci.verify = Verify::named(v).ok_or_else(|| {
                format!("--verify-on-approval takes pull-request, direct or off, not `{v}`")
            })?;
            ci.verify.readable_by(era)?;
        }
        let known = || {
            TARGETS
                .iter()
                .map(|(t, _)| *t)
                .collect::<Vec<_>>()
                .join(", ")
        };
        // docsys/0.5: the values read from the release, nothing written by hand
        if era.pinned_ci_install() {
            if sha256.is_some() {
                return Err("--ci-sha256: on a docsys/0.5 tree docsys reads the release archives' sha256 values from the release and writes them itself — leave the flag out".to_string());
            }
            if install == Some("release") {
                // docsys/0.6: the tree's installer holds the values (D-138)
                ci.install = if era.tree_installer() {
                    Install::Tree
                } else {
                    Install::ReleasePinned(Vec::new())
                };
                return Ok(Some(ci));
            }
        }
        match (install, sha256) {
            (None | Some("cargo"), None) => {}
            (None | Some("cargo"), Some(_)) => {
                return Err("--ci-sha256 belongs to --ci-install release".to_string())
            }
            (Some("release"), None) => {
                return Err(format!(
                    "--ci-install release needs --ci-sha256 <target>=<hex>,… — the sha256 of each release archive your runners download ({}), copied from the release page; docsys cannot know them offline",
                    known()
                ))
            }
            (Some("release"), Some(list)) => {
                let mut sums: Vec<(String, String)> = Vec::new();
                for entry in list.split(',').map(str::trim) {
                    let Some((target, hex)) = entry.split_once('=') else {
                        return Err(format!(
                            "--ci-sha256: `{entry}` is not <target>=<64 hex digits>"
                        ));
                    };
                    let (target, hex) = (target.trim(), hex.trim().to_ascii_lowercase());
                    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                        return Err(format!(
                            "--ci-sha256: `{entry}` is not <target>=<64 hex digits>"
                        ));
                    }
                    if !TARGETS.iter().any(|(t, _)| *t == target) {
                        return Err(format!(
                            "--ci-sha256: no release archive for `{target}` — one of {}",
                            known()
                        ));
                    }
                    if sums.iter().any(|(t, _)| t == target) {
                        return Err(format!("--ci-sha256 names `{target}` twice"));
                    }
                    sums.push((target.to_string(), hex));
                }
                ci.install = Install::Release(sums);
            }
            (Some(other), _) => {
                return Err(format!("--ci-install takes cargo or release, not `{other}`"))
            }
        }
        Ok(Some(ci))
    }
}

/// Everything a rendering depends on, recorded on its line 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workflow {
    pub version: String,
    pub branch: String,
    pub root: String,
    pub ci: Ci,
}

const STAMP: &str = "# docsys-template: ";

const BODY: &str = r#"# docsys documentation workflow, written by `docsys adopt` (D-111). Line 1 records how
# it was rendered and a hash of everything else in this file: while the hash holds,
# docsys may regenerate it; once the file is edited, it is yours.
name: docsys

on:
  pull_request:
    types: [opened, synchronize, reopened, closed]
@REVIEW@  push:
    branches: [@BRANCH@]

permissions:
  contents: read

jobs:
  docs:
    if: @DOCS_IF@
    runs-on: @RUNNER@
    concurrency:
      group: docsys-${{ github.event.pull_request.number || github.ref }}
      cancel-in-progress: true
    steps:
      - uses: actions/checkout@v5
        with:
          fetch-depth: 0
@INSTALL@      - run: docsys lint --repo . --root @ROOT@
      - run: docsys refs --repo . --root @ROOT@
      - if: github.event_name == 'pull_request'
        run: docsys gate --repo . --root @ROOT@ --range "origin/${{ github.base_ref }}...HEAD"
@VERIFY@"#;

const INSTALL_CARGO: &str = r#"      - name: docsys, the version the tree pins
        id: docsys-pin
        shell: bash
        run: |
          v=$(head -n 1 "@ROOT@/.docsys-version" 2>/dev/null || true)
          echo "version=${v:-@VERSION@}" >> "$GITHUB_OUTPUT"
      - uses: actions/cache@v4
        id: docsys-bin
        with:
          path: ~/.cargo/bin/docsys
          key: docsys-${{ steps.docsys-pin.outputs.version }}-${{ runner.os }}-${{ runner.arch }}
      - if: steps.docsys-bin.outputs.cache-hit != 'true'
        run: cargo install docsys --version "${{ steps.docsys-pin.outputs.version }}" --locked
"#;

const INSTALL_RELEASE: &str = r#"      - name: docsys @VERSION@, the release archive checked against its sha256
        shell: bash
        run: |
          v=$(head -n 1 "@ROOT@/.docsys-version" 2>/dev/null || true)
          if [ -n "$v" ] && [ "$v" != "@VERSION@" ]; then
            echo "docsys: the tree pins docsys $v; this workflow holds the sha256 values of @VERSION@ — write those of $v" >&2
            exit 1
          fi
          case "$(uname -s)-$(uname -m)" in
@ARMS@            *) echo "docsys: this workflow holds no sha256 for a $(uname -s)-$(uname -m) runner" >&2; exit 1 ;;
          esac
          name="docsys-v@VERSION@-$target"
          curl -fsSL -o "$RUNNER_TEMP/$name.tar.gz" "@RELEASES@/v@VERSION@/$name.tar.gz"
          got=$(cd "$RUNNER_TEMP" && { sha256sum "$name.tar.gz" 2>/dev/null || shasum -a 256 "$name.tar.gz"; } | cut -d ' ' -f 1)
          if [ "$got" != "$sum" ]; then
            echo "docsys: $name.tar.gz hashes to $got, not the $sum this workflow pins" >&2
            exit 1
          fi
          tar -xzf "$RUNNER_TEMP/$name.tar.gz" -C "$RUNNER_TEMP"
          echo "$RUNNER_TEMP/$name" >> "$GITHUB_PATH"
"#;

/// The docsys/0.6 install: the tree's installer, which holds the version and
/// the sha256 values, puts docsys on the PATH (D-138).
const INSTALL_TREE: &str = r#"      - name: docsys, the release the tree pins, by the tree's installer
        shell: bash
        run: bash @ROOT@/.docsys-install --print-dir >> "$GITHUB_PATH"
"#;

/// The file name of the tree's installer, beside `.docsys-version` (D-138).
pub const INSTALLER: &str = ".docsys-install";

/// The line that marks an installer as docsys's own: a file without it is
/// its owner's and is never written.
const INSTALLER_MARK: &str = "# docsys-install: written by `docsys upgrade`";

const INSTALLER_TEXT: &str = r#"#!/usr/bin/env bash
# docsys-install: written by `docsys upgrade` — do not edit; the version and the
# sha256 values move with the pin, and only `docsys upgrade` moves them (D-138).
# Installs the docsys release this tree pins, its archive checked against the
# sha256 values below: CI runs it, and so does a clone with no docsys. It needs
# curl, tar and sha256sum or shasum, and no credential. It writes the binary
# under ${DOCSYS_HOME:-~/.docsys}/versions/, where every docsys looks for a
# pinned version, and links it into ${DOCSYS_BIN_DIR:-~/.local/bin} when no
# docsys is on the PATH; it changes no file in the repository.
# --print-dir prints the directory that holds the binary, for a CI PATH.
set -euo pipefail
version=@VERSION@
case "$(uname -s)-$(uname -m)" in
@ARMS@  *) echo "docsys-install: no release archive for $(uname -s)-$(uname -m); install with cargo: cargo install docsys --version $version --locked" >&2; exit 1 ;;
esac
here=$(cd "$(dirname "$0")" && pwd)
pin=$(head -n 1 "$here/.docsys-version" 2>/dev/null | tr -d '[:space:]' || true)
if [ "$pin" != "$version" ]; then
  echo "docsys-install: .docsys-version names ${pin:-no version}, and this installer holds $version — run docsys upgrade --apply, which writes both" >&2
  exit 1
fi
dest="${DOCSYS_HOME:-$HOME/.docsys}/versions/$version/bin"
if [ ! -x "$dest/docsys" ]; then
  name="docsys-v$version-$target"
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  curl -fsSL --retry 3 -o "$tmp/$name.tar.gz" "${DOCSYS_RELEASES:-@RELEASES@}/v$version/$name.tar.gz"
  got=$(cd "$tmp" && { sha256sum "$name.tar.gz" 2>/dev/null || shasum -a 256 "$name.tar.gz"; } | cut -d ' ' -f 1)
  if [ "$got" != "$sum" ]; then
    echo "docsys-install: $name.tar.gz hashes to $got, not the $sum this installer pins" >&2
    exit 1
  fi
  tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
  mkdir -p "$dest"
  install -m 0755 "$tmp/$name/docsys" "$dest/docsys"
fi
if [ "${1:-}" = "--print-dir" ]; then
  echo "$dest"
  exit 0
fi
if ! command -v docsys >/dev/null; then
  bin="${DOCSYS_BIN_DIR:-$HOME/.local/bin}"
  mkdir -p "$bin"
  ln -sf "$dest/docsys" "$bin/docsys"
  case ":$PATH:" in
    *":$bin:"*) ;;
    *) echo "docsys-install: $bin is not on PATH; add it to your shell profile" >&2 ;;
  esac
fi
echo "docsys-install: docsys $version is installed in $dest"
"#;

/// The installer of release `version` for the tree, its archives' sha256
/// values `sums` as `docsys upgrade` read them from the release.
pub fn installer_text(version: &str, sums: &[(String, String)]) -> String {
    let arms: String = sums
        .iter()
        .map(|(target, hex)| {
            let uname = TARGETS
                .iter()
                .find(|(t, _)| t == target)
                .map_or("", |(_, u)| *u);
            format!("  {uname}) target={target} sum={hex} ;;\n")
        })
        .collect();
    fill(
        INSTALLER_TEXT,
        &[
            ("VERSION", version),
            ("ARMS", &arms),
            ("RELEASES", RELEASES),
        ],
    )
}

/// The version an installer docsys wrote holds; `None` for a file docsys did
/// not write, which is its owner's.
pub fn installer_version(text: &str) -> Option<String> {
    if !text
        .lines()
        .nth(1)
        .is_some_and(|l| l.starts_with(INSTALLER_MARK))
    {
        return None;
    }
    text.lines()
        .find_map(|l| l.strip_prefix("version="))
        .map(|v| v.trim().to_string())
}

/// The marker of a docsys/0.5 release install, in its step's name.
const PINNED_MARK: &str = "the release the tree pins";

const INSTALL_RELEASE_PINNED: &str = r#"      - name: docsys @VERSION@, the release the tree pins, checked against the sha256 its upgrade wrote
        shell: bash
        run: |
          # `docsys upgrade` wrote this version and its archives' sha256 values from
          # the release, reviewed with it: a pin moved alone installs nothing.
          v=$(head -n 1 "@ROOT@/.docsys-version" 2>/dev/null | tr -d '[:space:]' || true)
          if [ "$v" != "@VERSION@" ]; then
            echo "docsys: @ROOT@/.docsys-version names ${v:-no version}, and this workflow installs @VERSION@ — run docsys upgrade --apply, which pins both" >&2
            exit 1
          fi
          case "$(uname -s)-$(uname -m)" in
@ARMS@            *) echo "docsys: no release archive for a $(uname -s)-$(uname -m) runner" >&2; exit 1 ;;
          esac
          name="docsys-v@VERSION@-$target"
          curl -fsSL -o "$RUNNER_TEMP/$name.tar.gz" "@RELEASES@/v@VERSION@/$name.tar.gz"
          got=$(cd "$RUNNER_TEMP" && { sha256sum "$name.tar.gz" 2>/dev/null || shasum -a 256 "$name.tar.gz"; } | cut -d ' ' -f 1)
          if [ "$got" != "$sum" ]; then
            echo "docsys: $name.tar.gz hashes to $got, not the $sum this workflow pins" >&2
            exit 1
          fi
          tar -xzf "$RUNNER_TEMP/$name.tar.gz" -C "$RUNNER_TEMP"
          echo "$RUNNER_TEMP/$name" >> "$GITHUB_PATH"
"#;

const ARM_PREFIX: &str = "            ";

/// The `case` arms of a release install: each target's runner and sha256.
fn sum_arms(sums: &[(String, String)]) -> String {
    sums.iter()
        .map(|(target, hex)| {
            let uname = TARGETS
                .iter()
                .find(|(t, _)| t == target)
                .map_or("", |(_, u)| *u);
            format!("{ARM_PREFIX}{uname}) target={target} sum={hex} ;;\n")
        })
        .collect()
}

/// The docsys/0.5 release install of `version`, its archives' sha256 values
/// `sums`, as `docsys upgrade` read them from the release.
fn release_pinned_step(version: &str, root: &str, sums: &[(String, String)]) -> String {
    fill(
        INSTALL_RELEASE_PINNED,
        &[
            ("VERSION", version),
            ("ARMS", &sum_arms(sums)),
            ("RELEASES", RELEASES),
            ("ROOT", root),
        ],
    )
}

const VERIFY_HEAD: &str = r#"
  # A code review's approval is a maintainer's word (D-095, D-105). When a pull
  # request merges, every page it touched that carries `verification:` is recorded
  # as verified by each approver whose @login is on a .docmeta.yml maintainers:
  # entry, in a commit under that approver's identity. `docsys verify` skips any
  # other approver and fails on anything else. @WHERE@
  verify-on-approval:
    if: github.event_name == 'pull_request' && github.event.pull_request.merged == true
    runs-on: @RUNNER@
    permissions:
      contents: write
      pull-requests: @PR@
    steps:
      - uses: actions/checkout@v5
        with:
          ref: ${{ github.event.pull_request.base.ref }}
          fetch-depth: 0
@INSTALL@"#;

const WHERE_PULL_REQUEST: &str = "The records go to a branch
  # docsys/verify-<number> and reach the base through a follow-up pull request,
  # whose own merge finds nothing left to verify.";

const WHERE_DIRECT: &str = "The records are pushed to the
  # base branch; a branch that refuses direct pushes needs
  # `docsys adopt --verify-on-approval pull-request`.";

const LOGINS: &str = r#"          range="${{ github.event.pull_request.base.sha }}...${{ github.sha }}"
          logins=$(gh api "repos/${{ github.repository }}/pulls/$NUMBER/reviews" --jq '[.[] | select(.state == "APPROVED") | .user.login] | unique | .[]')
"#;

const VERIFY_PULL_REQUEST: &str = r#"      - env:
          GH_TOKEN: ${{ github.token }}
          BASE: ${{ github.event.pull_request.base.ref }}
          NUMBER: ${{ github.event.pull_request.number }}
        run: |
@LOGINS@          branch="docsys/verify-$NUMBER"
          git switch -c "$branch"
          before=$(git rev-parse HEAD)
          for login in $logins; do
            docsys verify --range "$range" --by "@$login" --commit --root @ROOT@
          done
          if [ "$(git rev-parse HEAD)" = "$before" ]; then
            echo "docsys: no page to record"
            exit 0
          fi
          git push origin "$branch"
          if ! gh pr create --base "$BASE" --head "$branch" --title "docs: verification records from #$NUMBER" --body "The approvals on #$NUMBER, recorded by docsys verify for each approver who is a declared maintainer."; then
            echo "docsys: the records are on $branch, but GitHub refused the pull request. Turn on Settings > Actions > General > Workflow permissions > Allow GitHub Actions to create and approve pull requests, then open it from $branch." >&2
            exit 1
          fi
"#;

const VERIFY_DIRECT: &str = r#"      - env:
          GH_TOKEN: ${{ github.token }}
          NUMBER: ${{ github.event.pull_request.number }}
        run: |
@LOGINS@          for login in $logins; do
            docsys verify --range "$range" --by "@$login" --commit --root @ROOT@
          done
          git push
"#;

/// `template` with every `@KEY@` replaced in one pass, so a value is never
/// read as a placeholder.
fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(at) = rest.find('@') {
        out.push_str(rest.get(..at).unwrap_or(""));
        let tail = rest.get(at..).unwrap_or("");
        match values.iter().find(|(k, _)| {
            tail.strip_prefix('@')
                .and_then(|t| t.strip_prefix(k))
                .is_some_and(|t| t.starts_with('@'))
        }) {
            Some((k, v)) => {
                out.push_str(v);
                rest = tail.get(k.len() + 2..).unwrap_or("");
            }
            None => {
                out.push('@');
                rest = tail.get(1..).unwrap_or("");
            }
        }
    }
    out.push_str(rest);
    out
}

fn root_of(root: &str) -> &str {
    if root.is_empty() {
        "."
    } else {
        root
    }
}

/// The parameters as line 1 records them, after the hash; a workflow with
/// no approval job records no mode (D-130).
fn params_line(w: &Workflow) -> String {
    let verify = match w.ci.verify {
        Verify::Off => String::new(),
        mode => format!(" verify={}", mode.name()),
    };
    format!(
        "branch={} runner={} install={}{verify} root={}",
        w.branch,
        w.ci.runner.join(","),
        match w.ci.install {
            Install::Cargo => "cargo",
            Install::Release(_) | Install::ReleasePinned(_) => "release",
            Install::Tree => "tree",
        },
        root_of(&w.root)
    )
}

/// The hash line 1 carries: of the whole file but the hash itself, so an
/// edit anywhere, a recorded parameter included, makes the file the owner's.
fn stamp_hash(version: &str, params: &str, body: &str) -> String {
    crate::fresh::sha256_hex(format!("{STAMP}{version} sha256: {params}\n{body}").as_bytes())
}

/// The workflow file, line 1 stamped.
/// A docsys/0.6 rendering (D-136, D-138): the pull request is checked by
/// the scoped gate alone, a push to the default branch over the whole tree,
/// and a merge queue's group by the scoped gate on the combined state.
const BODY_06: &str = r#"# docsys documentation workflow, written by `docsys adopt` (D-111). Line 1 records how
# it was rendered and a hash of everything else in this file: while the hash holds,
# docsys may regenerate it; once the file is edited, it is yours.
name: docsys

on:
  pull_request:
    types: [opened, synchronize, reopened]
  push:
    branches: [@BRANCH@]
  merge_group:

permissions:
  contents: read

jobs:
  docs:
    runs-on: @RUNNER@
    concurrency:
      group: docsys-${{ github.event.pull_request.number || github.ref }}
      cancel-in-progress: true
    steps:
      - uses: actions/checkout@v5
        with:
          fetch-depth: 0
@INSTALL@      - if: github.event_name == 'push'
        run: docsys lint --repo . --root @ROOT@
      - if: github.event_name == 'push'
        run: docsys refs --repo . --root @ROOT@
      - if: github.event_name == 'pull_request'
        run: docsys gate --repo . --root @ROOT@ --range "origin/${{ github.base_ref }}...HEAD"
      - if: github.event_name == 'merge_group'
        run: docsys gate --repo . --root @ROOT@ --range "${{ github.event.merge_group.base_sha }}...HEAD"
"#;

pub fn render(w: &Workflow) -> String {
    render_in(w, matches!(w.ci.install, Install::Tree))
}

/// The workflow file for a tree whose gate is `scoped` (docsys/0.6) or not:
/// a scoped tree's workflow checks a pull request with `gate --range` alone,
/// whichever way it installs docsys (D-136, D-138).
pub fn render_in(w: &Workflow, scoped: bool) -> String {
    let runner = match w.ci.runner.as_slice() {
        [one] => one.clone(),
        many => format!("[{}]", many.join(", ")),
    };
    let root = root_of(&w.root);
    let install = match &w.ci.install {
        Install::Cargo => fill(INSTALL_CARGO, &[("VERSION", &w.version), ("ROOT", root)]),
        Install::Release(sums) => fill(
            INSTALL_RELEASE,
            &[
                ("VERSION", &w.version),
                ("ARMS", &sum_arms(sums)),
                ("RELEASES", RELEASES),
                ("ROOT", root),
            ],
        ),
        Install::ReleasePinned(sums) => release_pinned_step(&w.version, root, sums),
        Install::Tree => fill(INSTALL_TREE, &[("ROOT", root)]),
    };
    if scoped {
        let body = fill(
            BODY_06,
            &[
                ("BRANCH", &w.branch),
                ("RUNNER", &runner),
                ("INSTALL", &install),
                ("ROOT", root),
            ],
        );
        let params = params_line(w);
        let hash = stamp_hash(&w.version, &params, &body);
        return format!("{STAMP}{} sha256:{hash} {params}\n{body}", w.version);
    }
    let job = |head_where: &str, pr: &str, steps: &str| {
        let head = fill(
            VERIFY_HEAD,
            &[
                ("WHERE", head_where),
                ("RUNNER", &runner),
                ("PR", pr),
                ("INSTALL", &install),
            ],
        );
        head + &fill(steps, &[("LOGINS", LOGINS), ("ROOT", root)])
    };
    let verify = match w.ci.verify {
        Verify::PullRequest => job(WHERE_PULL_REQUEST, "write", VERIFY_PULL_REQUEST),
        Verify::Direct => job(WHERE_DIRECT, "read", VERIFY_DIRECT),
        Verify::Off => String::new(),
    };
    let (review, docs_if) = ("", "github.event.action != 'closed'");
    let body = fill(
        BODY,
        &[
            ("REVIEW", review),
            ("DOCS_IF", docs_if),
            ("BRANCH", &w.branch),
            ("RUNNER", &runner),
            ("INSTALL", &install),
            ("ROOT", root),
            ("VERIFY", &verify),
        ],
    );
    let params = params_line(w);
    let hash = stamp_hash(&w.version, &params, &body);
    format!("{STAMP}{} sha256:{hash} {params}\n{body}", w.version)
}

/// The branch pushes are checked on: `origin/HEAD`'s, else a local `main` or
/// `master`, else the current one, else `main`. Never the current branch
/// while the repository has its own: an upgrade run on a feature branch
/// writes what the same upgrade writes on the base, so the branch merges
/// cleanly (D-124).
pub fn default_branch(repo: &Path) -> String {
    let ask = |args: &[&str]| {
        crate::git::cmd(repo)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    };
    ask(&[
        "symbolic-ref",
        "--quiet",
        "--short",
        "refs/remotes/origin/HEAD",
    ])
    .and_then(|r| r.strip_prefix("origin/").map(str::to_string))
    .or_else(|| {
        ["main", "master"]
            .into_iter()
            .map(str::to_string)
            .find(|b| {
                ask(&[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("refs/heads/{b}"),
                ])
                .is_some()
            })
    })
    .or_else(|| ask(&["symbolic-ref", "--quiet", "--short", "HEAD"]))
    .unwrap_or_else(|| "main".to_string())
}

/// What may happen to an existing `.github/workflows/docsys.yml`.
#[derive(Debug, PartialEq, Eq)]
pub enum Existing {
    /// Rendered from the stamped template and untouched since: regenerable
    /// with the parameters it records, at this binary's version. A docsys/0.4
    /// release install's sha256 values are the recorded version's; another
    /// version's archives need the person's new ones.
    Untouched { from: String, params: Workflow },
    /// Byte-equal to a rendering from before the stamp: regenerable. A 0.15
    /// rendering pushed its records to the base (`direct`); an earlier one had
    /// no verify job (`off`).
    Legacy {
        from: &'static str,
        params: Workflow,
    },
    /// Anything else is the owner's and is never written: only the diff to
    /// what this binary renders.
    Owned { diff: String },
}

/// The renderings from before the stamp, with the tree's root as `@ROOT@`.
const LEGACY: [(&str, &str, Verify); 2] = [
    (
        "0.11-0.14",
        include_str!("../migrations/workflow-0.11-0.14.yml"),
        Verify::Off,
    ),
    (
        "0.15",
        include_str!("../migrations/workflow-0.15.yml"),
        Verify::Direct,
    ),
];

pub const PATH: &str = ".github/workflows/docsys.yml";

/// The repository's workflow, classified; `None` when there is none. `root`
/// is the tree's, relative to the repository.
pub fn classify(repo: &Path, root: &str) -> Option<Existing> {
    let text = std::fs::read_to_string(repo.join(PATH)).ok()?;
    let current = Workflow {
        version: crate::agents::TEMPLATE_VERSION.to_string(),
        branch: default_branch(repo),
        root: root_of(root).to_string(),
        ci: Ci::default(),
    };
    Some(classify_text(&text, &current))
}

/// The parameters line 1 records, when its hash holds for the file.
fn stamped(text: &str) -> Option<(String, Workflow)> {
    let (first, body) = text.split_once('\n')?;
    let (version, rest) = first.strip_prefix(STAMP)?.split_once(" sha256:")?;
    let (hash, params) = rest.split_once(' ')?;
    if stamp_hash(version, params, body) != hash {
        return None;
    }
    let (fields, root) = params.split_once(" root=")?;
    let field = |key: &str| {
        fields
            .split(' ')
            .find_map(|f| f.strip_prefix(key)?.strip_prefix('='))
    };
    let install = match field("install")? {
        "cargo" => Install::Cargo,
        "tree" => Install::Tree,
        // a docsys/0.5 release install names the tree's pin in its step;
        // 0.16.0's read the release's SHA256SUMS and pinned no value
        "release" if body.contains(PINNED_MARK) => Install::ReleasePinned(release_sums(body)),
        "release" => match release_sums(body) {
            sums if sums.is_empty() => Install::ReleasePinned(sums),
            sums => Install::Release(sums),
        },
        _ => return None,
    };
    let w = Workflow {
        version: version.to_string(),
        branch: field("branch")?.to_string(),
        root: root.to_string(),
        ci: Ci {
            runner: field("runner")?.split(',').map(str::to_string).collect(),
            install,
            verify: match field("verify") {
                Some(mode) => Verify::named(mode)?,
                None => Verify::Off,
            },
        },
    };
    Some((version.to_string(), w))
}

/// The `(target, hex)` pairs of a release install's `case` arms.
fn release_sums(body: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in body.lines() {
        let Some(arm) = line
            .strip_prefix(ARM_PREFIX)
            .and_then(|l| l.split_once(") target="))
            .map(|(_, a)| a)
        else {
            continue;
        };
        let Some((target, rest)) = arm.split_once(" sum=") else {
            continue;
        };
        let hex = rest.trim_end_matches(" ;;");
        if !out.iter().any(|(t, _)| t == target) {
            out.push((target.to_string(), hex.to_string()));
        }
    }
    out
}

/// The tree's root as a rendering from before the stamp wrote it (empty for
/// the repository's top, which those renderings left blank).
fn legacy_root(text: &str) -> Option<&str> {
    text.lines().find_map(|l| {
        l.trim_start()
            .strip_prefix("- run: docsys refs --repo . --root ")
    })
}

/// `text` classified; `current` is what this binary renders for a file that
/// records nothing.
pub fn classify_text(text: &str, current: &Workflow) -> Existing {
    let text = text.replace("\r\n", "\n");
    let version = crate::agents::TEMPLATE_VERSION;
    if let Some((from, recorded)) = stamped(&text) {
        return Existing::Untouched {
            from,
            params: Workflow {
                version: version.to_string(),
                ..recorded
            },
        };
    }
    if let Some(root) = legacy_root(&text) {
        for (from, template, verify) in LEGACY {
            if fill(template, &[("ROOT", root)]) == text {
                return Existing::Legacy {
                    from,
                    params: Workflow {
                        root: root_of(root).to_string(),
                        ci: Ci {
                            verify,
                            ..Ci::default()
                        },
                        ..current.clone()
                    },
                };
            }
        }
    }
    // an edited stamped file is shown against its own parameters
    let target = text
        .split_once('\n')
        .and_then(|(first, body)| {
            let unchecked = first
                .strip_prefix(STAMP)?
                .split_once(" sha256:")?
                .1
                .split_once(' ')?
                .1;
            let resigned = format!(
                "{STAMP}{version} sha256:{} {unchecked}\n{body}",
                stamp_hash(version, unchecked, body)
            );
            stamped(&resigned).map(|(_, w)| w)
        })
        .unwrap_or_else(|| current.clone());
    let new = render(&Workflow {
        version: version.to_string(),
        ..target
    });
    Existing::Owned {
        diff: crate::diff::unified(
            &text,
            &new,
            PATH,
            &format!("{PATH} as docsys {version} writes it"),
            3,
        ),
    }
}

/// What installs docsys in a workflow its owner edited, against the pin
/// (D-111, D-120).
#[derive(Debug, PartialEq, Eq)]
pub enum Reinstall {
    /// nothing that installs docsys names a version or a sha256
    Pinned,
    /// the file with only what installs docsys replaced, and what changed
    Rewritten { text: String, what: String },
    /// what installs docsys is not found unambiguously: nothing is written,
    /// the 1-based line is named with what to change there
    Unclear { line: usize, what: String },
    /// a release install is to be written, and its sha256 values are still
    /// to be read from the release: call again with them
    NeedsRelease,
}

/// How a step installs docsys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Cargo,
    Release,
    /// an action, a script, or both of the above: not this binary's to replace
    Other,
}

/// A step of a `steps:` list: its 0-based first and last lines, and the
/// column of its `-`.
#[derive(Debug, Clone, Copy)]
struct Step {
    first: usize,
    last: usize,
    col: usize,
}

/// An entry of an `env:` mapping.
struct Entry {
    line: usize,
    col: usize,
    key: String,
    value: String,
    /// its value on its own line
    single: bool,
}

/// An `env:` mapping: its key's line and column, its last line, its entries.
struct Env {
    line: usize,
    col: usize,
    last: usize,
    entries: Vec<Entry>,
}

fn indent_of(l: &str) -> usize {
    l.len() - l.trim_start_matches(' ').len()
}

fn is_comment(l: &str) -> bool {
    l.trim_start().starts_with('#')
}

/// The column a `key:` that opens a block sits at, when `l` is one. A
/// `- key:` opens a list item too, which is never taken apart.
fn opens(l: &str, key: &str) -> Option<usize> {
    let rest = l.trim_start().strip_prefix(key)?.strip_prefix(':')?.trim();
    (rest.is_empty() || rest.starts_with('#')).then_some(indent_of(l))
}

/// Every step of every `steps:` list.
fn steps_of(lines: &[&str]) -> Vec<Step> {
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let Some(key) = opens(l, "steps") else {
            continue;
        };
        let mut col: Option<usize> = None;
        let mut cur: Option<Step> = None;
        for (j, m) in lines.iter().enumerate().skip(i + 1) {
            if m.trim().is_empty() {
                continue;
            }
            let ind = indent_of(m);
            if is_comment(m) {
                if let (Some(c), Some(s)) = (col, cur.as_mut()) {
                    if ind > c {
                        s.last = j;
                    }
                }
                continue;
            }
            let item = m.trim_start().starts_with("- ") || m.trim() == "-";
            let c = match col {
                Some(c) => c,
                None if item && ind >= key => *col.insert(ind),
                None => break,
            };
            if ind > c {
                if let Some(s) = cur.as_mut() {
                    s.last = j;
                }
            } else if ind == c && item {
                out.extend(cur.take());
                cur = Some(Step {
                    first: j,
                    last: j,
                    col: c,
                });
            } else {
                break;
            }
        }
        out.extend(cur);
    }
    out
}

/// Every `env:` mapping, its one-line entries read.
fn envs_of(lines: &[&str]) -> Vec<Env> {
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let Some(key) = opens(l, "env") else {
            continue;
        };
        let mut env = Env {
            line: i,
            col: indent_of(l),
            last: i,
            entries: Vec::new(),
        };
        let mut col: Option<usize> = None;
        for (j, m) in lines.iter().enumerate().skip(i + 1) {
            if m.trim().is_empty() {
                continue;
            }
            let ind = indent_of(m);
            if is_comment(m) {
                if ind > key {
                    env.last = j;
                }
                continue;
            }
            if ind <= key {
                break;
            }
            env.last = j;
            let c = *col.get_or_insert(ind);
            if ind > c {
                // a value on more lines than one is never a pin of a version
                if let Some(e) = env.entries.last_mut() {
                    e.single = false;
                }
                continue;
            }
            let (k, v) = m.trim().split_once(':').unwrap_or((m.trim(), ""));
            let v = v.split(" #").next().unwrap_or("").trim();
            env.entries.push(Entry {
                line: j,
                col: c,
                key: k.trim().to_string(),
                value: v.trim_matches(['"', '\'']).to_string(),
                single: true,
            });
        }
        out.push(env);
    }
    out
}

fn version_in(l: &str) -> Option<&str> {
    l.split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .find(|t| crate::dispatch::parse(t).is_some())
}

fn hash_in(l: &str) -> bool {
    l.split(|c: char| !c.is_ascii_hexdigit())
        .any(|t| t.len() == 64)
}

/// A value that pins: a version, `v` before it or not, or a sha256.
fn pins(value: &str) -> bool {
    crate::dispatch::parse(value.strip_prefix('v').unwrap_or(value)).is_some()
        || (value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Whether `text` names `key` as a whole word.
fn names(text: &str, key: &str) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    text.match_indices(key).any(|(at, _)| {
        let before = text.get(..at).and_then(|b| b.chars().next_back());
        let after = text.get(at + key.len()..).and_then(|a| a.chars().next());
        !before.is_some_and(word) && !after.is_some_and(word)
    })
}

/// The docsys a line runs, where it runs one: how it is named — `docsys` on
/// the PATH, or a path — and the word after it.
fn runs_docsys(l: &str) -> Option<(String, String)> {
    let t = l.trim_start();
    let t = t.strip_prefix("- ").unwrap_or(t).trim_start();
    let t = t.strip_prefix("run:").unwrap_or(t);
    let unquote = |w: &str| w.trim_matches(['"', '\'']).to_string();
    let words: Vec<&str> = t.split_whitespace().collect();
    let mut command = true;
    for (k, w) in words.iter().enumerate() {
        let bare = unquote(w.trim_start_matches("$(").trim_start_matches(['(', '{']));
        if command && (bare == "docsys" || bare.ends_with("/docsys")) {
            return Some((
                bare,
                words.get(k + 1).map(|n| unquote(n)).unwrap_or_default(),
            ));
        }
        command = matches!(
            *w,
            ";" | "&&" | "||" | "|" | "then" | "do" | "else" | "{" | "(" | "!"
        ) || w.ends_with(';')
            || (command && w.contains('=') && !w.starts_with('-'));
    }
    None
}

/// How a step installs docsys, when it does.
fn kind_of(step: &str) -> Option<Kind> {
    let low = step.to_ascii_lowercase();
    if !low.contains("docsys") {
        return None;
    }
    let action = low.lines().any(|l| {
        let t = l.trim_start();
        let t = t.strip_prefix("- ").unwrap_or(t).trim_start();
        t.strip_prefix("uses:")
            .is_some_and(|u| u.contains("docsys"))
    });
    let cargo = low
        .lines()
        .any(|l| l.contains("cargo install") && l.contains("docsys"));
    let release = low
        .lines()
        .any(|l| l.contains("curl ") || l.contains("wget ") || l.contains("gh release download"));
    match (action, cargo, release) {
        (true, _, _) | (false, true, true) => Some(Kind::Other),
        (false, true, false) => Some(Kind::Cargo),
        (false, false, true) => Some(Kind::Release),
        (false, false, false) => None,
    }
}

/// The lines of a comment block directly above line `at`, at `col`.
fn comments_above(lines: &[&str], at: usize, col: usize) -> std::ops::Range<usize> {
    let mut first = at;
    while first > 0
        && lines
            .get(first - 1)
            .is_some_and(|l| is_comment(l) && indent_of(l) == col)
    {
        first -= 1;
    }
    first..at
}

/// `step`, written for a `-` at column 6, moved to `col`.
fn at_column(step: &str, col: usize) -> String {
    step.lines()
        .map(|l| {
            let l = match col.checked_sub(6) {
                Some(more) if !l.is_empty() => format!("{}{l}", " ".repeat(more)),
                Some(_) => String::new(),
                None => l.get(6 - col..).unwrap_or(l).to_string(),
            };
            l + "\n"
        })
        .collect()
}

/// `text`, an owner's workflow, with what installs docsys replaced by this
/// version's install step for the tree at `root`: the install steps that name
/// another version or a sha256 of no version, read an `env:` entry that pins,
/// or read the release's SHA256SUMS when CI runs (0.16.0's), and those
/// entries, each with the comment block above it. Every other line stays.
/// `version` is the one the upgrade pins; `release`, its archives' sha256
/// values, once read — a release step asks for them (`NeedsRelease`).
pub fn reinstall(
    text: &str,
    root: &str,
    version: &str,
    release: Option<&[(String, String)]>,
) -> Reinstall {
    reinstall_with(text, root, version, release, false)
}

/// `reinstall` on a docsys/0.6 tree: what installs docsys becomes the step
/// that runs the tree's installer, which holds the version and its sha256
/// values, so the workflow holds no second copy (D-138).
pub fn reinstall_tree(text: &str, root: &str, version: &str) -> Reinstall {
    reinstall_with(text, root, version, None, true)
}

fn reinstall_with(
    text: &str,
    root: &str,
    version: &str,
    release: Option<&[(String, String)]>,
    tree: bool,
) -> Reinstall {
    let crlf = text.contains("\r\n");
    let text = text.replace("\r\n", "\n");
    let root = root_of(root);
    let pin = format!("{root}/.docsys-version");
    let lines: Vec<&str> = text.lines().collect();
    let code = |i: usize| lines.get(i).copied().filter(|l| !is_comment(l));
    let body = |s: &Step| -> String {
        (s.first..=s.last)
            .filter_map(code)
            .collect::<Vec<_>>()
            .join("\n")
    };
    let installs: Vec<(Step, Kind)> = steps_of(&lines)
        .into_iter()
        .filter_map(|s| kind_of(&body(&s)).map(|k| (s, k)))
        .collect();
    let in_steps =
        |i: usize, set: &[(Step, Kind)]| set.iter().any(|(s, _)| (s.first..=s.last).contains(&i));
    let envs = envs_of(&lines);
    let pinning: Vec<&Entry> = envs
        .iter()
        .flat_map(|e| &e.entries)
        .filter(|e| e.single && !in_steps(e.line, &installs) && pins(&e.value))
        .collect();
    // an install that holds the version the upgrade pins with its sha256
    // values is current; one that names another version, a sha256 of no
    // version, or the release's SHA256SUMS read when CI runs, is not
    let stale: Vec<(Step, Kind)> = installs
        .iter()
        .filter(|(s, k)| {
            let b = body(s);
            // docsys/0.6: a release install in the workflow is a second copy
            // of the values the tree's installer holds (D-138)
            if tree && *k == Kind::Release {
                return true;
            }
            let own = b.lines().any(|l| version_in(l) == Some(version));
            b.lines().any(|l| {
                version_in(l).is_some_and(|v| v != version)
                    || (hash_in(l) && !own)
                    || l.contains("SHA256SUMS")
            }) || pinning.iter().any(|e| names(&b, &e.key))
        })
        .copied()
        .collect();
    let read_elsewhere = |key: &str, own: usize| {
        (0..lines.len()).any(|i| i != own && code(i).is_some_and(|l| names(l, key)))
    };
    let gone: Vec<&Entry> = pinning
        .iter()
        .filter(|e| {
            stale.iter().any(|(s, _)| names(&body(s), &e.key))
                || (e.key.to_ascii_lowercase().contains("docsys")
                    && !read_elsewhere(&e.key, e.line))
        })
        .copied()
        .collect();

    // what goes: each entry with its comments, and a mapping left empty
    let mut removed: BTreeSet<usize> = BTreeSet::new();
    for e in &gone {
        removed.extend(comments_above(&lines, e.line, e.col));
        removed.insert(e.line);
    }
    for env in &envs {
        if !env.entries.is_empty() && env.entries.iter().all(|e| removed.contains(&e.line)) {
            removed.extend(comments_above(&lines, env.line, env.col));
            removed.extend(env.line..=env.last);
        }
    }
    let replaced: Vec<(usize, Step, Kind)> = stale
        .iter()
        .map(|(s, k)| (comments_above(&lines, s.first, s.col).start, *s, *k))
        .collect();
    let kept = |i: usize| {
        !removed.contains(&i) && !replaced.iter().any(|(a, s, _)| (*a..=s.last).contains(&i))
    };

    // the one place a person changes, where the rest cannot be told apart
    let mut unclear: Vec<(usize, String)> = Vec::new();
    let then = "and `docsys upgrade` replaces the install";
    for (s, kind) in &stale {
        if *kind == Kind::Other {
            unclear.push((s.first, format!("this step installs docsys in a way this upgrade cannot replace — make it install the version {pin} names, the archive checked against the sha256 values `docsys upgrade` writes from the release, as this version's install step does")));
        }
        for i in s.first..=s.last {
            if let Some((_, sub)) = code(i).and_then(runs_docsys) {
                if !(sub.is_empty() || sub.starts_with('-') || sub == "version" || sub == "help") {
                    unclear.push((s.first, format!("this step installs docsys and runs `docsys {sub}` too — give that a step of its own, {then}")));
                }
            }
        }
    }
    for e in &gone {
        if let Some(i) = (0..lines.len())
            .find(|&i| i != e.line && kept(i) && code(i).is_some_and(|l| names(l, &e.key)))
        {
            unclear.push((i, format!("this line reads {}, which goes with the install that reads it — read the version from {pin} here, {then}", e.key)));
        }
    }
    let handles = [
        "tar ",
        ".tar.gz",
        "chmod",
        "github_path",
        "path:",
        "cp ",
        "mv ",
        "ln ",
        "unzip",
        "install -m",
    ];
    for i in (0..lines.len()).filter(|&i| !stale.is_empty() && kept(i)) {
        let Some(l) = code(i) else { continue };
        let low = l.to_ascii_lowercase();
        if runs_docsys(l).is_some_and(|(how, _)| how != "docsys") {
            unclear.push((i, format!("this line runs docsys from where its install step put it, and this version's install step puts docsys on the PATH — run `docsys` here, {then}")));
        } else if low.contains("docsys") && handles.iter().any(|h| low.contains(h)) {
            unclear.push((i, format!("this line handles the docsys binary outside its install step — move it into that step or take it out, {then}")));
        }
    }
    for i in (0..lines.len()).filter(|&i| kept(i)) {
        let Some(l) = code(i).filter(|l| l.to_ascii_lowercase().contains("docsys")) else {
            continue;
        };
        if let Some(v) = version_in(l).filter(|v| *v != version) {
            unclear.push((i, format!("it installs docsys {v}, and {pin} pins {version} — make it install the version {pin} names, as this version's install step does")));
        } else if hash_in(l) && version_in(l).is_none() && !stale.is_empty() {
            unclear.push((i, "it holds a sha256 value for docsys outside its install step — the install step holds the values the upgrade writes; take this one out, with what reads it".to_string()));
        }
    }
    if let Some((i, what)) = unclear.into_iter().min_by_key(|(i, _)| *i) {
        return Reinstall::Unclear { line: i + 1, what };
    }
    if stale.is_empty() && gone.is_empty() {
        return Reinstall::Pinned;
    }
    let sums = match release {
        Some(sums) => sums,
        None if tree => &[],
        None if stale.iter().any(|(_, k)| *k != Kind::Cargo) => {
            return Reinstall::NeedsRelease;
        }
        None => &[],
    };

    let mut out: Vec<String> = Vec::new();
    let mut skipped = false;
    let mut i = 0;
    while let Some(l) = lines.get(i) {
        if let Some((_, s, kind)) = replaced.iter().find(|(a, _, _)| *a == i) {
            let step = match kind {
                Kind::Release if tree => fill(INSTALL_TREE, &[("ROOT", root)]),
                Kind::Cargo => fill(INSTALL_CARGO, &[("VERSION", version), ("ROOT", root)]),
                _ => release_pinned_step(version, root, sums),
            };
            out.extend(at_column(&step, s.col).lines().map(str::to_string));
            i = s.last + 1;
            skipped = false;
            continue;
        }
        i += 1;
        if removed.contains(&(i - 1)) {
            skipped = true;
            continue;
        }
        // the blank line that set the removed block apart goes with it
        if skipped && l.trim().is_empty() && out.last().is_none_or(|p| p.trim().is_empty()) {
            skipped = false;
            continue;
        }
        skipped = false;
        out.push(l.to_string());
    }
    let mut new = out.join("\n");
    if text.ends_with('\n') {
        new.push('\n');
    }
    if crlf {
        new = new.replace('\n', "\r\n");
    }
    let entries = match gone.len() {
        0 => String::new(),
        1 => "the `env:` line that names its version or a sha256 goes, with the comment right above it; ".to_string(),
        n => format!("the {n} `env:` lines that name its version or sha256 values go, with the comments right above them; "),
    };
    let how = if tree && stale.iter().all(|(_, k)| *k == Kind::Release) {
        format!("runs the tree's installer, `{root}/{INSTALLER}`, which installs the version {pin} names")
    } else if stale.iter().all(|(_, k)| *k == Kind::Cargo) {
        format!("installs the version {pin} names with cargo")
    } else {
        format!("installs docsys {version} and checks its archive against the sha256 values this upgrade read from the release")
    };
    let steps = match stale.len() {
        0 => String::new(),
        1 => format!("the install step becomes this version's, which {how}; "),
        n => format!("the {n} install steps become this version's, which {how}; "),
    };
    Reinstall::Rewritten {
        text: new,
        what: format!("{entries}{steps}every other line stays as you wrote it — the diff is below"),
    }
}

/// A one-line `- run: docsys <command> …` value of a step's first line, the
/// run text after `run:`.
fn run_value(l: &str) -> Option<&str> {
    let t = l.trim_start();
    let t = t.strip_prefix("- ").unwrap_or(t).trim_start();
    t.strip_prefix("run:").map(str::trim)
}

/// The docsys/0.6 shape of an owner's workflow, every other line kept as the
/// owner wrote it (D-136, D-138): a `push:` with no filter runs on the
/// default branch only; where a `gate --range` step checks the pull request,
/// `lint` and `refs` run on push alone and a merge queue's group is checked
/// by the gate on the combined state. `Ok(None)` when nothing changes; `Err`
/// names the 1-based line a person changes, when the shape cannot be told.
/// An owner's workflow in the docsys/0.6 shape and what changed in it; `None`
/// when nothing does; `Err` the 1-based line a person changes, and why.
pub type Scoped = Result<Option<(String, Vec<String>)>, (usize, String)>;

pub fn scope_owner(text: &str, root: &str, branch: &str) -> Scoped {
    let crlf = text.contains("\r\n");
    let text = text.replace("\r\n", "\n");
    let root = root_of(root);
    let lines: Vec<&str> = text.lines().collect();
    let Some(on) = lines.iter().position(|l| {
        let t = l.split(" #").next().unwrap_or(l).trim_end();
        t == "on:" || t.starts_with("on: ") || t == "\"on\":" || t == "'on':"
    }) else {
        return Ok(None);
    };
    let line = |i: usize| lines.get(i).copied().unwrap_or("");
    let head = line(on).split(" #").next().unwrap_or("").trim_end();
    if !(head == "on:" || head == "\"on\":" || head == "'on':") {
        return Err((on + 1, "the triggers are written on one line — write them as a block (`on:`, then one trigger per line), and `docsys upgrade` narrows `push:` to the default branch and adds `merge_group:`".to_string()));
    }
    // the `on:` block: its lines, the column of its keys
    let mut end = on;
    let mut col: Option<usize> = None;
    let mut keys: Vec<(usize, String)> = Vec::new();
    for (i, l) in lines.iter().enumerate().skip(on + 1) {
        if l.trim().is_empty() || is_comment(l) {
            continue;
        }
        let ind = indent_of(l);
        if ind == 0 {
            break;
        }
        end = i;
        let c = *col.get_or_insert(ind);
        if ind == c {
            let k = l.trim().split(':').next().unwrap_or("").trim().to_string();
            keys.push((i, k));
        }
    }
    let Some(col) = col else {
        return Ok(None);
    };
    if keys.iter().any(|(_, k)| k.starts_with('-')) {
        return Err((on + 1, "the triggers are written as a list — write them as a mapping (one `trigger:` a line), and `docsys upgrade` narrows `push:` to the default branch and adds `merge_group:`".to_string()));
    }
    let mut after: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let mut replace: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let mut what: Vec<String> = Vec::new();

    // push: with no filter → the default branch; a filter of the owner's
    // that names no branch and no tag runs on every branch, and keeps
    let mut every_branch = false;
    if let Some((i, _)) = keys.iter().find(|(_, k)| k == "push") {
        let value = line(*i).split_once(':').map_or("", |(_, v)| v);
        let value = value.split(" #").next().unwrap_or("").trim();
        let filter: Vec<&str> = lines
            .iter()
            .skip(i + 1)
            .take_while(|l| l.trim().is_empty() || is_comment(l) || indent_of(l) > col)
            .filter(|l| !l.trim().is_empty() && !is_comment(l))
            .map(|l| l.trim())
            .collect();
        if value.is_empty() && filter.is_empty() {
            after
                .entry(*i)
                .or_default()
                .push(format!("{}branches: [{branch}]", " ".repeat(col * 2)));
            what.push(format!(
                "`push:` runs on {branch} only, so a branch with a pull request is checked once"
            ));
        } else {
            let names = |t: &str| t.contains("branches:") || t.contains("tags:");
            every_branch = !names(value) && !filter.iter().any(|l| names(l));
        }
    }
    let on_push = if every_branch {
        format!("github.event_name == 'push' && github.ref_name == '{branch}'")
    } else {
        "github.event_name == 'push'".to_string()
    };

    // the steps: lint and refs on push, a merge queue's group by the gate
    let steps = steps_of(&lines);
    let step_text = |s: &Step| lines.get(s.first..=s.last).unwrap_or(&[]).join("\n");
    let gate = steps.iter().find(|s| {
        let b = step_text(s);
        b.contains("docsys gate") && b.contains("--range") && b.contains("pull_request")
    });
    if let Some(gate) = gate {
        for s in &steps {
            let first = line(s.first);
            let b = step_text(s);
            let runs = (s.first..=s.last).find_map(|i| run_value(line(i)));
            let whole_tree =
                runs.is_some_and(|r| r.starts_with("docsys lint") || r.starts_with("docsys refs"));
            if !whole_tree || b.contains("if:") {
                continue;
            }
            let pad = " ".repeat(s.col + 2);
            if s.first == s.last {
                let run = run_value(first).unwrap_or("");
                replace.insert(
                    s.first,
                    vec![
                        format!("{}- if: {on_push}", " ".repeat(s.col)),
                        format!("{pad}run: {run}"),
                    ],
                );
            } else {
                after
                    .entry(s.first)
                    .or_default()
                    .push(format!("{pad}if: {on_push}"));
            }
        }
        if !replace.is_empty() || after.keys().any(|k| *k > end) {
            what.push(if every_branch {
                format!("`lint` and `refs` check the whole tree on a push to {branch} only: your `push:` filter also runs on other branches, and there an error that {branch} carries would fail each push; the pull request is checked by `gate --range` alone, which reads both")
            } else {
                "`lint` and `refs` check the whole tree on push; the pull request is checked by `gate --range` alone, which reads both".to_string()
            });
        }
        if !text.contains("merge_group") {
            after.entry(gate.last).or_default().extend([
                format!("{}- if: github.event_name == 'merge_group'", " ".repeat(gate.col)),
                format!(
                    "{}run: docsys gate --repo . --root {root} --range \"${{{{ github.event.merge_group.base_sha }}}}...HEAD\"",
                    " ".repeat(gate.col + 2)
                ),
            ]);
            after
                .entry(end)
                .or_default()
                .push(format!("{}merge_group:", " ".repeat(col)));
            what.push("a merge queue's group is checked by the gate on the combined state (`merge_group:`)".to_string());
        }
    }
    if what.is_empty() {
        return Ok(None);
    }
    let mut out: Vec<String> = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        match replace.get(&i) {
            Some(new) => out.extend(new.iter().cloned()),
            None => out.push((*l).to_string()),
        }
        if let Some(extra) = after.get(&i) {
            out.extend(extra.iter().cloned());
        }
    }
    let mut new = out.join("\n");
    if text.ends_with('\n') {
        new.push('\n');
    }
    if crlf {
        new = new.replace('\n', "\r\n");
    }
    Ok(Some((new, what)))
}
