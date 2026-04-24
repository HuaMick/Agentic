//! Story 3 acceptance test: the proposed-classification rule.
//!
//! Justification (from stories/3.yml): proves the proposed-classification
//! rule — a story whose YAML has `status: proposed` renders as `proposed`
//! in the table regardless of whether any `test_runs` or `uat_signings`
//! rows exist for it. Without this, recording a test result for a story
//! we haven't started would inadvertently promote it past the proposed
//! gate.
//!
//! The scaffold builds a `TempDir` `stories/` with exactly one story
//! whose YAML says `status: proposed`, seeds `MemStore` with BOTH a
//! `test_runs` row (a Pass, to make the "even if evidence exists" case
//! concrete) AND a `uat_signings.verdict=pass` row at HEAD, constructs a
//! `Dashboard` against that store, and asserts the rendered table
//! classifies the row as `proposed`. The YAML-says-proposed rule must
//! win over any evidence — that is the gate this test pins. Red today is
//! compile-red via the missing `agentic_dashboard` public surface
//! (`Dashboard`, `DashboardError`).

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::json;
use tempfile::TempDir;

const STORY_ID: u32 = 9301;
const HEAD_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

const PROPOSED_FIXTURE: &str = r#"id: 9301
title: "A story that hasn't started yet"

outcome: |
  A fixture whose YAML status remains `proposed`; the dashboard must
  honour the YAML regardless of any store evidence.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_proposed.rs
      justification: |
        Present so the fixture is schema-valid. The live test drives
        Dashboard::render_table against this YAML.
  uat: |
    Render the table; assert the health column reads `proposed` for this
    row.

guidance: |
  Fixture authored inline for the proposed-classification scaffold. Not a
  real story.

depends_on: []
"#;

#[test]
fn story_with_status_proposed_renders_as_proposed_even_when_evidence_exists() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{STORY_ID}.yml")),
        PROPOSED_FIXTURE,
    )
    .expect("write proposed fixture");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed BOTH evidence tables. Neither row should change the verdict:
    // YAML `status: proposed` must win over any signal in the store.
    store
        .upsert(
            "test_runs",
            &STORY_ID.to_string(),
            json!({
                "story_id": STORY_ID,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed test_runs row");
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000000001",
                "story_id": STORY_ID,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "signed_at": "2026-04-19T00:00:00Z",
            }),
        )
        .expect("seed uat_signings row");

    let dashboard = Dashboard::new(store.clone(), stories_dir.clone(), HEAD_SHA.to_string());

    let rendered = dashboard
        .render_table()
        .expect("render_table should succeed on a single well-formed story");

    // The rendered table must contain this story's id and classify it as
    // `proposed`. We assert the substring `proposed` appears on the same
    // row as the story id; the dashboard may format columns any way it
    // likes, but the health cell is lowercase per the --json contract.
    let row = rendered
        .lines()
        .find(|line| line.contains(&STORY_ID.to_string()))
        .unwrap_or_else(|| {
            panic!("rendered table must contain a row for story {STORY_ID}; got:\n{rendered}")
        });
    assert!(
        row.contains("proposed"),
        "story {STORY_ID}'s row must classify as `proposed` (YAML wins over evidence); \
         got row: {row:?}\nfull table:\n{rendered}"
    );
    // Negative: evidence must not have bumped this into any other class.
    for bad in ["healthy", "unhealthy", "under_construction", "error"] {
        assert!(
            !row.contains(bad),
            "story {STORY_ID} classified as `{bad}` despite YAML `status: proposed`; \
             row: {row:?}\nfull table:\n{rendered}"
        );
    }
}
