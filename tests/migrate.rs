#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// Tests report through panics by design; the production lints stay strict.

//! Migration behavior locks: in-tree links follow their targets, out-of-tree
//! links are depth-corrected, frontmatter and skeleton are generated.

use docsys::migrate;
use std::fs;
use std::path::PathBuf;

fn tmp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

#[test]
fn apply_moves_rewrites_and_scaffolds() {
    let root = tmp_root("apply");
    fs::write(root.join("a.md"), "# Alpha\n\nAlpha intro.\n\nSee [b](b.md) and [out](../outside.md).\n")
        .unwrap_or_else(|_| panic!("write a"));
    fs::write(root.join("b.md"), "# Beta\n\nBeta intro.\n").unwrap_or_else(|_| panic!("write b"));

    let plan = "a.md\treference\nb.md\thowto\n";
    let done = migrate::apply(&root, plan, "en", None).unwrap_or_else(|e| panic!("apply: {e}"));
    assert_eq!(done.moved, 2);

    let a = fs::read_to_string(root.join("reference/a.md")).unwrap_or_default();
    assert!(a.starts_with("---\nid: a\ntype: reference\nupdated: "), "frontmatter: {a}");
    assert!(a.contains("[b](../howto/b.md)"), "in-tree link must follow the move: {a}");
    assert!(a.contains("[out](../../outside.md)"), "out-of-tree link must be depth-corrected: {a}");

    assert!(root.join(".docmeta.yml").exists());
    assert!(root.join("work/journal.md").exists());
    let router = fs::read_to_string(root.join("index.md")).unwrap_or_default();
    assert!(router.contains("- [[reference/a|Alpha]] -- Alpha intro."), "router: {router}");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn apply_refuses_todo_rows() {
    let root = tmp_root("todo");
    fs::write(root.join("x.md"), "# X\n").unwrap_or_else(|_| panic!("write x"));
    let err = migrate::apply(&root, "x.md\tTODO\n", "en", None).err().unwrap_or_default();
    assert!(err.contains("TODO"), "{err}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn apply_rewrites_inbound_repo_references() {
    let repo = tmp_root("repo");
    let docs = repo.join("docs");
    let _ = fs::create_dir_all(&docs);
    fs::write(docs.join("guide.md"), "# Guide\n\nGuide intro.\n").unwrap_or_default();
    fs::write(
        repo.join("README.md"),
        "See [guide](docs/guide.md) and https://example.com/x/docs/guide.md and arm.com/documentation/ddi0403.\n",
    )
    .unwrap_or_default();

    let done = migrate::apply(&docs, "guide.md\thowto\n", "en", Some(&repo))
        .unwrap_or_else(|e| std::panic::panic_any(e));
    assert_eq!(done.repo_rewrites.len(), 1, "{:?}", done.repo_rewrites);

    let readme = fs::read_to_string(repo.join("README.md")).unwrap_or_default();
    assert!(readme.contains("docs/howto/guide.md"), "{readme}");
    // The URL path segment also carried the moved file and was updated with it.
    assert!(readme.contains("example.com/x/docs/howto/guide.md"), "{readme}");
    // External URL that merely contains the word is neither rewritten nor a risk.
    assert!(readme.contains("arm.com/documentation/ddi0403"), "{readme}");
    assert!(done.repo_risks.is_empty(), "{:?}", done.repo_risks);

    let _ = fs::remove_dir_all(&repo);
}
