//! Story 3 acceptance test: row sort order, title truncation, and
//! Failing-tests-cell emptiness for benign classifications.
//!
//! Justification (from stories/3.yml): proves rendering correctness
//! independent of classification: rows sort `unhealthy` →
//! `under_construction` → `proposed` → `healthy`, titles longer than
//! the truncation limit (~35 chars) are truncated with an ellipsis, and
//! the `Failing tests` cell is empty for `proposed` and `healthy` rows.
//! Without this the most-important rows (unhealthy) can sink below noise
//! and long titles can break terminal layouts.
//!
//! The scaffold materialises one story per classification, writes
//! fixture YAMLs with deliberately long titles so the truncation
//! behaviour is exercised, seeds `MemStore` so each story's evidence
//! produces its intended classification, and then parses the rendered
//! table to assert (a) row order, (b) single-char ellipsis truncation
//! with the `…` U+2026 character (not three dots), and (c) empty
//! Failing tests cell for `proposed` and `healthy` rows.
//! Red today is compile-red via the missing `agentic_dashboard` public
//! surface.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::json;
use tempfile::TempDir;

const HEAD_SHA: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const OLD_SHA: &str = "0000000000000000000000000000000000000000";

const ID_UNHEALTHY: u32 = 9401;
const ID_UC: u32 = 9402;
const ID_PROPOSED: u32 = 9403;
const ID_HEALTHY: u32 = 9404;

// A 60-character title — definitely over the ~35-char truncation limit
// the story names. The renderer must truncate this to some prefix
// followed by a single U+2026 ellipsis.
const LONG_TITLE_UNHEALTHY: &str = "A very long title that exceeds thirty five characters easily";

fn fixture(id: u32, title: &str, status: &str) -> String {
    format!(
        r#"id: {id}
title: "{title}"

outcome: |
  Fixture row for the sort-and-truncation scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_sort_and_truncation.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Render the table; assert sort order and truncation behaviour.

guidance: |
  Fixture authored inline for the sort-and-truncation scaffold. Not a
  real story.

depends_on: []
"#
    )
}

#[test]
fn rows_sort_unhealthy_under_construction_proposed_healthy_and_long_titles_truncate_with_single_ellipsis(
) {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // Four fixtures, one per classification. The unhealthy story gets
    // the deliberately long title so truncation behaviour is observable
    // on that row.
    fs::write(
        stories_dir.join(format!("{ID_UNHEALTHY}.yml")),
        fixture(ID_UNHEALTHY, LONG_TITLE_UNHEALTHY, "unhealthy"),
    )
    .expect("write unhealthy fixture");
    fs::write(
        stories_dir.join(format!("{ID_UC}.yml")),
        fixture(ID_UC, "Story in flight", "under_construction"),
    )
    .expect("write under_construction fixture");
    fs::write(
        stories_dir.join(format!("{ID_PROPOSED}.yml")),
        fixture(ID_PROPOSED, "Story not yet started", "proposed"),
    )
    .expect("write proposed fixture");
    fs::write(
        stories_dir.join(format!("{ID_HEALTHY}.yml")),
        fixture(ID_HEALTHY, "Story signed at HEAD", "healthy"),
    )
    .expect("write healthy fixture");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Unhealthy: historical UAT pass + latest test_runs fail.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000009401",
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
                "id": "01900000-0000-7000-8000-000000009404",
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

    // UC and Proposed: no evidence seeded — YAML drives classification.

    let dashboard = Dashboard::new(store.clone(), stories_dir.clone(), HEAD_SHA.to_string());
    let rendered = dashboard
        .render_table()
        .expect("render_table should succeed on four well-formed stories");

    // Find the row line for each story id. Each story's row must exist.
    let row_for = |id: u32| -> (usize, &str) {
        let (idx, line) = rendered
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains(&id.to_string()))
            .unwrap_or_else(|| {
                panic!("rendered table must contain a row for story {id}; got:\n{rendered}")
            });
        (idx, line)
    };

    let (idx_unhealthy, row_unhealthy) = row_for(ID_UNHEALTHY);
    let (idx_uc, row_uc) = row_for(ID_UC);
    let (idx_proposed, row_proposed) = row_for(ID_PROPOSED);
    let (idx_healthy, row_healthy) = row_for(ID_HEALTHY);

    // Sort order: unhealthy < under_construction < proposed < healthy.
    assert!(
        idx_unhealthy < idx_uc,
        "unhealthy row (idx {idx_unhealthy}) must precede under_construction row \
         (idx {idx_uc}); rendered:\n{rendered}"
    );
    assert!(
        idx_uc < idx_proposed,
        "under_construction row (idx {idx_uc}) must precede proposed row \
         (idx {idx_proposed}); rendered:\n{rendered}"
    );
    assert!(
        idx_proposed < idx_healthy,
        "proposed row (idx {idx_proposed}) must precede healthy row \
         (idx {idx_healthy}); rendered:\n{rendered}"
    );

    // Classification sanity per row (defence in depth against a
    // same-substring bug in the sort assertions above).
    assert!(
        row_unhealthy.contains("unhealthy"),
        "unhealthy row must carry `unhealthy` health; got: {row_unhealthy:?}"
    );
    assert!(
        row_uc.contains("under_construction"),
        "uc row must carry `under_construction` health; got: {row_uc:?}"
    );
    assert!(
        row_proposed.contains("proposed"),
        "proposed row must carry `proposed` health; got: {row_proposed:?}"
    );
    assert!(
        row_healthy.contains("healthy") && !row_healthy.contains("unhealthy"),
        "healthy row must carry `healthy` (not `unhealthy`); got: {row_healthy:?}"
    );

    // Long title on the unhealthy row must be truncated with a single
    // U+2026 ellipsis, not three ASCII dots.
    assert!(
        row_unhealthy.contains('\u{2026}'),
        "long title must be truncated with the U+2026 ellipsis character `…`; \
         got row: {row_unhealthy:?}"
    );
    assert!(
        !row_unhealthy.contains("..."),
        "truncation must use the single-char ellipsis `…`, NOT three ASCII dots; \
         got row: {row_unhealthy:?}"
    );
    // The full long title must not appear verbatim — if it did, the
    // renderer failed to truncate.
    assert!(
        !row_unhealthy.contains(LONG_TITLE_UNHEALTHY),
        "long title appeared untruncated in the row; got: {row_unhealthy:?}"
    );

    // Failing tests cell is empty for proposed and healthy rows.
    // We approximate this by asserting no plausible test-file basename
    // (anything ending `.rs`) appears on those rows.
    assert!(
        !row_proposed.contains(".rs"),
        "Failing tests cell must be empty for `proposed` rows; \
         got row: {row_proposed:?}"
    );
    assert!(
        !row_healthy.contains(".rs"),
        "Failing tests cell must be empty for `healthy` rows; \
         got row: {row_healthy:?}"
    );
}
