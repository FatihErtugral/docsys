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
