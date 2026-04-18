//! Story 3 acceptance test: the single-story drill-down view.
//!
//! Justification (from stories/3.yml): proves the single-story
//! drill-down: `agentic stories health <id>` returns an expanded view
//! including the list of currently failing test file basenames (when
//! unhealthy) and the latest signing's commit and timestamp. Without
//! this an operator sees `unhealthy` in the table but has no path from
//! there to "which test, which commit?" without leaving the tool.
//!
//! The scaffold seeds `MemStore` so the target story is unhealthy (a
//! historical UAT pass + a failing latest `test_runs` with multiple
//! failing test basenames), calls `Dashboard::drilldown(story_id)`, and
//! asserts: (a) the output is NOT the table header (the drill-down is
//! an expanded single-story view); (b) every failing test basename
//! appears in the output (no truncation); (c) the latest UAT signing's
//! commit and RFC3339 timestamp both appear.
//! Red today is compile-red via the missing `agentic_dashboard` public
//! surface.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::json;
use tempfile::TempDir;

const STORY_ID: u32 = 9601;
const HEAD_SHA: &str = "2222222222222222222222222222222222222222";
const UAT_COMMIT: &str = "3333333333333333333333333333333333333333";
const UAT_SIGNED_AT: &str = "2026-04-18T12:34:56Z";

/// Three distinct failing basenames so we can assert each one appears
/// independently and there is no "truncated at N" elision.
const FAILING_TESTS: [&str; 3] =
    ["alpha_test.rs", "beta_test.rs", "gamma_longer_name_test.rs"];

const UNHEALTHY_FIXTURE: &str = r#"id: 9601
title: "An unhealthy story for the drill-down view"

outcome: |
  Fixture whose evidence is red so drill-down has something to name.

status: unhealthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_drilldown.rs
      justification: |
        Present so the fixture is schema-valid. The live test drives
        Dashboard::drilldown against this YAML after seeding red
        evidence.
  uat: |
    Call drilldown; assert basenames and the UAT signing's commit and
    timestamp appear.

guidance: |
  Fixture authored inline for the drill-down scaffold. Not a real story.

depends_on: []
"#;

#[test]
fn drilldown_names_every_failing_basename_and_latest_signings_commit_and_timestamp() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{STORY_ID}.yml")),
        UNHEALTHY_FIXTURE,
    )
    .expect("write unhealthy fixture");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Historical UAT pass at UAT_COMMIT (!= HEAD) with a specific
    // RFC3339 timestamp the drilldown is expected to surface.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000009601",
                "story_id": STORY_ID,
                "verdict": "pass",
                "commit": UAT_COMMIT,
                "signed_at": UAT_SIGNED_AT,
            }),
        )
        .expect("seed historical uat pass");

    // Latest test_runs is a Fail naming three basenames.
    store
        .upsert(
            "test_runs",
            &STORY_ID.to_string(),
            json!({
                "story_id": STORY_ID,
                "verdict": "fail",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": FAILING_TESTS,
            }),
        )
        .expect("seed failing test_runs");

    let dashboard = Dashboard::new(store.clone(), stories_dir.clone(), HEAD_SHA.to_string());
    let rendered = dashboard
        .drilldown(STORY_ID)
        .expect("drilldown should succeed for a known, unhealthy story id");

    // (a) Drill-down must not be the table header. Story 3's guidance
    // names the table header as `ID | Title | Health | Failing tests |
    // Healthy at`, so at minimum the pipe-separated header row must
    // not appear.
    assert!(
        !rendered.contains("Healthy at"),
        "drilldown output must NOT be the table view (`Healthy at` header absent); \
         got:\n{rendered}"
    );

    // (b) Every failing test basename must appear, with no truncation.
    for basename in FAILING_TESTS {
        assert!(
            rendered.contains(basename),
            "drilldown output must name failing test basename `{basename}` \
             (no truncation); got:\n{rendered}"
        );
    }

    // (c) The latest UAT signing's commit and RFC3339 timestamp both
    // appear. Commit may be full or a short form that prefixes the full
    // SHA; we accept either by requiring the first 7 chars to appear,
    // then also assert the timestamp verbatim.
    let short = &UAT_COMMIT[..7];
    assert!(
        rendered.contains(UAT_COMMIT) || rendered.contains(short),
        "drilldown output must name the latest UAT signing's commit \
         (full `{UAT_COMMIT}` or short `{short}`); got:\n{rendered}"
    );
    assert!(
        rendered.contains(UAT_SIGNED_AT),
        "drilldown output must name the latest UAT signing's RFC3339 \
         timestamp `{UAT_SIGNED_AT}`; got:\n{rendered}"
    );
}
