//! Story 10 acceptance test: cycle in `depends_on` surfaces a typed
//! error without panic and without partial output.
//!
//! Justification (from stories/10.yml): proves the invariant defence
//! — given a fixture `stories/` directory containing a hand-
//! constructed cycle (A depends_on B, B depends_on A — which story
//! 6's loader is supposed to reject at parse time), the dashboard
//! surfaces the loader's typed CycleError without panicking, without
//! rendering partial frontier data that would be meaningless, and
//! without crashing the dashboard process as a whole. Without this,
//! a future regression in the loader's cycle check would produce
//! either a panic trace or a silently-wrong frontier — both worse
//! than a loud failure.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::{Dashboard, DashboardError};
use agentic_store::{MemStore, Store};
use tempfile::TempDir;

const HEAD_SHA: &str = "7777777777777777777777777777777777777777";

const ID_A: u32 = 92301;
const ID_B: u32 = 92302;

fn fixture(id: u32, depends_on: &[u32]) -> String {
    let deps_yaml = {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} in a cycle"

outcome: |
  Fixture row for the cycle-rejected scaffold.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_selector_cycle_rejected.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Render frontier against a cycle; assert typed Cycle error.

guidance: |
  Fixture authored inline for the cycle-rejected scaffold. Not a real
  story.

{deps_yaml}
"#
    )
}

#[test]
fn cycle_in_depends_on_surfaces_typed_error_not_panic_or_partial_output() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // A depends_on B, B depends_on A — a two-node cycle.
    fs::write(
        stories_dir.join(format!("{ID_A}.yml")),
        fixture(ID_A, &[ID_B]),
    )
    .expect("write A depends_on=[B]");
    fs::write(
        stories_dir.join(format!("{ID_B}.yml")),
        fixture(ID_B, &[ID_A]),
    )
    .expect("write B depends_on=[A]");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let dashboard = Dashboard::new(store, stories_dir, HEAD_SHA.to_string());

    // Frontier render must return Err, not panic, not return partial Ok.
    let frontier_result = dashboard.render_frontier_json();
    match frontier_result {
        Err(DashboardError::Cycle { .. }) => {
            // Correct: typed cycle error surfaced.
        }
        Err(other) => {
            panic!("expected DashboardError::Cycle on a two-node cycle; got other error: {other:?}")
        }
        Ok(output) => panic!(
            "frontier render against a cycle must return Err(Cycle), not Ok with partial \
             output; got:\n{output}"
        ),
    }

    // A selector call must also fail loudly with Cycle (not panic,
    // not empty-list success).
    let selector_result = dashboard.list_selector(&format!("+{ID_A}"));
    match selector_result {
        Err(DashboardError::Cycle { .. }) => {
            // Correct: typed cycle error surfaced.
        }
        Err(other) => panic!(
            "expected DashboardError::Cycle on a selector call against a cycle; got other \
             error: {other:?}"
        ),
        Ok(output) => panic!(
            "selector call against a cycle must return Err(Cycle), not Ok with partial \
             output; got:\n{output}"
        ),
    }
}
