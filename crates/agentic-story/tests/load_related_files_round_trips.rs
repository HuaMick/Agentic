//! Story 9 acceptance test: the schema-and-loader happy path for
//! `related_files`.
//!
//! Justification (from stories/9.yml): proves the schema-and-loader happy
//! path for the new field — a story YAML whose `related_files` is a
//! non-empty array of strings loads into a `Story` value whose
//! `related_files` field is semantically equal to what's on disk,
//! preserving order. Without this the field is invisible downstream and
//! the classifier has nothing to intersect against.
//!
//! Per the story's guidance (schema change section) the loader surface
//! grows `pub related_files: Vec<String>` on `Story`, defaulting to an
//! empty vec on deserialization. The scaffold writes a fixture YAML with
//! an ordered, non-empty `related_files` array and asserts the in-memory
//! `Story::related_files` is `Vec<String>`-equal to the on-disk order.
//! Red today is compile-red via the missing `related_files` field on
//! `agentic_story::Story`.

use std::fs;

use agentic_story::Story;
use tempfile::TempDir;

/// Fixture with a deliberately ordered multi-entry `related_files` array.
/// Order must round-trip — downstream glob-matching may short-circuit on
/// first match, and a silent reorder would make that short-circuit
/// return a misleading "which pattern matched" answer.
const ROUND_TRIP_YAML: &str = r#"id: 91
title: "Fixture for the related_files round-trip test"

outcome: |
  A developer can load this fixture from disk and observe the
  related_files array preserved in memory exactly as it was authored,
  including the order of its entries.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_related_files_round_trips.rs
      justification: |
        The very scaffold you are reading. Present so this fixture is
        itself schema-valid.
  uat: |
    Read the loaded Story, eyeball the related_files field, confirm it
    matches the fixture on disk.

guidance: |
  Fixture authored inline for the related_files round-trip test. Not a
  real story.

related_files:
  - "crates/agentic-uat/src/**"
  - "schemas/story.schema.json"
  - "docs/decisions/0005-red-green-is-a-contract.md"

depends_on: []
"#;

#[test]
fn load_related_files_round_trips_non_empty_array_preserving_order() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("91.yml");
    fs::write(&path, ROUND_TRIP_YAML).expect("write fixture");

    let story: Story = Story::load(&path).expect("valid story must load");

    // Exact Vec<String> equality: content AND order must round-trip.
    let expected: Vec<String> = vec![
        "crates/agentic-uat/src/**".to_string(),
        "schemas/story.schema.json".to_string(),
        "docs/decisions/0005-red-green-is-a-contract.md".to_string(),
    ];
    assert_eq!(
        story.related_files, expected,
        "related_files must round-trip as a Vec<String> with content and \
         order exactly equal to the fixture on disk; got {:?}",
        story.related_files
    );
}
