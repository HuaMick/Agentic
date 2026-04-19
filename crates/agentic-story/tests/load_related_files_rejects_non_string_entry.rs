//! Story 9 acceptance test: the type boundary on `related_files` entries.
//!
//! Justification (from stories/9.yml): proves the type boundary — a
//! story YAML whose `related_files` is present but contains a non-string
//! entry (e.g. `[42]`, `[null]`, or a nested array) is rejected by the
//! loader with a typed error naming the field. Without this, a malformed
//! entry would surface downstream as a panic inside the dashboard's
//! glob-matching step rather than at parse time where the author can
//! act on it.
//!
//! Per the story's guidance the loader validates `related_files` is an
//! array of strings (anything more complex — glob syntax validation,
//! absolute-path rejection, etc. — is out of scope). The scaffold writes
//! a fixture with `related_files: [42]` (an integer entry) and asserts
//! the loader returns a typed error whose payload names the field.
//! Red today is compile-red via the missing `related_files` field on
//! `agentic_story::Story`; once implemented it becomes runtime-red if
//! the loader accepts the integer silently.

use std::fs;

use agentic_story::{Story, StoryError};
use tempfile::TempDir;

/// Fixture whose `related_files` contains a non-string entry. The YAML
/// itself is parseable (it's a valid YAML array); the loader must reject
/// it at the type-check layer with a typed error naming `related_files`.
const NON_STRING_ENTRY_YAML: &str = r#"id: 93
title: "Fixture with a non-string related_files entry"

outcome: |
  A developer loads this fixture and observes a typed error naming
  related_files as the field whose contents failed type-check.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_related_files_rejects_non_string_entry.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Run the loader against this fixture; observe the typed error.

guidance: |
  Fixture authored inline for the non-string related_files rejection
  test. Not a real story.

related_files:
  - 42

depends_on: []
"#;

#[test]
fn load_related_files_rejects_non_string_entry_with_typed_error_naming_field() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("93.yml");
    fs::write(&path, NON_STRING_ENTRY_YAML).expect("write fixture");

    let result = Story::load(&path);
    let err = result.expect_err(
        "a story whose `related_files` contains a non-string entry must be rejected",
    );

    // The error must be a typed loader variant (not a generic parse
    // error or a bubbled-up I/O error) whose payload names the
    // offending field so the author can act without opening the schema.
    // Accepts SchemaViolation { field } because that is the existing
    // typed-error shape for structural type mismatches on the Story
    // struct.
    let (field, message) = match &err {
        StoryError::SchemaViolation { field, message } => (field.clone(), message.clone()),
        other => {
            let rendered = other.to_string();
            panic!(
                "expected StoryError::SchemaViolation for a non-string `related_files` \
                 entry; got variant {other:?} rendered as {rendered:?}"
            );
        }
    };

    // The error must name `related_files` in either `field` or `message`.
    assert!(
        field.contains("related_files") || message.contains("related_files"),
        "SchemaViolation for a non-string `related_files` entry must name \
         the field `related_files` in either `field` or `message`; \
         got field={field:?}, message={message:?}"
    );

    // Critical: the rejection must be because the ENTRY is non-string,
    // NOT because the loader does not know about `related_files` at
    // all. Pre-story-9 the loader rejected `related_files` with a
    // `deny_unknown_fields` violation ("unknown field `related_files`");
    // that is NOT what this scaffold proves. The implementer must
    // declare `related_files: Vec<String>` on `RawStory` so the type
    // mismatch surfaces at the `Vec<String>` deserialisation layer —
    // where serde's error vocabulary is "invalid type" / "expected a
    // string" rather than "unknown field".
    let full = format!("{field} {message}");
    assert!(
        !full.contains("unknown field"),
        "non-string `related_files` entry must be rejected AT THE ENTRY's \
         type-check layer, not as a `deny_unknown_fields` unknown-field \
         rejection of the whole property. The loader must accept the \
         property and reject its contents. Got field={field:?}, \
         message={message:?}"
    );
    let mentions_type_or_value = full.contains("string")
        || full.contains("invalid type")
        || full.contains("expected")
        || full.contains("42");
    assert!(
        mentions_type_or_value,
        "SchemaViolation for a non-string `related_files` entry must carry a \
         diagnostic pointing at the entry's type or value (one of `string`, \
         `invalid type`, `expected`, or the offending value `42`); \
         got field={field:?}, message={message:?}"
    );
}
