#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]
// A base that learns from the trees it consumes: a page whose sources are
// consumed pages (`@ns/id`), a connector's records landing once with their
// provenance, the git connector, and the digest `status` derives.

use docsys::{consume, export, inbox, lint_in, model::Severity, status};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-jarvis-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn git(dir: &Path, args: &[&str]) {
    assert!(Command::new("git")
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap()
        .success());
}

/// A provider project with one reference page, committed.
fn provider(hub: &Path, name: &str, id: &str, title: &str, body: &str) -> PathBuf {
    let repo = hub.join(name);
    fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "user.name", "t"]);
    let docs = repo.join("docs");
    write(
        &docs,
        ".docmeta.yml",
        &format!(
            "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\nnamespace: {name}\n"
        ),
    );
    write(
        &docs,
        "index.md",
        &format!("# docs\n\n- [[reference/{id}|{title}]] -- {title}.\n"),
    );
    write(
        &docs,
        &format!("reference/{id}.md"),
        &format!(
            "---\nid: {id}\ntype: reference\nupdated: 2026-09-02\n---\n# {title}\n\n\
             This page states {title}; read it before changing it.\n\n{body}\n"
        ),
    );
    write(&repo, "src/lib.rs", "pub fn f() {}\n");
    git(&repo, &["add", "-A"]);
    git(
        &repo,
        &[
            "commit",
            "-q",
            "-m",
            &format!("{name}: {title}\n\nThe reason it is so."),
        ],
    );
    repo
}

/// The base: a knowledge base that is its own repository.
fn base(hub: &Path) -> PathBuf {
    let b = hub.join("jarvis");
    fs::create_dir_all(&b).unwrap();
    git(&b, &["init", "-q"]);
    git(&b, &["config", "user.email", "t@example.invalid"]);
    git(&b, &["config", "user.name", "t"]);
    docsys::migrate::init_profile(&b, "en", "knowledge-base").unwrap();
    let dm = b.join(".docmeta.yml");
    let text = fs::read_to_string(&dm)
        .unwrap()
        .replace("domains: []", "domains: [coding]");
    fs::write(&dm, text).unwrap();
    b
}

fn errors(root: &Path) -> Vec<String> {
    let (r, _) = lint_in(root, Some(root));
    r.findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| format!("{} {} {}", f.rule, f.file, f.subject))
        .collect()
}

#[test]
fn a_page_learned_from_consumed_trees_cites_them_by_identifier() {
    let today = docsys::migrate::today();
    let hub = tmp("learn");
    provider(
        &hub,
        "relay",
        "retry-policy",
        "Retry policy",
        "Four attempts, then the dead-letter store.",
    );
    provider(
        &hub,
        "ledger",
        "transfer-semantics",
        "Transfer semantics",
        "A transfer is atomic.",
    );
    let b = base(&hub);
    write(
        &b,
        "wiki/coding/explanation/failure-handling.md",
        &format!("---\nid: failure-handling\ntype: explanation\ndomain: coding\nverification: unverified\nupdated: {today}\nsources: [@relay/retry-policy, @ledger/transfer-semantics]\n---\n# Failure handling across the projects\n\nThis page explains how the projects treat a failing operation; read it before designing a new one.\n\nrelay retries (doc: @relay/retry-policy), ledger refuses (doc: @ledger/transfer-semantics).\n"),
    );
    write(
        &b,
        "wiki/coding/index.md",
        "# coding\n\n- [[coding/explanation/failure-handling|Failure handling]] -- across the projects.\n",
    );
    write(
        &b,
        "wiki/index.md",
        "# Knowledge base\n\n- [[coding/index|Coding]] -- code.\n",
    );
    // before the providers are consumed: the citations are unresolved errors
    let errs = errors(&b);
    assert!(
        errs.contains(
            &"R-059 wiki/coding/explanation/failure-handling.md @relay/retry-policy".to_string()
        ),
        "{errs:?}"
    );
    consume::add(&b, hub.join("relay").to_str().unwrap(), None).unwrap();
    consume::add(&b, hub.join("ledger").to_str().unwrap(), None).unwrap();
    export::fetch(&b).unwrap();
    git(&b, &["add", "-A"]);
    git(&b, &["commit", "-q", "-m", "base"]);
    assert!(errors(&b).is_empty(), "{:?}", errors(&b));
    // an inline `doc: @ns/id` resolves against the materialization too: no
    // warning left once the namespace is fetched (R-139)
    let (r, _) = lint_in(&b, Some(&b));
    assert!(
        r.findings.iter().all(|f| f.rule.0 != "R-076"),
        "{:?}",
        r.findings
    );
    // the materializations are the providers' contract: refs never reads
    // them as this repository's stray pages or dangling citations
    let tree = docsys::tree::DocTree::load(&b).unwrap();
    let refs = docsys::refs::run(&b, &tree);
    assert!(refs.findings.is_empty(), "{:?}", refs.findings);
    // the digest sees the consumed namespaces; a docsys/0.5 base lists no
    // page as unverified (D-130)
    let s = status::status(&b, Some(&b)).unwrap();
    assert_eq!(s.profile, "knowledge-base");
    assert_eq!(s.permanent, 1);
    assert!(s.unverified.is_empty(), "{:?}", s.unverified);
    let names: Vec<&str> = s.consumed.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, vec!["ledger", "relay"]);
    assert!(
        s.consumed
            .iter()
            .all(|n| n.pages == 1 && n.fetched.is_some()),
        "{:?}",
        s.consumed
    );
    let text = status::render(&s, &b);
    assert!(
        text.contains("consumed: ledger 1 page(s) fetched"),
        "{text}"
    );
    assert!(text.contains("wiki: 1 page(s)\n"), "{text}");
    assert!(status::render_json(&s).contains("\"consumed\":[{\"name\":\"ledger\""));
}

#[test]
fn the_git_connector_lands_each_commit_once_and_the_inbox_shows_in_status() {
    let hub = tmp("pull");
    let relay = provider(
        &hub,
        "relay",
        "retry-policy",
        "Retry policy",
        "Four attempts.",
    );
    let b = base(&hub);
    git(&b, &["add", "-A"]);
    git(&b, &["commit", "-q", "-m", "base"]);
    let landed = inbox::pull_git(&b, &relay, "30.days", None, None, false).unwrap();
    assert_eq!(landed.len(), 1, "{landed:?}");
    assert!(landed[0].starts_with("captured: raw/inbox/"), "{landed:?}");
    let rel = landed[0].trim_start_matches("captured: ");
    let record = fs::read_to_string(b.join(rel)).unwrap();
    assert!(
        record.starts_with("---\nsource: git\nsource_id: relay@"),
        "{record}"
    );
    assert!(
        record.contains("title: \"relay: relay: Retry policy\""),
        "{record}"
    );
    assert!(record.contains("The reason it is so."), "{record}");
    assert!(
        record.contains("- docs/reference/retry-policy.md"),
        "{record}"
    );
    // a second pull: nothing new, the same record named
    let again = inbox::pull_git(&b, &relay, "30.days", None, None, false).unwrap();
    assert_eq!(again.len(), 1);
    assert!(
        again[0].starts_with("already captured: raw/inbox/"),
        "{again:?}"
    );
    assert_eq!(
        fs::read_dir(b.join("raw/inbox"))
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "md"))
            .count(),
        1
    );
    // a record is a record: lint accepts a raw file with provenance, and the
    // digest counts it
    assert!(errors(&b).is_empty(), "{:?}", errors(&b));
    let s = status::status(&b, Some(&b)).unwrap();
    assert_eq!(s.inbox, 1);
    assert!(s.inbox_oldest.is_some());
    assert!(status::render(&s, &b).contains("inbox: 1 note(s), oldest"));
}

#[test]
fn an_assistants_memory_stands_up_in_one_command_and_again() {
    let hub = tmp("assistant");
    provider(
        &hub,
        "relay",
        "retry-policy",
        "Retry policy",
        "Four attempts.",
    );
    provider(
        &hub,
        "ledger",
        "transfer-semantics",
        "Transfer semantics",
        "Atomic.",
    );
    // another base under the same directory is not a source of contracts
    let other = hub.join("notes");
    docsys::migrate::init_profile(&other, "en", "knowledge-base").unwrap();
    let b = hub.join("jarvis");
    let done = docsys::assistant::run(
        &b,
        std::slice::from_ref(&hub),
        &["coding".into(), "ops".into()],
    )
    .unwrap();
    assert!(
        done.steps.iter().any(|s| s.starts_with("base: created")),
        "{:?}",
        done.steps
    );
    assert!(
        done.steps
            .iter()
            .any(|s| s.contains("domains: [coding, ops]")),
        "{:?}",
        done.steps
    );
    assert!(
        done.steps.iter().any(|s| s.contains("skipped notes")),
        "{:?}",
        done.steps
    );
    let mut consumed = done.consumed.clone();
    consumed.sort();
    assert_eq!(consumed, vec!["ledger", "relay"]);
    // a project's commits are events: none lands without the person's word (D-133)
    assert!(
        !done.steps.iter().any(|s| s.starts_with("records:")),
        "{:?}",
        done.steps
    );
    assert!(
        b.join(".git").exists(),
        "a repository, for the gate and the record layer"
    );
    assert!(b.join(".claude/hooks/pre-commit-docs.sh").is_file());
    assert!(b.join(".federation/relay/retry-policy.md").is_file());
    let s = status::status(&b, Some(&b)).unwrap();
    assert_eq!(s.inbox, 0);
    assert_eq!(s.consumed.len(), 2);
    assert!(errors(&b).is_empty(), "{:?}", errors(&b));
    // again: nothing duplicated, nothing rewritten
    let again = docsys::assistant::run(&b, std::slice::from_ref(&hub), &[]).unwrap();
    assert!(
        again
            .steps
            .iter()
            .any(|s| s == "base: kept (already a knowledge base)"),
        "{:?}",
        again.steps
    );
    assert!(
        again.steps.iter().any(|s| s == "domains: kept"),
        "{:?}",
        again.steps
    );
    let s = status::status(&b, Some(&b)).unwrap();
    assert_eq!(s.inbox, 0);
    // a project tree is refused as the memory
    let err = docsys::assistant::run(&hub.join("relay").join("docs"), &[], &[]).unwrap_err();
    assert!(err.contains("knowledge base"), "{err}");
}

#[test]
fn the_git_connector_skips_bookkeeping_unless_asked() {
    let hub = tmp("noise");
    let relay = provider(
        &hub,
        "relay",
        "retry-policy",
        "Retry policy",
        "Four attempts.",
    );
    // a docs-only commit with no body: bookkeeping
    let page = relay.join("docs/reference/retry-policy.md");
    fs::write(
        &page,
        fs::read_to_string(&page)
            .unwrap()
            .replace("2026-09-02", "2026-09-03"),
    )
    .unwrap();
    git(&relay, &["add", "-A"]);
    git(&relay, &["commit", "-q", "-m", "docs: bump updated"]);
    let b = base(&hub);
    git(&b, &["add", "-A"]);
    git(&b, &["commit", "-q", "-m", "base"]);
    let landed = inbox::pull_git(&b, &relay, "30.days", None, None, false).unwrap();
    assert_eq!(landed.len(), 1, "{landed:?}");
    assert!(landed[0].contains("relay-relay-retry-policy"), "{landed:?}");
    let all = inbox::pull_git(&b, &relay, "30.days", None, None, true).unwrap();
    assert_eq!(all.len(), 2, "{all:?}");
    assert!(
        all.iter()
            .any(|l| l.starts_with("captured:") && l.contains("bump-updated")),
        "{all:?}"
    );
}

#[test]
fn a_base_inside_another_repository_is_told_whose_history_it_shares() {
    let hub = tmp("nested");
    let relay = provider(
        &hub,
        "relay",
        "retry-policy",
        "Retry policy",
        "Four attempts.",
    );
    // a base under the provider's own repository: no `git init`, but a clear word
    let nested = relay.join("memory");
    let done = docsys::assistant::run(&nested, &[], &["coding".to_string()]).unwrap();
    assert!(
        done.steps
            .iter()
            .any(|s| s.starts_with("git: inside repository ")),
        "{:?}",
        done.steps
    );
    assert!(
        !done
            .steps
            .iter()
            .any(|s| s.contains("repository initialized")),
        "{:?}",
        done.steps
    );
    assert!(!nested.join(".git").exists());
    let _ = fs::remove_dir_all(&hub);
}

#[test]
fn the_limit_counts_commits_worth_reading_and_a_namespace_override_names_the_records() {
    let hub = tmp("limit");
    let relay = provider(
        &hub,
        "relay",
        "retry-policy",
        "Retry policy",
        "Four attempts.",
    );
    // a second commit worth reading (a body), then three bookkeeping commits:
    // docs only, no body — the connector skips them unless asked
    write(&relay, "src/lib.rs", "pub fn f() {}\npub fn g() {}\n");
    git(&relay, &["add", "-A"]);
    git(
        &relay,
        &[
            "commit",
            "-q",
            "-m",
            "relay: g\n\nBecause f alone was not enough; measured.",
        ],
    );
    let today = docsys::migrate::today();
    for i in 1..=3 {
        let p = relay.join("docs/reference/retry-policy.md");
        let t = fs::read_to_string(&p).unwrap();
        fs::write(
            &p,
            format!(
                "{}\nEdited {i}.\n",
                t.replace("updated: 2026-09-02", &format!("updated: {today}"))
            ),
        )
        .unwrap();
        git(&relay, &["add", "-A"]);
        git(&relay, &["commit", "-q", "-m", &format!("docs: touch {i}")]);
    }
    let b = base(&hub);
    git(&b, &["add", "-A"]);
    git(&b, &["commit", "-q", "-m", "base"]);

    // --limit 1 counts commits worth reading: the newest bookkeeping ones are passed over
    let landed = inbox::pull_git(&b, &relay, "2000-01-01", None, Some(1), false).unwrap();
    let captured: Vec<&String> = landed
        .iter()
        .filter(|l| l.starts_with("captured: "))
        .collect();
    assert_eq!(captured.len(), 1, "{landed:?}");
    assert!(!captured[0].contains("touch"), "{landed:?}");
    // the same pull lands nothing twice
    let again = inbox::pull_git(&b, &relay, "2000-01-01", None, Some(1), false).unwrap();
    assert!(
        again.iter().all(|l| l.starts_with("already captured: ")),
        "{again:?}"
    );
    // --all lets the bookkeeping commits through
    let all = inbox::pull_git(&b, &relay, "2000-01-01", None, Some(2), true).unwrap();
    assert!(
        all.iter()
            .any(|l| l.starts_with("captured: ") && l.contains("touch")),
        "{all:?}"
    );
    // --since after every commit: nothing worth reading in the span
    let none = inbox::pull_git(&b, &relay, "2030-01-01", None, None, false).unwrap();
    assert!(
        !none.iter().any(|l| l.starts_with("captured: ")),
        "{none:?}"
    );
    // --as names the records under another namespace
    let named = inbox::pull_git(&b, &relay, "2000-01-01", Some("rl"), None, false).unwrap();
    let rec = named
        .iter()
        .find_map(|l| l.strip_prefix("captured: "))
        .expect("a record under rl");
    let text = fs::read_to_string(b.join(rec)).unwrap();
    assert!(text.contains("source_id: rl@"), "{text}");
    assert!(text.contains("title: \"rl: "), "{text}");
    let _ = fs::remove_dir_all(&hub);
}

/// A page whose bytes the base already holds keeps them and its fetch date,
/// with or without a manifest; a changed page is written, dated (D-133).
#[test]
fn an_unchanged_page_keeps_its_bytes_and_its_fetch_date() {
    let hub = tmp("unchanged");
    let relay = provider(
        &hub,
        "relay",
        "retry-policy",
        "Retry policy",
        "Four attempts.",
    );
    let b = base(&hub);
    consume::add(&b, relay.to_str().unwrap(), None).unwrap();
    export::fetch(&b).unwrap();
    let side = b.join(".federation/relay/retry-policy.provenance.yml");
    let page = b.join(".federation/relay/retry-policy.md");
    let earlier: String = fs::read_to_string(&side)
        .unwrap()
        .lines()
        .map(|l| {
            if l.starts_with("fetched:") {
                "fetched: 2000-01-01\n".to_string()
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    fs::write(&side, &earlier).unwrap();
    let held = fs::read_to_string(&page).unwrap();
    let said = export::fetch(&b).unwrap();
    assert_eq!(fs::read_to_string(&side).unwrap(), earlier, "{said:?}");
    assert_eq!(fs::read_to_string(&page).unwrap(), held);
    assert!(
        said.iter().any(|l| l.contains("1 unchanged, skipped")),
        "{said:?}"
    );
    write(
        &relay,
        "docs/reference/retry-policy.md",
        "---\nid: retry-policy\ntype: reference\nupdated: 2026-09-02\n---\n# Retry policy\n\n\
         This page states Retry policy; read it before changing it.\n\nFive attempts.\n",
    );
    git(&relay, &["commit", "-qam", "five attempts"]);
    export::fetch(&b).unwrap();
    assert!(fs::read_to_string(&page)
        .unwrap()
        .contains("Five attempts."));
    assert!(!fs::read_to_string(&side)
        .unwrap()
        .contains("fetched: 2000-01-01"));
}

/// A project the base learns from is read at its default branch — never the
/// branch checked out in the person's checkout, nor its edits in progress —
/// by fetch and by the git connector alike; the checkout they read through
/// stays out of the base's history (D-133).
#[test]
fn a_local_provider_is_read_at_its_default_branch() {
    let hub = tmp("default-branch");
    let upstream = provider(
        &hub,
        "relay",
        "retry-policy",
        "Retry policy",
        "Four attempts.",
    );
    let work = hub.join("work");
    assert!(Command::new("git")
        .args(["clone", "-q"])
        .arg(&upstream)
        .arg(&work)
        .status()
        .unwrap()
        .success());
    git(&work, &["config", "user.email", "t@example.invalid"]);
    git(&work, &["config", "user.name", "t"]);
    git(&work, &["checkout", "-q", "-b", "feature"]);
    let page = |n: &str| {
        format!(
            "---\nid: retry-policy\ntype: reference\nupdated: 2026-09-02\n---\n# Retry policy\n\n\
             This page states Retry policy; read it before changing it.\n\n{n} attempts.\n"
        )
    };
    write(&work, "docs/reference/retry-policy.md", &page("Six"));
    git(&work, &["commit", "-qam", "six on the branch\n\nWhy."]);
    write(&work, "docs/reference/retry-policy.md", &page("Seven"));
    let b = base(&hub);
    consume::add(&b, work.to_str().unwrap(), Some("relay")).unwrap();
    export::fetch(&b).unwrap();
    let held = fs::read_to_string(b.join(".federation/relay/retry-policy.md")).unwrap();
    assert!(held.contains("Four attempts."), "{held}");
    assert!(
        fs::read_to_string(work.join("docs/reference/retry-policy.md"))
            .unwrap()
            .contains("Seven attempts."),
        "the person's checkout is as they left it"
    );
    let landed = inbox::pull_git(&b, &work, "2000-01-01", Some("relay"), None, true).unwrap();
    assert_eq!(landed.len(), 1, "{landed:?}");
    git(&b, &["add", "-A"]);
    let staged = Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(&b)
        .output()
        .unwrap();
    let staged = String::from_utf8_lossy(&staged.stdout);
    assert!(!staged.contains(".checkouts"), "{staged}");
    assert!(
        staged.contains(".federation/relay/retry-policy.md"),
        "{staged}"
    );
}
