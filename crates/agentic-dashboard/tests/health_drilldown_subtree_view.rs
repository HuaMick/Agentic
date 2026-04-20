//! Story 10 acceptance test: drilldown renders the target's subtree
//! (target + transitive ancestors + transitive descendants).
//!
//! Justification (from stories/10.yml): proves the subtree drilldown
//! — `Dashboard::drilldown(<id>)` returns a view containing the
//! target story's row PLUS a topologically sorted list of its
//! transitive ancestors (stories it depends_on — direct and
//! transitive) AND its transitive descendants (stories that depend_on
//! it — direct and transitive). Each entry carries id, title, and
//! classified health. Without this, drilldown is still the story-3
//! single-row view and the "lineage view" half of the epic objective
//! is unshipped.
//!
//! Fixture:
//!   A (proposed, leaf-most upstream)
//!   B (under_construction, depends_on=[A])
//!   TARGET (proposed, depends_on=[B])
//!   C (proposed, depends_on=[TARGET])
//!   D (proposed, depends_on=[C])
//!   UNRELATED (proposed, no edges)
//!
//! Drilldown on TARGET must reference A and B (transitive ancestors)
//! AND C and D (transitive descendants). UNRELATED must NOT appear.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use tempfile::TempDir;

const HEAD_SHA: &str = "3333333333333333333333333333333333333333";

const ID_A: u32 = 91901;
const ID_B: u32 = 91902;
const ID_TARGET: u32 = 91903;
const ID_C: u32 = 91904;
const ID_D: u32 = 91905;
const ID_UNRELATED: u32 = 91906;

fn fixture(id: u32, title_suffix: &str, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} {title_suffix} for drilldown-subtree scaffold"

outcome: |
  Fixture row for the drilldown-subtree scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_drilldown_subtree_view.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Call drilldown(TARGET); assert ancestors + descendants sections.

guidance: |
  Fixture authored inline for the drilldown-subtree scaffold. Not a
  real story.

{deps_yaml}
"#
    )
}

#[test]
fn drilldown_shows_transitive_ancestors_and_descendants_of_the_target() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    fs::write(
        stories_dir.join(format!("{ID_A}.yml")),
        fixture(ID_A, "ANCESTOR_ROOT", "proposed", &[]),
    )
    .expect("write A");
    fs::write(
        stories_dir.join(format!("{ID_B}.yml")),
        fixture(ID_B, "ANCESTOR_MID", "under_construction", &[ID_A]),
    )
    .expect("write B depends_on=[A]");
    fs::write(
        stories_dir.join(format!("{ID_TARGET}.yml")),
        fixture(ID_TARGET, "TARGET", "proposed", &[ID_B]),
    )
    .expect("write TARGET depends_on=[B]");
    fs::write(
        stories_dir.join(format!("{ID_C}.yml")),
        fixture(ID_C, "DESC_MID", "proposed", &[ID_TARGET]),
    )
    .expect("write C depends_on=[TARGET]");
    fs::write(
        stories_dir.join(format!("{ID_D}.yml")),
        fixture(ID_D, "DESC_LEAF", "proposed", &[ID_C]),
    )
    .expect("write D depends_on=[C]");
    fs::write(
        stories_dir.join(format!("{ID_UNRELATED}.yml")),
        fixture(ID_UNRELATED, "UNRELATED", "proposed", &[]),
    )
    .expect("write UNRELATED");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let dashboard = Dashboard::new(store, stories_dir, HEAD_SHA.to_string());
    let rendered = dashboard
        .drilldown(ID_TARGET)
        .expect("drilldown should succeed for a known id");

    // Target's own id must appear.
    assert!(
        rendered.contains(&ID_TARGET.to_string()),
        "drilldown output must reference the target id {ID_TARGET}; got:\n{rendered}"
    );

    // Transitive ancestors A and B must appear.
    assert!(
        rendered.contains(&ID_A.to_string()),
        "drilldown must list transitive ancestor A (id {ID_A}); got:\n{rendered}"
    );
    assert!(
        rendered.contains(&ID_B.to_string()),
        "drilldown must list direct ancestor B (id {ID_B}); got:\n{rendered}"
    );

    // Transitive descendants C and D must appear.
    assert!(
        rendered.contains(&ID_C.to_string()),
        "drilldown must list direct descendant C (id {ID_C}); got:\n{rendered}"
    );
    assert!(
        rendered.contains(&ID_D.to_string()),
        "drilldown must list transitive descendant D (id {ID_D}); got:\n{rendered}"
    );

    // UNRELATED must not appear anywhere in the rendering.
    assert!(
        !rendered.contains(&ID_UNRELATED.to_string()),
        "drilldown must NOT reference the unrelated story (id {ID_UNRELATED}); got:\n{rendered}"
    );

    // The output must carry identifiable "Ancestors" and "Descendants"
    // section headers so the operator can tell the lineage apart from
    // a flat list.
    let lower = rendered.to_lowercase();
    assert!(
        lower.contains("ancestor"),
        "drilldown output must have an Ancestors section header; got:\n{rendered}"
    );
    assert!(
        lower.contains("descendant"),
        "drilldown output must have a Descendants section header; got:\n{rendered}"
    );

    // Each ancestor/descendant entry carries classified health. The
    // fixture uses a mix of `proposed` and `under_construction`; both
    // strings must show up somewhere in the rendering.
    assert!(
        rendered.contains("proposed"),
        "drilldown entries must name classified health; `proposed` \
         missing from:\n{rendered}"
    );
    assert!(
        rendered.contains("under_construction"),
        "drilldown entries must name classified health; `under_construction` \
         missing from:\n{rendered}"
    );
}
