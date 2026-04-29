//! Story 21 acceptance test: the default frontier view hides BOTH
//! retired and healthy stories, and excludes retired from summary
//! denominators. This pins both pruning invariants in one fixture —
//! story 21's retired-exclusion AND story 10's healthy-exclusion
//! (cross-referenced below).
//!
//! Justification (from stories/21.yml):
//! Proves the frontier lens excludes retired stories alongside the
//! healthy-exclusion rule story 10 owns: given a corpus containing
//! one `proposed` story (the only frontier candidate), one
//! `healthy` story, and one `retired (superseded_by: <successor>)`
//! story (with its successor present and healthy), the default
//! `agentic stories health` rendering (no mode flag) emits a table
//! containing exactly the proposed story's row, with NO row for the
//! retired story AND NO row for the healthy stories. The retired-
//! exclusion rule composes with story 10's healthy-exclusion rule —
//! both must hold. The summary counts at the table's foot also
//! exclude retired from their denominators (retired is neither
//! healthy nor unhealthy — it is off-tree).
//!
//! Cross-reference: this test composes story 21's retired-exclusion
//! invariant with story 10's healthy-hidden invariant
//! (`crates/agentic-dashboard/tests/health_frontier_default_hides_healthy_stories.rs`).
//! Both rules must hold simultaneously in the rendered frontier view.

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
const ID_PROPOSED: u32 = 92104;

fn fixture(id: u32, status: &str, extra: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for frontier-hides-retired-and-healthy scaffold"

outcome: |
  Fixture row for the frontier-hides-retired-and-healthy scaffold.

status: {status}
{extra}
patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_frontier_hides_retired.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Render frontier view; retired and healthy absent, proposed present.

guidance: |
  Fixture authored inline for the frontier-hides-retired-and-healthy
  scaffold. Not a real story.

depends_on: []
"#
    )
}

#[test]
fn frontier_default_hides_retired_and_healthy_stories_in_rows_and_summary_denominators() {
    // Cross-reference: Status::Retired must exist on the enum for
    // this test to compile. The line below is the compile-red anchor
    // guarding against future enum regressions.
    let _retired_variant_exists: Status = Status::Retired;

    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // Three-story fixture that exercises BOTH pruning invariants:
    //   - 92101 retired -> 92103 (off-tree, must be excluded)
    //   - 92102 healthy  (story 10 healthy-exclusion, must be excluded)
    //   - 92103 healthy  (the successor of 92101, must be excluded)
    //   - 92104 proposed (the only frontier candidate)
    fs::write(
        stories_dir.join(format!("{ID_RETIRED}.yml")),
        fixture(
            ID_RETIRED,
            "retired",
            &format!(
                "\nsuperseded_by: {ID_SUCCESSOR}\nretired_reason: |\n  Retired because successor {ID_SUCCESSOR} inherited the contract.\n"
            ),
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
    fs::write(
        stories_dir.join(format!("{ID_PROPOSED}.yml")),
        fixture(ID_PROPOSED, "proposed", ""),
    )
    .expect("write proposed fixture");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed UAT pass + test_runs pass at HEAD for the two healthy
    // stories (92102 and 92103) only. 92104 (proposed) gets no
    // signing rows; 92101 (retired) gets no signing rows.
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

    // ====================================================================
    // (a) JSON frontier: stories[] ids (sorted ascending) must equal
    // exactly [ID_PROPOSED]. Neither ID_RETIRED nor ID_HEALTHY nor
    // ID_SUCCESSOR may appear.
    // ====================================================================
    let json_rendered = dashboard
        .render_frontier_json()
        .expect("render_frontier_json should succeed");
    let parsed: Value = serde_json::from_str(&json_rendered)
        .unwrap_or_else(|e| panic!("frontier JSON must parse: {e}; raw:\n{json_rendered}"));
    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));
    let mut ids: Vec<u64> = stories
        .iter()
        .filter_map(|s| s.get("id").and_then(|v| v.as_u64()))
        .collect();
    ids.sort();

    assert_eq!(
        ids,
        vec![ID_PROPOSED as u64],
        "frontier JSON must contain exactly the proposed id ({ID_PROPOSED}); \
         retired ({ID_RETIRED}), healthy ({ID_HEALTHY}), and successor \
         ({ID_SUCCESSOR}) must all be excluded. got sorted ids: {ids:?}"
    );

    // ====================================================================
    // (b) Frontier table: a row must start with `{ID_PROPOSED} |` (or
    // {ID_PROPOSED}| or {ID_PROPOSED}<separator>), AND no row may start
    // with the three excluded ids (92101, 92102, 92103).
    // ====================================================================
    let table_rendered = dashboard
        .render_frontier_table()
        .expect("render_frontier_table should succeed");

    let proposed_id_str = ID_PROPOSED.to_string();
    let proposed_row_present = table_rendered.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with(&format!("{proposed_id_str} |"))
            || trimmed.starts_with(&format!("{proposed_id_str}|"))
            || trimmed == proposed_id_str
    });
    assert!(
        proposed_row_present,
        "frontier table must include a row for the proposed story \
         (id {ID_PROPOSED}); got table:\n{table_rendered}"
    );

    for excluded_id in [ID_RETIRED, ID_HEALTHY, ID_SUCCESSOR] {
        let excluded_id_str = excluded_id.to_string();
        let excluded_row_present = table_rendered.lines().any(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with(&format!("{excluded_id_str} |"))
                || trimmed.starts_with(&format!("{excluded_id_str}|"))
                || trimmed == excluded_id_str
        });
        assert!(
            !excluded_row_present,
            "frontier table must NOT include a row for excluded story \
             (id {excluded_id}); got table:\n{table_rendered}"
        );
    }

    // ====================================================================
    // (c) Summary denominator: the four-status denominator
    // (healthy + unhealthy + proposed + under_construction) must match
    // the rendered frontier row count — retired must not inflate any
    // count. With healthy and retired both excluded from rendering,
    // only the proposed row remains.
    // ====================================================================
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
        denom as usize,
        stories.len(),
        "summary denominators must match the rendered frontier row count; \
         retired stories must not inflate any count. summary={summary}, \
         rendered_rows={}",
        stories.len()
    );
}
