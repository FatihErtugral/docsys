#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::ptr_arg
)]
// Tests report through panics by design; the production lints stay strict.

use docsys::graduate;
use std::fs;
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-grad-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

const SOURCE: &str = "---\nstatus: active\nupdated: 2026-08-10\n---\nIntro.\n\n## Contract surface\nThe key is the SHA of cart-id + day.\n\n## Custom section\nFree-form notes.\n";

fn setup(root: &PathBuf) {
    fs::create_dir_all(root.join("reference")).unwrap();
    fs::create_dir_all(root.join("work/features")).unwrap();
    fs::write(
        root.join("reference/keys.md"),
        "---\nid: keys\ntype: reference\nupdated: 2026-08-01\n---\nThis page holds the key contract; read it first.\n",
    )
    .unwrap();
    fs::write(root.join("work/features/x.md"), SOURCE).unwrap();
}

#[test]
fn blocks_keep_template_headings_and_checksum() {
    let bs = graduate::blocks(SOURCE);
    assert_eq!(bs.len(), 3, "intro + two sections");
    assert!(bs[1].keep_heading, "template heading stays");
    assert!(!bs[2].keep_heading, "custom heading moves whole");
}

#[test]
fn apply_moves_bytes_links_source_and_records_graduation() {
    let root = tmp("apply");
    setup(&root);
    let plan = graduate::plan(&root, "work/features/x.md").unwrap();
    let filled = plan.replace("1\tkeep", "1\tmove:reference/keys");
    let done = graduate::apply(&root, &filled, true).unwrap();
    assert_eq!(done.moved, 1);

    let src = fs::read_to_string(root.join("work/features/x.md")).unwrap();
    assert!(src.contains("graduated_to: [keys]"), "{src}");
    assert!(
        src.contains("## Contract surface\nMoved to [[reference/keys|keys]]."),
        "{src}"
    );
    assert!(!src.contains("SHA of cart-id"), "body left the source");

    let dest = fs::read_to_string(root.join("reference/keys.md")).unwrap();
    assert!(
        dest.contains("The key is the SHA of cart-id + day."),
        "byte-exact arrival"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn apply_refuses_drift_and_missing_destination() {
    let root = tmp("drift");
    setup(&root);
    let plan = graduate::plan(&root, "work/features/x.md").unwrap();
    // Source changes after planning → checksum mismatch → refuse (D-024).
    fs::write(
        root.join("work/features/x.md"),
        SOURCE.replace("cart-id", "session-id"),
    )
    .unwrap();
    let filled = plan.replace("1\tkeep", "1\tmove:reference/keys");
    let err = graduate::apply(&root, &filled, true).err().unwrap();
    assert!(err.contains("re-plan"), "{err}");

    // Missing destination → refuse with the R-099 pointer.
    fs::write(root.join("work/features/x.md"), SOURCE).unwrap();
    let plan2 = graduate::plan(&root, "work/features/x.md").unwrap();
    let bad = plan2.replace("1\tkeep", "1\tmove:reference/nowhere");
    let err2 = graduate::apply(&root, &bad, true).err().unwrap();
    assert!(err2.contains("R-099"), "{err2}");
    let _ = fs::remove_dir_all(&root);
}

// ── R-082 at the gate (D-089): a graduated file is frozen ───────────────────

fn git_in(dir: &std::path::Path, args: &[&str]) {
    assert!(
        std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap()
            .success(),
        "git {args:?}"
    );
}

fn errors_in(root: &std::path::Path, repo: &std::path::Path) -> Vec<String> {
    let (r, _) = docsys::lint_in(root, Some(repo));
    r.findings
        .iter()
        .filter(|f| f.severity == docsys::model::Severity::Error)
        .map(|f| format!("{} {} [{}]", f.rule.0, f.file, f.subject))
        .collect()
}

#[test]
fn a_graduated_file_is_frozen_at_the_gate() {
    let repo = tmp("r082");
    git_in(&repo, &["init", "-q"]);
    git_in(&repo, &["config", "user.email", "t@example.invalid"]);
    git_in(&repo, &["config", "user.name", "t"]);
    let root = repo.join("docs");
    let today = docsys::migrate::today();
    let w = |rel: &str, text: &str| {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    };
    w(
        ".docmeta.yml",
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n",
    );
    w(
        "index.md",
        "# docs\n\n- [[reference/keys|Keys]] -- the canonical form.\n",
    );
    w(
        "reference/keys.md",
        &format!("---\nid: keys\ntype: reference\nupdated: {today}\n---\n# Keys\n\nThis page states the canonical form; read it before hashing a key.\n\nSorted fields.\n"),
    );
    w("work/journal.md", "# Journal\n");
    w("work/debt.md", "# Debt\n");
    w("work/questions.md", "# Questions\n");
    let page = format!(
        "---\nid: cart-key\nstatus: graduated\nconfirmed: owner, {today}\ngraduated_to: [keys]\nupdated: {today}\n---\n## Context\n\nWhy the key changed.\n\n## Decision\n\nMoved to [[reference/keys|keys]].\n"
    );
    w("work/features/cart-key.md", &page);
    git_in(&repo, &["add", "-A"]);
    git_in(&repo, &["commit", "-q", "-m", "graduated"]);
    assert!(
        errors_in(&root, &repo).is_empty(),
        "{:?}",
        errors_in(&root, &repo)
    );

    // a content change
    fs::write(
        root.join("work/features/cart-key.md"),
        format!("{page}\nA line added after graduation.\n"),
    )
    .unwrap();
    let errs = errors_in(&root, &repo);
    assert!(
        errs.contains(&"R-082 work/features/cart-key.md [content]".to_string()),
        "{errs:?}"
    );
    // a transition out of graduated
    fs::write(
        root.join("work/features/cart-key.md"),
        page.replace("status: graduated", "status: active"),
    )
    .unwrap();
    let errs = errors_in(&root, &repo);
    assert!(
        errs.contains(&"R-082 work/features/cart-key.md [status]".to_string()),
        "{errs:?}"
    );
    // frontmatter only (§2.4): not a content change
    fs::write(
        root.join("work/features/cart-key.md"),
        page.replace(&format!("updated: {today}"), "updated: 2099-01-01"),
    )
    .unwrap();
    let errs = errors_in(&root, &repo);
    assert!(
        !errs.iter().any(|e| e.starts_with("R-082")),
        "an `updated:` bump is not content: {errs:?}"
    );
    // deleted
    fs::remove_file(root.join("work/features/cart-key.md")).unwrap();
    let errs = errors_in(&root, &repo);
    assert!(
        errs.contains(&"R-082 work/features/cart-key.md [deleted]".to_string()),
        "{errs:?}"
    );
    let _ = fs::remove_dir_all(&repo);
}

// ── D-127: on a docsys/0.5 tree graduation ends by removing the work file ───

const FEATURE: &str = "---\nid: cart-key\nstatus: active\n---\n# Cart key\n\n## Context\n\nWhy the key changed: carts collided across days.\n\n## Decision\n\nThe key is the SHA of cart-id + day.\n\n## Contract surface\n\nSorted fields, joined by `|`.\n\n## Rejected alternatives\n";

/// A committed docsys/0.5 tree with a feature file and its two prepared
/// destinations.
fn tree_0_5(name: &str, spec: &str, maintainers: &str) -> (PathBuf, PathBuf) {
    let repo = tmp(name);
    git_in(&repo, &["init", "-q"]);
    git_in(&repo, &["config", "user.email", "t@example.invalid"]);
    git_in(&repo, &["config", "user.name", "t"]);
    git_in(&repo, &["config", "commit.gpgsign", "false"]);
    let root = repo.join("docs");
    let w = |rel: &str, text: &str| {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    };
    w(
        ".docmeta.yml",
        &format!("spec: {spec}\nprofile: project\ndefault_content_language: en\nmaintainers: {maintainers}\n"),
    );
    w(
        "index.md",
        "# docs\n\n- [[reference/|Reference]] -- contracts.\n- [[explanation/|Explanation]] -- decisions.\n",
    );
    w(
        "reference/keys.md",
        "---\nid: keys\ntype: reference\n---\n# Keys\n\nThis page states the cart key contract; read it before hashing a key.\n",
    );
    w(
        "explanation/cart-key-choice.md",
        "---\nid: cart-key-choice\ntype: explanation\n---\n# Why the cart key\n\nThis page explains why the cart key is built as it is.\n",
    );
    w("work/features/cart-key.md", FEATURE);
    git_in(&repo, &["add", "-A"]);
    git_in(&repo, &["commit", "-q", "-m", "a feature and its pages"]);
    (repo, root)
}

/// The block's bytes as the source holds them (R-090).
fn block_bytes(source: &str, heading: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let b = graduate::blocks(source)
        .into_iter()
        .find(|b| b.snippet == heading)
        .unwrap();
    lines[b.body_start..b.end].join("\n")
}

fn filled(root: &std::path::Path, contract: &str) -> String {
    graduate::plan(root, "work/features/cart-key.md")
        .unwrap()
        .replace("2\tkeep", "2\tmove:explanation/cart-key-choice")
        .replace("3\tkeep", &format!("3\t{contract}"))
}

#[test]
fn on_a_0_5_tree_graduation_ends_by_removing_the_work_file() {
    let (repo, root) = tree_0_5("removes", "docsys/0.5", "[]");
    let plan = filled(&root, "move:reference/keys");
    let done = graduate::apply_confirmed(&root, &plan, false, "owner").unwrap();
    assert_eq!(done.moved, 2);
    assert_eq!(done.removed.as_deref(), Some("work/features/cart-key.md"));
    let left: Vec<_> = fs::read_dir(root.join("work/features"))
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "no feature file left in work/: {left:?}");
    // the blocks arrive byte for byte
    let choice = fs::read_to_string(root.join("explanation/cart-key-choice.md")).unwrap();
    assert!(
        choice.contains(&block_bytes(FEATURE, "## Decision")),
        "{choice}"
    );
    let keys = fs::read_to_string(root.join("reference/keys.md")).unwrap();
    assert!(
        keys.contains(&block_bytes(FEATURE, "## Contract surface")),
        "{keys}"
    );
    // the commit names the destinations and carries the person's word
    let message = done.message.unwrap();
    let subject = message.lines().next().unwrap();
    assert_eq!(
        subject, "docs: graduate cart-key into cart-key-choice, keys",
        "{message}"
    );
    assert!(message.ends_with("\nConfirmed-by: owner\n"), "{message}");
    git_in(&repo, &["add", "-A"]);
    git_in(&repo, &["commit", "-q", "-m", &message]);
    assert!(
        errors_in(&root, &repo).is_empty(),
        "{:?}",
        errors_in(&root, &repo)
    );
    // history keeps the file
    git_in(
        &repo,
        &["cat-file", "-e", "HEAD~1:docs/work/features/cart-key.md"],
    );
    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn the_removal_waits_for_every_block_of_value_and_for_the_persons_word() {
    // a block outside the retained sections that stays: refused, by name,
    // and nothing is written
    let (repo, root) = tree_0_5(
        "refuses",
        "docsys/0.5",
        "[\"owner <owner@example.invalid>\"]",
    );
    let plan = filled(&root, "keep");
    let err = graduate::apply_confirmed(&root, &plan, false, "owner")
        .err()
        .unwrap();
    assert!(
        err.contains("block 3") && err.contains("## Contract surface"),
        "{err}"
    );
    assert_eq!(
        fs::read_to_string(root.join("work/features/cart-key.md")).unwrap(),
        FEATURE
    );
    assert!(
        !fs::read_to_string(root.join("explanation/cart-key-choice.md"))
            .unwrap()
            .contains("SHA of cart-id")
    );
    // a word from someone who is no maintainer: refused (R-208)
    let plan = filled(&root, "move:reference/keys");
    let err = graduate::apply_confirmed(&root, &plan, false, "visitor")
        .err()
        .unwrap();
    assert!(err.contains("R-208"), "{err}");
    // a plan that sends nothing anywhere is no graduation
    let nothing = graduate::plan(&root, "work/features/cart-key.md").unwrap();
    let err = graduate::apply_confirmed(&root, &nothing, false, "owner")
        .err()
        .unwrap();
    assert!(err.contains("block 2"), "{err}");
    let only_context = FEATURE
        .replace("\nThe key is the SHA of cart-id + day.\n", "")
        .replace("\nSorted fields, joined by `|`.\n", "");
    fs::write(root.join("work/features/cart-key.md"), &only_context).unwrap();
    let nothing = graduate::plan(&root, "work/features/cart-key.md").unwrap();
    let err = graduate::apply_confirmed(&root, &nothing, true, "owner")
        .err()
        .unwrap();
    assert!(err.contains("needs a destination"), "{err}");
    // a file already graduated keeps its place (R-082)
    fs::write(
        root.join("work/features/cart-key.md"),
        FEATURE.replace("status: active", "status: graduated"),
    )
    .unwrap();
    let err = graduate::apply_confirmed(&root, &plan, true, "owner")
        .err()
        .unwrap();
    assert!(err.contains("keeps its place"), "{err}");
    fs::write(root.join("work/features/cart-key.md"), FEATURE).unwrap();
    // without the word the file stays, as before (R-091)
    let done = graduate::apply(&root, &plan, false).unwrap();
    assert_eq!(done.removed, None);
    assert!(root.join("work/features/cart-key.md").exists());
    let _ = fs::remove_dir_all(&repo);

    // a docsys/0.4 tree keeps its files
    let (repo, root) = tree_0_5("era", "docsys/0.4", "[]");
    let plan = filled(&root, "move:reference/keys");
    let err = graduate::apply_confirmed(&root, &plan, false, "owner")
        .err()
        .unwrap();
    assert!(err.contains("docsys/0.5"), "{err}");
    assert!(root.join("work/features/cart-key.md").exists());
    let _ = fs::remove_dir_all(&repo);
}
