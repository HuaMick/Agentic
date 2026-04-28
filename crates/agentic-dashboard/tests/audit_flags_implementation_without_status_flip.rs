//! Story 25 acceptance test: audit flags category 1 (implementation-
//! without-flip) drift.
//!
//! Justification (from stories/25.yml): proves category 1 — given a
//! fixture corpus containing one story whose YAML has `status: proposed`
//! AND every path enumerated in its `acceptance.tests[].file` exists on
//! disk AND each of those test files passes when invoked, the audit
//! report names that story id under the implementation-without-flip
//! category and lists each passing test file path that proves the
//! implementation already shipped. Stories whose status is anything
//! other than `proposed`, or whose tests do not all exist, or whose
//! tests do not all pass, are NOT listed under category 1. Without
//! this test, the canonical drift mode this story exists to surface —
//! exactly what tripped stories 16, 18, 19 during the Phase 0 batch
//! when build-rust forgot to flip `proposed → under_construction` on
//! pickup — is undetectable, and operators continue to discover the
//! violation only by accident weeks later.
//!
//! Red today is compile-red: the `agentic_dashboard::audit` module
//! and its `run_audit` entry point + `AuditReport` value do not yet
//! exist on the dashboard's `pub` surface. The kit corpus comes from
//! `agentic_test_support::FixtureCorpus` (story 26), but the
//! `acceptance.tests[]` shape is authored inline here because the
//! kit's `StoryFixture::to_yaml` emits an empty `tests: []` and the
//! audit's whole signal is "what's in `acceptance.tests[].file`."

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use agentic_dashboard::audit::{run_audit, AuditReport};
use agentic_store::{MemStore, Store};
use agentic_test_support::FixtureCorpus;

const ID_DRIFTED: u32 = 250101;
const ID_CONTROL: u32 = 250102;
const HEAD_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// Author a story YAML naming `test_file_path` as the single
/// `acceptance.tests[].file` entry, with the given `status:`.
fn fixture_yaml(id: u32, status: &str, test_file_path: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for story-25 cat-1 drift scaffold"

outcome: |
  Fixture story for the implementation-without-flip drift scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: {test_file_path}
      justification: |
        Present so the fixture is schema-valid and the audit has a
        path to inspect for category-1 drift. The live test passes
        when invoked, simulating already-shipped implementation.
  uat: |
    Drive the audit against this YAML; assert it appears under
    implementation-without-flip when status is proposed.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#
    )
}

/// Write a passing test source file at `path` so the audit (which
/// classifies via the same evidence the dashboard reads) sees the
/// test as "exists and passes."
fn write_passing_test_source(path: &PathBuf) {
    fs::create_dir_all(path.parent().expect("test path has parent"))
        .expect("create parent dir for fixture test source");
    fs::write(
        path,
        r#"#[test]
fn passes() {
    assert!(true);
}
"#,
    )
    .expect("write passing fixture test source");
}

#[test]
fn audit_flags_proposed_story_with_all_tests_passing_under_implementation_without_flip() {
    let corpus = FixtureCorpus::new();
    let stories_dir = corpus.stories_dir();
    let root = corpus.path();

    // Drifted story: status=proposed but its acceptance test file
    // exists on disk and passes when invoked. This is the exact
    // shape stories 16/18/19 had during the Phase 0 violation.
    let drifted_test_path = root.join("fixture_tests").join("cat1_drifted.rs");
    write_passing_test_source(&drifted_test_path);
    let drifted_yaml = fixture_yaml(
        ID_DRIFTED,
        "proposed",
        drifted_test_path.to_str().expect("test path utf8"),
    );
    fs::write(stories_dir.join(format!("{ID_DRIFTED}.yml")), drifted_yaml)
        .expect("write drifted fixture story");

    // Control story: status=proposed AND its acceptance test does NOT
    // exist on disk yet. This is a legitimate proposed story (no
    // implementation shipped) and MUST NOT appear in any category.
    let control_test_path = root.join("fixture_tests").join("cat1_control_absent.rs");
    let control_yaml = fixture_yaml(
        ID_CONTROL,
        "proposed",
        control_test_path.to_str().expect("control path utf8"),
    );
    fs::write(stories_dir.join(format!("{ID_CONTROL}.yml")), control_yaml)
        .expect("write control fixture story");

    // Audit reads test-pass evidence from the same store the dashboard
    // reads. Seed a Pass test_runs row for the drifted story so the
    // dashboard classifier (which the audit shares per the story's
    // delegation contract) sees its test green.
    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    store
        .upsert(
            "test_runs",
            &ID_DRIFTED.to_string(),
            serde_json::json!({
                "story_id": ID_DRIFTED,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-27T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed Pass test_runs row for drifted story");

    let report: AuditReport = run_audit(&stories_dir, store.clone(), HEAD_SHA.to_string())
        .expect("audit must succeed against a clean tempdir corpus");

    let drifted_ids: Vec<u32> = report
        .implementation_without_flip
        .iter()
        .map(|entry| entry.id)
        .collect();

    assert!(
        drifted_ids.contains(&ID_DRIFTED),
        "audit must flag story {ID_DRIFTED} (status=proposed, all tests pass) \
         under implementation-without-flip; got ids={drifted_ids:?}"
    );
    assert!(
        !drifted_ids.contains(&ID_CONTROL),
        "audit must NOT flag the control story {ID_CONTROL} (status=proposed \
         but tests absent) under implementation-without-flip; got ids={drifted_ids:?}"
    );

    // The drifted entry must list the passing test file path, so an
    // operator scanning the report can see WHICH evidence proves the
    // implementation already shipped.
    let drifted_entry = report
        .implementation_without_flip
        .iter()
        .find(|entry| entry.id == ID_DRIFTED)
        .expect("drifted entry must be present (asserted above)");

    let drifted_path_str = drifted_test_path.to_string_lossy().to_string();
    assert!(
        drifted_entry
            .passing_tests
            .iter()
            .any(|p| p == &drifted_path_str),
        "category-1 entry for {ID_DRIFTED} must list the passing test path \
         {drifted_path_str:?}; got passing_tests={:?}",
        drifted_entry.passing_tests
    );

    // Cross-category isolation: the drifted story must NOT also appear
    // under the other three categories.
    for (label, ids) in [
        ("promotion_ready", &report.promotion_ready),
        ("test_builder_not_started", &report.test_builder_not_started),
        (
            "healthy_with_failing_test",
            &report.healthy_with_failing_test,
        ),
    ] {
        let ids_in_cat: Vec<u32> = ids.iter().map(|e| e.id).collect();
        assert!(
            !ids_in_cat.contains(&ID_DRIFTED),
            "story {ID_DRIFTED} must appear under ONLY implementation_without_flip; \
             also found under {label}: {ids_in_cat:?}"
        );
    }
}
