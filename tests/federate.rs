#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
// Two repositories sharing one feature, one composed document — the founding
// goal's first working slice (filesystem transport, D-034). Foreign entries
// compose only from a verified local materialization, never a live query.

use std::fs;
use std::path::{Path, PathBuf};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docsys-fed-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

/// Provider: a firmware repo documenting the shared feature; one page is
/// internal and must never cross the boundary (R-135).
fn build_provider(name: &str) -> PathBuf {
    let root = tmp(name).join("docs");
    write(
        &root,
        ".docmeta.yml",
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n",
    );
    write(
        &root,
        "index.md",
        "# docs\n\n- [[howto/shared-feature|Shared feature]] -- Using it on the device.\n",
    );
    write(
        &root,
        "howto/shared-feature.md",
        "---\nid: shared-feature\ntype: howto\naudience: end-user\nupdated: 2026-08-16\n---\n\n\
         # Using the shared feature\n\nPress the button; the device answers.\n\n\
         ## Pairing\n\nHold both buttons for two seconds.\n",
    );
    write(
        &root,
        "reference/secret-keys.md",
        "---\nid: secret-keys\ntype: reference\ninternal: true\nupdated: 2026-08-16\n---\n\n\
         # Secret keys\n\nNever exported.\n",
    );
    root
}

/// Consumer: the app repo with its own page, consuming the provider.
fn build_consumer(name: &str, provider_root: &Path) -> PathBuf {
    let root = tmp(name).join("docs");
    write(
        &root,
        ".docmeta.yml",
        &format!(
            "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n\
             consume: [firmware={}]\n",
            provider_root.display()
        ),
    );
    write(
        &root,
        "index.md",
        "# docs\n\n- [[howto/app-side|App side]] -- The same feature in the app.\n",
    );
    write(
        &root,
        "howto/app-side.md",
        "---\nid: app-side\ntype: howto\naudience: end-user\nupdated: 2026-08-16\n---\n\n\
         # The feature in the app\n\nTap the card; the app mirrors the device.\n",
    );
    root
}

#[test]
fn two_repos_one_feature_one_document() {
    let provider = build_provider("prov");
    let consumer = build_consumer("cons", &provider);
    // materialize
    let summary = docsys::export::fetch(&consumer).unwrap();
    assert!(
        summary.iter().any(|s| s.contains("firmware")),
        "{summary:?}"
    );
    assert!(consumer
        .join(".federation/firmware/shared-feature.md")
        .is_file());
    assert!(consumer
        .join(".federation/firmware/shared-feature.provenance.yml")
        .is_file());
    // R-135: the internal page never crossed the boundary
    assert!(!consumer
        .join(".federation/firmware/secret-keys.md")
        .exists());
    // compose across the boundary: local page + foreign page, one document
    let done = docsys::export::feature(
        &consumer,
        &[
            "app-side".to_string(),
            "@firmware/shared-feature".to_string(),
        ],
        false,
        Some("Shared feature guide".to_string()),
        None,
        Some("end-user"),
    )
    .unwrap();
    assert_eq!(done.pages, 2);
    assert!(done.output.contains("Tap the card"), "{}", done.output);
    assert!(done.output.contains("Press the button"), "{}", done.output);
    assert!(done.output.contains("### Pairing"), "{}", done.output);
    assert!(
        done.output
            .contains("doc: @firmware/shared-feature · .federation/firmware/shared-feature.md"),
        "{}",
        done.output
    );
    assert!(done.output.contains("fetched: "), "{}", done.output);
}

#[test]
fn unfetched_and_tampered_foreign_entries_are_refused() {
    let provider = build_provider("prov2");
    let consumer = build_consumer("cons2", &provider);
    // not fetched yet → refusal names the fix
    let err = docsys::export::feature(
        &consumer,
        &["@firmware/shared-feature".to_string()],
        false,
        None,
        None,
        None,
    )
    .unwrap_err();
    assert!(err.contains("not materialized"), "{err}");
    // fetched, then edited locally → the hash refuses (R-137)
    docsys::export::fetch(&consumer).unwrap();
    let mat = consumer.join(".federation/firmware/shared-feature.md");
    let mut text = fs::read_to_string(&mat).unwrap();
    text.push_str("a line someone typed into a derived artifact\n");
    fs::write(&mat, text).unwrap();
    let err = docsys::export::feature(
        &consumer,
        &["@firmware/shared-feature".to_string()],
        false,
        None,
        None,
        None,
    )
    .unwrap_err();
    assert!(err.contains("provenance hash"), "{err}");
}

#[test]
fn a_template_names_the_estate_and_git_is_a_transport() {
    // Two "microservices" as git repositories under one base — the estate
    // pattern: one template line, a list of names, no per-service paths.
    let base = tmp("estate");
    for svc in ["auth", "billing"] {
        let repo = base.join(format!("{svc}-repo"));
        let docs = repo.join("docs");
        write(
            &docs,
            ".docmeta.yml",
            "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n",
        );
        write(
            &docs,
            "index.md",
            &format!("# docs\n\n- [[howto/use-{svc}|Use {svc}]] -- Using it.\n"),
        );
        write(
            &docs,
            &format!("howto/use-{svc}.md"),
            &format!(
                "---\nid: use-{svc}\ntype: howto\nupdated: 2026-08-16\n---\n\n\
                 # Using {svc}\n\nThe {svc} service in one page.\n"
            ),
        );
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.email", "t@example.invalid"],
            vec!["config", "user.name", "t"],
            vec!["add", "-A"],
            vec!["commit", "-q", "-m", "docs"],
        ] {
            assert!(std::process::Command::new("git")
                .args(&args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success());
        }
    }
    // The estate repo: docmeta + maps, nothing else. One template, two names.
    let estate = tmp("estate-root").join("docs");
    write(
        &estate,
        ".docmeta.yml",
        &format!(
            "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n\
             consume_base: \"file://{}/{{ns}}-repo#docs\"\nconsume: [auth, billing]\n",
            base.display()
        ),
    );
    write(&estate, "index.md", "# estate\n");
    let summary = docsys::export::fetch(&estate).unwrap();
    assert_eq!(summary.len(), 2, "{summary:?}");
    assert!(estate.join(".federation/auth/use-auth.md").is_file());
    assert!(estate.join(".federation/billing/use-billing.md").is_file());
    // one document from two services, out of a repo that owns no content
    let done = docsys::export::feature(
        &estate,
        &[
            "@auth/use-auth".to_string(),
            "@billing/use-billing".to_string(),
        ],
        false,
        Some("Estate guide".to_string()),
        None,
        None,
    )
    .unwrap();
    assert_eq!(done.pages, 2);
    assert!(done.output.contains("The auth service"), "{}", done.output);
    assert!(
        done.output.contains("The billing service"),
        "{}",
        done.output
    );
    // a provider moves on; re-fetch picks the new state up (R-148's refresh)
    let auth_page = base.join("auth-repo/docs/howto/use-auth.md");
    let text = fs::read_to_string(&auth_page)
        .unwrap()
        .replace("in one page", "in one updated page");
    fs::write(&auth_page, text).unwrap();
    assert!(std::process::Command::new("git")
        .args(["commit", "-aqm", "update"])
        .current_dir(base.join("auth-repo"))
        .status()
        .unwrap()
        .success());
    docsys::export::fetch(&estate).unwrap();
    let refreshed = fs::read_to_string(estate.join(".federation/auth/use-auth.md")).unwrap();
    assert!(refreshed.contains("in one updated page"), "{refreshed}");
}

#[test]
fn the_estate_draft_lists_fetched_pages() {
    let provider = build_provider("prov3");
    let consumer = build_consumer("cons3", &provider);
    docsys::export::fetch(&consumer).unwrap();
    let draft = docsys::export::plan(&consumer, None).unwrap();
    assert!(draft.contains("- [[@firmware/shared-feature|"), "{draft}");
    // the local page drafts alongside the foreign one
    assert!(draft.contains("- [[app-side|"), "{draft}");
}

#[test]
fn a_manifest_indexes_the_namespace_and_skips_unchanged_pages() {
    let provider = build_provider("prov-man");
    // the provider publishes its index beside its docs root
    let text = docsys::export::manifest(&provider).unwrap();
    assert!(text.starts_with("manifest: 1\n"), "{text}");
    assert!(text.contains("- id: shared-feature"), "{text}");
    assert!(text.contains("  hash: fnv:"), "{text}");
    // R-134: bodies never travel — title and the one-sentence summary are
    // identity metadata and the deliberate exception, so the Pairing section
    // (real body) must be absent while the summary line is present.
    assert!(text.contains("  summary: Press the button"), "{text}");
    assert!(!text.contains("Hold both buttons"), "{text}");
    // R-135: the internal page appears nowhere
    assert!(!text.contains("secret-keys"), "{text}");
    fs::write(provider.join("manifest.docsys"), &text).unwrap();

    let consumer = build_consumer("cons-man", &provider);
    let first = docsys::export::fetch(&consumer).unwrap();
    assert!(first.iter().any(|s| s.contains("1 page")), "{first:?}");
    let materialized = consumer.join(".federation/firmware/shared-feature.md");
    let stamp = fs::metadata(&materialized).unwrap().modified().unwrap();

    // second fetch, nothing changed upstream: the page is skipped, its bytes
    // and its fetch date survive
    let second = docsys::export::fetch(&consumer).unwrap();
    assert!(
        second.iter().any(|s| s.contains("unchanged, skipped")),
        "{second:?}"
    );
    assert_eq!(
        fs::metadata(&materialized).unwrap().modified().unwrap(),
        stamp
    );

    // the provider edits the page and republishes: the change comes through
    let page = provider.join("howto/shared-feature.md");
    let edited = fs::read_to_string(&page)
        .unwrap()
        .replace("the device answers", "the device answers twice");
    fs::write(&page, edited).unwrap();
    fs::write(
        provider.join("manifest.docsys"),
        docsys::export::manifest(&provider).unwrap(),
    )
    .unwrap();
    let third = docsys::export::fetch(&consumer).unwrap();
    assert!(!third.iter().any(|s| s.contains("unchanged")), "{third:?}");
    assert!(fs::read_to_string(&materialized)
        .unwrap()
        .contains("answers twice"));
}

#[test]
fn an_unimplemented_manifest_major_is_refused_by_name() {
    let provider = build_provider("prov-ver");
    fs::write(
        provider.join("manifest.docsys"),
        "manifest: 99\nnamespace: firmware\n",
    )
    .unwrap();
    let consumer = build_consumer("cons-ver", &provider);
    let err = docsys::export::fetch(&consumer).unwrap_err();
    assert!(err.contains("version 99"), "{err}");
}
