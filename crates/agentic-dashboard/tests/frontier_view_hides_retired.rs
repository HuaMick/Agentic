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
//! Red today is compile-red: the `Status::Retired` variant does not
//! yet exist on `agentic_story::Status` — it lands in story 6's
//! amendment pass bundled with this story's retirement-lifecycle
//! additions (see stories/21.yml "Schema edit coordination"). The
//! frontier renderer already exists as `render_frontier_table` and
//! `render_frontier_json`; this scaffold pins the retirement-aware
//! filter once the enum value is available.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use agentic_story::Status;
use serde_json::{json, Value};
use tempfile::TempDir;

const HEAD_SHA: &str = "3030303030303030303030303030303030303030";

// A retired fossil + its healthy successor + one unrelated healthy
// story. The frontier view must show only the two healthy stories,
// and the summary denominators must exclude the retired story.
const ID_RETIRED: u32 = 93101;
const ID_SUCCESSOR: u32 = 93102;
const ID_UNRELATED: u32 = 93103;

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
    Render frontier view; assert retired row absent and summary
    denominators exclude retired.

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

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed a passing UAT signing + passing test_runs at HEAD for both
    // healthy stories so the classifier promotes them to `healthy`.
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

    // (a) Frontier JSON: retired id must not appear in stories[].
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
    let ids: Vec<u64> = stories
        .iter()
        .filter_map(|s| s.get("id").and_then(|v| v.as_u64()))
        .collect();
    assert!(
        !ids.contains(&(ID_RETIRED as u64)),
        "frontier JSON must NOT include retired story id {ID_RETIRED}; got ids: {ids:?}\n\
         full JSON:\n{parsed}"
    );

    // (b) Summary denominators must not credit retired — retired is
    // off-tree. The sum of the four frontier-status counts must equal
    // the number of rows rendered, never `rows + retired_count`.
    let summary = parsed.get("summary").unwrap_or_else(|| {
        panic!("frontier JSON must carry a top-level `summary` object; got: {parsed}")
    });
    let denom: u64 = ["healthy", "unhealthy", "proposed", "under_construction"]
        .iter()
        .map(|k| {
            summary
                .get(*k)
                .and_then(|v| v.as_u64())
                .unwrap_or_else(|| panic!("summary.{k} must be a non-negative integer; got {summary}"))
        })
        .sum();
    assert_eq!(
        denom,
        stories.len() as u64,
        "summary denominators must match frontier row count; retired must not inflate any \
         count. summary={summary}, rendered_rows={}",
        stories.len()
    );

    // (c) Frontier table: the retired id must not appear as a row id.
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
        "frontier table must NOT include a row for retired story {ID_RETIRED}; \
         got table:\n{table_rendered}"
    );

    // (d) Positive complement: both healthy stories DO appear in the
    // frontier output — without this, the "no retired row" assertion
    // could pass vacuously if the frontier renderer returned an empty
    // table for unrelated reasons.
    for expected in [ID_SUCCESSOR, ID_UNRELATED] {
        assert!(
            ids.contains(&(expected as u64)),
            "frontier JSON must include healthy story id {expected}; got ids: {ids:?}\n\
             full JSON:\n{parsed}"
        );
    }
}
