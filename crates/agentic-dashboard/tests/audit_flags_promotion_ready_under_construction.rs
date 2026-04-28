//! Story 25 acceptance test: audit flags category 2 (promotion-ready) drift.
//!
//! Justification (from stories/25.yml): proves category 2 — given a
//! fixture corpus containing one story whose YAML has `status:
//! under_construction` AND every path enumerated in its
//! `acceptance.tests[].file` exists on disk AND each of those test
//! files passes, the audit report names that story id under the
//! promotion-ready category and signals that the story is ready for
//! the UAT prove-it gate. A story whose status is `under_construction`
//! but whose tests do not all exist or do not all pass is NOT listed
//! under category 2 (it routes to category 3 or stays absent from
//! the report respectively). Without this test, the audit cannot tell
//! an operator "you have a story sitting at green-but-unsigned" —
//! the most actionable surface, because the next step is a single
//! UAT run rather than any code change.
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

const ID_PROMOTION_READY: u32 = 250201;
const ID_NEGATIVE_ABSENT: u32 = 250202;
const HEAD_SHA: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn fixture_yaml(id: u32, status: &str, test_file_path: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for story-25 cat-2 drift scaffold"

outcome: |
  Fixture story for the promotion-ready drift scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: {test_file_path}
      justification: |
        Present so the fixture is schema-valid and the audit has a
        path to inspect for category-2 drift. The live test passes
        when invoked, simulating the green-but-unsigned shape.
  uat: |
    Drive the audit; assert promotion-ready membership.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#
    )
}

fn write_passing_test_source(path: &PathBuf) {
    fs::create_dir_all(path.parent().expect("test path has parent")).expect("create parent dir");
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
fn audit_flags_under_construction_story_with_all_tests_passing_under_promotion_ready() {
    let corpus = FixtureCorpus::new();
    let stories_dir = corpus.stories_dir();
    let root = corpus.path();

    // Promotion-ready story: status=under_construction AND its
    // acceptance test file exists AND a Pass test_runs row exists.
    // This is "ready for UAT" — exactly one `agentic uat <id>
    // --verdict pass` away from healthy.
    let ready_test_path = root.join("fixture_tests").join("cat2_ready.rs");
    write_passing_test_source(&ready_test_path);
    let ready_yaml = fixture_yaml(
        ID_PROMOTION_READY,
        "under_construction",
        ready_test_path.to_str().expect("ready path utf8"),
    );
    fs::write(
        stories_dir.join(format!("{ID_PROMOTION_READY}.yml")),
        ready_yaml,
    )
    .expect("write promotion-ready fixture story");

    // Negative control: status=under_construction but the acceptance
    // test file does NOT exist. This is category 3 territory, NOT
    // category 2 — it must NOT appear under promotion_ready.
    let absent_test_path = root.join("fixture_tests").join("cat2_negative_absent.rs");
    let absent_yaml = fixture_yaml(
        ID_NEGATIVE_ABSENT,
        "under_construction",
        absent_test_path.to_str().expect("absent path utf8"),
    );
    fs::write(
        stories_dir.join(format!("{ID_NEGATIVE_ABSENT}.yml")),
        absent_yaml,
    )
    .expect("write negative-control fixture story");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    store
        .upsert(
            "test_runs",
            &ID_PROMOTION_READY.to_string(),
            serde_json::json!({
                "story_id": ID_PROMOTION_READY,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-27T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed Pass test_runs row for promotion-ready story");

    let report: AuditReport = run_audit(&stories_dir, root, store.clone(), HEAD_SHA.to_string())
        .expect("audit must succeed against a clean tempdir corpus");

    let ready_ids: Vec<u32> = report
        .promotion_ready
        .iter()
        .map(|entry| entry.id)
        .collect();

    assert!(
        ready_ids.contains(&ID_PROMOTION_READY),
        "audit must flag story {ID_PROMOTION_READY} (status=under_construction, \
         all tests present and passing) under promotion_ready; got ids={ready_ids:?}"
    );
    assert!(
        !ready_ids.contains(&ID_NEGATIVE_ABSENT),
        "audit must NOT flag the negative-control story {ID_NEGATIVE_ABSENT} \
         (status=under_construction but test file absent) under promotion_ready; \
         got ids={ready_ids:?}"
    );

    // Cross-category isolation: the promotion-ready story must NOT
    // also appear under the other three categories.
    for (label, ids) in [
        (
            "implementation_without_flip",
            &report.implementation_without_flip,
        ),
        ("test_builder_not_started", &report.test_builder_not_started),
        (
            "healthy_with_failing_test",
            &report.healthy_with_failing_test,
        ),
    ] {
        let ids_in_cat: Vec<u32> = ids.iter().map(|e| e.id).collect();
        assert!(
            !ids_in_cat.contains(&ID_PROMOTION_READY),
            "story {ID_PROMOTION_READY} must appear under ONLY promotion_ready; \
             also found under {label}: {ids_in_cat:?}"
        );
    }
}
