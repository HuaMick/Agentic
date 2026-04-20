//! Story 10 acceptance test: a selector with an unknown id returns
//! `DashboardError::UnknownStory` naming the missing id.
//!
//! Justification (from stories/10.yml): proves clean failure on bad
//! input at the library boundary — `Dashboard::list_selector("+99999")`
//! (where 99999 has no corresponding `stories/99999.yml`) returns a
//! typed `DashboardError::UnknownStory` naming the missing id, does
//! not panic, and does not return a partial result set. Without this,
//! CLI wrappers (and story 12's CI selector caller) get either a
//! crash or an empty list indistinguishable from "no ancestors" —
//! either one is a worse operator experience than a named error.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::{Dashboard, DashboardError};
use agentic_store::{MemStore, Store};
use tempfile::TempDir;

const HEAD_SHA: &str = "8888888888888888888888888888888888888888";

const ID_KNOWN: u32 = 92401;
const ID_MISSING: u32 = 99999;

fn fixture(id: u32) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for unknown-id-selector scaffold"

outcome: |
  Fixture row for the unknown-id-selector scaffold.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_selector_unknown_id.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Call list_selector("+99999"); assert UnknownStory{{id: 99999}}.

guidance: |
  Fixture authored inline for the unknown-id-selector scaffold. Not a
  real story.

depends_on: []
"#
    )
}

#[test]
fn selector_with_unknown_id_returns_typed_unknown_story_error_naming_the_id() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(stories_dir.join(format!("{ID_KNOWN}.yml")), fixture(ID_KNOWN))
        .expect("write known fixture");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let dashboard = Dashboard::new(store, stories_dir, HEAD_SHA.to_string());

    let result = dashboard.list_selector(&format!("+{ID_MISSING}"));
    match result {
        Err(DashboardError::UnknownStory { id }) => {
            assert_eq!(
                id, ID_MISSING,
                "UnknownStory error must name the missing id {ID_MISSING}; got {id}"
            );
        }
        Err(other) => panic!(
            "expected DashboardError::UnknownStory{{id: {ID_MISSING}}}; got other error: {other:?}"
        ),
        Ok(output) => panic!(
            "list_selector against an unknown id must return Err(UnknownStory), not Ok with \
             partial/empty output; got:\n{output}"
        ),
    }

    // Display impl must also name the id so CLI wrappers can print it.
    let err = dashboard
        .list_selector(&format!("+{ID_MISSING}"))
        .expect_err("unknown id must error");
    let msg = err.to_string();
    assert!(
        msg.contains(&ID_MISSING.to_string()),
        "DashboardError::UnknownStory Display must contain the missing id {ID_MISSING}; \
         got: {msg}"
    );
}
