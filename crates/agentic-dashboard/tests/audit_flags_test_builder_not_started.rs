//! Story 25 acceptance test: audit flags category 3 (test-builder-not-
//! started) drift.
//!
//! Justification (from stories/25.yml): proves category 3 — given a
//! fixture corpus containing one story whose YAML has `status:
//! under_construction` AND ZERO of the paths enumerated in its
//! `acceptance.tests[].file` exist on disk yet, the audit report
//! names that story id under the test-builder-not-started category.
//! A story with at least one acceptance-test file present on disk
//! does NOT route to category 3 (mixed presence is a different signal
//! — either the test-builder pass partially shipped or a scaffold
//! was deleted; operator decides). Without this test, a stalled
//! test-builder pickup looks identical to a freshly flipped story,
//! and an operator scanning for "what's actually moving forward"
//! cannot distinguish the two.
//!
//! Red today is compile-red: the `agentic_dashboard::audit` module
//! and its `run_audit` entry point + `AuditReport` value do not yet
//! exist on the dashboard's `pub` surface.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use agentic_dashboard::audit::{run_audit, AuditReport};
use agentic_store::{MemStore, Store};
use agentic_test_support::FixtureCorpus;

const ID_NOT_STARTED: u32 = 250301;
const ID_MIXED_PRESENCE: u32 = 250302;
const HEAD_SHA: &str = "cccccccccccccccccccccccccccccccccccccccc";

/// Author a story YAML with TWO acceptance.tests[].file entries, so
/// "all-absent" (category 3) and "mixed-presence" (NOT category 3) are
/// observably distinct shapes.
fn fixture_yaml(id: u32, status: &str, test_paths: &[&str]) -> String {
    let tests_yaml: Vec<String> = test_paths
        .iter()
        .map(|p| {
            format!(
                "    - file: {p}\n      justification: |\n        \
                 Present so the fixture is schema-valid; the audit\n        \
                 only inspects file presence for category 3.\n"
            )
        })
        .collect();
    let tests_block = tests_yaml.join("");
    format!(
        r#"id: {id}
title: "Fixture {id} for story-25 cat-3 drift scaffold"

outcome: |
  Fixture story for the test-builder-not-started drift scaffold.

status: {status}

patterns: []

acceptance:
  tests:
{tests_block}  uat: |
    Drive the audit; assert test-builder-not-started membership.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#
    )
}

fn touch_file(path: &PathBuf) {
    fs::create_dir_all(path.parent().expect("test path has parent"))
        .expect("create parent dir");
    fs::write(
        path,
        r#"#[test]
fn placeholder() {
    assert!(true);
}
"#,
    )
    .expect("touch fixture test source");
}

#[test]
fn audit_flags_under_construction_story_with_all_tests_absent_under_test_builder_not_started() {
    let corpus = FixtureCorpus::new();
    let stories_dir = corpus.stories_dir();
    let root = corpus.path();

    // Drifted story: status=under_construction AND BOTH acceptance
    // test paths are absent on disk.
    let absent_a = root.join("fixture_tests").join("cat3_absent_a.rs");
    let absent_b = root.join("fixture_tests").join("cat3_absent_b.rs");
    let absent_a_str = absent_a.to_string_lossy().to_string();
    let absent_b_str = absent_b.to_string_lossy().to_string();
    let drifted_yaml = fixture_yaml(
        ID_NOT_STARTED,
        "under_construction",
        &[&absent_a_str, &absent_b_str],
    );
    fs::write(
        stories_dir.join(format!("{ID_NOT_STARTED}.yml")),
        drifted_yaml,
    )
    .expect("write not-started fixture story");

    // Mixed-presence control: status=under_construction with TWO test
    // paths, ONE of which exists on disk and ONE does not. This is
    // NOT category 3 — operator-judgment territory; the audit must
    // NOT classify it.
    let mixed_present = root.join("fixture_tests").join("cat3_mixed_present.rs");
    let mixed_absent = root.join("fixture_tests").join("cat3_mixed_absent.rs");
    touch_file(&mixed_present);
    let mixed_yaml = fixture_yaml(
        ID_MIXED_PRESENCE,
        "under_construction",
        &[
            &mixed_present.to_string_lossy(),
            &mixed_absent.to_string_lossy(),
        ],
    );
    fs::write(
        stories_dir.join(format!("{ID_MIXED_PRESENCE}.yml")),
        mixed_yaml,
    )
    .expect("write mixed-presence fixture story");

    // Audit reads the same store the dashboard reads. Category 3
    // queries file presence only — test_runs is irrelevant per the
    // story guidance — but seed the store anyway so the run path is
    // representative.
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let report: AuditReport = run_audit(&stories_dir, store.clone(), HEAD_SHA.to_string())
        .expect("audit must succeed against a clean tempdir corpus");

    let cat3_ids: Vec<u32> = report
        .test_builder_not_started
        .iter()
        .map(|entry| entry.id)
        .collect();

    assert!(
        cat3_ids.contains(&ID_NOT_STARTED),
        "audit must flag story {ID_NOT_STARTED} (status=under_construction, \
         all acceptance tests absent) under test_builder_not_started; \
         got ids={cat3_ids:?}"
    );
    assert!(
        !cat3_ids.contains(&ID_MIXED_PRESENCE),
        "audit must NOT flag mixed-presence story {ID_MIXED_PRESENCE} under \
         test_builder_not_started — mixed presence is operator-judgment \
         territory, not category 3; got ids={cat3_ids:?}"
    );

    // Cross-category isolation: the not-started story must NOT
    // appear under the other three categories.
    for (label, ids) in [
        (
            "implementation_without_flip",
            &report.implementation_without_flip,
        ),
        ("promotion_ready", &report.promotion_ready),
        (
            "healthy_with_failing_test",
            &report.healthy_with_failing_test,
        ),
    ] {
        let ids_in_cat: Vec<u32> = ids.iter().map(|e| e.id).collect();
        assert!(
            !ids_in_cat.contains(&ID_NOT_STARTED),
            "story {ID_NOT_STARTED} must appear under ONLY \
             test_builder_not_started; also found under {label}: {ids_in_cat:?}"
        );
    }

    // Mixed-presence must NOT appear ANYWHERE in the report — the
    // audit is silent on it and the operator decides.
    for (label, ids) in [
        (
            "implementation_without_flip",
            &report.implementation_without_flip,
        ),
        ("promotion_ready", &report.promotion_ready),
        (
            "test_builder_not_started",
            &report.test_builder_not_started,
        ),
        (
            "healthy_with_failing_test",
            &report.healthy_with_failing_test,
        ),
    ] {
        let ids_in_cat: Vec<u32> = ids.iter().map(|e| e.id).collect();
        assert!(
            !ids_in_cat.contains(&ID_MIXED_PRESENCE),
            "mixed-presence story {ID_MIXED_PRESENCE} must NOT appear in \
             the audit report under any category; found under {label}: {ids_in_cat:?}"
        );
    }
}
