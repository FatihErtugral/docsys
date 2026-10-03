//! The GitHub workflow `adopt` writes (D-111), and what may happen to one
//! that exists. Every parameter of a rendering is on its line 1 with a hash
//! of the rest of the file, so a later binary can tell a file nobody edited —
//! regenerable with the same parameters — from one that is its owner's, which
//! is never written, only shown as a diff.

use std::path::Path;

/// Where a release's archives are downloaded from.
pub const RELEASES: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/releases/download");

/// How the runners get docsys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Install {
    /// `cargo install docsys --version <v> --locked`, cached by version
    Cargo,
    /// the release archive for the runner's target, checked against the
    /// sha256 the person supplied: `(target, hex)`
    Release(Vec<(String, String)>),
}

/// What the verify-on-approval job does with the records (D-105).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verify {
    PullRequest,
    Direct,
    /// docsys/0.5: a maintainer's approval adds `Approved-by:` to the pull
    /// request's description, and the merge commit carries it (D-126)
    Description,
    Off,
}

impl Verify {
    pub fn name(self) -> &'static str {
        match self {
            Verify::PullRequest => "pull-request",
            Verify::Direct => "direct",
            Verify::Description => "description",
            Verify::Off => "off",
        }
    }

    /// The mode a tree reads: on docsys/0.5 the approval rides the pull
    /// request's description into history (D-126); before, the job writes
    /// the records a docsys/0.4 tree reads (D-105).
    pub fn of_era(era: crate::era::Era) -> Verify {
        if era.verification_from_history() {
            Verify::Description
        } else {
            Verify::PullRequest
        }
    }

    /// A mode whose job records what the tree never reads is refused.
    pub fn readable_by(self, era: crate::era::Era) -> Result<(), String> {
        match (era.verification_from_history(), self) {
            (true, Verify::PullRequest | Verify::Direct) => Err(format!(
                "--verify-on-approval {}: its job writes records into pages, and a docsys/0.5 tree reads an approval from history instead (D-126) — use `description` (the default) or `off`",
                self.name()
            )),
            (false, Verify::Description) => Err(
                "--verify-on-approval description: the `Approved-by:` line it adds is read from history, which a docsys/0.4 tree does not do (D-118) — use `pull-request` (its default), `direct` or `off`, or move the tree with `docsys upgrade`"
                    .to_string(),
            ),
            _ => Ok(()),
        }
    }

    fn named(s: &str) -> Option<Verify> {
        [
            Verify::PullRequest,
            Verify::Direct,
            Verify::Description,
            Verify::Off,
        ]
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
            verify: Verify::Description,
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
                format!(
                    "--verify-on-approval takes description, pull-request, direct or off, not `{v}`"
                )
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

const ARM_PREFIX: &str = "            ";

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

const VERIFY_DESCRIPTION: &str = r#"
  # A maintainer's approval is their word (D-095, D-126). When a declared
  # maintainer approves a pull request, this job adds `Approved-by: @login` to
  # its description: the description changes, the commit does not, so every
  # check's verdict stays. With the repository's squash and merge messages set
  # to the pull request's title and description, the line lands in the merge
  # commit, and docsys reads the verification from there — no record, no
  # follow-up pull request. The maintainers are the base branch's.
  approval:
    if: github.event_name == 'pull_request_review' && github.event.review.state == 'approved'
    runs-on: @RUNNER@
    permissions:
      contents: read
      pull-requests: write
    steps:
      - uses: actions/checkout@v5
        with:
          ref: ${{ github.event.pull_request.base.ref }}
@INSTALL@      - env:
          GH_TOKEN: ${{ github.token }}
          NUMBER: ${{ github.event.pull_request.number }}
          LOGIN: ${{ github.event.review.user.login }}
        run: |
          line=$(docsys verify --approval "@$LOGIN" --root @ROOT@)
          if [ -z "$line" ]; then
            echo "docsys: @$LOGIN is not a declared maintainer — nothing to add"
            exit 0
          fi
          body=$(gh pr view "$NUMBER" --json body --jq .body)
          case "$body" in
            *"$line"*) echo "docsys: the description says it already"; exit 0 ;;
          esac
          gh pr edit "$NUMBER" --body "$(printf '%s\n\n%s\n' "$body" "$line")"
"#;

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

/// The parameters as line 1 records them, after the hash.
fn params_line(w: &Workflow) -> String {
    format!(
        "branch={} runner={} install={} verify={} root={}",
        w.branch,
        w.ci.runner.join(","),
        match w.ci.install {
            Install::Cargo => "cargo",
            Install::Release(_) => "release",
        },
        w.ci.verify.name(),
        root_of(&w.root)
    )
}

/// The hash line 1 carries: of the whole file but the hash itself, so an
/// edit anywhere, a recorded parameter included, makes the file the owner's.
fn stamp_hash(version: &str, params: &str, body: &str) -> String {
    crate::fresh::sha256_hex(format!("{STAMP}{version} sha256: {params}\n{body}").as_bytes())
}

/// The workflow file, line 1 stamped.
pub fn render(w: &Workflow) -> String {
    let runner = match w.ci.runner.as_slice() {
        [one] => one.clone(),
        many => format!("[{}]", many.join(", ")),
    };
    let root = root_of(&w.root);
    let install = match &w.ci.install {
        Install::Cargo => fill(INSTALL_CARGO, &[("VERSION", &w.version), ("ROOT", root)]),
        Install::Release(sums) => {
            let arms: String = sums
                .iter()
                .map(|(target, hex)| {
                    let uname = TARGETS
                        .iter()
                        .find(|(t, _)| t == target)
                        .map_or("", |(_, u)| *u);
                    format!("{ARM_PREFIX}{uname}) target={target} sum={hex} ;;\n")
                })
                .collect();
            fill(
                INSTALL_RELEASE,
                &[
                    ("VERSION", &w.version),
                    ("ARMS", &arms),
                    ("RELEASES", RELEASES),
                    ("ROOT", root),
                ],
            )
        }
    };
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
        Verify::Description => fill(
            VERIFY_DESCRIPTION,
            &[("RUNNER", &runner), ("INSTALL", &install), ("ROOT", root)],
        ),
        Verify::Off => String::new(),
    };
    // a review triggers the approval job only, never the docs checks
    let (review, docs_if) = if w.ci.verify == Verify::Description {
        (
            "  pull_request_review:\n    types: [submitted]\n",
            "github.event_name != 'pull_request_review' && github.event.action != 'closed'",
        )
    } else {
        ("", "github.event.action != 'closed'")
    };
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
    /// with the parameters it records, at this binary's version. A release
    /// install's sha256 values are the recorded version's; another version's
    /// archives need the person's new ones.
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
        "release" => Install::Release(release_sums(body)),
        _ => return None,
    };
    let w = Workflow {
        version: version.to_string(),
        branch: field("branch")?.to_string(),
        root: root.to_string(),
        ci: Ci {
            runner: field("runner")?.split(',').map(str::to_string).collect(),
            install,
            verify: Verify::named(field("verify")?)?,
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
