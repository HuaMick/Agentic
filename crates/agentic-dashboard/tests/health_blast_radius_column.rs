//! Story 10 acceptance test: the `↓` blast-radius column.
//!
//! Justification (from stories/10.yml): proves the `↓` column
//! contract — for a frontier story `S` whose immediate downstreams
//! are `[A, B]` where `B` itself has two further downstreams `[C, D]`,
//! `S`'s row lists immediate downstream ids `A, B` and carries
//! `blocks_total == 4` (the transitive descendant set `{A, B, C, D}`).
//! A leaf frontier story renders with `blocks_total == 0`. Without
//! this, the operator cannot distinguish a frontier story that
//! unblocks one leaf from one that unblocks a whole subtree — which
//! is the primary signal "fix this first" depends on.
//!
//! Fixture:
//!   S (proposed)
//!   A (proposed, depends_on=[S])
//!   B (proposed, depends_on=[S])
//!   C (proposed, depends_on=[B])
//!   D (proposed, depends_on=[B])
//!   LEAF_ONLY (proposed)  — separate component, no downstreams
//!
//! S sits at the root of {A, B, C, D} (transitive set size 4).
//! LEAF_ONLY has no downstreams.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::Value;
use tempfile::TempDir;

const HEAD_SHA: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

const ID_S: u32 = 91401;
const ID_A: u32 = 91402;
const ID_B: u32 = 91403;
const ID_C: u32 = 91404;
const ID_D: u32 = 91405;
const ID_LEAF_ONLY: u32 = 91406;

fn fixture(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for blast-radius scaffold"

outcome: |
  Fixture row for the blast-radius scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_blast_radius_column.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Render JSON; assert downstream and blocks_total.

guidance: |
  Fixture authored inline for the blast-radius scaffold. Not a real
  story.

{deps_yaml}
"#
    )
}

#[test]
fn blast_radius_column_lists_immediate_downstreams_and_transitive_count() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    fs::write(
        stories_dir.join(format!("{ID_S}.yml")),
        fixture(ID_S, "proposed", &[]),
    )
    .expect("write S");
    fs::write(
        stories_dir.join(format!("{ID_A}.yml")),
        fixture(ID_A, "proposed", &[ID_S]),
    )
    .expect("write A depends_on=[S]");
    fs::write(
        stories_dir.join(format!("{ID_B}.yml")),
        fixture(ID_B, "proposed", &[ID_S]),
    )
    .expect("write B depends_on=[S]");
    fs::write(
        stories_dir.join(format!("{ID_C}.yml")),
        fixture(ID_C, "proposed", &[ID_B]),
    )
    .expect("write C depends_on=[B]");
    fs::write(
        stories_dir.join(format!("{ID_D}.yml")),
        fixture(ID_D, "proposed", &[ID_B]),
    )
    .expect("write D depends_on=[B]");
    fs::write(
        stories_dir.join(format!("{ID_LEAF_ONLY}.yml")),
        fixture(ID_LEAF_ONLY, "proposed", &[]),
    )
    .expect("write LEAF_ONLY");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let dashboard = Dashboard::new(store, stories_dir, HEAD_SHA.to_string());
    let rendered = dashboard
        .render_frontier_json()
        .expect("render_frontier_json should succeed");

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

    // S is a root (no ancestors) so it IS on the frontier.
    let s_row = find(ID_S);

    // Immediate downstream list: [A, B].
    let downstream: Vec<u64> = s_row
        .get("downstream")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("S row must carry a `downstream` array; got: {s_row}"))
        .iter()
        .filter_map(|v| v.as_u64())
        .collect();
    assert_eq!(
        downstream.len(),
        2,
        "S must have exactly two immediate downstreams (A, B); got {downstream:?}"
    );
    assert!(
        downstream.contains(&(ID_A as u64)),
        "S's `downstream` must include A (id {ID_A}); got {downstream:?}"
    );
    assert!(
        downstream.contains(&(ID_B as u64)),
        "S's `downstream` must include B (id {ID_B}); got {downstream:?}"
    );
    // Indirect descendants C, D must NOT appear in the immediate list.
    assert!(
        !downstream.contains(&(ID_C as u64)) && !downstream.contains(&(ID_D as u64)),
        "S's `downstream` is the IMMEDIATE downstream list, not transitive; \
         must not include C or D; got {downstream:?}"
    );

    // blocks_total == 4 (the transitive descendant set {A, B, C, D}).
    let s_blocks_total = s_row
        .get("blocks_total")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| panic!("S row must carry integer blocks_total; got: {s_row}"));
    assert_eq!(
        s_blocks_total, 4,
        "S.blocks_total must equal the transitive descendant count {{A, B, C, D}} = 4; \
         got {s_blocks_total}"
    );

    // LEAF_ONLY is a leaf frontier story: blocks_total == 0.
    let leaf_row = find(ID_LEAF_ONLY);
    let leaf_blocks_total = leaf_row
        .get("blocks_total")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| {
            panic!("LEAF_ONLY row must carry integer blocks_total; got: {leaf_row}")
        });
    assert_eq!(
        leaf_blocks_total, 0,
        "A leaf frontier story must render with blocks_total == 0; got {leaf_blocks_total}"
    );
}
