//! Story 10 acceptance test: `<id>+` selector returns target +
//! transitive descendants, topologically ordered.
//!
//! Justification (from stories/10.yml): proves the `<id>+` selector
//! at the library boundary — `Dashboard::list_selector("<id>+")`
//! returns the target story plus every transitive descendant (stories
//! that depend_on it recursively), in topological order (dependencies
//! before dependents), exclusive of any ancestor. Without this, the
//! selector grammar is asymmetric — operators could look upstream but
//! not downstream, and story 12's CI-subtree use case has no shared
//! primitive to reuse.
//!
//! Fixture: ANC -> TARGET (ancestor, must be excluded). TARGET ->
//! D1 -> D2 (descendants).

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::Value;
use tempfile::TempDir;

const HEAD_SHA: &str = "5555555555555555555555555555555555555555";

const ID_ANC: u32 = 92101;
const ID_TARGET: u32 = 92102;
const ID_D1: u32 = 92103;
const ID_D2: u32 = 92104;

fn fixture(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for id+ selector scaffold"

outcome: |
  Fixture row for the id+ selector scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_selector_id_plus_lists_descendants.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Call list_selector("TARGET+"); assert descendants, no ancestors.

guidance: |
  Fixture authored inline for the id+ selector scaffold. Not a real
  story.

{deps_yaml}
"#
    )
}

#[test]
fn id_plus_selector_returns_target_and_transitive_descendants_in_topological_order() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    fs::write(
        stories_dir.join(format!("{ID_ANC}.yml")),
        fixture(ID_ANC, "proposed", &[]),
    )
    .expect("write ANC");
    fs::write(
        stories_dir.join(format!("{ID_TARGET}.yml")),
        fixture(ID_TARGET, "proposed", &[ID_ANC]),
    )
    .expect("write TARGET depends_on=[ANC]");
    fs::write(
        stories_dir.join(format!("{ID_D1}.yml")),
        fixture(ID_D1, "proposed", &[ID_TARGET]),
    )
    .expect("write D1 depends_on=[TARGET]");
    fs::write(
        stories_dir.join(format!("{ID_D2}.yml")),
        fixture(ID_D2, "proposed", &[ID_D1]),
    )
    .expect("write D2 depends_on=[D1]");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let dashboard = Dashboard::new(store, stories_dir, HEAD_SHA.to_string());
    let rendered = dashboard
        .list_selector(&format!("{ID_TARGET}+"))
        .expect("list_selector(TARGET+) should succeed");

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

    // Must include target + both transitive descendants.
    for &expected in [ID_TARGET, ID_D1, ID_D2].iter() {
        assert!(
            ids_ordered.contains(&(expected as u64)),
            "TARGET+ must include id {expected}; got {ids_ordered:?}"
        );
    }

    // Must EXCLUDE the ancestor.
    assert!(
        !ids_ordered.contains(&(ID_ANC as u64)),
        "TARGET+ must exclude ancestors (id {ID_ANC} present); got {ids_ordered:?}"
    );

    // Exactly three entries: target + two transitive descendants.
    assert_eq!(
        ids_ordered.len(),
        3,
        "TARGET+ must list exactly target + transitive descendants (3); got {ids_ordered:?}"
    );

    // Topological order: dependency before dependent. TARGET comes
    // before D1; D1 comes before D2.
    let pos = |id: u32| -> usize {
        ids_ordered
            .iter()
            .position(|v| *v == id as u64)
            .unwrap_or_else(|| panic!("id {id} not found in {ids_ordered:?}"))
    };
    assert!(
        pos(ID_TARGET) < pos(ID_D1),
        "topological order: TARGET (id {ID_TARGET}) must come before D1 (id {ID_D1}); got {ids_ordered:?}"
    );
    assert!(
        pos(ID_D1) < pos(ID_D2),
        "topological order: D1 (id {ID_D1}) must come before D2 (id {ID_D2}); got {ids_ordered:?}"
    );

    let view = parsed
        .get("view")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("list_selector JSON must carry top-level `view`; got: {parsed}"));
    assert_eq!(
        view, "descendants",
        "id+ selector must advertise view=\"descendants\"; got {view:?}"
    );
}
