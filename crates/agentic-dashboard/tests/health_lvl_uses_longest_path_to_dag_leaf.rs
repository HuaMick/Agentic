//! Story 10 acceptance test: `lvl` computes the LONGEST path to a
//! DAG leaf, not the shortest.
//!
//! Justification (from stories/10.yml): proves the `lvl` computation
//! — given a fixture where story `X` has two descendant paths to the
//! DAG leaves — one of length 1 and one of length 3 — `X.lvl == -3`,
//! not `-1`. The LONGEST path to a leaf wins. Without this, two
//! stories with different "how much work unblocks" signals get the
//! same `lvl` number and the primary sort (`lvl` ascending) lies
//! about which frontier story unblocks the most downstream work.
//!
//! The fixture builds X with two descendant branches:
//!   short branch: X -> L_SHORT (leaf)                      (length 1)
//!   long branch:  X -> M1 -> M2 -> L_LONG (leaf)           (length 3)
//! All stories are `under_construction` so they render in frontier/
//! all views and carry `lvl` in the JSON payload.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::Value;
use tempfile::TempDir;

const HEAD_SHA: &str = "dddddddddddddddddddddddddddddddddddddddd";

const ID_X: u32 = 91301;
const ID_SHORT_LEAF: u32 = 91302;
const ID_M1: u32 = 91303;
const ID_M2: u32 = 91304;
const ID_LONG_LEAF: u32 = 91305;

fn fixture(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for lvl-longest-path scaffold"

outcome: |
  Fixture row for the lvl-longest-path scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_lvl_uses_longest_path_to_dag_leaf.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Render JSON; assert X.lvl == -3, tracking the longest path.

guidance: |
  Fixture authored inline for the lvl-longest-path scaffold. Not a real
  story.

{deps_yaml}
"#
    )
}

#[test]
fn lvl_uses_longest_path_to_dag_leaf_not_shortest() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // depends_on points from dependent to dependency. X is the
    // upstream that unblocks downstream work; downstreams declare
    // X in their depends_on.
    fs::write(
        stories_dir.join(format!("{ID_X}.yml")),
        fixture(ID_X, "under_construction", &[]),
    )
    .expect("write X");
    fs::write(
        stories_dir.join(format!("{ID_SHORT_LEAF}.yml")),
        fixture(ID_SHORT_LEAF, "under_construction", &[ID_X]),
    )
    .expect("write short-leaf depends_on=[X]");
    fs::write(
        stories_dir.join(format!("{ID_M1}.yml")),
        fixture(ID_M1, "under_construction", &[ID_X]),
    )
    .expect("write M1 depends_on=[X]");
    fs::write(
        stories_dir.join(format!("{ID_M2}.yml")),
        fixture(ID_M2, "under_construction", &[ID_M1]),
    )
    .expect("write M2 depends_on=[M1]");
    fs::write(
        stories_dir.join(format!("{ID_LONG_LEAF}.yml")),
        fixture(ID_LONG_LEAF, "under_construction", &[ID_M2]),
    )
    .expect("write long-leaf depends_on=[M2]");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let dashboard = Dashboard::new(store, stories_dir, HEAD_SHA.to_string());

    // Use render_all_json so every story — including X — appears in
    // the JSON regardless of frontier filter.
    let rendered = dashboard
        .render_all_json()
        .expect("render_all_json should succeed");

    let parsed: Value = serde_json::from_str(&rendered)
        .unwrap_or_else(|e| panic!("JSON must parse: {e}; raw:\n{rendered}"));
    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));

    let find = |id: u32| -> &Value {
        stories
            .iter()
            .find(|s| s.get("id").and_then(|v| v.as_u64()) == Some(id as u64))
            .unwrap_or_else(|| panic!("stories[] must include id {id}; got: {parsed}"))
    };

    let lvl_of = |id: u32| -> i64 {
        find(id)
            .get("lvl")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| panic!("story {id} must carry integer `lvl`; got: {parsed}"))
    };

    // Leaves carry lvl == 0.
    assert_eq!(
        lvl_of(ID_SHORT_LEAF),
        0,
        "short-leaf (id {ID_SHORT_LEAF}) must be a DAG leaf with lvl == 0"
    );
    assert_eq!(
        lvl_of(ID_LONG_LEAF),
        0,
        "long-leaf (id {ID_LONG_LEAF}) must be a DAG leaf with lvl == 0"
    );

    // M2 is one step above the long leaf.
    assert_eq!(
        lvl_of(ID_M2),
        -1,
        "M2 (id {ID_M2}) has a single downstream leaf; lvl must be -1"
    );
    // M1 is two steps above the long leaf.
    assert_eq!(
        lvl_of(ID_M1),
        -2,
        "M1 (id {ID_M1}) has two-step path to long leaf; lvl must be -2"
    );

    // X has TWO descendant paths to a leaf: one of length 1 (via
    // short-leaf) and one of length 3 (via M1 -> M2 -> long-leaf).
    // The LONGEST wins, so X.lvl must be -3.
    assert_eq!(
        lvl_of(ID_X),
        -3,
        "X (id {ID_X}) has a short path (length 1) and a long path (length 3) \
         to a DAG leaf; lvl must track the LONGEST path, i.e. -3, not -1"
    );
}
