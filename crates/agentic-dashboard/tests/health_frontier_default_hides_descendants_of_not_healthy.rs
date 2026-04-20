//! Story 10 acceptance test: frontier hides descendants of not-healthy.
//!
//! Justification (from stories/10.yml): proves the frontier-filter
//! invariant — given a fixture `stories/` directory where `A`
//! (`proposed`) is depended on by `B` (`under_construction`), the
//! default view renders `A` but does NOT render `B` — because `B` has
//! a not-healthy ancestor (`A`), which means starting work on `B` is
//! blocked until `A` is done. Without this, the default view
//! degenerates to "every not-healthy story" and the operator is back
//! to a flat list of work they cannot start.
//!
//! The scaffold writes a two-story fixture (A proposed, B
//! under_construction, B.depends_on = [A]), calls the dashboard's
//! frontier view (via `render_frontier_json`, so the assertion is on
//! a structured row set rather than a specific table glyph), and
//! asserts (a) A appears in `stories[]`, (b) B does NOT appear.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::Value;
use tempfile::TempDir;

const HEAD_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ID_A: u32 = 91001;
const ID_B: u32 = 91002;

fn fixture(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for frontier descendants-hidden scaffold"

outcome: |
  Fixture row for the frontier descendants-hidden scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_frontier_default_hides_descendants_of_not_healthy.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Run the dashboard frontier view; assert A appears, B does not.

guidance: |
  Fixture authored inline for the frontier descendants-hidden scaffold.
  Not a real story.

{deps_yaml}
"#
    )
}

#[test]
fn frontier_default_hides_descendants_of_not_healthy_ancestors() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{ID_A}.yml")),
        fixture(ID_A, "proposed", &[]),
    )
    .expect("write A proposed");
    fs::write(
        stories_dir.join(format!("{ID_B}.yml")),
        fixture(ID_B, "under_construction", &[ID_A]),
    )
    .expect("write B under_construction depends_on=[A]");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let dashboard = Dashboard::new(store, stories_dir, HEAD_SHA.to_string());
    let rendered = dashboard
        .render_frontier_json()
        .expect("render_frontier_json should succeed on two-story fixture");

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
        ids.contains(&(ID_A as u64)),
        "frontier must include A (id {ID_A}, proposed, no not-healthy ancestor); got ids: {ids:?}"
    );
    assert!(
        !ids.contains(&(ID_B as u64)),
        "frontier must NOT include B (id {ID_B}, under_construction with not-healthy ancestor A); \
         got ids: {ids:?}"
    );
}
