//! Story 10 acceptance test: frontier shows `proposed` with healthy
//! (or empty) ancestry.
//!
//! Justification (from stories/10.yml): proves the status-range rule
//! — a story whose status is `proposed` and whose ancestry is
//! entirely `healthy` (or empty) appears in the default view. Without
//! this, new work that has not yet been started is invisible until it
//! enters `under_construction` — the classic "my roadmap is not in my
//! dashboard" gap the frontier view is supposed to close.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::{json, Value};
use tempfile::TempDir;

const HEAD_SHA: &str = "cccccccccccccccccccccccccccccccccccccccc";
const ID_HEALTHY: u32 = 91201;
const ID_PROPOSED_DESC: u32 = 91202;
const ID_PROPOSED_STANDALONE: u32 = 91203;

fn fixture(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for proposed-on-frontier scaffold"

outcome: |
  Fixture row for the proposed-on-frontier scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_frontier_default_shows_proposed.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Run the dashboard frontier view; assert proposed rows appear.

guidance: |
  Fixture authored inline for the proposed-on-frontier scaffold. Not a
  real story.

{deps_yaml}
"#
    )
}

#[test]
fn frontier_default_shows_proposed_story_with_healthy_or_empty_ancestry() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{ID_HEALTHY}.yml")),
        fixture(ID_HEALTHY, "healthy", &[]),
    )
    .expect("write healthy");
    fs::write(
        stories_dir.join(format!("{ID_PROPOSED_DESC}.yml")),
        fixture(ID_PROPOSED_DESC, "proposed", &[ID_HEALTHY]),
    )
    .expect("write proposed-with-healthy-ancestor");
    fs::write(
        stories_dir.join(format!("{ID_PROPOSED_STANDALONE}.yml")),
        fixture(ID_PROPOSED_STANDALONE, "proposed", &[]),
    )
    .expect("write proposed-standalone");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed healthy UAT + passing test_runs at HEAD so ID_HEALTHY
    // classifies healthy.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000091201",
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
        .render_frontier_json()
        .expect("render_frontier_json should succeed");

    let parsed: Value = serde_json::from_str(&rendered)
        .unwrap_or_else(|e| panic!("frontier JSON must parse: {e}; raw:\n{rendered}"));
    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));
    let ids: Vec<u64> = stories
        .iter()
        .filter_map(|s| s.get("id").and_then(|v| v.as_u64()))
        .collect();

    assert!(
        ids.contains(&(ID_PROPOSED_STANDALONE as u64)),
        "frontier must include the standalone `proposed` story \
         (id {ID_PROPOSED_STANDALONE}, no ancestors); got ids: {ids:?}"
    );
    assert!(
        ids.contains(&(ID_PROPOSED_DESC as u64)),
        "frontier must include the `proposed` story with purely healthy \
         ancestry (id {ID_PROPOSED_DESC}); got ids: {ids:?}"
    );
    assert!(
        !ids.contains(&(ID_HEALTHY as u64)),
        "frontier must NOT include the healthy ancestor (id {ID_HEALTHY}); \
         got ids: {ids:?}"
    );
}
