#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]
//! The acceptance test of twenty merges a day: twenty branches, each one
//! typical documentation change, integrated into one tree in three orders
//! under merge, squash and rebase. On a docsys/0.5 tree no file under the
//! documentation root conflicts but a topic file of debt or questions — a
//! risk the owner accepted (D-124), counted apart — and no approval needs a
//! follow-up pull request (D-126). The same branches on a docsys/0.4 tree,
//! under the same binary, are the control: there the shared files conflict
//! and approvals need follow-ups, so a harness that saw nothing would fail.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn run(dir: &Path, program: &str, args: &[&str], day: &str) -> Output {
    let at = format!("{day}T12:00:00+00:00");
    Command::new(program)
        .args(args)
        .current_dir(dir)
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin_dir().display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("DOCSYS_TODAY", day)
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .env_remove("DOCSYS_DISPATCHED")
        .env("GIT_AUTHOR_DATE", &at)
        .env("GIT_COMMITTER_DATE", &at)
        .output()
        .unwrap()
}

fn git(dir: &Path, args: &[&str], day: &str) -> Output {
    let mut all = vec![
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.hooksPath=/dev/null",
    ];
    all.extend_from_slice(args);
    run(dir, "git", &all, day)
}

fn ok(out: Output, what: &str) -> String {
    assert!(
        out.status.success(),
        "{what}: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn docsys(dir: &Path, args: &[&str], day: &str) -> String {
    ok(run(dir, "docsys", args, day), &format!("docsys {args:?}"))
}

fn commit(dir: &Path, message: &str, day: &str) {
    ok(git(dir, &["add", "-A"], day), "add");
    let staged = git(dir, &["diff", "--cached", "--quiet"], day);
    if !staged.status.success() {
        ok(git(dir, &["commit", "-qm", message], day), "commit");
    }
}

fn append(path: &Path, text: &str) {
    let mut now = fs::read_to_string(path).unwrap_or_default();
    now.push_str(text);
    fs::write(path, now).unwrap();
}

const DAY0: &str = "2026-09-01";
const PAGES: [(&str, &[&str]); 5] = [
    ("alpha", &["f1", "f2"]),
    ("beta", &["f3"]),
    ("gamma", &["f4"]),
    ("delta", &["f5"]),
    ("epsilon", &["f6"]),
];

/// A repository adopted by this docsys, at `spec`, with five pinned pages,
/// three debts and two questions; tagged `base`.
fn base(name: &str, spec: &str) -> PathBuf {
    let repo = std::env::temp_dir().join(format!("docsys-n3-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(repo.join("src")).unwrap();
    ok(git(&repo, &["init", "-q", "-b", "main"], DAY0), "init");
    ok(
        git(&repo, &["config", "user.email", "t@example.invalid"], DAY0),
        "email",
    );
    ok(git(&repo, &["config", "user.name", "t"], DAY0), "name");
    // pull requests merge as CI and a host merge them: no local git hook
    ok(
        git(&repo, &["config", "core.hooksPath", "/dev/null"], DAY0),
        "hooks",
    );
    let code: String = (1..=8)
        .map(|i| format!("pub fn f{i}(x: u32) -> u32 {{\n    let y = x + {i};\n    y * 2\n}}\n\n"))
        .collect();
    fs::write(repo.join("src/app.rs"), code).unwrap();
    fs::write(repo.join("README.md"), "# fixture\n").unwrap();
    commit(&repo, "code", DAY0);
    docsys(&repo, &["adopt"], DAY0);
    let meta = repo.join("docs/.docmeta.yml");
    let text = fs::read_to_string(&meta)
        .unwrap()
        .replace("spec: docsys/0.5", &format!("spec: {spec}"))
        .replace(
            "maintainers: []",
            "maintainers: [\"t <t@example.invalid> @t\"]",
        );
    fs::write(&meta, text).unwrap();
    let v04 = spec == "docsys/0.4";
    for (id, symbols) in PAGES {
        docsys(
            &repo,
            &[
                "page",
                "new",
                "reference",
                id,
                "--title",
                id,
                "--unverified",
            ],
            DAY0,
        );
        let page = repo.join(format!("docs/reference/{id}.md"));
        let head = fs::read_to_string(&page).unwrap();
        let head = head.split("<!--").next().unwrap().trim_end().to_string();
        let mut text =
            format!("{head}\n\nThis page states what {id} does; read it before changing it.\n");
        for k in 1..=4 {
            text.push_str(&format!(
                "\nParagraph {k} of {id}: one fact, stated once.\n"
            ));
        }
        fs::write(&page, text.replace("sources: []", "sources: [src/app.rs]")).unwrap();
        if v04 {
            append(
                &repo.join("docs/index.md"),
                &format!("- [[reference/{id}|{id}]] -- what {id} does.\n"),
            );
        }
        for s in symbols {
            docsys(
                &repo,
                &[
                    "pin",
                    &format!("reference/{id}"),
                    "src/app.rs",
                    "--symbol",
                    s,
                ],
                DAY0,
            );
        }
    }
    for (k, topic) in [(1, Some("alpha")), (2, Some("beta")), (3, None)] {
        let text = format!("debt {k}");
        let mut args = vec![
            "debt",
            "add",
            text.as_str(),
            "--deferred",
            "later",
            "--repay-when",
            "soon",
        ];
        if let Some(t) = topic {
            args.extend(["--topic", t]);
        }
        docsys(&repo, &args, DAY0);
    }
    for k in 1..=2 {
        docsys(&repo, &["question", "add", &format!("question {k}?")], DAY0);
    }
    commit(&repo, "pages, pins, debt, questions", DAY0);
    ok(git(&repo, &["tag", "base"], DAY0), "tag");
    repo
}

const ACTIONS: [&str; 20] = [
    "add-page:0",
    "add-page:1",
    "add-page:2",
    "edit-body:alpha:1",
    "edit-body:alpha:3",
    "edit-body:beta:2",
    "add-debt:0:alpha",
    "add-debt:1:gamma",
    "add-debt:2:",
    "close-debt:debt 1:1",
    "close-debt:debt 2:2",
    "add-question:0:alpha",
    "add-question:1:",
    "close-question:question 1:1",
    "journal:0:f7",
    "journal:1:f8",
    "refresh-pin:alpha:f1",
    "refresh-pin:alpha:f2",
    "verify:gamma",
    "verify:delta",
];

fn change_code(repo: &Path, f: &str) {
    let path = repo.join("src/app.rs");
    let text = fs::read_to_string(&path).unwrap().replacen(
        &format!("pub fn {f}(x: u32) -> u32 {{\n    let y = x + "),
        &format!("pub fn {f}(x: u32) -> u32 {{\n    let y = x.saturating_add(1) + "),
        1,
    );
    fs::write(path, text).unwrap();
}

/// One branch from `base`, doing what an agent does for `action`.
fn branch(repo: &Path, n: usize, action: &str, v04: bool) -> String {
    let day = format!("2026-09-{:02}", 2 + n);
    let day = day.as_str();
    let parts: Vec<&str> = action.split(':').collect();
    let name = format!("b{n:02}-{}", parts[0]);
    ok(
        git(repo, &["checkout", "-q", "-b", &name, "base"], day),
        "branch",
    );
    let mut message = name.clone();
    match parts[0] {
        "add-page" => {
            let id = format!("new{}", parts[1]);
            docsys(
                repo,
                &[
                    "page",
                    "new",
                    "reference",
                    &id,
                    "--title",
                    &id,
                    "--unverified",
                ],
                day,
            );
            let page = repo.join(format!("docs/reference/{id}.md"));
            let head = fs::read_to_string(&page).unwrap();
            let head = head.split("<!--").next().unwrap().trim_end().to_string();
            fs::write(
                &page,
                format!("{head}\n\nThis page states new fact {id}; read it first.\n"),
            )
            .unwrap();
            if v04 {
                append(
                    &repo.join("docs/index.md"),
                    &format!("- [[reference/{id}|{id}]] -- new fact {id}.\n"),
                );
            }
        }
        "edit-body" => {
            let (id, k) = (parts[1], parts[2]);
            let page = repo.join(format!("docs/reference/{id}.md"));
            let text = fs::read_to_string(&page).unwrap().replace(
                &format!("Paragraph {k} of {id}: one fact, stated once."),
                &format!("Paragraph {k} of {id}: one fact, stated once, now sharper."),
            );
            fs::write(&page, text).unwrap();
            // what the agent's post-edit relay runs after the edit
            let payload = format!(
                "{{\"tool_name\":\"Edit\",\"tool_input\":{{\"file_path\":\"{}\"}}}}",
                page.display()
            );
            let mut child = Command::new(bin_dir().join("docsys"))
                .args(["hook", "post-tool-use", "--root", "docs", "--stdin"])
                .current_dir(repo)
                .env("DOCSYS_TODAY", day)
                .env("DOCSYS_NO_AUTO_INSTALL", "1")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap();
            use std::io::Write as _;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(payload.as_bytes())
                .unwrap();
            child.wait().unwrap();
        }
        "add-debt" => {
            let text = format!("new debt {}", parts[1]);
            let mut args = vec![
                "debt",
                "add",
                text.as_str(),
                "--deferred",
                "later",
                "--repay-when",
                "soon",
            ];
            if !parts[2].is_empty() {
                args.extend(["--topic", parts[2]]);
            }
            docsys(repo, &args, day);
        }
        "close-debt" => {
            // docsys/0.4 names an item by its number in the ledger
            let which = if v04 { parts[2] } else { parts[1] };
            docsys(repo, &["debt", "close", which, "--note", "done"], day);
            if !v04 {
                message = format!("{name}\n\nResolved: done");
            }
        }
        "add-question" => {
            let text = format!("new question {}?", parts[1]);
            let mut args = vec!["question", "add", text.as_str()];
            if !parts[2].is_empty() {
                args.extend(["--topic", parts[2]]);
            }
            docsys(repo, &args, day);
        }
        "close-question" => {
            let which = if v04 { parts[2] } else { parts[1] };
            docsys(repo, &["question", "close", which, "--answer", "yes"], day);
            if !v04 {
                message = format!("{name}\n\nAnswered: yes");
            }
        }
        "journal" => {
            change_code(repo, parts[2]);
            let out = docsys(
                repo,
                &[
                    "journal",
                    "add",
                    &format!("branch {n} changed {} and says why", parts[2]),
                    "--title",
                    &format!("work {n}"),
                ],
                day,
            );
            if !v04 {
                // docsys/0.5: the entry is the commit message it prints
                message = out.trim().to_string();
            }
        }
        "refresh-pin" => {
            change_code(repo, parts[2]);
            docsys(
                repo,
                &["pin", "--refresh", &format!("reference/{}", parts[1])],
                day,
            );
        }
        "verify" => {
            docsys(
                repo,
                &["verify", &format!("reference/{}", parts[1]), "--by", "t"],
                day,
            );
        }
        other => panic!("{other}"),
    }
    commit(repo, &message, day);
    ok(git(repo, &["checkout", "-q", "main"], day), "back");
    name
}

/// Keep both sides of every conflict, as a person does with two lists.
fn keep_both(repo: &Path, day: &str) -> Vec<String> {
    let files: Vec<String> = ok(
        git(repo, &["diff", "--name-only", "--diff-filter=U"], day),
        "conflicts",
    )
    .lines()
    .map(str::to_string)
    .collect();
    for f in &files {
        let path = repo.join(f);
        match fs::read_to_string(&path) {
            Ok(text) => {
                let kept: String = text
                    .lines()
                    .filter(|l| {
                        !l.starts_with("<<<<<<< ")
                            && !l.starts_with("=======")
                            && !l.starts_with(">>>>>>> ")
                    })
                    .map(|l| format!("{l}\n"))
                    .collect();
                fs::write(&path, kept).unwrap();
                ok(git(repo, &["add", "--", f], day), "add resolved");
            }
            Err(_) => {
                let _ = git(repo, &["rm", "-q", "--cached", "--", f], day);
            }
        }
    }
    files
}

#[derive(Default, Debug)]
struct Tally {
    /// conflicted files under the documentation root, topic files aside
    docs: BTreeMap<String, usize>,
    /// conflicted topic files of debt and questions (D-124's accepted risk)
    topics: BTreeMap<String, usize>,
    /// conflicted files outside the documentation root
    other: BTreeMap<String, usize>,
    /// approvals whose records needed a follow-up pull request
    follow_ups: usize,
}

impl Tally {
    fn count(&mut self, files: &[String]) {
        for f in files {
            let bucket =
                if f.starts_with("docs/work/debt/") || f.starts_with("docs/work/questions/") {
                    &mut self.topics
                } else if f.starts_with("docs/") {
                    &mut self.docs
                } else {
                    &mut self.other
                };
            *bucket.entry(f.clone()).or_default() += 1;
        }
    }
}

/// Integrate `names` into a `main` reset to `base`, one pull request at a
/// time; after each, the approval job of a tree's era runs.
fn integrate(repo: &Path, names: &[String], mode: &str, tally: &mut Tally) {
    let day = "2026-09-30";
    ok(git(repo, &["checkout", "-q", "-f", "main"], day), "main");
    ok(git(repo, &["reset", "-q", "--hard", "base"], day), "reset");
    for name in names {
        let before = ok(git(repo, &["rev-parse", "HEAD"], day), "head")
            .trim()
            .to_string();
        let mut files = Vec::new();
        match mode {
            "merge" => {
                let merged = git(repo, &["merge", "--no-ff", "-q", "-m", name, name], day);
                if !merged.status.success() {
                    files = keep_both(repo, day);
                    let _ = git(repo, &["commit", "-qm", name], day);
                }
            }
            "squash" => {
                let _ = git(repo, &["merge", "--squash", "-q", name], day);
                files = keep_both(repo, day);
                let _ = git(
                    repo,
                    &["commit", "-qm", &format!("{name}\n\nApproved-by: t")],
                    day,
                );
            }
            _ => {
                ok(
                    git(repo, &["checkout", "-q", "-B", "rebasing", name], day),
                    "copy",
                );
                let mut step = git(repo, &["rebase", "-q", "main"], day);
                for _ in 0..20 {
                    if step.status.success() {
                        break;
                    }
                    let now = keep_both(repo, day);
                    if now.is_empty() {
                        step = git(repo, &["-c", "core.editor=true", "rebase", "--skip"], day);
                        continue;
                    }
                    files.extend(now);
                    step = git(
                        repo,
                        &["-c", "core.editor=true", "rebase", "--continue"],
                        day,
                    );
                }
                ok(step, "rebase");
                ok(git(repo, &["checkout", "-q", "main"], day), "main");
                ok(
                    git(repo, &["merge", "-q", "--ff-only", "rebasing"], day),
                    "ff",
                );
            }
        }
        tally.count(&files);
        // the approval job: on docsys/0.4 it records each page the pull
        // request touched, and the records need a follow-up pull request
        let after = ok(git(repo, &["rev-parse", "HEAD"], day), "head")
            .trim()
            .to_string();
        let range = format!("{before}...{after}");
        let _ = run(
            repo,
            "docsys",
            &["verify", "--range", &range, "--by", "@t", "--commit"],
            day,
        );
        let now = ok(git(repo, &["rev-parse", "HEAD"], day), "head")
            .trim()
            .to_string();
        let left = ok(git(repo, &["status", "--porcelain"], day), "status");
        if !left.trim().is_empty() {
            commit(repo, "the follow-up's records", day);
        }
        if now != after || !left.trim().is_empty() {
            tally.follow_ups += 1;
        }
    }
}

/// The three orders: a fixed shuffle per seed, so a failure reproduces.
fn order(names: &[String], seed: u64) -> Vec<String> {
    let mut out = names.to_vec();
    let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    for i in (1..out.len()).rev() {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let j = usize::try_from(state >> 33).unwrap() % (i + 1);
        out.swap(i, j);
    }
    out
}

fn measure(spec: &str, modes: &[&str], seeds: &[u64]) -> Tally {
    let label = spec.replace(['/', '.'], "-");
    let repo = base(&label, spec);
    let v04 = spec == "docsys/0.4";
    let names: Vec<String> = ACTIONS
        .iter()
        .enumerate()
        .map(|(n, a)| branch(&repo, n, a, v04))
        .collect();
    let mut tally = Tally::default();
    for mode in modes {
        for seed in seeds {
            integrate(&repo, &order(&names, *seed), mode, &mut tally);
        }
    }
    let _ = fs::remove_dir_all(&repo);
    tally
}

#[test]
fn twenty_branches_merge_with_no_documentation_conflict_and_no_follow_up() {
    let now = measure("docsys/0.5", &["merge", "squash", "rebase"], &[1, 2, 3]);
    eprintln!("docsys/0.5: {now:?}");
    assert!(
        now.docs.is_empty(),
        "documentation files conflicted on a docsys/0.5 tree: {:?}",
        now.docs
    );
    assert_eq!(
        now.follow_ups, 0,
        "approvals needed follow-up pull requests"
    );

    // the control: the same branches on a docsys/0.4 tree conflict where many
    // branches write one file, and approvals need follow-ups — the harness
    // sees what it measures; one order under merge is enough to show it
    let before = measure("docsys/0.4", &["merge"], &[1]);
    eprintln!("docsys/0.4: {before:?}");
    assert!(
        !before.docs.is_empty(),
        "the harness saw no conflict on a docsys/0.4 tree: {before:?}"
    );
    assert!(before.follow_ups > 0, "{before:?}");
}
