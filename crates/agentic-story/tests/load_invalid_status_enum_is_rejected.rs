//! Story 6 acceptance test: a story whose `status` field is not one of the
//! four enum values is rejected with a typed error naming both the field
//! and the offending value.
//!
//! Justification (from stories/6.yml): proves enum-boundary enforcement —
//! a story file whose `status` field is a string not in `proposed |
//! under_construction | healthy | unhealthy` is rejected with a typed
//! error naming the field AND the offending value. Without this, a typo
//! in `status:` (or a leftover legacy value like `tested`) loads as a
//! default or a catch-all and the four-status model the dashboard depends
//! on is silently violated.
//!
//! Per the story's guidance the error variant for this case is
//! `StoryError::UnknownStatus { value }`, the "leftover legacy value"
//! used in the UAT walkthrough is `tested`, and the four accepted values
//! come from the schema's `status.enum`.

use std::fs;

use agentic_story::{Story, StoryError};
use tempfile::TempDir;

/// Fixture identical to a valid story except `status: tested` — a legacy
/// value the new enum does not include (see the story's bad-status
/// walkthrough). The loader must reject with a typed error naming the
/// offending value.
const INVALID_STATUS_YAML: &str = r#"id: 42
title: "Fixture with a legacy status value"

outcome: |
  A developer loads this fixture and observes a typed error naming the
  offending status value.

status: tested

patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_invalid_status_enum_is_rejected.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Run the loader against this fixture and observe the error.

guidance: |
  Fixture authored inline for the invalid-status test.

depends_on: []
"#;

#[test]
fn load_invalid_status_enum_is_rejected() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("42.yml");
    fs::write(&path, INVALID_STATUS_YAML).expect("write fixture");

    let result = Story::load(&path);
    let err = result.expect_err(
        "a story with an out-of-enum `status` value must be rejected",
    );

    // The error must name both the field (`status`) and the offending
    // value (`tested`) so the error message is actionable without
    // opening the schema.
    match err {
        StoryError::UnknownStatus { ref value } => {
            assert_eq!(
                value, "tested",
                "UnknownStatus must carry the offending value verbatim; got value={value:?}"
            );
        }
        other => panic!(
            "expected StoryError::UnknownStatus naming `tested`, got {other:?}"
        ),
    }
}
