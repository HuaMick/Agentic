//! Story 25 acceptance test: audit returns the empty report on a
//! clean corpus.
//!
//! Justification (from stories/25.yml): proves the empty-report
//! happy path — given a fixture corpus where every story's declared
//! status agrees with its actual implementation state across all
//! four categories — proposed stories with no shipped tests,
//! under_construction stories with partial or red tests, healthy
//! stories with passing tests — the audit report contains zero
//! entries under any of the four categories AND the process exits
//! 0. Without this test, the audit could silently flag false
//! positives on a clean corpus (e.g. counting test absence as
//! drift in a `proposed` story where absence is correct), and
//! operators would lose trust in the report after the first
//! noise-storm.
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

const ID_CLEAN_PROPOSED: u32 = 250501;
const ID_CLEAN_UC_RED: u32 = 250502;
const ID_CLEAN_HEALTHY: u32 = 250503;
const HEAD_SHA: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

fn fixture_yaml(id: u32, status: &str, test_file_path: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for story-25 clean-corpus scaffold"

outcome: |
  Fixture story for the clean-corpus scaffold. Status agrees with
  reality, so this story must NOT appear in the audit report.

status: {status}

patterns: []

acceptance:
  tests:
    - file: {test_file_path}
      justification: |
        Present so the fixture is schema-valid; the audit reads
        file presence and test_runs evidence to decide drift.
  uat: |
    Drive the audit; assert the report is empty across all four
    categories.

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
fn audit_emits_empty_report_with_explicit_no_drift_signal_on_a_clean_corpus() {
    let corpus = FixtureCorpus::new();
    let stories_dir = corpus.stories_dir();
    let root = corpus.path();

    // Clean proposed: status=proposed, test path absent (legitimate).
    let proposed_test = root.join("fixture_tests").join("clean_proposed.rs");
    let proposed_yaml = fixture_yaml(
        ID_CLEAN_PROPOSED,
        "proposed",
        proposed_test.to_str().expect("proposed path utf8"),
    );
    fs::write(
        stories_dir.join(format!("{ID_CLEAN_PROPOSED}.yml")),
        proposed_yaml,
    )
    .expect("write clean-proposed fixture");

    // Clean under_construction with red test: status=under_construction,
    // test exists but test_runs latest verdict is Fail. Per story 25
    // guidance this stays absent from the report (categories 2+3
    // both miss; category 4 only fires on `healthy` YAML).
    let uc_test = root.join("fixture_tests").join("clean_uc_red.rs");
    write_test_source(&uc_test);
    let uc_yaml = fixture_yaml(
        ID_CLEAN_UC_RED,
        "under_construction",
        uc_test.to_str().expect("uc path utf8"),
    );
    fs::write(stories_dir.join(format!("{ID_CLEAN_UC_RED}.yml")), uc_yaml)
        .expect("write clean-uc fixture");

    // Clean healthy: status=healthy AND latest test_runs row is Pass
    // AND latest UAT pass commit equals HEAD.
    let healthy_test = root.join("fixture_tests").join("clean_healthy.rs");
    write_test_source(&healthy_test);
    let healthy_yaml = fixture_yaml(
        ID_CLEAN_HEALTHY,
        "healthy",
        healthy_test.to_str().expect("healthy path utf8"),
    );
    fs::write(
        stories_dir.join(format!("{ID_CLEAN_HEALTHY}.yml")),
        healthy_yaml,
    )
    .expect("write clean-healthy fixture");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    // Clean UC story has a Fail test_runs row, but YAML=under_construction
    // so it never enters category 4 and its file presence + Fail
    // verdict means category 2 misses too.
    store
        .upsert(
            "test_runs",
            &ID_CLEAN_UC_RED.to_string(),
            serde_json::json!({
                "story_id": ID_CLEAN_UC_RED,
                "verdict": "fail",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-27T00:00:00Z",
                "failing_tests": ["clean_uc_red.rs"],
            }),
        )
        .expect("seed Fail test_runs row for clean UC fixture");
    // Clean healthy: UAT pass at HEAD AND test_runs Pass at HEAD.
    store
        .append(
            "uat_signings",
            serde_json::json!({
                "id": "01900000-0000-7000-8000-000000025503",
                "story_id": ID_CLEAN_HEALTHY,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "signed_at": "2026-04-26T00:00:00Z",
            }),
        )
        .expect("seed UAT pass for clean healthy");
    store
        .upsert(
            "test_runs",
            &ID_CLEAN_HEALTHY.to_string(),
            serde_json::json!({
                "story_id": ID_CLEAN_HEALTHY,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-27T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed Pass test_runs for clean healthy");

    let report: AuditReport = run_audit(&stories_dir, store.clone(), HEAD_SHA.to_string())
        .expect("audit must succeed against a clean tempdir corpus");

    // All four arrays must be empty — the corpus is by construction
    // free of drift.
    assert!(
        report.implementation_without_flip.is_empty(),
        "clean corpus must produce zero implementation_without_flip entries; \
         got {:?}",
        report
            .implementation_without_flip
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>()
    );
    assert!(
        report.promotion_ready.is_empty(),
        "clean corpus must produce zero promotion_ready entries; got {:?}",
        report
            .promotion_ready
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>()
    );
    assert!(
        report.test_builder_not_started.is_empty(),
        "clean corpus must produce zero test_builder_not_started entries; \
         got {:?}",
        report
            .test_builder_not_started
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>()
    );
    assert!(
        report.healthy_with_failing_test.is_empty(),
        "clean corpus must produce zero healthy_with_failing_test entries; \
         got {:?}",
        report
            .healthy_with_failing_test
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>()
    );

    // Per story 25's "Output shape — human-readable" guidance, the
    // clean-corpus run emits an explicit "No drift detected" line
    // (or equivalent) — not silent empty stdout. The library-level
    // report must carry an observable equivalent so a CLI shim can
    // render it. Per the story: "a concrete affirmative line, not a
    // silent empty output". The library exposes that via the
    // `is_empty()` predicate the `Display`-renderer keys off.
    assert!(
        report.is_empty(),
        "clean corpus must observably report no drift via AuditReport::is_empty()"
    );
    let rendered = report.to_string();
    let lower = rendered.to_lowercase();
    assert!(
        lower.contains("no drift"),
        "clean-corpus rendered report must include an explicit \"No drift\" \
         affirmative line per story 25's human-readable contract; got:\n{rendered}"
    );
}
