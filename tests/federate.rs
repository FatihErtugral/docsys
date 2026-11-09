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

/// A git repository holding a docs tree with one page per id.
fn git_provider(dir: &Path, ids: &[&str]) {
    let docs = dir.join("docs");
    write(
        &docs,
        ".docmeta.yml",
        "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\n",
    );
    let mut index = String::from("# docs\n\n");
    for id in ids {
        index.push_str(&format!("- [[howto/{id}|{id}]] -- The {id} page.\n"));
        write(
            &docs,
            &format!("howto/{id}.md"),
            &format!(
                "---\nid: {id}\ntype: howto\nupdated: 2026-08-16\n---\n\n# {id}\n\nThe {id} page.\n"
            ),
        );
    }
    write(&docs, "index.md", &index);
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "t@example.invalid"],
        vec!["config", "user.name", "t"],
        vec!["config", "commit.gpgsign", "false"],
        vec!["add", "-A"],
        vec!["commit", "-q", "-m", "docs"],
    ] {
        assert!(std::process::Command::new("git")
            .args(&args)
            .current_dir(dir)
            .status()
            .unwrap()
            .success());
    }
}

/// A consumer of `consume` entries, with a page that cites `cites`.
fn consumer_of(name: &str, consume: &str) -> PathBuf {
    let root = tmp(name).join("docs");
    write(
        &root,
        ".docmeta.yml",
        &format!(
            "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\nconsume: [{consume}]\n"
        ),
    );
    write(&root, "index.md", "# docs\n");
    root
}

fn errors_of(report: &docsys::checks::Report, subject: &str) -> Vec<String> {
    report
        .findings
        .iter()
        .filter(|f| f.subject == subject)
        .map(|f| format!("{} {} {}", f.severity.tag(), f.rule, f.message))
        .collect()
}

#[test]
fn an_unknown_id_in_a_fetched_namespace_is_an_error() {
    let provider = build_provider("prov-unknown");
    let consumer = build_consumer("cons-unknown", &provider);
    docsys::export::fetch(&consumer).unwrap();
    let repo = consumer.parent().unwrap().to_path_buf();
    write(
        &repo,
        "src/app.js",
        "// doc: @firmware/shared-feature\n// doc: @firmware/missing-page\n// doc: @other/never-fetched\n",
    );
    write(
        &consumer,
        "howto/cites.md",
        "---\nid: cites\ntype: howto\nupdated: 2026-08-16\nsources: [\"@firmware/missing-page\"]\n---\n\n\
         # Cites\n\nThe firmware page: doc: @firmware/missing-page\n",
    );
    let tree = docsys::tree::DocTree::load(&consumer).unwrap();
    // code: the namespace was fetched and holds no such id — dangling (R-139)
    let refs = docsys::refs::run(&repo, &tree);
    let missing = errors_of(&refs, "@firmware/missing-page");
    assert!(
        matches!(missing.as_slice(), [one] if one.starts_with("ERROR R-076") && one.contains("has no identifier")),
        "{missing:?}"
    );
    assert!(errors_of(&refs, "@firmware/shared-feature").is_empty());
    // a namespace never fetched here keeps its report and its two commands
    let unfetched = errors_of(&refs, "@other/never-fetched");
    assert!(
        matches!(unfetched.as_slice(), [one] if one.starts_with("WARN R-076 ") && one.contains("docsys fetch")),
        "{unfetched:?}"
    );
    // a page's citation and its `sources:` entry name the same cause
    let (lint, _) = docsys::lint(&consumer);
    let page = errors_of(&lint, "@firmware/missing-page");
    assert!(
        page.iter()
            .any(|f| f.starts_with("ERROR R-076") && f.contains("has no identifier")),
        "{page:?}"
    );
    assert!(
        page.iter()
            .any(|f| f.contains("R-059") && f.contains("has no identifier")),
        "{page:?}"
    );
}

#[test]
fn fetch_removes_the_pages_a_provider_no_longer_exports() {
    let provider = build_provider("prov-prune");
    let consumer = build_consumer("cons-prune", &provider);
    write(
        &provider,
        "howto/second.md",
        "---\nid: second\ntype: howto\nupdated: 2026-08-16\n---\n\n# Second\n\nA second page.\n",
    );
    docsys::export::fetch(&consumer).unwrap();
    let fed = consumer.join(".federation/firmware");
    assert!(fed.join("shared-feature.md").is_file());
    assert!(fed.join("second.md").is_file());
    // the provider removes one page and makes the other internal
    fs::remove_file(provider.join("howto/shared-feature.md")).unwrap();
    let second = provider.join("howto/second.md");
    let text = fs::read_to_string(&second)
        .unwrap()
        .replace("type: howto\n", "type: howto\ninternal: true\n");
    fs::write(&second, text).unwrap();
    let summary = docsys::export::fetch(&consumer).unwrap();
    for id in ["shared-feature", "second"] {
        assert!(!fed.join(format!("{id}.md")).exists(), "{id}: {summary:?}");
        assert!(
            !fed.join(format!("{id}.provenance.yml")).exists(),
            "{id}: {summary:?}"
        );
    }
    assert!(
        summary
            .iter()
            .any(|s| s.contains("2 no longer exported, removed")),
        "{summary:?}"
    );
    // a citation of the removed page is dangling now, not answered by a copy
    write(
        &consumer,
        "howto/cites.md",
        "---\nid: cites\ntype: howto\nupdated: 2026-08-16\n---\n\n# Cites\n\nThe page: doc: @firmware/shared-feature\n",
    );
    let (lint, _) = docsys::lint(&consumer);
    let cited = errors_of(&lint, "@firmware/shared-feature");
    assert!(
        cited.iter().any(|f| f.starts_with("ERROR R-076")),
        "{cited:?}"
    );
}

#[test]
fn one_unreachable_provider_leaves_the_others_fetched() {
    let base = tmp("unreachable");
    git_provider(&base.join("svc-b"), &["b-page"]);
    let consumer = consumer_of(
        "cons-unreachable",
        &format!(
            "svc-a=file://{}/nope.git, svc-b=file://{}/svc-b",
            base.display(),
            base.display()
        ),
    );
    // fresh state: svc-a never answered, svc-b is fetched all the same
    let err = docsys::export::fetch(&consumer).unwrap_err();
    assert!(
        consumer.join(".federation/svc-b/b-page.md").is_file(),
        "{err}"
    );
    assert!(err.contains("svc-a: not fetched"), "{err}");
    assert!(err.contains("nothing of it is materialized here"), "{err}");
    assert!(err.contains("svc-b: 1 page(s)"), "{err}");
    // the command says so and exits 2: a provider was not read
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_docsys"))
        .args(["fetch", "--root"])
        .arg(&consumer)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("svc-b: 1 page(s)"), "{stdout}");
    assert!(stdout.contains("svc-a: not fetched"), "{stdout}");
}

#[test]
fn an_unreachable_provider_keeps_its_pages_and_says_their_age() {
    let base = tmp("keeps");
    git_provider(&base.join("svc-a"), &["a-page"]);
    git_provider(&base.join("svc-b"), &["b-page"]);
    let consumer = consumer_of(
        "cons-keeps",
        &format!(
            "svc-a=file://{}/svc-a, svc-b=file://{}/svc-b",
            base.display(),
            base.display()
        ),
    );
    docsys::export::fetch(&consumer).unwrap();
    // svc-a goes away; svc-b moves on
    fs::rename(base.join("svc-a"), base.join("svc-a-moved")).unwrap();
    let page = base.join("svc-b/docs/howto/b-page.md");
    let text = fs::read_to_string(&page)
        .unwrap()
        .replace("The b-page page.", "The b-page page. A new sentence.");
    fs::write(&page, text).unwrap();
    assert!(std::process::Command::new("git")
        .args(["commit", "-aqm", "update"])
        .current_dir(base.join("svc-b"))
        .status()
        .unwrap()
        .success());
    let err = docsys::export::fetch(&consumer).unwrap_err();
    let b = fs::read_to_string(consumer.join(".federation/svc-b/b-page.md")).unwrap();
    assert!(b.contains("A new sentence."), "{err}\n{b}");
    assert!(
        consumer.join(".federation/svc-a/a-page.md").is_file(),
        "{err}"
    );
    assert!(
        err.contains("svc-a: not fetched")
            && err.contains("keeping 1 page(s) under .federation/svc-a/, fetched ")
            && err.contains("day(s) old"),
        "{err}"
    );
}

#[test]
fn a_changed_location_is_fetched_from_where_it_is_declared() {
    let base = tmp("moved");
    git_provider(&base.join("first"), &["first-page"]);
    git_provider(&base.join("second"), &["second-page"]);
    let docmeta = |loc: &str| {
        format!(
            "spec: docsys/0.4\nprofile: project\ndefault_content_language: en\nconsume: [svc=file://{}/{loc}]\n",
            base.display()
        )
    };
    let consumer = consumer_of("cons-moved", "placeholder=none");
    write(&consumer, ".docmeta.yml", &docmeta("first"));
    docsys::export::fetch(&consumer).unwrap();
    assert!(consumer.join(".federation/svc/first-page.md").is_file());
    // the declared location moves to another repository: its pages are read
    write(&consumer, ".docmeta.yml", &docmeta("second"));
    let summary = docsys::export::fetch(&consumer).unwrap();
    assert!(
        consumer.join(".federation/svc/second-page.md").is_file(),
        "{summary:?}"
    );
    assert!(
        !consumer.join(".federation/svc/first-page.md").exists(),
        "{summary:?}"
    );
    // a location that answers nothing fails that provider, named by its URL
    write(&consumer, ".docmeta.yml", &docmeta("missing"));
    let err = docsys::export::fetch(&consumer).unwrap_err();
    assert!(err.contains("svc: not fetched"), "{err}");
    assert!(err.contains("/missing"), "{err}");
    assert!(
        consumer.join(".federation/svc/second-page.md").is_file(),
        "{err}"
    );
}
