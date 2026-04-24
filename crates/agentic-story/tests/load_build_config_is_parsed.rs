//! Story 6 acceptance test (amendment — story 17 trigger): the optional
//! `build_config` block parses into a typed `Option<BuildConfig>` with
//! `max_inner_loop_iterations` and `models` preserved verbatim.
//!
//! Justification (from stories/6.yml): proves the happy path for the new
//! optional `build_config` field — a story YAML carrying
//! `build_config: {max_inner_loop_iterations: 5, models: []}` loads
//! through `Story::load` (and `Story::load_dir`) into a typed `Story`
//! whose `build_config` is `Some(BuildConfig { max_inner_loop_iterations:
//! 5, models: vec![] })`. A populated models array (e.g. `models:
//! ["claude-sonnet-4-6", "claude-opus-4-7"]`) preserves order and content
//! on round-trip. No other story field is perturbed when the block is
//! present. Without this, story 17's `build_config` contract has no
//! loader-level pinning and each downstream consumer would re-parse the
//! YAML shape independently, producing N slightly different readers of
//! the same block.
//!
//! Red today is compile-red: `BuildConfig` and `Story::build_config` do
//! not yet exist in `agentic-story`, so the `use` import and the
//! struct-field accesses below do not resolve.

use std::fs;

use agentic_story::{BuildConfig, Status, Story};
use tempfile::TempDir;

/// Minimal fixture: build_config declared with two models in a pinned
/// order. The loader must preserve both the integer and the vec on
/// round-trip, and must not perturb any other top-level field.
const BUILD_CONFIG_POPULATED_YAML: &str = r#"id: 42
title: "Fixture story carrying a populated build_config"

outcome: |
  A build_config block with iterations and an ordered models list round-
  trips through the loader without perturbing any other field.

status: under_construction

build_config:
  max_inner_loop_iterations: 5
  models:
    - claude-sonnet-4-6
    - claude-opus-4-7

patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_build_config_is_parsed.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Load the fixture and print build_config via Debug; observe the struct.

guidance: |
  Fixture authored inline for the build_config happy-path round-trip.

depends_on: []
"#;

#[test]
fn load_build_config_is_parsed() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("42.yml");
    fs::write(&path, BUILD_CONFIG_POPULATED_YAML).expect("write fixture");

    let story: Story =
        Story::load(&path).expect("a story with a valid build_config must load");

    let build_config: &BuildConfig = story
        .build_config
        .as_ref()
        .expect("build_config: {...} must round-trip as Some(...)");
    assert_eq!(
        build_config.max_inner_loop_iterations, 5,
        "max_inner_loop_iterations must round-trip verbatim; got {}",
        build_config.max_inner_loop_iterations
    );
    assert_eq!(
        build_config.models,
        vec![
            "claude-sonnet-4-6".to_string(),
            "claude-opus-4-7".to_string(),
        ],
        "models list must round-trip preserving order and content; got {:?}",
        build_config.models
    );

    // The new block must not perturb the other top-level fields.
    assert_eq!(story.id, 42, "id must round-trip unchanged");
    assert_eq!(
        story.status,
        Status::UnderConstruction,
        "status must round-trip unchanged"
    );
    assert_eq!(
        story.title,
        "Fixture story carrying a populated build_config",
        "title must round-trip unchanged"
    );
}
