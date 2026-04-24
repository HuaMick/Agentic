//! Story 6 acceptance test (amendment — story 21 trigger): the fifth enum
//! value `retired` round-trips through the loader, together with its two
//! companion optional fields `superseded_by: <integer>` and
//! `retired_reason: <string>`.
//!
//! Justification (from stories/6.yml): proves the fifth enum value
//! `retired` round-trips through the loader — a story file whose `status`
//! field is the string `retired` loads cleanly into a typed `Story` value
//! whose `status` reads back as `Retired`. A retired story may carry an
//! optional `superseded_by: <integer>` edge pointing at the successor
//! that inherited its health responsibility; when present,
//! `story.superseded_by` reads back as `Some(<id>)`; when absent, it
//! reads back as `None` (terminal retirement — a legitimate shape for
//! experiments abandoned without replacement). A retired story may also
//! carry an optional `retired_reason: String` prose field documenting
//! why the retirement happened; it round-trips verbatim when present,
//! `None` when absent. Without this test, the vocabulary story 21
//! introduces (retired, supersession, era) has no loader-level anchor
//! and every downstream consumer (dashboard canopy view, ancestor gate's
//! chain-walk) would have to re-invent the YAML shape from observation.
//!
//! Red today is compile-red: `Status::Retired`, `Story::superseded_by`,
//! and `Story::retired_reason` do not yet exist in `agentic-story`, so
//! the `use` import and the struct-field accesses below do not resolve.

use std::fs;

use agentic_story::{Status, Story};
use tempfile::TempDir;

/// Fixture with `status: retired`, a `superseded_by: 1` edge, and a
/// non-empty `retired_reason:` block. The loader must preserve all three
/// on round-trip.
const RETIRED_WITH_SUCCESSOR_YAML: &str = r#"id: 7
title: "Fossil story, retired in favour of a successor"

outcome: |
  A retired story sits off-tree and names its successor; the loader
  round-trips all three new fields (status, superseded_by, retired_reason).

status: retired

superseded_by: 1

retired_reason: |
  Folded into story 1 during the 2026-04-20 consolidate-over-fragment
  pass. Kept on disk so downstream tooling can walk the supersession
  chain rather than interpret a missing id as a ghost.

patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_retired_status_is_accepted.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Run the loader against this fixture and observe the retired shape.

guidance: |
  Fixture authored inline for the retired-status round-trip test.

depends_on: []
"#;

/// Fixture with `status: retired` but NO `superseded_by` and NO
/// `retired_reason`. Terminal retirement — a legitimate shape for
/// experiments abandoned without a replacement. Both optional fields
/// must read back as `None`, not as typed errors.
const RETIRED_TERMINAL_YAML: &str = r#"id: 8
title: "Fossil story retired with no successor"

outcome: |
  An abandoned experiment retired without replacement. No successor
  pointer, no prose reason — still a valid shape.

status: retired

patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_retired_status_is_accepted.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Run the loader against this fixture and observe the terminal shape.

guidance: |
  Fixture authored inline for the terminal-retirement round-trip test.

depends_on: []
"#;

#[test]
fn load_retired_status_is_accepted() {
    // Successor case: retired story points at id 1 via superseded_by and
    // carries a non-empty retired_reason.
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("7.yml");
    fs::write(&path, RETIRED_WITH_SUCCESSOR_YAML).expect("write fixture");

    let story: Story =
        Story::load(&path).expect("a story with status: retired must load cleanly");

    assert_eq!(
        story.status,
        Status::Retired,
        "status: retired must round-trip as the Retired variant; got {:?}",
        story.status
    );

    assert_eq!(
        story.superseded_by,
        Some(1),
        "superseded_by: 1 must round-trip as Some(1); got {:?}",
        story.superseded_by
    );

    let reason = story
        .retired_reason
        .as_ref()
        .expect("retired_reason must round-trip as Some(...)");
    assert!(
        reason.contains("2026-04-20"),
        "retired_reason must round-trip verbatim; got {:?}",
        reason
    );
}

#[test]
fn load_retired_terminal_is_accepted_with_none_options() {
    // Terminal case: retired story carries neither superseded_by nor
    // retired_reason. Both optional fields must read back as None.
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("8.yml");
    fs::write(&path, RETIRED_TERMINAL_YAML).expect("write fixture");

    let story: Story = Story::load(&path).expect(
        "a story with status: retired and no superseded_by/retired_reason must load",
    );

    assert_eq!(
        story.status,
        Status::Retired,
        "status must round-trip as Retired; got {:?}",
        story.status
    );
    assert_eq!(
        story.superseded_by, None,
        "superseded_by absent must read back as None; got {:?}",
        story.superseded_by
    );
    assert_eq!(
        story.retired_reason, None,
        "retired_reason absent must read back as None; got {:?}",
        story.retired_reason
    );
}
