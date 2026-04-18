//! Story 3 acceptance test: the fell-from-grace classifier under both
//! triggers.
//!
//! Justification (from stories/3.yml): proves the fell-from-grace rule
//! under both triggers: (a) a story with a historical
//! `uat_signings.verdict=pass` whose latest `test_runs.verdict=fail`
//! renders as `unhealthy`, AND (b) a story with a historical UAT pass
//! whose latest UAT commit no longer equals HEAD also renders as
//! `unhealthy`. The `Failing tests` cell is populated from
//! `test_runs.failing_tests`. Without this the whole point of the
//! dashboard — surfacing was-healthy-now-broken — is missing.
//!
//! The scaffold runs the two triggers back-to-back in a single test:
//!   (a) historical UAT pass + latest test_runs=fail → `unhealthy`,
//!       `Failing tests` cell names the failing test basenames.
//!   (b) historical UAT pass at an old commit != HEAD + test_runs
//!       absent → `unhealthy`.
//! Red today is compile-red via the missing `agentic_dashboard` public
//! surface.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::json;
use tempfile::TempDir;

const STORY_ID: u32 = 9304;
const HEAD_SHA: &str = "cccccccccccccccccccccccccccccccccccccccc";
const OLD_SHA: &str = "dddddddddddddddddddddddddddddddddddddddd";

/// Fixture with YAML `status: unhealthy` — the dashboard classifier
/// reads YAML plus evidence and must compute `unhealthy` here; the YAML
/// status is consistent with the evidence, which is the normal state
/// once the dashboard has run once.  (A `status: healthy` YAML with
/// evidence that disagrees is the `error` case, covered separately.)
fn fixture_yaml(status: &str) -> String {
    format!(
        r#"id: {STORY_ID}
title: "A story whose evidence went red after it was signed"

outcome: |
  A fixture whose historical UAT pass is followed by either a failing
  test_runs row or a HEAD commit that no longer matches the UAT
  signing's commit. Both triggers must classify as `unhealthy`.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_unhealthy_fell_from_grace.rs
      justification: |
        Present so the fixture is schema-valid. The live test drives
        Dashboard::render_table against this YAML under two distinct
        fell-from-grace triggers.
  uat: |
    Render the table under each trigger; assert the health column reads
    `unhealthy` and the Failing tests cell names the basenames on the
    test_runs=fail trigger.

guidance: |
  Fixture authored inline for the fell-from-grace scaffold. Not a real
  story.

depends_on: []
"#
    )
}

fn make_stories(yaml: &str) -> (TempDir, PathBuf) {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(stories_dir.join(format!("{STORY_ID}.yml")), yaml)
        .expect("write fell-from-grace fixture");
    (tmp, stories_dir)
}

#[test]
fn fell_from_grace_classifies_unhealthy_under_both_triggers() {
    // -----------------------------------------------------------------
    // Trigger (a): historical UAT pass + latest test_runs=fail.
    // -----------------------------------------------------------------
    // YAML says `unhealthy` because by the time the dashboard is looking
    // at this story, the evidence has already gone red. The classifier
    // still needs to compute `unhealthy` from the evidence — the YAML
    // status is corroborating, not authoritative.
    let (_tmp_a, stories_a) = make_stories(&fixture_yaml("unhealthy"));
    let store_a: Arc<dyn Store> = Arc::new(MemStore::new());
    // Historical UAT pass at HEAD — story was signed healthy.
    store_a
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-00000000930a",
                "story_id": STORY_ID,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "signed_at": "2026-04-18T00:00:00Z",
            }),
        )
        .expect("seed historical UAT pass");
    // Latest test_runs is a Fail naming two basenames.
    store_a
        .upsert(
            "test_runs",
            &STORY_ID.to_string(),
            json!({
                "story_id": STORY_ID,
                "verdict": "fail",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": ["critical_path.rs", "regression_check.rs"],
            }),
        )
        .expect("seed failing test_runs row");

    let dashboard_a = Dashboard::new(store_a.clone(), stories_a.clone(), HEAD_SHA.to_string());
    let rendered_a = dashboard_a
        .render_table()
        .expect("render_table should succeed on trigger (a)");
    let row_a = rendered_a
        .lines()
        .find(|line| line.contains(&STORY_ID.to_string()))
        .unwrap_or_else(|| {
            panic!(
                "[trigger a] rendered table must contain a row for story {STORY_ID}; \
                 got:\n{rendered_a}"
            )
        });
    assert!(
        row_a.contains("unhealthy"),
        "[trigger a] story {STORY_ID} must classify as `unhealthy` when latest \
         test_runs is Fail after a historical UAT pass; got row: {row_a:?}\n\
         full table:\n{rendered_a}"
    );
    // Failing tests cell names the basenames of the failing test files.
    assert!(
        row_a.contains("critical_path.rs"),
        "[trigger a] Failing tests cell must name `critical_path.rs`; \
         got row: {row_a:?}"
    );
    assert!(
        row_a.contains("regression_check.rs"),
        "[trigger a] Failing tests cell must name `regression_check.rs`; \
         got row: {row_a:?}"
    );

    // -----------------------------------------------------------------
    // Trigger (b): historical UAT pass at OLD_SHA + test_runs absent.
    // -----------------------------------------------------------------
    let (_tmp_b, stories_b) = make_stories(&fixture_yaml("unhealthy"));
    let store_b: Arc<dyn Store> = Arc::new(MemStore::new());
    // Historical UAT pass at a commit that is NOT HEAD.
    store_b
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-00000000930b",
                "story_id": STORY_ID,
                "verdict": "pass",
                "commit": OLD_SHA,
                "signed_at": "2026-04-17T00:00:00Z",
            }),
        )
        .expect("seed historical UAT pass at OLD_SHA");
    // No test_runs row at all.

    let dashboard_b = Dashboard::new(store_b.clone(), stories_b.clone(), HEAD_SHA.to_string());
    let rendered_b = dashboard_b
        .render_table()
        .expect("render_table should succeed on trigger (b)");
    let row_b = rendered_b
        .lines()
        .find(|line| line.contains(&STORY_ID.to_string()))
        .unwrap_or_else(|| {
            panic!(
                "[trigger b] rendered table must contain a row for story {STORY_ID}; \
                 got:\n{rendered_b}"
            )
        });
    assert!(
        row_b.contains("unhealthy"),
        "[trigger b] story {STORY_ID} must classify as `unhealthy` when latest \
         UAT commit != HEAD and test_runs is absent; got row: {row_b:?}\n\
         full table:\n{rendered_b}"
    );
}
