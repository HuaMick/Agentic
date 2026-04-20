//! Story 15 acceptance test: `plan` emits one `PlanEntry` per acceptance
//! test, each with the five fixed top-level keys the justification names,
//! and a strict JSON round-trip rejects any extra keys.
//!
//! Justification (from stories/15.yml acceptance.tests[0]): proves the
//! plan shape — given a fixture story with two `acceptance.tests[]`
//! entries, `agentic test-build plan <id> --json` emits a JSON array of
//! exactly two plan entries, each carrying the top-level keys `file`,
//! `target_crate`, `justification`, `expected_red_path` (`compile` or
//! `runtime`), and `fixture_preconditions` (array). Extra keys are
//! rejected by a strict parser. Without this, downstream tooling has
//! nothing stable to parse.
//!
//! Red today is compile-red: `TestBuilder::plan` and `PlanEntry` are
//! the new API surface story 15 adds; neither exists in
//! `agentic-test-builder`'s `lib.rs` yet, so the scaffold fails
//! `cargo check` on unresolved items. Once build-rust lands the plan
//! function and the serialisable `PlanEntry` struct, this test
//! exercises it end-to-end.

use std::fs;
use std::path::PathBuf;

use agentic_story::Story;
use agentic_test_builder::{PlanEntry, TestBuilder};
use serde::Deserialize;
use tempfile::TempDir;

const STORY_ID: u32 = 99_015_001;

const FIXTURE_YAML: &str = r#"id: 99015001
title: "Fixture for story 15 plan-shape acceptance test"

outcome: |
  Fixture story used to prove `TestBuilder::plan` emits one
  structured entry per acceptance test.

status: proposed

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-crate/tests/returns_one_for_valid_input.rs
      justification: |
        Proves the fixture crate's `returns_one` function returns
        1 for a valid input; this is the observable the test pins.
    - file: crates/fixture-crate/tests/returns_two_for_other_input.rs
      justification: |
        Proves the fixture crate's `returns_two` function returns
        2 for the second documented input; pinned here so the
        planner emits two entries and the array-shape claim holds.
  uat: |
    Not executed by this scaffold; present so the fixture is
    schema-valid.

guidance: |
  Fixture-only. Not a real story.

depends_on: []
"#;

/// Strict deserialisation target. `deny_unknown_fields` is the teeth the
/// justification names — a plan that grew a sixth key would fail here,
/// not silently round-trip.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictPlanEntry {
    file: String,
    target_crate: String,
    justification: String,
    expected_red_path: String,
    fixture_preconditions: Vec<String>,
}

#[test]
fn plan_emits_one_entry_per_acceptance_test_with_strict_json_shape() {
    // Arrange: write a fixture story with exactly two acceptance.tests[]
    // entries to a tempdir, then load it via the shared story loader.
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    let story = Story::load(&story_path).expect("load fixture story");
    assert_eq!(
        story.acceptance.tests.len(),
        2,
        "fixture must have exactly two acceptance.tests[] entries"
    );

    // Act: call the NEW library surface. `TestBuilder::plan` must be a
    // pure read over the story — no I/O, no cargo shell-out.
    let plan: Vec<PlanEntry> = TestBuilder::plan(&story);

    // Assert: one PlanEntry per acceptance test, in order.
    assert_eq!(
        plan.len(),
        2,
        "plan must emit one entry per acceptance.tests[] entry"
    );

    // The plan must round-trip through a STRICT JSON parser: exactly the
    // five documented keys, no extras. This is the doc-blind-auditable
    // claim the justification pins.
    let json_value =
        serde_json::to_value(&plan).expect("PlanEntry must be serde::Serialize into JSON");
    let json_str = json_value.to_string();
    let strict: Vec<StrictPlanEntry> = serde_json::from_str(&json_str)
        .expect("plan JSON must round-trip through a deny_unknown_fields parser");

    assert_eq!(strict.len(), 2, "strict parse must see two entries");

    // Per-entry field content: each scaffold's file matches the story,
    // target_crate is derived from the path, justification is verbatim,
    // expected_red_path is one of the two documented values, and
    // fixture_preconditions is an array (possibly empty).
    for (idx, (planned, fixture)) in strict.iter().zip(story.acceptance.tests.iter()).enumerate() {
        assert_eq!(
            PathBuf::from(&planned.file),
            fixture.file,
            "plan[{idx}].file must equal the story's acceptance.tests[{idx}].file"
        );
        assert_eq!(
            planned.target_crate, "fixture-crate",
            "plan[{idx}].target_crate must be derived from `crates/<name>/tests/...`"
        );
        assert_eq!(
            planned.justification.trim(),
            fixture.justification.trim(),
            "plan[{idx}].justification must be the story's justification verbatim \
             (not paraphrased — the user reads the source)"
        );
        assert!(
            matches!(planned.expected_red_path.as_str(), "compile" | "runtime"),
            "plan[{idx}].expected_red_path must be 'compile' or 'runtime'; got {:?}",
            planned.expected_red_path
        );
        // fixture_preconditions is an array; empty is legal since the
        // fixture's guidance names no preconditions.
        assert!(
            planned.fixture_preconditions.is_empty()
                || planned
                    .fixture_preconditions
                    .iter()
                    .all(|p| !p.trim().is_empty()),
            "plan[{idx}].fixture_preconditions must be an array of non-empty strings; got {:?}",
            planned.fixture_preconditions
        );
    }
}
