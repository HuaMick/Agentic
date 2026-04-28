//! Story 25 acceptance test: audit flags category 4 (healthy-with-
//! failing-test) drift, AND pins the deduplication contract with
//! `agentic stories health`.
//!
//! Justification (from stories/25.yml): proves category 4 and pins
//! the deduplication contract — given a fixture corpus containing
//! one story whose YAML has `status: healthy` AND at least one
//! acceptance-test file currently fails, the audit report names
//! that story id under the healthy-with-failing-test category. The
//! audit MUST source its red-test signal from the same classifier
//! `agentic stories health` already uses to flag fell-from-grace
//! stories (test against the same `test_runs` row shape and the
//! same dashboard library entry point), not a parallel
//! reimplementation that could drift. Without this test, two
//! readers of the same evidence (`stories health` and `stories
//! audit`) could disagree about whether a healthy story is
//! currently red, and the corpus gains a second source of truth
//! for a question that has exactly one answer.
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

const ID_FELL_FROM_GRACE: u32 = 250401;
const ID_CLEAN_HEALTHY: u32 = 250402;
const HEAD_SHA: &str = "dddddddddddddddddddddddddddddddddddddddd";

fn fixture_yaml(id: u32, status: &str, test_file_path: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for story-25 cat-4 drift scaffold"

outcome: |
  Fixture story for the healthy-with-failing-test drift scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: {test_file_path}
      justification: |
        Present so the fixture is schema-valid; the audit reads
        test redness from the dashboard's test_runs classifier.
  uat: |
    Drive the audit against this YAML; assert membership in
    healthy-with-failing-test when test_runs is Fail.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#
    )
}

fn write_test_source(path: &PathBuf) {
    fs::create_dir_all(path.parent().expect("test path has parent")).expect("create parent dir");
    fs::write(
        path,
        r#"#[test]
fn placeholder() {
    assert!(true);
}
"#,
    )
    .expect("write fixture test source");
}

#[test]
fn audit_flags_healthy_story_with_failing_test_run_under_healthy_with_failing_test() {
    let corpus = FixtureCorpus::new();
    let stories_dir = corpus.stories_dir();
    let root = corpus.path();

    // Drifted story: YAML status=healthy AND a Fail test_runs row
    // exists for it. The dashboard's existing fell-from-grace
    // classifier (story 3) reads exactly this combination from
    // exactly this `test_runs` row shape. Audit MUST agree.
    let drifted_test_path = root.join("fixture_tests").join("cat4_drifted.rs");
    write_test_source(&drifted_test_path);
    let drifted_yaml = fixture_yaml(
        ID_FELL_FROM_GRACE,
        "healthy",
        drifted_test_path.to_str().expect("drifted path utf8"),
    );
    fs::write(
        stories_dir.join(format!("{ID_FELL_FROM_GRACE}.yml")),
        drifted_yaml,
    )
    .expect("write fell-from-grace fixture story");

    // Control: YAML status=healthy AND a Pass test_runs row. Must
    // NOT appear under healthy_with_failing_test.
    let clean_test_path = root.join("fixture_tests").join("cat4_clean.rs");
    write_test_source(&clean_test_path);
    let clean_yaml = fixture_yaml(
        ID_CLEAN_HEALTHY,
        "healthy",
        clean_test_path.to_str().expect("clean path utf8"),
    );
    fs::write(
        stories_dir.join(format!("{ID_CLEAN_HEALTHY}.yml")),
        clean_yaml,
    )
    .expect("write clean-healthy fixture story");

    // Seed the same `test_runs` row shape story 2 writes and the
    // dashboard reads. The audit consumes this through the
    // dashboard's classifier; a second cargo-test-walking codepath
    // inside the audit would be the very drift this test exists to
    // prevent.
    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    // Historical UAT pass at HEAD so the dashboard's fell-from-grace
    // classifier has the "was healthy" precondition.
    store
        .append(
            "uat_signings",
            serde_json::json!({
                "id": "01900000-0000-7000-8000-000000025401",
                "story_id": ID_FELL_FROM_GRACE,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "signed_at": "2026-04-26T00:00:00Z",
            }),
        )
        .expect("seed historical UAT pass for fell-from-grace fixture");
    store
        .upsert(
            "test_runs",
            &ID_FELL_FROM_GRACE.to_string(),
            serde_json::json!({
                "story_id": ID_FELL_FROM_GRACE,
                "verdict": "fail",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-27T00:00:00Z",
                "failing_tests": ["cat4_drifted.rs"],
            }),
        )
        .expect("seed Fail test_runs row for fell-from-grace fixture");
    // Clean control: UAT pass at HEAD AND test_runs Pass.
    store
        .append(
            "uat_signings",
            serde_json::json!({
                "id": "01900000-0000-7000-8000-000000025402",
                "story_id": ID_CLEAN_HEALTHY,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "signed_at": "2026-04-26T01:00:00Z",
            }),
        )
        .expect("seed UAT pass for clean control");
    store
        .upsert(
            "test_runs",
            &ID_CLEAN_HEALTHY.to_string(),
            serde_json::json!({
                "story_id": ID_CLEAN_HEALTHY,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-27T01:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed Pass test_runs for clean control");

    let report: AuditReport = run_audit(&stories_dir, store.clone(), HEAD_SHA.to_string())
        .expect("audit must succeed against a clean tempdir corpus");

    let cat4_ids: Vec<u32> = report
        .healthy_with_failing_test
        .iter()
        .map(|entry| entry.id)
        .collect();

    assert!(
        cat4_ids.contains(&ID_FELL_FROM_GRACE),
        "audit must flag story {ID_FELL_FROM_GRACE} (YAML=healthy, latest \
         test_runs=fail) under healthy_with_failing_test; got ids={cat4_ids:?}"
    );
    assert!(
        !cat4_ids.contains(&ID_CLEAN_HEALTHY),
        "audit must NOT flag clean control {ID_CLEAN_HEALTHY} under \
         healthy_with_failing_test; got ids={cat4_ids:?}"
    );

    // The drifted entry must list the failing test basename(s). Per
    // story 25's "Output shape — `--json`" guidance, category-4
    // entries carry `failing_tests: ["<basename>", ...]` matching
    // story 2's `failing_tests` column shape — basenames, not full
    // paths.
    let drifted_entry = report
        .healthy_with_failing_test
        .iter()
        .find(|entry| entry.id == ID_FELL_FROM_GRACE)
        .expect("drifted entry must be present (asserted above)");
    assert!(
        drifted_entry
            .failing_tests
            .iter()
            .any(|t| t == "cat4_drifted.rs"),
        "category-4 entry for {ID_FELL_FROM_GRACE} must list the failing \
         basename `cat4_drifted.rs` (matching story 2's failing_tests \
         column shape); got failing_tests={:?}",
        drifted_entry.failing_tests
    );

    // Deduplication contract: the audit's category-4 result must
    // agree with the dashboard's existing fell-from-grace
    // classifier. We assert on the dashboard-rendered output for
    // the same fixture: if the dashboard renders the drifted story
    // as `unhealthy`, the audit MUST list it under category 4. If
    // it renders the clean control as `healthy`, the audit MUST
    // NOT list it under category 4.
    use agentic_dashboard::Dashboard;
    let dashboard = Dashboard::new(store.clone(), stories_dir.clone(), HEAD_SHA.to_string());
    let rendered = dashboard
        .render_table()
        .expect("dashboard render_table must succeed on the same fixture");

    let drifted_row = rendered
        .lines()
        .find(|line| line.contains(&ID_FELL_FROM_GRACE.to_string()))
        .unwrap_or_else(|| {
            panic!(
                "dashboard table must contain a row for {ID_FELL_FROM_GRACE}; \
                 got:\n{rendered}"
            )
        });
    assert!(
        drifted_row.contains("unhealthy"),
        "dashboard must classify {ID_FELL_FROM_GRACE} as `unhealthy` (the \
         signal the audit's category 4 MUST source from); got row: {drifted_row:?}"
    );

    // Cross-category isolation: the drifted story must NOT also
    // appear under the other three categories.
    for (label, ids) in [
        (
            "implementation_without_flip",
            &report.implementation_without_flip,
        ),
        ("promotion_ready", &report.promotion_ready),
        ("test_builder_not_started", &report.test_builder_not_started),
    ] {
        let ids_in_cat: Vec<u32> = ids.iter().map(|e| e.id).collect();
        assert!(
            !ids_in_cat.contains(&ID_FELL_FROM_GRACE),
            "story {ID_FELL_FROM_GRACE} must appear under ONLY \
             healthy_with_failing_test; also found under {label}: {ids_in_cat:?}"
        );
    }
}
