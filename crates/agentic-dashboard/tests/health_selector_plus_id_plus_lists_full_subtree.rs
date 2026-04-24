//! Story 10 acceptance test: `+<id>+` selector returns target +
//! transitive ancestors AND transitive descendants, deduplicated,
//! in a single topological order.
//!
//! Justification (from stories/10.yml): proves the `+<id>+` selector
//! at the library boundary — `Dashboard::list_selector("+<id>+")`
//! returns the target story plus every transitive ancestor AND every
//! transitive descendant, deduplicated, in a single topological
//! order. Without this, the third leg of the dbt grammar is unshipped
//! and the drilldown view (which uses the same traversal internally)
//! has no tested primitive to share.
//!
//! Fixture: ANC_ROOT -> ANC_MID -> TARGET -> DESC_MID -> DESC_LEAF.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::Value;
use tempfile::TempDir;

const HEAD_SHA: &str = "6666666666666666666666666666666666666666";

const ID_ANC_ROOT: u32 = 92201;
const ID_ANC_MID: u32 = 92202;
const ID_TARGET: u32 = 92203;
const ID_DESC_MID: u32 = 92204;
const ID_DESC_LEAF: u32 = 92205;
const ID_UNRELATED: u32 = 92206;

fn fixture(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for +id+ selector scaffold"

outcome: |
  Fixture row for the +id+ selector scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_selector_plus_id_plus_lists_full_subtree.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Call list_selector("+TARGET+"); assert full subtree.

guidance: |
  Fixture authored inline for the +id+ selector scaffold. Not a real
  story.

{deps_yaml}
"#
    )
}

#[test]
fn plus_id_plus_selector_returns_full_subtree_deduplicated_in_single_topological_order() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    fs::write(
        stories_dir.join(format!("{ID_ANC_ROOT}.yml")),
        fixture(ID_ANC_ROOT, "proposed", &[]),
    )
    .expect("write ANC_ROOT");
    fs::write(
        stories_dir.join(format!("{ID_ANC_MID}.yml")),
        fixture(ID_ANC_MID, "proposed", &[ID_ANC_ROOT]),
    )
    .expect("write ANC_MID depends_on=[ANC_ROOT]");
    fs::write(
        stories_dir.join(format!("{ID_TARGET}.yml")),
        fixture(ID_TARGET, "proposed", &[ID_ANC_MID]),
    )
    .expect("write TARGET depends_on=[ANC_MID]");
    fs::write(
        stories_dir.join(format!("{ID_DESC_MID}.yml")),
        fixture(ID_DESC_MID, "proposed", &[ID_TARGET]),
    )
    .expect("write DESC_MID depends_on=[TARGET]");
    fs::write(
        stories_dir.join(format!("{ID_DESC_LEAF}.yml")),
        fixture(ID_DESC_LEAF, "proposed", &[ID_DESC_MID]),
    )
    .expect("write DESC_LEAF depends_on=[DESC_MID]");
    fs::write(
        stories_dir.join(format!("{ID_UNRELATED}.yml")),
        fixture(ID_UNRELATED, "proposed", &[]),
    )
    .expect("write UNRELATED");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let dashboard = Dashboard::new(store, stories_dir, HEAD_SHA.to_string());
    let rendered = dashboard
        .list_selector(&format!("+{ID_TARGET}+"))
        .expect("list_selector(+TARGET+) should succeed");

    let parsed: Value = serde_json::from_str(&rendered)
        .unwrap_or_else(|e| panic!("list_selector JSON must parse: {e}; raw:\n{rendered}"));
    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));

    let ids_ordered: Vec<u64> = stories
        .iter()
        .filter_map(|s| s.get("id").and_then(|v| v.as_u64()))
        .collect();

    // All five subtree members present.
    for &expected in [
        ID_ANC_ROOT,
        ID_ANC_MID,
        ID_TARGET,
        ID_DESC_MID,
        ID_DESC_LEAF,
    ]
    .iter()
    {
        assert!(
            ids_ordered.contains(&(expected as u64)),
            "+TARGET+ must include id {expected}; got {ids_ordered:?}"
        );
    }

    // Unrelated story excluded.
    assert!(
        !ids_ordered.contains(&(ID_UNRELATED as u64)),
        "+TARGET+ must exclude the unrelated story (id {ID_UNRELATED} present); got {ids_ordered:?}"
    );

    // Exactly five entries (deduplicated — target appears once even
    // though it's both "an ancestor of its descendants" and "a
    // descendant of its ancestors").
    assert_eq!(
        ids_ordered.len(),
        5,
        "+TARGET+ must emit exactly five entries (target plus 2 ancestors plus 2 descendants, \
         deduplicated); got {ids_ordered:?}"
    );

    // No id appears twice.
    let mut sorted = ids_ordered.clone();
    sorted.sort();
    let mut dedup = sorted.clone();
    dedup.dedup();
    assert_eq!(
        sorted, dedup,
        "+TARGET+ output must be deduplicated (each id appears at most once); got {ids_ordered:?}"
    );

    // Single topological order: ancestors before target, target
    // before descendants.
    let pos = |id: u32| -> usize {
        ids_ordered
            .iter()
            .position(|v| *v == id as u64)
            .unwrap_or_else(|| panic!("id {id} not found in {ids_ordered:?}"))
    };
    assert!(
        pos(ID_ANC_ROOT) < pos(ID_ANC_MID),
        "ANC_ROOT must precede ANC_MID; got {ids_ordered:?}"
    );
    assert!(
        pos(ID_ANC_MID) < pos(ID_TARGET),
        "ANC_MID must precede TARGET; got {ids_ordered:?}"
    );
    assert!(
        pos(ID_TARGET) < pos(ID_DESC_MID),
        "TARGET must precede DESC_MID; got {ids_ordered:?}"
    );
    assert!(
        pos(ID_DESC_MID) < pos(ID_DESC_LEAF),
        "DESC_MID must precede DESC_LEAF; got {ids_ordered:?}"
    );

    let view = parsed
        .get("view")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("list_selector JSON must carry top-level `view`; got: {parsed}"));
    assert_eq!(
        view, "subtree",
        "+id+ selector must advertise view=\"subtree\"; got {view:?}"
    );
}
