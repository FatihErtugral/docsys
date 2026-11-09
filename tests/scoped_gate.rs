#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! docsys/0.6: a check scoped to a change blocks on what the change adds
//! (D-136); a stale pin names the change that moved its region, and a merge
//! of changes that each re-read the region is reported (R-111, D-135); a
//! pull request's refusal names its way out (R-209). Every repository here is
//! a temporary git repository driven through the binary and its git hook.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
}

/// PATH with this build first: the git hook a commit runs is this docsys.
fn path() -> String {
    format!(
        "{}:{}",
        bin().parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-scoped-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(dir: &Path, program: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut c = Command::new(program);
    c.args(args)
        .current_dir(dir)
        .env("PATH", path())
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid");
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().unwrap()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let mut all = vec!["-c", "commit.gpgsign=false"];
    all.extend_from_slice(args);
    let out = run(dir, Path::new("git"), &all, &[]);
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// `git commit` through the clone's gate: its status and what it printed.
fn commit_through_gate(dir: &Path, message: &str) -> (i32, String) {
    git(dir, &["add", "-A"]);
    let out = run(
        dir,
        Path::new("git"),
        &["-c", "commit.gpgsign=false", "commit", "-q", "-m", message],
        &[],
    );
    (
        out.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// A commit the gate does not judge: the test sets the repository up.
fn commit_quietly(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(
        dir,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-q",
            "-m",
            message,
        ],
    );
}

fn docsys(dir: &Path, args: &[&str]) -> Output {
    run(dir, &bin(), args, &[])
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn write(dir: &Path, rel: &str, body: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, body).unwrap();
}

fn page(id: &str, body: &str) -> String {
    format!("---\nid: {id}\ntype: reference\n---\n# {id}\n\n{body}\n")
}

/// A repository `docsys adopt` set up — a docsys/0.6 tree with its git gate —
/// carrying debt: a dangling wiki-link and a dangling `doc:` citation.
fn adopted_with_debt(name: &str) -> PathBuf {
    let repo = tmp(name);
    git(&repo, &["init", "-q", "-b", "main"]);
    write(&repo, "README.md", "# r\n");
    let out = docsys(&repo, &["adopt", "--no-report"]);
    assert!(out.status.success(), "{out:?}");
    write(&repo, "src/a.ts", "export const a = 1; // doc: ghost\n");
    write(
        &repo,
        "docs/reference/overview.md",
        &page("overview", "The overview links [[reference/missing]]."),
    );
    commit_quietly(&repo, "the tree with its debt");
    repo
}

/// D-136: the git gate is hard on a tree with debt; a commit that adds
/// nothing wrong passes, naming the debt as inherited; one that adds an
/// error is stopped by that error alone.
#[test]
fn a_commit_inherits_its_bases_debt_and_is_stopped_by_its_own_errors() {
    let repo = adopted_with_debt("local");
    let hook = fs::read_to_string(repo.join(".git/hooks/pre-commit")).unwrap();
    assert!(hook.contains("docsys_gate_exit=1"), "{hook}");
    write(&repo, "src/b.ts", "export const b = 2;\n");
    write(
        &repo,
        "docs/reference/b.md",
        &page("b", "This page describes b."),
    );
    let (code, said) = commit_through_gate(&repo, "add b");
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("inherited from "), "{said}");
    assert!(
        said.contains("-- 0 error(s) (2 inherited, not blocking), 0 warning(s)"),
        "{said}"
    );
    write(&repo, "src/c.ts", "export const c = 3; // doc: phantom\n");
    write(
        &repo,
        "docs/reference/c.md",
        &page("c", "This page describes c."),
    );
    let (code, said) = commit_through_gate(&repo, "add c");
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("ERROR R-076 src/c.ts [phantom]"), "{said}");
    assert!(
        said.contains("-- 1 error(s) (2 inherited, not blocking)"),
        "{said}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// D-136: a pull request's check, `gate --range`, compares its range with the
/// merge base: an error the base carries is inherited, one the range adds
/// fails the check.
#[test]
fn a_pull_requests_check_fails_only_on_what_its_range_adds() {
    let repo = adopted_with_debt("range");
    let base = git(&repo, &["rev-parse", "--short=8", "HEAD"]);
    git(&repo, &["checkout", "-q", "-b", "unrelated"]);
    write(&repo, "src/b.ts", "export const b = 2;\n");
    write(
        &repo,
        "docs/reference/b.md",
        &page("b", "This page describes b."),
    );
    commit_quietly(&repo, "unrelated, documented");
    let out = docsys(&repo, &["gate", "--range", "main...HEAD"]);
    let said = text(&out);
    assert_eq!(out.status.code(), Some(0), "{said}");
    assert!(said.contains(&format!("inherited from {base}")), "{said}");
    git(&repo, &["checkout", "-q", "main"]);
    git(&repo, &["checkout", "-q", "-b", "broken"]);
    write(
        &repo,
        "docs/reference/d.md",
        &page("d", "This page links [[reference/nowhere]]."),
    );
    commit_quietly(&repo, "a dangling link");
    let out = docsys(&repo, &["gate", "--range", "main...HEAD"]);
    let said = text(&out);
    assert_eq!(out.status.code(), Some(1), "{said}");
    assert!(
        said.contains("ERROR R-071 reference/d.md [reference/nowhere]"),
        "{said}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// D-136: a shallow checkout cannot tell inherited from added: one line says
/// why, and every error blocks.
#[test]
fn a_shallow_checkout_blocks_on_every_error_and_says_why() {
    let repo = adopted_with_debt("shallow-src");
    let clone = tmp("shallow");
    let _ = fs::remove_dir_all(&clone);
    git(
        repo.parent().unwrap(),
        &[
            "clone",
            "-q",
            "--depth",
            "1",
            &format!("file://{}", repo.display()),
            clone.to_str().unwrap(),
        ],
    );
    write(&clone, "src/b.ts", "export const b = 2;\n");
    write(
        &clone,
        "docs/reference/b.md",
        &page("b", "This page describes b."),
    );
    git(&clone, &["add", "-A"]);
    let out = docsys(&clone, &["gate"]);
    let said = text(&out);
    assert_eq!(out.status.code(), Some(1), "{said}");
    assert!(said.contains("history is shallow here"), "{said}");
    assert!(!said.contains("inherited from"), "{said}");
    let _ = fs::remove_dir_all(&repo);
    let _ = fs::remove_dir_all(&clone);
}

/// R-209, D-125: a range that changes code and no documentation is refused,
/// the refusal names its way out, and the summary counts it as an error; a
/// `Docs:` line in a commit of the range answers it.
#[test]
fn a_range_with_code_and_no_docs_names_its_way_out_and_counts_as_an_error() {
    let repo = tmp("ci-only");
    git(&repo, &["init", "-q", "-b", "main"]);
    write(&repo, "README.md", "# r\n");
    assert!(docsys(&repo, &["adopt", "--no-report"]).status.success());
    commit_quietly(&repo, "adopted");
    git(&repo, &["checkout", "-q", "-b", "ci"]);
    write(&repo, ".github/linter.yml", "rules: {}\n");
    commit_quietly(&repo, "a CI configuration");
    let out = docsys(&repo, &["gate", "--range", "main...HEAD"]);
    let said = text(&out);
    assert_eq!(out.status.code(), Some(1), "{said}");
    assert!(
        said.contains("GATE range changes with no docs change: .github/linter.yml — add the page, or a `Docs: <why>` line to a commit of this range"),
        "{said}"
    );
    assert!(said.contains("-- 1 error(s), 0 warning(s)"), "{said}");
    git(
        &repo,
        &[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "ci",
            "-m",
            "Docs: CI configuration only",
        ],
    );
    let out = docsys(&repo, &["gate", "--range", "main...HEAD"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    // the local question asks for that line too
    assert!(docsys::say::ask(false).contains("with that `Docs:` line in its message"));
    let _ = fs::remove_dir_all(&repo);
}

// ---------------------------------------------------------------- pins

const TOOL: &str = "export function deliver(a: number) {\n  const x = a + 1;\n  const k = 7;\n  const m = 8;\n  const y = x * 2;\n  return y + k + m;\n}\n";

/// A docsys tree at `spec` with one page pinned to `deliver`, committed on
/// `develop`.
fn pinned(name: &str, spec: &str) -> PathBuf {
    let repo = tmp(name);
    git(&repo, &["init", "-q", "-b", "develop"]);
    assert!(docsys(&repo, &["init", "--root", "docs"]).status.success());
    let meta = repo.join("docs/.docmeta.yml");
    let m = fs::read_to_string(&meta).unwrap();
    fs::write(
        &meta,
        m.replace("spec: docsys/0.6", &format!("spec: {spec}")),
    )
    .unwrap();
    write(&repo, "src/tool.ts", TOOL);
    write(
        &repo,
        "docs/reference/tool.md",
        &page(
            "tool",
            "This page describes deliver; read it when deliver changes.",
        ),
    );
    let out = docsys(
        &repo,
        &[
            "pin",
            "reference/tool.md",
            "src/tool.ts",
            "--symbol",
            "deliver",
        ],
    );
    assert!(out.status.success(), "{out:?}");
    commit_quietly(&repo, "base");
    repo
}

fn edit(repo: &Path, from: &str, to: &str) {
    let p = repo.join("src/tool.ts");
    let t = fs::read_to_string(&p).unwrap();
    assert!(t.contains(from), "{from}");
    fs::write(&p, t.replacen(from, to, 1)).unwrap();
}

fn refresh(repo: &Path) {
    let out = docsys(repo, &["pin", "--refresh", "reference/tool.md"]);
    assert!(out.status.success(), "{out:?}");
}

/// A branch from `develop` that moves `from` → `to`, refreshed or not.
fn branch(repo: &Path, name: &str, from: &str, to: &str, refreshed: bool) {
    git(repo, &["checkout", "-q", "develop"]);
    git(repo, &["checkout", "-q", "-b", name]);
    edit(repo, from, to);
    if refreshed {
        refresh(repo);
    }
    commit_quietly(repo, &format!("{name} work"));
}

fn squash(repo: &Path, name: &str, subject: &str) {
    git(repo, &["checkout", "-q", "develop"]);
    git(repo, &["merge", "-q", "--squash", name]);
    commit_quietly(repo, subject);
}

fn r111(repo: &Path) -> (Option<i32>, Vec<String>) {
    let out = docsys(repo, &["lint"]);
    let lines = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.contains(" R-111 "))
        .map(str::to_string)
        .collect();
    (out.status.code(), lines)
}

/// R-111 names the change that moved a region no acknowledgement matches:
/// its pull request, title, author, day and commit (D-135).
#[test]
fn a_stale_pin_names_the_change_that_moved_its_region() {
    let repo = pinned("names", "docsys/0.5");
    edit(&repo, "a + 1", "a + 2");
    git(&repo, &["add", "-A"]);
    git(
        &repo,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-q",
            "--author",
            "Ana Lee <ana@example.invalid>",
            "-m",
            "ABC-1 | deliver adds two (#12)",
        ],
    );
    let sha = git(&repo, &["rev-parse", "--short=8", "HEAD"]);
    let day = git(&repo, &["log", "-1", "--format=%as"]);
    let (code, lines) = r111(&repo);
    assert_eq!(code, Some(1), "{lines:?}");
    let want = format!("since #12 \"ABC-1 | deliver adds two\" (Ana Lee, {day}, {sha}) —");
    assert!(
        lines.iter().any(|l| l.contains(&want)),
        "{want}\n{lines:#?}"
    );
    let _ = fs::remove_dir_all(&repo);
}

/// D-135: on a docsys/0.6 tree two squash merges that each re-read the
/// region are reported, naming both; a later change with no re-read is an
/// error naming it. On a docsys/0.5 tree the same merge is an error.
#[test]
fn a_squash_merge_of_re_read_changes_is_reported_on_0_6() {
    for spec in ["docsys/0.6", "docsys/0.5"] {
        let repo = pinned(&format!("squash-{}", &spec[7..]), spec);
        branch(&repo, "pr-a", "a + 1", "a + 2", true);
        branch(&repo, "pr-b", "x * 2", "x * 3", true);
        squash(&repo, "pr-a", "ABC-1 | pr-a (#1)");
        squash(&repo, "pr-b", "ABC-2 | pr-b (#2)");
        let (code, lines) = r111(&repo);
        if spec == "docsys/0.6" {
            assert_eq!(code, Some(0), "{lines:?}");
            assert!(
                lines.iter().any(|l| l.starts_with("WARN R-111 ")
                    && l.contains("combines #1 and #2, each re-read on its own")),
                "{lines:#?}"
            );
            edit(&repo, "const k = 7", "const k = 6");
            commit_quietly(&repo, "ABC-3 | a hotfix (#3)");
            let (code, lines) = r111(&repo);
            assert_eq!(code, Some(1), "{lines:?}");
            assert!(
                lines
                    .iter()
                    .any(|l| l.starts_with("ERROR R-111 ") && l.contains("since #3 ")),
                "{lines:#?}"
            );
        } else {
            assert_eq!(code, Some(1), "{lines:?}");
            assert!(
                lines.iter().any(|l| l.starts_with("ERROR R-111 ")
                    && l.contains("it combines #1 and #2, each re-read on its own")),
                "{lines:#?}"
            );
        }
        let _ = fs::remove_dir_all(&repo);
    }
}

/// D-135: merge commits are read along the first parent, and a merged head
/// that moved the region after its re-read is an error.
#[test]
fn a_merge_commit_is_read_along_its_first_parent() {
    let repo = pinned("merge", "docsys/0.6");
    branch(&repo, "pr-a", "a + 1", "a + 2", true);
    branch(&repo, "pr-b", "x * 2", "x * 3", true);
    git(&repo, &["checkout", "-q", "develop"]);
    for (n, b) in [(1, "pr-a"), (2, "pr-b")] {
        git(
            &repo,
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "merge",
                "-q",
                "--no-ff",
                b,
                "-m",
                &format!("Merge pull request #{n} from org/{b}"),
            ],
        );
    }
    let (code, lines) = r111(&repo);
    assert_eq!(code, Some(0), "{lines:?}");
    assert!(
        lines.iter().any(|l| l.contains("combines #1 and #2")),
        "{lines:#?}"
    );
    branch(&repo, "pr-x", "const m = 8", "const m = 9", true);
    edit(&repo, "const k = 7", "const k = 5");
    commit_quietly(&repo, "pr-x moves it again");
    git(&repo, &["checkout", "-q", "develop"]);
    git(
        &repo,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "merge",
            "-q",
            "--no-ff",
            "pr-x",
            "-m",
            "Merge pull request #4 from org/pr-x",
        ],
    );
    let (code, lines) = r111(&repo);
    assert_eq!(code, Some(1), "{lines:?}");
    assert!(lines.iter().any(|l| l.contains("since #4 ")), "{lines:#?}");
    let _ = fs::remove_dir_all(&repo);
}

/// D-135: a squash of a pull request whose head never re-read its last
/// change is an error: the acknowledgement the squash added is checked
/// against the pull request's own text, not taken on trust.
#[test]
fn a_squash_whose_head_was_stale_is_an_error() {
    let repo = pinned("stale-head", "docsys/0.6");
    branch(&repo, "pr-a", "a + 1", "a + 2", true);
    branch(&repo, "pr-y", "x * 2", "x * 3", true);
    edit(&repo, "return y + k + m", "return y + k - m");
    commit_quietly(&repo, "pr-y moves it again, no re-read");
    squash(&repo, "pr-a", "ABC-1 | pr-a (#1)");
    squash(&repo, "pr-y", "ABC-5 | pr-y (#5)");
    let (code, lines) = r111(&repo);
    assert_eq!(code, Some(1), "{lines:?}");
    assert!(lines.iter().any(|l| l.contains("since #5 ")), "{lines:#?}");
    let _ = fs::remove_dir_all(&repo);
}

/// D-135: a shallow checkout cannot read the combination from history: the
/// error stays, and says why.
#[test]
fn a_shallow_checkout_keeps_the_error_and_says_why() {
    let repo = pinned("shallow-pins", "docsys/0.6");
    branch(&repo, "pr-a", "a + 1", "a + 2", true);
    branch(&repo, "pr-b", "x * 2", "x * 3", true);
    squash(&repo, "pr-a", "ABC-1 | pr-a (#1)");
    squash(&repo, "pr-b", "ABC-2 | pr-b (#2)");
    let clone = tmp("shallow-pins-clone");
    let _ = fs::remove_dir_all(&clone);
    git(
        repo.parent().unwrap(),
        &[
            "clone",
            "-q",
            "--depth",
            "1",
            "--branch",
            "develop",
            &format!("file://{}", repo.display()),
            clone.to_str().unwrap(),
        ],
    );
    let (code, lines) = r111(&clone);
    assert_eq!(code, Some(1), "{lines:?}");
    assert!(
        lines.iter().any(|l| l.contains("(history is shallow here")),
        "{lines:#?}"
    );
    let _ = fs::remove_dir_all(&repo);
    let _ = fs::remove_dir_all(&clone);
}

/// D-135: a pull request that moves a combined region, or edits its page,
/// re-reads the combination: its check fails until it does, and the report
/// stops once it has.
#[test]
fn a_pull_request_that_touches_a_combined_region_re_reads_it() {
    let repo = pinned("touch", "docsys/0.6");
    branch(&repo, "pr-a", "a + 1", "a + 2", true);
    branch(&repo, "pr-b", "x * 2", "x * 3", true);
    squash(&repo, "pr-a", "ABC-1 | pr-a (#1)");
    squash(&repo, "pr-b", "ABC-2 | pr-b (#2)");
    git(&repo, &["checkout", "-q", "-b", "pr-g"]);
    let p = repo.join("docs/reference/tool.md");
    fs::write(
        &p,
        fs::read_to_string(&p)
            .unwrap()
            .replace("changes.", "changes, and adds k and m."),
    )
    .unwrap();
    commit_quietly(&repo, "pr-g edits the page");
    let out = docsys(&repo, &["gate", "--range", "develop...HEAD"]);
    let said = text(&out);
    assert_eq!(out.status.code(), Some(1), "{said}");
    assert!(
        said.contains("(this change moves the region or its page, so it re-reads the combination)"),
        "{said}"
    );
    refresh(&repo);
    commit_quietly(&repo, "pr-g re-reads it");
    let out = docsys(&repo, &["gate", "--range", "develop...HEAD"]);
    let said = text(&out);
    assert_eq!(out.status.code(), Some(0), "{said}");
    assert!(!said.contains("R-111"), "{said}");
    let _ = fs::remove_dir_all(&repo);
}
