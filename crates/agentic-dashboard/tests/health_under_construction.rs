//! Story 3 acceptance test: the under-construction classification rule.
//!
//! Justification (from stories/3.yml): proves the under-construction rule
//! — a story whose YAML has `status: under_construction` AND has never
//! had a `uat_signings.verdict=pass` row renders as `under_construction`,
//! whether `test_runs` is currently pass, fail, or absent. Without this,
//! a never-passed story whose tests happen to be red would be
//! misreported as "fell from grace" — but it never rose in the first
//! place.
//!
//! The scaffold exercises three sub-fixtures in a single test function
//! to keep the "three parameterisations" justification in one file, each
//! rebuilding the dashboard against a fresh `MemStore`:
//!   (a) `test_runs` row with `verdict=pass`;
//!   (b) `test_runs` row with `verdict=fail` (with failing_tests);
//!   (c) no `test_runs` row at all.
//! In all three sub-cases, NO `uat_signings.verdict=pass` row ever
//! existed for this story. All three must render as `under_construction`.
//! Red today is compile-red via the missing `agentic_dashboard` public
//! surface.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::{json, Value};
use tempfile::TempDir;

const STORY_ID: u32 = 9302;
const HEAD_SHA: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

const UNDER_CONSTRUCTION_FIXTURE: &str = r#"id: 9302
title: "A story in flight that never passed UAT"

outcome: |
  A fixture whose YAML status is `under_construction` and whose UAT has
  never been signed Pass. The dashboard must classify it as
  `under_construction` regardless of current test_runs state.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_under_construction.rs
      justification: |
        Present so the fixture is schema-valid. The live test drives
        Dashboard::render_table against this YAML under three distinct
        test_runs sub-fixtures (pass, fail, absent).
  uat: |
    Render the table; assert the health column reads `under_construction`
    under all three test_runs conditions.

guidance: |
  Fixture authored inline for the under-construction-classification
  scaffold. Not a real story.

depends_on: []
"#;

/// Build a fresh `TempDir`-rooted `stories/` directory containing only
/// the under-construction fixture. Returns the tempdir (kept alive by
/// the caller) and the stories-dir path.
fn make_stories_dir() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{STORY_ID}.yml")),
        UNDER_CONSTRUCTION_FIXTURE,
    )
    .expect("write fixture");
    (tmp, stories_dir)
}

/// Construct a dashboard against a fresh `MemStore` seeded with the
/// caller-supplied rows. `test_run` is `None` for the "absent" case.
/// No `uat_signings.verdict=pass` row is ever seeded — the whole point
/// of this test.
fn render_with_seed(stories_dir: &Path, test_run: Option<Value>) -> String {
    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    if let Some(row) = test_run {
        store
            .upsert("test_runs", &STORY_ID.to_string(), row)
            .expect("seed test_runs row");
    }
    let dashboard = Dashboard::new(
        store.clone(),
        stories_dir.to_path_buf(),
        HEAD_SHA.to_string(),
    );
    dashboard
        .render_table()
        .expect("render_table should succeed on a single well-formed story")
}

fn assert_row_is_under_construction(scenario: &str, rendered: &str) {
    let row = rendered
        .lines()
        .find(|line| line.contains(&STORY_ID.to_string()))
        .unwrap_or_else(|| {
            panic!(
                "[{scenario}] rendered table must contain a row for story {STORY_ID}; got:\n{rendered}"
            )
        });
    assert!(
        row.contains("under_construction"),
        "[{scenario}] story {STORY_ID} must classify as `under_construction` \
         (never-passed means cannot fall from grace); got row: {row:?}\n\
         full table:\n{rendered}"
    );
    for bad in ["unhealthy", "healthy", "proposed", "error"] {
        assert!(
            !row.contains(bad),
            "[{scenario}] story {STORY_ID} classified as `{bad}` despite YAML \
             `status: under_construction` and no historical UAT pass; row: {row:?}\n\
             full table:\n{rendered}"
        );
    }
}

#[test]
fn never_passed_under_construction_story_classifies_under_construction_regardless_of_test_runs() {
    let (_tmp, stories_dir) = make_stories_dir();

    // Sub-case (a): latest test_runs is Pass.
    let rendered = render_with_seed(
        &stories_dir,
        Some(json!({
            "story_id": STORY_ID,
            "verdict": "pass",
            "commit": HEAD_SHA,
            "ran_at": "2026-04-19T00:00:00Z",
            "failing_tests": [],
        })),
    );
    assert_row_is_under_construction("test_runs=pass", &rendered);

    // Sub-case (b): latest test_runs is Fail.
    let rendered = render_with_seed(
        &stories_dir,
        Some(json!({
            "story_id": STORY_ID,
            "verdict": "fail",
            "commit": HEAD_SHA,
            "ran_at": "2026-04-19T00:00:00Z",
            "failing_tests": ["some_test.rs"],
        })),
    );
    assert_row_is_under_construction("test_runs=fail", &rendered);

    // Sub-case (c): no test_runs row at all.
    let rendered = render_with_seed(&stories_dir, None);
    assert_row_is_under_construction("test_runs=absent", &rendered);
}
