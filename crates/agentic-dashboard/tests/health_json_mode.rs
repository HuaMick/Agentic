//! Story 3 acceptance test: the `--json` contract.
//!
//! Justification (from stories/3.yml): proves the `--json` contract:
//! emits an object with `stories[]` and `summary{}` keys, includes FULL
//! commit SHAs (not truncated), preserves the same data as the table
//! mode, and is parseable by `serde_json` round-trip. Without this
//! downstream tooling (CI status checks, future web UI) cannot consume
//! the dashboard programmatically without scraping the TTY output.
//!
//! The scaffold builds the same four-status corpus as the sort-and-
//! truncation scaffold, calls `Dashboard::render_json()`, and asserts:
//!   1. The output parses via `serde_json::from_str`.
//!   2. Top-level has `stories` (array) and `summary` (object).
//!   3. Every story's `uat_commit` / `test_run_commit` field, when
//!      present, is a FULL 40-char hex SHA — not the table's truncated
//!      7-char form.
//!   4. Absent fields are emitted as JSON null (not omitted), so
//!      consumers do not have to distinguish "missing key" from "key
//!      with null value".
//!   5. `summary` counts add up to `stories.len()`.
//! Red today is compile-red via the missing `agentic_dashboard` public
//! surface.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::{json, Value};
use tempfile::TempDir;

const HEAD_SHA: &str = "ffffffffffffffffffffffffffffffffffffffff";
const OLD_SHA: &str = "1111111111111111111111111111111111111111";

const ID_UNHEALTHY: u32 = 9501;
const ID_UC: u32 = 9502;
const ID_PROPOSED: u32 = 9503;
const ID_HEALTHY: u32 = 9504;

fn fixture(id: u32, status: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for JSON-mode"

outcome: |
  Fixture row for the JSON-mode scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_json_mode.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Render --json; assert the contract.

guidance: |
  Fixture authored inline for the JSON-mode scaffold. Not a real story.

depends_on: []
"#
    )
}

#[test]
fn render_json_emits_stories_and_summary_with_full_shas_and_null_absent_fields() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{ID_UNHEALTHY}.yml")),
        fixture(ID_UNHEALTHY, "unhealthy"),
    )
    .expect("write unhealthy fixture");
    fs::write(
        stories_dir.join(format!("{ID_UC}.yml")),
        fixture(ID_UC, "under_construction"),
    )
    .expect("write uc fixture");
    fs::write(
        stories_dir.join(format!("{ID_PROPOSED}.yml")),
        fixture(ID_PROPOSED, "proposed"),
    )
    .expect("write proposed fixture");
    fs::write(
        stories_dir.join(format!("{ID_HEALTHY}.yml")),
        fixture(ID_HEALTHY, "healthy"),
    )
    .expect("write healthy fixture");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Unhealthy: historical UAT pass at OLD_SHA + failing test_runs at HEAD.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000009501",
                "story_id": ID_UNHEALTHY,
                "verdict": "pass",
                "commit": OLD_SHA,
                "signed_at": "2026-04-18T00:00:00Z",
            }),
        )
        .expect("seed unhealthy uat pass");
    store
        .upsert(
            "test_runs",
            &ID_UNHEALTHY.to_string(),
            json!({
                "story_id": ID_UNHEALTHY,
                "verdict": "fail",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": ["broken.rs"],
            }),
        )
        .expect("seed unhealthy test_runs fail");

    // Healthy: UAT pass at HEAD + test_runs pass.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000009504",
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

    // UC and Proposed have no evidence — the `uat_*` / `test_run_*`
    // fields on their JSON rows must therefore appear as JSON null.

    let dashboard = Dashboard::new(store.clone(), stories_dir.clone(), HEAD_SHA.to_string());
    let rendered = dashboard
        .render_json()
        .expect("render_json should succeed on four well-formed stories");

    // (1) Round-trips through serde_json.
    let parsed: Value = serde_json::from_str(&rendered)
        .unwrap_or_else(|e| panic!("render_json output must parse as JSON: {e}; raw:\n{rendered}"));

    // (2) Top-level shape.
    let obj = parsed
        .as_object()
        .unwrap_or_else(|| panic!("top-level JSON must be an object; got: {parsed}"));
    let stories = obj
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level must have `stories` as an array; got: {parsed}"));
    let summary = obj
        .get("summary")
        .and_then(|v| v.as_object())
        .unwrap_or_else(|| panic!("top-level must have `summary` as an object; got: {parsed}"));

    assert_eq!(
        stories.len(),
        4,
        "stories array must contain one entry per fixture (4); got {}: {parsed}",
        stories.len()
    );

    // Helper: fetch the story entry by id.
    let find_story = |id: u32| -> &Value {
        stories
            .iter()
            .find(|s| s.get("id").and_then(|v| v.as_u64()) == Some(id as u64))
            .unwrap_or_else(|| panic!("stories[] must include an entry for id {id}; got: {parsed}"))
    };

    // (3) Full 40-char SHAs on present commit fields.
    let healthy = find_story(ID_HEALTHY);
    let healthy_uat_commit = healthy
        .get("uat_commit")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("healthy row must carry a string uat_commit; got: {healthy}"));
    assert_eq!(
        healthy_uat_commit, HEAD_SHA,
        "healthy row uat_commit must be the FULL 40-char SHA; got {healthy_uat_commit:?}"
    );
    assert_eq!(
        healthy_uat_commit.len(),
        40,
        "uat_commit must be 40 chars in JSON mode (full SHA, not short); got {healthy_uat_commit:?}"
    );

    let unhealthy = find_story(ID_UNHEALTHY);
    let unhealthy_uat_commit = unhealthy
        .get("uat_commit")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            panic!("unhealthy row must carry a string uat_commit; got: {unhealthy}")
        });
    assert_eq!(
        unhealthy_uat_commit, OLD_SHA,
        "unhealthy row uat_commit must be OLD_SHA (full 40 chars); got {unhealthy_uat_commit:?}"
    );

    // (4) Null-able fields emitted as JSON null when absent, not omitted.
    let proposed_row = find_story(ID_PROPOSED);
    for field in [
        "uat_commit",
        "uat_signed_at",
        "test_run_commit",
        "test_run_at",
    ] {
        let v = proposed_row.get(field).unwrap_or_else(|| {
            panic!(
                "proposed row must EMIT `{field}` as JSON null (not omit it); got: {proposed_row}"
            )
        });
        assert!(
            v.is_null(),
            "proposed row `{field}` must be JSON null when absent; got {v}"
        );
    }
    let uc_row = find_story(ID_UC);
    for field in [
        "uat_commit",
        "uat_signed_at",
        "test_run_commit",
        "test_run_at",
    ] {
        let v = uc_row.get(field).unwrap_or_else(|| {
            panic!(
                "under_construction row must EMIT `{field}` as JSON null (not omit it); \
                 got: {uc_row}"
            )
        });
        assert!(
            v.is_null(),
            "under_construction row `{field}` must be JSON null when absent; got {v}"
        );
    }

    // (5) Summary counts add up to stories.len().
    let counted: u64 = ["healthy", "unhealthy", "under_construction", "proposed", "error"]
        .iter()
        .map(|k| {
            summary
                .get(*k)
                .and_then(|v| v.as_u64())
                .unwrap_or_else(|| panic!("summary.{k} must be an unsigned integer; got: {summary:?}"))
        })
        .sum();
    assert_eq!(
        counted,
        stories.len() as u64,
        "summary counts (healthy+unhealthy+under_construction+proposed+error = {counted}) \
         must sum to stories.len() ({}); summary: {summary:?}",
        stories.len()
    );
}
