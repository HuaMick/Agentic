//! Story 10 acceptance test: `+<id>` selector returns target +
//! transitive ancestors, topologically ordered.
//!
//! Justification (from stories/10.yml): proves the `+<id>` selector
//! at the library boundary — `Dashboard::list_selector("+<id>")`
//! returns the target story plus every transitive ancestor (stories
//! it depends_on recursively) in topological order (dependencies
//! before dependents), exclusive of any descendant. Without this,
//! operators have no surgical "show me this story's upstream" slice
//! and the dbt-style lineage grammar the epic commits to is unshipped
//! at the library boundary where the CLI wires plug in.
//!
//! Fixture: A -> B -> TARGET (ancestors) and TARGET -> DESC
//! (descendant, must be excluded from `+TARGET`).

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::Value;
use tempfile::TempDir;

const HEAD_SHA: &str = "4444444444444444444444444444444444444444";

const ID_A: u32 = 92001;
const ID_B: u32 = 92002;
const ID_TARGET: u32 = 92003;
const ID_DESC: u32 = 92004;

fn fixture(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for +id selector scaffold"

outcome: |
  Fixture row for the +id selector scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_selector_plus_id_lists_ancestors.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Call list_selector("+TARGET"); assert ancestors, no descendants.

guidance: |
  Fixture authored inline for the +id selector scaffold. Not a real
  story.

{deps_yaml}
"#
    )
}

#[test]
fn plus_id_selector_returns_target_and_transitive_ancestors_in_topological_order() {
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
        fixture(ID_B, "proposed", &[ID_A]),
    )
    .expect("write B depends_on=[A]");
    fs::write(
        stories_dir.join(format!("{ID_TARGET}.yml")),
        fixture(ID_TARGET, "proposed", &[ID_B]),
    )
    .expect("write TARGET depends_on=[B]");
    fs::write(
        stories_dir.join(format!("{ID_DESC}.yml")),
        fixture(ID_DESC, "proposed", &[ID_TARGET]),
    )
    .expect("write DESC depends_on=[TARGET]");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let dashboard = Dashboard::new(store, stories_dir, HEAD_SHA.to_string());
    let rendered = dashboard
        .list_selector(&format!("+{ID_TARGET}"))
        .expect("list_selector(+TARGET) should succeed");

    let parsed: Value = serde_json::from_str(&rendered)
        .unwrap_or_else(|e| panic!("list_selector JSON must parse: {e}; raw:\n{rendered}"));
    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));

    // Collect the ordered id list exactly as emitted.
    let ids_ordered: Vec<u64> = stories
        .iter()
        .filter_map(|s| s.get("id").and_then(|v| v.as_u64()))
        .collect();

    // Must include target + both ancestors.
    for &expected in [ID_A, ID_B, ID_TARGET].iter() {
        assert!(
            ids_ordered.contains(&(expected as u64)),
            "+TARGET must include id {expected}; got {ids_ordered:?}"
        );
    }

    // Must EXCLUDE the descendant.
    assert!(
        !ids_ordered.contains(&(ID_DESC as u64)),
        "+TARGET must exclude descendants (id {ID_DESC} present); got {ids_ordered:?}"
    );

    // Exactly three entries: target + two transitive ancestors.
    assert_eq!(
        ids_ordered.len(),
        3,
        "+TARGET must list exactly target + transitive ancestors (3); got {ids_ordered:?}"
    );

    // Topological order: dependency before dependent. A comes before
    // B; B comes before TARGET.
    let pos = |id: u32| -> usize {
        ids_ordered
            .iter()
            .position(|v| *v == id as u64)
            .unwrap_or_else(|| panic!("id {id} not found in {ids_ordered:?}"))
    };
    assert!(
        pos(ID_A) < pos(ID_B),
        "topological order: A (id {ID_A}) must come before B (id {ID_B}); got {ids_ordered:?}"
    );
    assert!(
        pos(ID_B) < pos(ID_TARGET),
        "topological order: B (id {ID_B}) must come before TARGET (id {ID_TARGET}); got {ids_ordered:?}"
    );

    let view = parsed
        .get("view")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("list_selector JSON must carry top-level `view`; got: {parsed}"));
    assert_eq!(
        view, "ancestors",
        "+id selector must advertise view=\"ancestors\"; got {view:?}"
    );
}
