#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::ptr_arg)]
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
    assert!(src.contains("## Contract surface\nMoved to [[reference/keys|keys]]."), "{src}");
    assert!(!src.contains("SHA of cart-id"), "body left the source");

    let dest = fs::read_to_string(root.join("reference/keys.md")).unwrap();
    assert!(dest.contains("The key is the SHA of cart-id + day."), "byte-exact arrival");
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
