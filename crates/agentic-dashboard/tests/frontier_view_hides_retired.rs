//! Story 3 acceptance test: the default (frontier) lens excludes
//! retired stories from both the rendered rows and the summary
//! denominators.
//!
//! Justification (from stories/3.yml): proves the default (frontier)
//! lens excludes retired stories: given a fixture corpus containing
//! at least one story with `status: retired` and one with
//! `status: healthy`, invoking `agentic stories health` with no mode
//! flag renders a table whose rows contain no entry for the retired
//! story, and whose summary counts at the foot exclude retired from
//! their denominators (retired is neither healthy nor unhealthy — it
//! is off-tree). Without this, the dashboard's frontier discipline
//! degrades as the corpus accumulates retired eras: the default view
//! becomes cluttered with stories that have been explicitly pruned,
//! defeating the lens's purpose and undoing the frontier filter
//! story 10 established for the non-retirement statuses.
//!
//! Story 10 cross-reference (2026-04-30 amendment). The original
//! fixture (retired + healthy-only) was paired with assertions that
//! the healthy stories render in the frontier — a contract
//! incompatible with story 10's healthy-exclusion rule. The
//! re-authored fixture adds a non-healthy (proposed) story so the
//! frontier has rows to render: this test asserts (a) the non-
//! healthy story renders, (b) the retired story does not, (c) the
//! healthy stories do not, and (d) the summary denominator excludes
//! retired. The retired-exclusion contract this test pins is
//! unchanged.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use agentic_story::Status;
use serde_json::{json, Value};
use tempfile::TempDir;

const HEAD_SHA: &str = "3030303030303030303030303030303030303030";

// A retired fossil + its healthy successor + one unrelated healthy
// story + one proposed story (the only frontier candidate after the
// filter excludes retired and healthy alongside story 10's rule).
const ID_RETIRED: u32 = 93101;
const ID_SUCCESSOR: u32 = 93102;
const ID_UNRELATED: u32 = 93103;
const ID_PROPOSED: u32 = 93104;

fn fixture(id: u32, status: &str, extra: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for frontier-hides-retired (story 3 amendment)"

outcome: |
  Fixture row for the frontier-hides-retired scaffold.

status: {status}
{extra}
patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/frontier_view_hides_retired.rs
      justification: |
        Present so the fixture is schema-valid; the live test drives
        Dashboard's frontier renderer against this file.
  uat: |
    Render frontier view; assert retired and healthy rows absent and
    summary denominators exclude retired.

guidance: |
  Fixture authored inline for the frontier-hides-retired scaffold.
  Not a real story.

depends_on: []
"#
    )
}

#[test]
fn default_frontier_lens_excludes_retired_stories_from_rows_and_summary_denominators() {
    // Compile-red anchor: `Status::Retired` must exist on the enum
    // for this test to compile. Until story 6's amendment lands the
    // new enum value, this is the natural compile-red edge.
    let _retired_variant_exists: Status = Status::Retired;

    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // Retired fossil pointing at a healthy successor — referentially
    // complete so the loader's supersession-edge validation accepts
    // the corpus.
    fs::write(
        stories_dir.join(format!("{ID_RETIRED}.yml")),
        fixture(
            ID_RETIRED,
            "retired",
            &format!(
                "\nsuperseded_by: {ID_SUCCESSOR}\nretired_reason: |\n  Folded into successor {ID_SUCCESSOR} for this scaffold's frontier check.\n"
            ),
        ),
    )
    .expect("write retired fixture");
    fs::write(
        stories_dir.join(format!("{ID_SUCCESSOR}.yml")),
        fixture(ID_SUCCESSOR, "healthy", ""),
    )
    .expect("write successor fixture");
    fs::write(
        stories_dir.join(format!("{ID_UNRELATED}.yml")),
        fixture(ID_UNRELATED, "healthy", ""),
    )
    .expect("write unrelated healthy fixture");
    // The proposed story is the only frontier candidate — story 10's
    // healthy-exclusion rule removes the two healthy stories, story
    // 21's retired-exclusion rule removes the retired one. Without
    // this row the rendered frontier would be empty and assertion (a)
    // would fire vacuously.
    fs::write(
        stories_dir.join(format!("{ID_PROPOSED}.yml")),
        fixture(ID_PROPOSED, "proposed", ""),
    )
    .expect("write proposed fixture");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed a passing UAT signing + passing test_runs at HEAD for the
    // two healthy stories so the classifier promotes them to
    // `healthy`. The proposed story (93104) gets no signing rows;
    // the retired story (93101) gets none.
    for id in [ID_SUCCESSOR, ID_UNRELATED] {
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
    // (a) Frontier JSON: stories[] ids (sorted ascending) must equal
    // exactly [ID_PROPOSED]. Retired (93101), and both healthy stories
    // (93102, 93103) must all be excluded — story 21's retired-exclusion
    // composes with story 10's healthy-exclusion.
    // ====================================================================
    let json_rendered = dashboard
        .render_frontier_json()
        .expect("render_frontier_json should succeed on a well-formed corpus");
    let parsed: Value = serde_json::from_str(&json_rendered).unwrap_or_else(|e| {
        panic!("frontier JSON must parse via serde_json::from_str: {e}; raw:\n{json_rendered}")
    });
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
         retired ({ID_RETIRED}), successor ({ID_SUCCESSOR}), and unrelated \
         healthy ({ID_UNRELATED}) must all be excluded. got sorted ids: {ids:?}\n\
         full JSON:\n{parsed}"
    );

    // ====================================================================
    // (b) Frontier table: a row must start with `{ID_PROPOSED} |`, AND
    // no row may start with any of the three excluded ids (93101, 93102,
    // 93103).
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

    for excluded_id in [ID_RETIRED, ID_SUCCESSOR, ID_UNRELATED] {
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
    // (c) Summary denominators must not credit retired — retired is
    // off-tree. The sum of the four frontier-status counts equals the
    // number of rendered rows (1 — the proposed story). With healthy
    // stories excluded from rendering by story 10's rule, summary.healthy
    // is 0; the retired contributes 0 to all four counts.
    // ====================================================================
    let summary = parsed.get("summary").unwrap_or_else(|| {
        panic!("frontier JSON must carry a top-level `summary` object; got: {parsed}")
    });
    let denom: u64 = ["healthy", "unhealthy", "proposed", "under_construction"]
        .iter()
        .map(|k| {
            summary.get(*k).and_then(|v| v.as_u64()).unwrap_or_else(|| {
                panic!("summary.{k} must be a non-negative integer; got {summary}")
            })
        })
        .sum();
    assert_eq!(
        denom as usize,
        stories.len(),
        "summary denominators must match the rendered frontier row count \
         (the proposed story is the only frontier-visible row); retired \
         must not inflate any count. summary={summary}, rendered_rows={}",
        stories.len()
    );
    assert_eq!(
        summary.get("healthy").and_then(|v| v.as_u64()),
        Some(0),
        "summary.healthy must be 0 because the two healthy stories \
         ({ID_SUCCESSOR}, {ID_UNRELATED}) are excluded from frontier \
         rendering by story 10's healthy-exclusion rule; got summary={summary}"
    );
}
