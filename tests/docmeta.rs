#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
//! A `.docmeta.yml` value reads one way wherever it is read (D-002): a
//! trailing comment, quotes or a list's shape mean the same to the era, the
//! commit policy, the profile and every writer that consults a key.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-docmeta-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docsys"))
}

fn git(dir: &Path, args: &[&str]) -> Output {
    let path = format!(
        "{}:{}",
        bin().parent().unwrap().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("PATH", path)
        .env_remove("DOCSYS_DISPATCHED")
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .output()
        .unwrap()
}

fn ok(dir: &Path, args: &[&str]) {
    let out = git(dir, args);
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

fn docsys(dir: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .current_dir(dir)
        .env_remove("DOCSYS_DISPATCHED")
        .env("DOCSYS_NO_AUTO_INSTALL", "1")
        .output()
        .unwrap()
}

/// An adopted repository whose `.docmeta.yml` lines are rewritten by `edit`.
fn adopted(name: &str, edit: impl Fn(&str) -> String) -> PathBuf {
    let repo = tmp(name);
    ok(&repo, &["init", "-q", "-b", "main"]);
    ok(&repo, &["config", "user.email", "t@example.invalid"]);
    ok(&repo, &["config", "user.name", "t"]);
    ok(&repo, &["config", "commit.gpgsign", "false"]);
    fs::write(repo.join("main.rs"), "fn main() {}\n").unwrap();
    let out = docsys(&repo, &["adopt"]);
    assert!(out.status.success(), "{out:?}");
    let meta = repo.join("docs/.docmeta.yml");
    let text = fs::read_to_string(&meta).unwrap();
    fs::write(&meta, edit(&text)).unwrap();
    ok(&repo, &["add", "-A"]);
    ok(
        &repo,
        &["commit", "-qm", "adopt", "-m", "Docs: the tree itself"],
    );
    repo
}

fn commented(text: &str, key: &str, comment: &str) -> String {
    text.lines()
        .map(|l| {
            if l.starts_with(&format!("{key}:")) {
                format!("{l}   # {comment}")
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[test]
fn a_commented_spec_line_is_the_spec_it_names() {
    let repo = adopted("spec", |t| {
        commented(
            &t.replace("commit_policy: ask", "commit_policy: require"),
            "spec",
            "required",
        )
    });
    let root = repo.join("docs");
    assert_eq!(
        docsys::era::Era::at(&root),
        docsys::era::Era::of(&docsys::tree::DocTree::load(&root).unwrap())
    );
    // R-209 holds: code alone with a message that says nothing is refused
    fs::write(repo.join("main.rs"), "fn main() { run() }\n").unwrap();
    ok(&repo, &["add", "main.rs"]);
    let out = git(&repo, &["commit", "-qm", "run on start"]);
    assert!(!out.status.success(), "{out:?}");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("Docs: <why>"), "{err}");
    assert!(!err.contains("declares docsys/0.4"), "{err}");
    ok(&repo, &["reset", "-q"]);
    // a debt lands in its topic's file, the 0.5 format
    let added = docsys(
        &repo,
        &[
            "debt",
            "add",
            "run has no timeout",
            "--deferred",
            "x",
            "--repay-when",
            "y",
        ],
    );
    assert!(added.status.success(), "{added:?}");
    assert!(repo.join("docs/work/debt/general.md").is_file());
    assert!(!repo.join("docs/work/debt.md").exists());
}

/// A tree whose `spec:` line names no version is served as docsys/0.4
/// (D-118), and the version notice says why in lint's terms: it declares
/// nothing, or a value that is no version — never `docsys/0.4`.
#[test]
fn the_version_notice_says_what_the_spec_line_declares() {
    let dir = tmp("notice");
    let init = docsys(&dir, &["init"]);
    assert!(init.status.success(), "{init:?}");
    let meta = dir.join("docs/.docmeta.yml");
    let text = fs::read_to_string(&meta).unwrap();
    for (line, notice, finding) in [
        (
            "spec: docsys/0.4   # c",
            "this tree declares docsys/0.4 and is served by its rules;",
            "",
        ),
        (
            "spec:   # only a comment",
            "this tree declares no spec and is served by docsys/0.4's rules;",
            "missing `spec:`",
        ),
        (
            "",
            "this tree declares no spec and is served by docsys/0.4's rules;",
            "missing `spec:`",
        ),
        (
            "spec: docsys/0.5#x",
            "this tree declares `docsys/0.5#x`, no `docsys/0.<minor>` version, and is served by docsys/0.4's rules;",
            "`docsys/0.5#x` is not an implemented `docsys/0.<minor>` version",
        ),
    ] {
        let spec = text.lines().next().unwrap();
        assert!(spec.starts_with("spec: "), "{text}");
        fs::write(&meta, text.replacen(spec, line, 1)).unwrap();
        let out = docsys(&dir, &["lint"]);
        let err = String::from_utf8_lossy(&out.stderr);
        let said = String::from_utf8_lossy(&out.stdout);
        assert!(err.contains(&format!("docsys: {notice}")), "{line}: {err}");
        assert_eq!(err.matches("docsys: this tree").count(), 1, "{line}: {err}");
        if !finding.is_empty() {
            assert!(!err.contains("declares docsys/0.4"), "{line}: {err}");
            assert!(said.contains(finding), "{line}: {said}");
        }
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_commented_commit_policy_is_the_policy_it_names() {
    let repo = adopted("policy", |t| {
        commented(
            &t.replace("commit_policy: ask", "commit_policy: \"require\""),
            "commit_policy",
            "every commit says why",
        )
    });
    assert_eq!(
        docsys::hook::commit_policy(&repo.join("docs")),
        docsys::hook::CommitPolicy::Require
    );
}

#[test]
fn a_profile_reads_as_its_value_and_never_as_its_comment() {
    // a project whose comment mentions a knowledge base is a project
    let repo = adopted("profile-project", |t| {
        commented(t, "profile", "not a knowledge-base")
    });
    let root = repo.join("docs");
    assert!(!docsys::hook::is_knowledge_base(&root));
    let again = docsys::adopt::run(&repo, &root, "en");
    assert!(again.is_ok(), "{:?}", again.err());
    fs::create_dir_all(root.join("work/features")).unwrap();
    fs::write(
        root.join("work/features/x.md"),
        "---\nid: x\ntype: feature\nstatus: done\n---\n# X\n\n## Context\n\nWhy.\n",
    )
    .unwrap();
    let plan = docsys::graduate::plan(&root, "work/features/x.md");
    assert!(
        plan.as_ref()
            .err()
            .is_none_or(|e| !e.contains("knowledge-base")),
        "{plan:?}"
    );
    // a knowledge base whose profile line carries a comment is one
    let base = tmp("profile-base").join("base");
    docsys::migrate::init_profile(&base, "en", "knowledge-base").unwrap();
    let meta = base.join(".docmeta.yml");
    let text = fs::read_to_string(&meta).unwrap();
    fs::write(&meta, commented(&text, "profile", "the assistant's memory")).unwrap();
    assert!(docsys::hook::is_knowledge_base(&base));
    let done = docsys::assistant::run(&base, &[], &[]);
    assert!(done.is_ok(), "{:?}", done.err());
}

#[test]
fn a_commented_namespace_is_kept_as_its_value() {
    let repo = adopted("namespace", |t| {
        let t = t
            .lines()
            .filter(|l| !l.starts_with("namespace:"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{t}\nnamespace: billing   # what consumers write\n")
    });
    let root = repo.join("docs");
    let again = docsys::adopt::run(&repo, &root, "en").unwrap();
    assert!(
        again
            .summary
            .iter()
            .any(|l| l == "namespace: billing (kept)"),
        "{:?}",
        again.summary
    );
}

#[test]
fn an_empty_domains_list_with_a_comment_is_empty() {
    let base = tmp("domains").join("base");
    docsys::migrate::init_profile(&base, "en", "knowledge-base").unwrap();
    let meta = base.join(".docmeta.yml");
    let text = fs::read_to_string(&meta).unwrap();
    let text = text
        .lines()
        .map(|l| {
            if l.starts_with("domains:") {
                "domains: []   # chosen later".to_string()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(&meta, text).unwrap();
    docsys::assistant::run(&base, &[], &["ops".into()]).unwrap();
    let tree = docsys::tree::DocTree::load(&base).unwrap();
    assert_eq!(tree.docmeta_list("domains"), ["ops"]);
}

/// A list that never closes with `]` is malformed input, and a writer that
/// replaces a field by the lines the reader joined to it would lose the
/// fields the open list swallowed: each writer refuses, naming the field and
/// its line, and writes nothing.
#[test]
fn an_unclosed_list_is_refused_and_nothing_is_written() {
    // `consume add`
    let repo = adopted("unclosed-consume", |t| {
        format!("{t}consume: [billing,\nmanifest_url: https://example.invalid/m\n")
    });
    let meta = repo.join("docs/.docmeta.yml");
    let before = fs::read(&meta).unwrap();
    let provider = tmp("unclosed-provider");
    docsys::migrate::init_profile(&provider.join("docs"), "en", "project").unwrap();
    let done = docsys::consume::add(
        &repo.join("docs"),
        &provider.to_string_lossy(),
        Some("auth"),
    );
    let err = done.expect_err("an unclosed list is refused");
    assert!(err.contains("`consume`") && err.contains("line"), "{err}");
    assert_eq!(fs::read(&meta).unwrap(), before);
    // the assistant's domains
    let base = tmp("unclosed-domains").join("base");
    docsys::migrate::init_profile(&base, "en", "knowledge-base").unwrap();
    let bmeta = base.join(".docmeta.yml");
    let text = fs::read_to_string(&bmeta).unwrap();
    let text = text.replace("domains: []", "domains: [ops,");
    fs::write(&bmeta, &text).unwrap();
    let done = docsys::assistant::run(&base, &[], &["ops".into()]);
    let err = done.expect_err("an unclosed list is refused");
    assert!(err.contains("`domains`") && err.contains("line"), "{err}");
    assert_eq!(fs::read_to_string(&bmeta).unwrap(), text);
}
