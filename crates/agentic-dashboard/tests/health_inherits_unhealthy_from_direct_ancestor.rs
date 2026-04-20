//! Story 13 acceptance test: the core ancestor-inheritance rule at the
//! classifier boundary.
//!
//! Justification (from stories/13.yml): proves the core inheritance rule
//! at the classifier boundary: given a story `<id>` whose own on-disk
//! `status` is `healthy`, whose latest `uat_signings.verdict=pass`
//! commit equals HEAD, whose latest `test_runs.verdict` is `pass`, and
//! whose `related_files` (if any) have not changed — but whose direct
//! ancestor `<A>` classifies as something other than `healthy` (e.g.
//! `<A>` is `under_construction`) — the dashboard classifies `<id>`
//! as `unhealthy`, NOT `healthy`. Without this, a story can claim
//! healthy while standing on an unproven parent and the dashboard's
//! read view silently approves the forgery.
//!
//! The scaffold materialises two fixture stories in a tempdir: A
//! under_construction (no UAT signing — classifies `under_construction`),
//! and L (leaf) whose own signals are all clean and healthy AT HEAD,
//! depends_on = [A]. The assertion is that L's rendered JSON row
//! classifies `"unhealthy"`, not `"healthy"` — the direct-ancestor
//! inheritance rule fires. Red today is runtime-red because the
//! current classifier only looks at a story's own signals and emits
//! `"healthy"` for L despite A being broken.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::{json, Value};
use tempfile::TempDir;

const HEAD_SHA: &str = "1313130000000000000000000000000000000001";
const ID_A: u32 = 913011; // direct ancestor — under_construction
const ID_L: u32 = 913012; // leaf — own signals all healthy

fn fixture(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for direct-ancestor inheritance scaffold"

outcome: |
  Fixture row for the direct-ancestor inheritance scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_inherits_unhealthy_from_direct_ancestor.rs
      justification: |
        Present so the fixture is schema-valid. The live test drives
        Dashboard against this YAML to exercise the ancestor rule.
  uat: |
    Render the dashboard; assert L classifies as unhealthy when its
    direct ancestor A is not healthy.

guidance: |
  Fixture authored inline for the direct-ancestor inheritance scaffold.
  Not a real story.

{deps_yaml}
"#
    )
}

#[test]
fn leaf_with_clean_own_signals_classifies_unhealthy_when_direct_ancestor_is_not_healthy() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // A: under_construction, no UAT signing — will classify under_construction.
    fs::write(
        stories_dir.join(format!("{ID_A}.yml")),
        fixture(ID_A, "under_construction", &[]),
    )
    .expect("write A under_construction");

    // L: status healthy, depends_on = [A].
    fs::write(
        stories_dir.join(format!("{ID_L}.yml")),
        fixture(ID_L, "healthy", &[ID_A]),
    )
    .expect("write L healthy depends_on=[A]");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed L's own signals to be clean: UAT pass @ HEAD, test_runs pass.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000913012",
                "story_id": ID_L,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "signed_at": "2026-04-19T00:00:00Z",
            }),
        )
        .expect("seed L uat_signings pass@HEAD");
    store
        .upsert(
            "test_runs",
            &ID_L.to_string(),
            json!({
                "story_id": ID_L,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed L test_runs pass");

    // A carries no UAT signing — the classifier emits `under_construction`
    // for A based on its YAML status.

    let dashboard = Dashboard::new(store.clone(), stories_dir.clone(), HEAD_SHA.to_string());
    let rendered = dashboard
        .render_json()
        .expect("render_json should succeed on the two-story fixture");

    let parsed: Value = serde_json::from_str(&rendered)
        .unwrap_or_else(|e| panic!("render_json output must parse as JSON: {e}; raw:\n{rendered}"));

    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));

    // Sanity: A classifies as under_construction (not healthy). This is the
    // precondition that the ancestor-inheritance rule triggers on.
    let a_row = stories
        .iter()
        .find(|s| s.get("id").and_then(|v| v.as_u64()) == Some(ID_A as u64))
        .unwrap_or_else(|| panic!("stories[] must include A (id {ID_A}); got: {parsed}"));
    let a_health = a_row
        .get("health")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("A row must carry health; got: {a_row}"));
    assert_ne!(
        a_health, "healthy",
        "precondition: A must classify as not-healthy so the inheritance \
         rule has something to fire on; got A.health={a_health} on row {a_row}"
    );

    // The load-bearing assertion: L must classify as `unhealthy`, NOT
    // `healthy`, because its direct ancestor A is not healthy — even
    // though L's own signals are all clean.
    let l_row = stories
        .iter()
        .find(|s| s.get("id").and_then(|v| v.as_u64()) == Some(ID_L as u64))
        .unwrap_or_else(|| panic!("stories[] must include L (id {ID_L}); got: {parsed}"));
    let l_health = l_row
        .get("health")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("L row must carry health; got: {l_row}"));
    assert_eq!(
        l_health, "unhealthy",
        "L's own signals are all clean (YAML healthy, UAT pass @ HEAD, \
         test_runs pass) but its direct ancestor A is {a_health} — the \
         direct-ancestor inheritance rule must flip L's classification \
         to `unhealthy`, not leave it at `healthy`. Got L.health={l_health} \
         on row {l_row}"
    );
}
