//! Story 10 acceptance test: frontier hides `healthy` stories.
//!
//! Justification (from stories/10.yml): proves the healthy-hidden
//! rule — given a fixture corpus with both `healthy` and not-healthy
//! stories, the default view emits zero rows for any `healthy` story
//! (neither in the table nor in the `--json` `stories[]` array).
//! Without this the dashboard grows healthy noise and the attention-
//! focused shape the frontier was built for is diluted to "a list
//! with most of the stories the operator does not need to look at
//! today."

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::{json, Value};
use tempfile::TempDir;

const HEAD_SHA: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const ID_HEALTHY: u32 = 91101;
const ID_PROPOSED: u32 = 91102;

fn fixture(id: u32, status: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for healthy-hidden scaffold"

outcome: |
  Fixture row for the healthy-hidden scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_frontier_default_hides_healthy_stories.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Run the dashboard frontier view; assert healthy rows absent.

guidance: |
  Fixture authored inline for the healthy-hidden scaffold. Not a real
  story.

depends_on: []
"#
    )
}

#[test]
fn frontier_default_hides_healthy_stories_in_both_table_and_json_rows() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{ID_HEALTHY}.yml")),
        fixture(ID_HEALTHY, "healthy"),
    )
    .expect("write healthy fixture");
    fs::write(
        stories_dir.join(format!("{ID_PROPOSED}.yml")),
        fixture(ID_PROPOSED, "proposed"),
    )
    .expect("write proposed fixture");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed healthy UAT + passing test_runs at HEAD so the healthy
    // classifier rule fires for ID_HEALTHY.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000091101",
                "story_id": ID_HEALTHY,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "signed_at": "2026-04-19T00:00:00Z",
            }),
        )
        .expect("seed healthy uat pass");
    store
        .upsert(
            "test_runs",
            &ID_HEALTHY.to_string(),
            json!({
                "story_id": ID_HEALTHY,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed healthy test_runs pass");

    let dashboard = Dashboard::new(store, stories_dir, HEAD_SHA.to_string());

    // (a) JSON: healthy id absent from stories[].
    let json_rendered = dashboard
        .render_frontier_json()
        .expect("render_frontier_json should succeed");
    let parsed: Value = serde_json::from_str(&json_rendered)
        .unwrap_or_else(|e| panic!("frontier JSON must parse: {e}; raw:\n{json_rendered}"));
    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));
    let ids: Vec<u64> = stories
        .iter()
        .filter_map(|s| s.get("id").and_then(|v| v.as_u64()))
        .collect();

    assert!(
        !ids.contains(&(ID_HEALTHY as u64)),
        "frontier JSON must NOT include the healthy story (id {ID_HEALTHY}); got ids: {ids:?}"
    );
    assert!(
        ids.contains(&(ID_PROPOSED as u64)),
        "frontier JSON must include the proposed story (id {ID_PROPOSED}); got ids: {ids:?}"
    );

    // (b) Table: healthy id must not appear as a row id either.
    let table_rendered = dashboard
        .render_frontier_table()
        .expect("render_frontier_table should succeed");
    // Require that no table row begins with the healthy id's digits +
    // the column separator. Checking the bare id substring would be
    // flaky if e.g. a title contained it, so we check per-line.
    let healthy_id_str = ID_HEALTHY.to_string();
    let healthy_row_present = table_rendered.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with(&format!("{healthy_id_str} |"))
            || trimmed.starts_with(&format!("{healthy_id_str}|"))
            || trimmed == healthy_id_str
    });
    assert!(
        !healthy_row_present,
        "frontier table must NOT include a row for the healthy story \
         (id {ID_HEALTHY}); got table:\n{table_rendered}"
    );
}
