//! Story 6 acceptance test: loading a syntactically valid story yields a
//! `Story` value semantically equal to the file on disk.
//!
//! Justification (from stories/6.yml): proves the happy path — loading a
//! syntactically valid story whose fields match `schemas/story.schema.json`
//! yields a `Story` value whose observable fields (id, title, outcome,
//! status, patterns, acceptance.tests, acceptance.uat, guidance, depends_on)
//! are semantically equal to what the file on disk contains. Without this,
//! nothing downstream (the dashboard, the UAT gate, the recorder) can trust
//! that what they see in memory is what the author wrote on disk.
//!
//! Per the story's guidance the loader exposes `Story::load` for a single
//! file. Fixtures are built inline as string literals — the live
//! `stories/` directory is not touched.

use std::fs;

use agentic_story::{Status, Story};
use tempfile::TempDir;

const VALID_STORY_YAML: &str = r#"id: 42
title: "A valid fixture story for the loader round-trip test"

outcome: |
  A developer can load this fixture from disk and observe every field
  preserved in memory exactly as it was authored.

status: proposed

patterns:
  - standalone-resilient-library

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_valid_story_round_trips.rs
      justification: |
        The very scaffold you are reading. Present so this fixture is
        itself schema-valid.
  uat: |
    Read the printed Story, eyeball the fields, confirm they match.

guidance: |
  Fixture authored inline for the round-trip test. Not a real story.

depends_on: []
"#;

#[test]
fn load_valid_story_round_trips() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("42.yml");
    fs::write(&path, VALID_STORY_YAML).expect("write fixture");

    let story: Story = Story::load(&path).expect("valid story must load");

    assert_eq!(story.id, 42, "id must round-trip");
    assert_eq!(
        story.title, "A valid fixture story for the loader round-trip test",
        "title must round-trip"
    );
    assert!(
        story
            .outcome
            .starts_with("A developer can load this fixture"),
        "outcome must round-trip; got {:?}",
        story.outcome
    );
    assert_eq!(
        story.status,
        Status::Proposed,
        "status must round-trip as the Proposed variant"
    );
    assert_eq!(
        story.patterns,
        vec!["standalone-resilient-library".to_string()],
        "patterns array must round-trip in order"
    );

    assert_eq!(
        story.acceptance.tests.len(),
        1,
        "acceptance.tests length must round-trip; got {}",
        story.acceptance.tests.len()
    );
    let test = &story.acceptance.tests[0];
    assert_eq!(
        test.file.to_string_lossy(),
        "crates/agentic-story/tests/load_valid_story_round_trips.rs",
        "test file path must round-trip"
    );
    assert!(
        test.justification
            .contains("The very scaffold you are reading"),
        "test justification must round-trip; got {:?}",
        test.justification
    );

    assert!(
        story.acceptance.uat.contains("Read the printed Story"),
        "acceptance.uat must round-trip; got {:?}",
        story.acceptance.uat
    );
    assert!(
        story.guidance.starts_with("Fixture authored inline"),
        "guidance must round-trip; got {:?}",
        story.guidance
    );
    assert!(
        story.depends_on.is_empty(),
        "depends_on must round-trip as empty; got {:?}",
        story.depends_on
    );
}
