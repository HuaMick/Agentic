//! Story 21 acceptance test: the default frontier view hides
//! retired stories and excludes them from summary denominators.
//!
//! Justification (from stories/21.yml):
//! Proves the frontier lens excludes retired stories: given a
//! corpus containing one story with `status: retired` and one with
//! `status: healthy`, the default `agentic stories health`
//! rendering (no mode flag) emits a table containing exactly the
//! healthy story's row and no row for the retired story. The
//! summary counts at the table's foot also exclude retired from
//! their denominators (retired is neither healthy nor unhealthy —
//! it is off-tree).
//!
//! Red today is compile-red: the `Status::Retired` variant does
//! not yet exist on the `agentic_story::Status` enum (it lands in
//! story 6's amendment pass bundled with this story's schema
//! additions), so constructing fixture YAML with `status: retired`
//! fails either at load time (as `UnknownStatus`) or — once story
//! 6 ships the enum value — surfaces as a live rendering test that
//! will run runtime-red until the dashboard's frontier filter
//! drops retired rows.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use agentic_story::Status;
use serde_json::{json, Value};
use tempfile::TempDir;

const HEAD_SHA: &str = "2110110110110110110110110110110110110110";
const ID_RETIRED: u32 = 92101;
const ID_HEALTHY: u32 = 92102;
const ID_SUCCESSOR: u32 = 92103;

fn fixture(id: u32, status: &str, extra: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for frontier-hides-retired scaffold"

outcome: |
  Fixture row for the frontier-hides-retired scaffold.

status: {status}
{extra}
patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_frontier_hides_retired.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Render frontier view; retired story absent, healthy present.

guidance: |
  Fixture authored inline for the frontier-hides-retired scaffold.
  Not a real story.

depends_on: []
"#
    )
}

#[test]
fn frontier_default_hides_retired_stories_in_rows_and_summary_denominators() {
    // Cross-reference: Status::Retired must exist on the enum for
    // this test to compile. Until story 6's amendment adds it, the
    // line below is the natural compile-red edge.
    let _retired_variant_exists: Status = Status::Retired;

    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // The retired fossil points at a successor (ID_SUCCESSOR) so
    // the corpus is referentially complete — the loader's
    // supersession-edge validation is not the subject here but
    // must not reject this fixture either.
    fs::write(
        stories_dir.join(format!("{ID_RETIRED}.yml")),
        fixture(
            ID_RETIRED,
            "retired",
            &format!("\nsuperseded_by: {ID_SUCCESSOR}\nretired_reason: |\n  Retired because successor {ID_SUCCESSOR} inherited the contract.\n"),
        ),
    )
    .expect("write retired fixture");
    fs::write(
        stories_dir.join(format!("{ID_HEALTHY}.yml")),
        fixture(ID_HEALTHY, "healthy", ""),
    )
    .expect("write healthy fixture");
    fs::write(
        stories_dir.join(format!("{ID_SUCCESSOR}.yml")),
        fixture(ID_SUCCESSOR, "healthy", ""),
    )
    .expect("write successor fixture");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed healthy UAT + passing test_runs at HEAD for both
    // currently-healthy stories so the classifier accepts them.
    for id in [ID_HEALTHY, ID_SUCCESSOR] {
        store
            .append(
                "uat_signings",
                json!({
                    "id": format!("01900000-0000-7000-8000-0000000{id}"),
                    "story_id": id,
                    "verdict": "pass",
                    "commit": HEAD_SHA,
                    "signed_at": "2026-04-23T00:00:00Z",
                }),
            )
            .expect("seed uat pass");
        store
            .upsert(
                "test_runs",
                &id.to_string(),
                json!({
                    "story_id": id,
                    "verdict": "pass",
                    "commit": HEAD_SHA,
                    "ran_at": "2026-04-23T00:00:00Z",
                    "failing_tests": [],
                }),
            )
            .expect("seed test_runs pass");
    }

    let dashboard = Dashboard::new(store, stories_dir, HEAD_SHA.to_string());

    // (a) JSON frontier: retired id absent from stories[].
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
        !ids.contains(&(ID_RETIRED as u64)),
        "frontier JSON must NOT include the retired story (id {ID_RETIRED}); got ids: {ids:?}"
    );

    // (b) Summary counts at the table foot do not credit retired
    // to any denominator — retired is off-tree. The summary shape
    // is `summary.healthy`, `summary.unhealthy`, `summary.proposed`,
    // `summary.under_construction`; the sum of those four must
    // equal the number of rows rendered in the frontier array,
    // never `rows + retired_count`.
    let summary = parsed
        .get("summary")
        .unwrap_or_else(|| panic!("frontier JSON must carry top-level `summary`; got: {parsed}"));
    let denom: u64 = ["healthy", "unhealthy", "proposed", "under_construction"]
        .iter()
        .map(|k| {
            summary
                .get(*k)
                .and_then(|v| v.as_u64())
                .unwrap_or_else(|| panic!("summary.{k} must be a number; got {summary}"))
        })
        .sum();
    assert_eq!(
        denom,
        stories.len() as u64,
        "summary denominators must match the frontier row count; \
         retired stories must not inflate any count. summary={summary}, \
         rendered_rows={}",
        stories.len()
    );

    // (c) Table frontier: retired id must not appear as a row id.
    let table_rendered = dashboard
        .render_frontier_table()
        .expect("render_frontier_table should succeed");
    let retired_id_str = ID_RETIRED.to_string();
    let retired_row_present = table_rendered.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with(&format!("{retired_id_str} |"))
            || trimmed.starts_with(&format!("{retired_id_str}|"))
            || trimmed == retired_id_str
    });
    assert!(
        !retired_row_present,
        "frontier table must NOT include a row for the retired story \
         (id {ID_RETIRED}); got table:\n{table_rendered}"
    );
}
