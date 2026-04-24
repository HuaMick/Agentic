//! Story 6 acceptance test (amended 2026-04-23 for story 21 trigger): a
//! story whose `status` field is not one of the five enum values
//! `proposed | under_construction | healthy | unhealthy | retired` is
//! rejected with a typed error naming both the field and the offending
//! value.
//!
//! Justification (from stories/6.yml): proves enum-boundary enforcement —
//! a story file whose `status` field is a string not in the five-valued
//! enum `proposed | under_construction | healthy | unhealthy | retired`
//! is rejected with a typed error naming the field AND the offending
//! value. The canonical invalid example pinned by this test is `status:
//! deprecated` (a plausible-looking legacy value that is deliberately
//! NOT in the enum — distinct from `tested`, which the pre-amendment
//! test used and which is now a common near-miss but still invalid).
//! Without this, a typo in `status:` loads as a default or a catch-all
//! and the five-status model the dashboard depends on is silently
//! violated. This test's invalid example must remain invalid after any
//! future enum extension — pick a value with no realistic lifecycle
//! semantics.
//!
//! Red today is runtime-red for the `deprecated` case: the loader's
//! existing `status` matcher rejects anything other than the first four
//! names with `UnknownStatus { value: "deprecated" }`, which satisfies
//! the assertions here. The amendment's purpose is to pin the invalid
//! example as `deprecated` (not `tested`) so a future enum extension
//! that reused `tested` as a real value would not silently flip this
//! test from red to passing-but-wrong.
//!
//! When story 21's `retired` enum value ships, the loader must ALSO
//! accept `status: retired` cleanly — the second test case below pins
//! that direction and is runtime-red today (the loader rejects
//! `retired` with `UnknownStatus`, so the `expect` panics with the
//! wrong message).

use std::fs;

use agentic_story::{Status, Story, StoryError};
use tempfile::TempDir;

/// Fixture identical to a valid story except `status: deprecated` — a
/// plausible-looking legacy value the five-value enum does not include.
/// The loader must reject with a typed error naming the offending
/// value.
const INVALID_STATUS_YAML: &str = r#"id: 42
title: "Fixture with a legacy-looking status value"

outcome: |
  A developer loads this fixture and observes a typed error naming the
  offending status value.

status: deprecated

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

/// Fixture identical in shape but using `status: retired` — a value the
/// amended five-value enum DOES include. The loader must accept it and
/// round-trip the value as `Status::Retired`.
const RETIRED_STATUS_YAML: &str = r#"id: 42
title: "Fixture with the retired status value"

outcome: |
  A developer loads this fixture and observes the retired variant.

status: retired

patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_invalid_status_enum_is_rejected.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Run the loader against this fixture and observe Status::Retired.

guidance: |
  Fixture authored inline for the retired-is-valid counter-case.

depends_on: []
"#;

#[test]
fn load_invalid_status_enum_is_rejected() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("42.yml");
    fs::write(&path, INVALID_STATUS_YAML).expect("write fixture");

    let result = Story::load(&path);
    let err = result.expect_err("a story with an out-of-enum `status` value must be rejected");

    // The error must name both the field (`status`) and the offending
    // value (`deprecated`) so the error message is actionable without
    // opening the schema.
    match err {
        StoryError::UnknownStatus { ref value } => {
            assert_eq!(
                value, "deprecated",
                "UnknownStatus must carry the offending value verbatim; got \
                 value={value:?}"
            );
        }
        other => panic!(
            "expected StoryError::UnknownStatus naming `deprecated`, got \
             {other:?}"
        ),
    }
}

#[test]
fn load_retired_status_round_trips_as_variant() {
    // Amendment direction: the five-value enum INCLUDES `retired`.
    // Story 6's amendment requires the loader to accept this value as a
    // valid lifecycle variant; the sibling
    // `load_retired_status_is_accepted.rs` scaffold pins the full
    // round-trip including the companion optional fields. This
    // assertion pairs with the invalid-case above to pin the boundary
    // in ONE place: the five values accept, all others reject.
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("42.yml");
    fs::write(&path, RETIRED_STATUS_YAML).expect("write fixture");

    let story =
        Story::load(&path).expect("status: retired must load cleanly under the five-value enum");
    assert_eq!(
        story.status,
        Status::Retired,
        "status: retired must round-trip as Status::Retired; got {:?}",
        story.status
    );
}
