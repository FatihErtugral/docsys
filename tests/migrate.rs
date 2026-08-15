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
    let done = migrate::apply(&root, plan, "en").unwrap_or_else(|e| panic!("apply: {e}"));
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
    let err = migrate::apply(&root, "x.md\tTODO\n", "en").err().unwrap_or_default();
    assert!(err.contains("TODO"), "{err}");
    let _ = fs::remove_dir_all(&root);
}
