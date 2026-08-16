#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// `export product` composes; it never chooses and never half-composes (D-032).

use std::fs;
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-export-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

/// A project tree with two permanent pages and one flowing work file.
fn build_tree(name: &str) -> (PathBuf, PathBuf) {
    let repo = tmp(name);
    let root = repo.join("docs");
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
        "# docs\n\n- [[reference/token-ttl|Token lifetime]] -- How long a token lives.\n\
         - [[howto/install|Install]] -- Getting the tool onto a machine.\n",
    );
    w(
        "reference/token-ttl.md",
        "---\nid: token-ttl\ntype: reference\nupdated: 2026-08-16\n---\n\n\
         # Token lifetime\n\nAccess tokens live one hour. Refresh extends them.\n\n\
         See [[howto/install|Install]] for setup.\n\n\
         ## Refresh flow\n\nThe refresh endpoint rotates the pair.\n\n\
         ```\n# a comment inside a fence must not shift\n```\n",
    );
    w(
        "howto/install.md",
        "---\nid: install\ntype: howto\nupdated: 2026-08-16\ninternal: true\n---\n\n\
         # Install\n\nDownload the binary and put it on PATH.\n",
    );
    w(
        "work/features/pending.md",
        "---\nid: pending\nstatus: active\nupdated: 2026-08-16\n---\n\n# Pending work\n",
    );
    (repo, root)
}

#[test]
fn product_composes_verbatim_with_shifted_headings_and_stamps() {
    let (repo, root) = build_tree("compose");
    let map = repo.join("widget.md");
    fs::write(
        &map,
        "# Widget\n\nWidget is a sample product.\n\n\
         ## Features\n- [[token-ttl|Token lifetime]] -- How long a token lives.\n\n\
         ## Setup\n- [[install|Install]] -- Getting started.\n",
    )
    .unwrap();
    let done = docsys::export::product(&root, &map, None, None).unwrap();
    let out = &done.output;
    assert!(out.contains("# Widget"), "{out}");
    assert!(out.contains("Widget is a sample product."), "{out}");
    assert!(out.contains("\n## Features\n"), "{out}");
    assert!(out.contains("\n### Token lifetime\n"), "{out}");
    // headings shift two levels; fenced lines do not
    assert!(out.contains("\n#### Refresh flow\n"), "{out}");
    assert!(
        out.contains("\n# a comment inside a fence must not shift\n"),
        "{out}"
    );
    // prose is carried verbatim
    assert!(out.contains("Access tokens live one hour."), "{out}");
    // source stamps carry the identifier and the file
    assert!(out.contains("doc: token-ttl"), "{out}");
    assert!(out.contains("reference/token-ttl.md"), "{out}");
    // the internal page composes, with the R-019 warning
    assert_eq!(done.pages, 2);
    assert!(
        done.warnings.iter().any(|w| w.contains("install")),
        "{:?}",
        done.warnings
    );
}

#[test]
fn product_refuses_to_half_compose() {
    let (repo, root) = build_tree("refuse");
    let map = repo.join("widget.md");
    fs::write(
        &map,
        "# Widget\n\n## Features\n\
         - [[ghost|Ghost]] -- Resolves nowhere.\n\
         - [[pending|Pending]] -- Still flowing.\n\
         - [[@other/page|Foreign]] -- Another namespace.\n",
    )
    .unwrap();
    let err = docsys::export::product(&root, &map, None, None).unwrap_err();
    assert!(err.contains("`ghost`"), "{err}");
    assert!(err.contains("flowing"), "{err}");
    assert!(err.contains("not materialized"), "{err}");
}

#[test]
fn declared_languages_are_read_never_determined() {
    let (repo, root) = build_tree("lang");
    // token-ttl declares its own language; install inherits the tree default.
    let page = root.join("reference/token-ttl.md");
    let text = fs::read_to_string(&page).unwrap().replace(
        "type: reference\n",
        "type: reference\nlang: tr\n", // declaration only — content stays as-is
    );
    fs::write(&page, text).unwrap();
    let map = repo.join("widget.md");
    fs::write(
        &map,
        "# Widget\n\n## Features\n- [[token-ttl|Token lifetime]] -- Lifetime.\n\
         - [[install|Install]] -- Setup.\n",
    )
    .unwrap();
    // no intent stated → the mixture is reported once
    let done = docsys::export::product(&root, &map, None, None).unwrap();
    assert!(
        done.warnings.iter().any(|w| w.contains("mixed declared")),
        "{:?}",
        done.warnings
    );
    // intent stated → each odd page is named; nothing is translated
    let done = docsys::export::product(&root, &map, Some("en"), None).unwrap();
    assert!(
        done.warnings
            .iter()
            .any(|w| w.contains("`token-ttl`") && w.contains("R-122")),
        "{:?}",
        done.warnings
    );
    assert!(done.output.contains("Access tokens live one hour."));
}

#[test]
fn feature_composes_a_slice_and_follow_widens_one_hop() {
    let (_repo, root) = build_tree("feature");
    // one id, no map file: the slice is the whole ceremony
    let done = docsys::export::feature(&root, &["token-ttl".to_string()], false, None, None, None)
        .unwrap();
    assert_eq!(done.pages, 1);
    // default title comes from the first page; pages sit one level up
    assert!(
        done.output.contains("\n# Token lifetime\n"),
        "{}",
        done.output
    );
    assert!(
        done.output.contains("\n## Token lifetime\n"),
        "{}",
        done.output
    );
    assert!(
        done.output.contains("\n### Refresh flow\n"),
        "{}",
        done.output
    );
    // --follow widens one hop along the page's own wiki-links
    let done = docsys::export::feature(
        &root,
        &["token-ttl".to_string()],
        true,
        Some("SubGHz guide".to_string()),
        None,
        None,
    )
    .unwrap();
    assert_eq!(done.pages, 2);
    assert!(
        done.output.contains("\n# SubGHz guide\n"),
        "{}",
        done.output
    );
    assert!(done.output.contains("doc: install"), "{}", done.output);
}

#[test]
fn unchanged_output_leaves_the_file_untouched() {
    let (repo, root) = build_tree("wif");
    let out_path = repo.join("dist.md");
    let one = docsys::export::feature(&root, &["token-ttl".to_string()], false, None, None, None)
        .unwrap();
    assert!(docsys::export::write_if_changed(&out_path, &one.output).unwrap());
    // an older file with a different generation date but the same content
    let aged = fs::read_to_string(&out_path)
        .unwrap()
        .replace("· generated: ", "· generated: 1999-01-01 not ");
    fs::write(&out_path, &aged).unwrap();
    assert!(
        !docsys::export::write_if_changed(&out_path, &one.output).unwrap(),
        "same content must not rewrite the file"
    );
    assert_eq!(
        fs::read_to_string(&out_path).unwrap(),
        aged,
        "the untouched file keeps its honest generation date"
    );
    // different content is rewritten
    let two =
        docsys::export::feature(&root, &["token-ttl".to_string()], true, None, None, None).unwrap();
    assert!(docsys::export::write_if_changed(&out_path, &two.output).unwrap());
}

#[test]
fn audience_is_declared_never_guessed_and_undeclared_reads_developer() {
    let (_repo, root) = build_tree("aud");
    // install declares end-user; token-ttl stays undeclared (= developer, D-033)
    let page = root.join("howto/install.md");
    let text = fs::read_to_string(&page)
        .unwrap()
        .replace("type: howto\n", "type: howto\naudience: end-user\n");
    fs::write(&page, text).unwrap();
    // a named page of the wrong audience is refused, not skipped
    let err = docsys::export::feature(
        &root,
        &["token-ttl".to_string()],
        false,
        None,
        None,
        Some("end-user"),
    )
    .unwrap_err();
    assert!(err.contains("`developer` page"), "{err}");
    // a followed page of the wrong audience is a named gap, not content
    let done = docsys::export::feature(
        &root,
        &["token-ttl".to_string()],
        true,
        None,
        None,
        Some("developer"),
    )
    .unwrap();
    assert_eq!(done.pages, 1);
    assert!(
        done.warnings.iter().any(|w| w.contains("gap:")),
        "{:?}",
        done.warnings
    );
    // the draft filters by audience; an audience nobody writes for is a
    // whole-tree gap, named
    let draft = docsys::export::plan(&root, Some("end-user")).unwrap();
    assert!(draft.contains("install"), "{draft}");
    assert!(!draft.contains("token-ttl"), "{draft}");
    let err = docsys::export::plan(&root, Some("designer")).unwrap_err();
    assert!(err.contains("designer"), "{err}");
}

#[test]
fn plan_drafts_a_map_grouped_by_type() {
    let (_repo, root) = build_tree("plan");
    let draft = docsys::export::plan(&root, None).unwrap();
    assert!(draft.contains("## reference"), "{draft}");
    assert!(
        draft.contains("- [[token-ttl|Token lifetime]] -- "),
        "{draft}"
    );
    assert!(draft.contains("Access tokens live one hour."), "{draft}");
    // the flowing page is not offered — only permanent pages compose
    assert!(!draft.contains("pending"), "{draft}");
}
