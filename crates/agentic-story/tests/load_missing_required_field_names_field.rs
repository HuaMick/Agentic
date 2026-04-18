//! Story 6 acceptance test: a story file missing a required field is
//! rejected with a typed error whose payload names the missing field.
//!
//! Justification (from stories/6.yml): proves the typed-error contract on
//! structural failure — a story file missing a required field (e.g. no
//! `outcome:`) is rejected by the loader with a typed error whose payload
//! names the missing field in text a human can act on without opening a
//! schema file. Without this, schema drift during authoring surfaces as
//! cryptic serde errors that the story-writer agent and downstream tooling
//! cannot parse into a corrective action.
//!
//! Per the story's guidance the public error surface is a typed enum
//! (not `anyhow::Error`) with a variant for schema violations that names
//! the offending field. The assertions below match that contract.

use std::fs;

use agentic_story::{Story, StoryError};
use tempfile::TempDir;

/// A story fixture identical in shape to a valid story EXCEPT the required
/// `outcome:` field is absent. The loader must reject with a typed error
/// whose message names the missing field.
const MISSING_OUTCOME_YAML: &str = r#"id: 42
title: "Fixture missing the required outcome field"

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_missing_required_field_names_field.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Run the loader against this fixture and observe the error.

guidance: |
  Fixture authored inline for the missing-required-field test.

depends_on: []
"#;

#[test]
fn load_missing_required_field_names_field() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("42.yml");
    fs::write(&path, MISSING_OUTCOME_YAML).expect("write fixture");

    let result = Story::load(&path);
    let err = result.expect_err(
        "a story missing the required `outcome` field must be rejected",
    );

    // The error must be the typed schema-violation variant, not a generic
    // parse error or a bubbled-up I/O error. The variant carries the
    // offending field name so humans can act without opening the schema.
    match err {
        StoryError::SchemaViolation { ref field, .. } => {
            assert!(
                field.contains("outcome"),
                "SchemaViolation must name the missing field `outcome`; got field={field:?}"
            );
        }
        other => panic!(
            "expected StoryError::SchemaViolation naming `outcome`, got {other:?}"
        ),
    }
}
