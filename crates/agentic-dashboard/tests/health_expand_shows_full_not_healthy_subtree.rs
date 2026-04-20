//! Story 10 acceptance test: `--expand` shows the full not-healthy
//! subtree.
//!
//! Justification (from stories/10.yml): proves `--expand` semantics
//! — running the dashboard with `--expand` against the frontier
//! fixture from `health_frontier_default_hides_descendants_of_not_healthy`
//! emits rows for EVERY not-healthy story (frontier + descendants of
//! frontier stories that are themselves not healthy), NOT only the
//! frontier. Healthy stories remain hidden. Without this, an operator
//! diagnosing "why is my frontier root deep" has no view between
//! "frontier only" and "everything."
//!
//! Fixture:
//!   A: proposed (frontier root)
//!   B: under_construction, depends_on=[A]  (would be hidden by frontier filter)
//!   H: healthy (must remain hidden in --expand too)

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::{json, Value};
use tempfile::TempDir;

const HEAD_SHA: &str = "1111111111111111111111111111111111111111";

const ID_A: u32 = 91501;
const ID_B: u32 = 91502;
const ID_HEALTHY: u32 = 91503;

fn fixture(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for expand-not-healthy-subtree scaffold"

outcome: |
  Fixture row for the expand-not-healthy-subtree scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_expand_shows_full_not_healthy_subtree.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Render expand JSON; assert frontier + descendants, no healthy.

guidance: |
  Fixture authored inline for the expand-not-healthy-subtree scaffold.
  Not a real story.

{deps_yaml}
"#
    )
}

#[test]
fn expand_shows_every_not_healthy_story_and_keeps_healthy_hidden() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    fs::write(
        stories_dir.join(format!("{ID_A}.yml")),
        fixture(ID_A, "proposed", &[]),
    )
    .expect("write A");
    fs::write(
        stories_dir.join(format!("{ID_B}.yml")),
        fixture(ID_B, "under_construction", &[ID_A]),
    )
    .expect("write B depends_on=[A]");
    fs::write(
        stories_dir.join(format!("{ID_HEALTHY}.yml")),
        fixture(ID_HEALTHY, "healthy", &[]),
    )
    .expect("write healthy");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed healthy UAT + passing test_runs so ID_HEALTHY classifies
    // `healthy`.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000091503",
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
    let rendered = dashboard
        .render_expand_json()
        .expect("render_expand_json should succeed");

    let parsed: Value = serde_json::from_str(&rendered)
        .unwrap_or_else(|e| panic!("expand JSON must parse: {e}; raw:\n{rendered}"));

    // Top-level `view` key documents the projection.
    let view = parsed
        .get("view")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("expand JSON must carry top-level `view`; got: {parsed}"));
    assert_eq!(
        view, "expand",
        "expand JSON must advertise view=\"expand\"; got {view:?}"
    );

    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));
    let ids: Vec<u64> = stories
        .iter()
        .filter_map(|s| s.get("id").and_then(|v| v.as_u64()))
        .collect();

    // Both not-healthy stories present.
    assert!(
        ids.contains(&(ID_A as u64)),
        "expand must include frontier root A (id {ID_A}); got ids: {ids:?}"
    );
    assert!(
        ids.contains(&(ID_B as u64)),
        "expand must include descendant-of-not-healthy B (id {ID_B}) — that is \
         the distinguishing feature vs the frontier view; got ids: {ids:?}"
    );
    // Healthy story hidden.
    assert!(
        !ids.contains(&(ID_HEALTHY as u64)),
        "expand must NOT include the healthy story (id {ID_HEALTHY}); got ids: {ids:?}"
    );
}
