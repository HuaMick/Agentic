//! Story 17 acceptance test: the happy path for the new optional
//! `build_config` field.
//!
//! Justification (from stories/17.yml): proves the happy path for the new
//! optional field — a story YAML carrying a valid
//! `build_config: {max_inner_loop_iterations: N, models: ["<string>", ...]}`
//! loads through `Story::load` into a typed `Story` value whose
//! `build_config` is `Some(BuildConfig { max_inner_loop_iterations: N,
//! models: vec!["<string>", ...] })`. The int is preserved; the models
//! vector preserves order and content; no other story field is perturbed
//! alongside the new block. Without this, the "author declares, system
//! respects" contract the outcome promises has no proof.
//!
//! Per the story's guidance the new type is `pub struct BuildConfig { pub
//! max_inner_loop_iterations: u32, pub models: Vec<String> }`, and the
//! `Story` struct grows a `pub build_config: Option<BuildConfig>` field.
//! Red today is compile-red: neither `BuildConfig` nor
//! `Story::build_config` yet exist in `agentic-story`, so the scaffold's
//! `use` and its struct-field access do not resolve.

use std::fs;

use agentic_story::{BuildConfig, Status, Story};
use tempfile::TempDir;

/// Fixture carrying a valid `build_config:` block plus every other
/// top-level field populated with non-default content, so this test can
/// simultaneously prove that (a) `build_config` round-trips and (b) the
/// other nine top-level fields are not perturbed by the new block's
/// presence.
const ROUND_TRIP_YAML: &str = r#"id: 1701
title: "Fixture for the build_config round-trip test"

outcome: |
  A developer can load this fixture from disk and observe the
  build_config field preserved in memory exactly as authored, with
  every other story field also parsing cleanly.

status: proposed

patterns:
  - standalone-resilient-library

build_config:
  max_inner_loop_iterations: 7
  models:
    - claude-sonnet-4-6
    - claude-opus-4-7

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_build_config_round_trips.rs
      justification: |
        The very scaffold you are reading. Present so this fixture is
        itself schema-valid.
  uat: |
    Load this fixture, print the build_config field, confirm it matches
    the authored values.

guidance: |
  Fixture authored inline for the build_config round-trip test. Not a
  real story.

related_files:
  - crates/agentic-story/src/**

depends_on:
  - 6
"#;

#[test]
fn load_build_config_round_trips_iterations_and_models_in_order() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("1701.yml");
    fs::write(&path, ROUND_TRIP_YAML).expect("write fixture");

    let story: Story = Story::load(&path).expect("valid story must load");

    // The new field round-trips as Some(BuildConfig { ... }) — the
    // author declared a block, so the typed value is present.
    let expected_models: Vec<String> = vec![
        "claude-sonnet-4-6".to_string(),
        "claude-opus-4-7".to_string(),
    ];
    let expected = BuildConfig {
        max_inner_loop_iterations: 7,
        models: expected_models.clone(),
    };
    assert_eq!(
        story.build_config,
        Some(expected),
        "build_config must round-trip as Some(BuildConfig {{ \
         max_inner_loop_iterations: 7, models: [...] }}); got {:?}",
        story.build_config
    );

    // Pull the field back out separately so the assertion error cites
    // the two sub-fields in isolation if the equality check regresses.
    let bc = story
        .build_config
        .as_ref()
        .expect("build_config must be Some after the above assert_eq");
    assert_eq!(
        bc.max_inner_loop_iterations, 7,
        "max_inner_loop_iterations must round-trip as 7; got {}",
        bc.max_inner_loop_iterations
    );
    assert_eq!(
        bc.models, expected_models,
        "models vector must round-trip with content AND order exactly \
         equal to the fixture; got {:?}",
        bc.models
    );

    // No other story field is perturbed by the presence of the new block.
    assert_eq!(story.id, 1701, "id must round-trip alongside build_config");
    assert_eq!(
        story.title, "Fixture for the build_config round-trip test",
        "title must round-trip alongside build_config"
    );
    assert!(
        story.outcome.starts_with("A developer can load this fixture"),
        "outcome must round-trip alongside build_config; got {:?}",
        story.outcome
    );
    assert_eq!(
        story.status,
        Status::Proposed,
        "status must round-trip alongside build_config"
    );
    assert_eq!(
        story.patterns,
        vec!["standalone-resilient-library".to_string()],
        "patterns must round-trip alongside build_config"
    );
    assert_eq!(
        story.acceptance.tests.len(),
        1,
        "acceptance.tests length must round-trip alongside build_config"
    );
    assert!(
        story.acceptance.uat.contains("Load this fixture"),
        "acceptance.uat must round-trip alongside build_config; got {:?}",
        story.acceptance.uat
    );
    assert!(
        story.guidance.starts_with("Fixture authored inline"),
        "guidance must round-trip alongside build_config; got {:?}",
        story.guidance
    );
    assert_eq!(
        story.depends_on,
        vec![6u32],
        "depends_on must round-trip alongside build_config; got {:?}",
        story.depends_on
    );
    assert_eq!(
        story.related_files,
        vec!["crates/agentic-story/src/**".to_string()],
        "related_files must round-trip alongside build_config; got {:?}",
        story.related_files
    );
}
