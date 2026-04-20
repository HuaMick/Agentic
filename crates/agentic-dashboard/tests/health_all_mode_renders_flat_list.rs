//! Story 10 acceptance test: `--all` renders the flat-list escape
//! hatch.
//!
//! Justification (from stories/10.yml): proves the escape hatch —
//! running the dashboard with `--all` renders one row per story
//! (healthy included, frontier and non-frontier alike), matching the
//! row set story 3's healthy dashboard used to render by default.
//! Without this, the flat-list view that tooling and muscle memory
//! depend on has no path — the default shift would be a capability
//! removal, not a lens shift.
//!
//! Fixture: one story of each interesting status (healthy, proposed,
//! under_construction) plus a proposed story whose ancestor is not-
//! healthy (would be hidden by the frontier filter). `--all` must
//! emit a row for every one.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::{json, Value};
use tempfile::TempDir;

const HEAD_SHA: &str = "2222222222222222222222222222222222222222";

const ID_HEALTHY: u32 = 91701;
const ID_PROPOSED_ROOT: u32 = 91702;
const ID_UC_DESCENDANT: u32 = 91703;
const ID_STANDALONE_UC: u32 = 91704;

fn fixture(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for all-flat-list scaffold"

outcome: |
  Fixture row for the all-flat-list scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_all_mode_renders_flat_list.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Render --all JSON; assert every story appears.

guidance: |
  Fixture authored inline for the all-flat-list scaffold. Not a real
  story.

{deps_yaml}
"#
    )
}

#[test]
fn all_mode_renders_every_story_regardless_of_health_or_frontier_membership() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    fs::write(
        stories_dir.join(format!("{ID_HEALTHY}.yml")),
        fixture(ID_HEALTHY, "healthy", &[]),
    )
    .expect("write healthy");
    fs::write(
        stories_dir.join(format!("{ID_PROPOSED_ROOT}.yml")),
        fixture(ID_PROPOSED_ROOT, "proposed", &[]),
    )
    .expect("write proposed-root");
    fs::write(
        stories_dir.join(format!("{ID_UC_DESCENDANT}.yml")),
        fixture(ID_UC_DESCENDANT, "under_construction", &[ID_PROPOSED_ROOT]),
    )
    .expect("write uc-descendant depends_on=[proposed_root]");
    fs::write(
        stories_dir.join(format!("{ID_STANDALONE_UC}.yml")),
        fixture(ID_STANDALONE_UC, "under_construction", &[]),
    )
    .expect("write standalone-uc");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed healthy UAT + passing test_runs so ID_HEALTHY classifies
    // `healthy`.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000091701",
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
        .render_all_json()
        .expect("render_all_json should succeed");

    let parsed: Value = serde_json::from_str(&rendered)
        .unwrap_or_else(|e| panic!("--all JSON must parse: {e}; raw:\n{rendered}"));

    let view = parsed
        .get("view")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("--all JSON must carry top-level `view`; got: {parsed}"));
    assert_eq!(
        view, "all",
        "--all JSON must advertise view=\"all\"; got {view:?}"
    );

    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));
    let ids: Vec<u64> = stories
        .iter()
        .filter_map(|s| s.get("id").and_then(|v| v.as_u64()))
        .collect();

    for expected in [
        ID_HEALTHY,
        ID_PROPOSED_ROOT,
        ID_UC_DESCENDANT,
        ID_STANDALONE_UC,
    ] {
        assert!(
            ids.contains(&(expected as u64)),
            "--all must include every fixture story (missing id {expected}); got ids: {ids:?}"
        );
    }

    assert_eq!(
        stories.len(),
        4,
        "--all must emit exactly one row per story (4 fixtures); got {}",
        stories.len()
    );
}
