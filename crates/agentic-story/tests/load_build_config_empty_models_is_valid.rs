//! Story 17 acceptance test: `models: []` is a valid author declaration,
//! and so is omitting the `models:` key under an otherwise-present
//! `build_config:` block.
//!
//! Justification (from stories/17.yml): proves `models: []` is a valid
//! author declaration, not a schema violation — a story whose
//! `build_config.models` is an empty array (or omits the `models:` key,
//! falling back to `Vec::new()`) loads into a `Story` whose
//! `build_config.models` is `vec![]`. The semantic is "author has no
//! opinion on model; runtime picks a default," and is distinct from
//! `build_config` being entirely absent (which is "author has no
//! opinion on ANY of this"). Without this, authors would be forced to
//! either omit build_config entirely (losing their budget opinion) or
//! to invent a sentinel model string, neither of which matches the
//! outcome's "declares their budget and model selection" — the two
//! are independently optional.
//!
//! Per the story's guidance the two sub-fields are independently
//! optional from the author's point of view: `max_inner_loop_iterations`
//! is REQUIRED whenever `build_config:` is present, but `models:`
//! defaults to `Vec::new()`. The schema description says "Empty array
//! means the author declined to pin; runtime picks a default. Distinct
//! from build_config being absent." Red today is compile-red: neither
//! `BuildConfig` nor `Story::build_config` yet exist.

use std::fs;

use agentic_story::{BuildConfig, Story};
use tempfile::TempDir;

/// Fixture whose `build_config.models` is an explicit empty array.
/// Expected: `Some(BuildConfig { max_inner_loop_iterations: 3, models:
/// vec![] })`, distinct from the `None` case pinned by
/// `load_absent_build_config_is_none_not_default_error.rs`.
const EXPLICIT_EMPTY_YAML: &str = r#"id: 1706
title: "Fixture with build_config.models = []"

outcome: |
  A developer loads this fixture and observes build_config =
  Some(BuildConfig { max_inner_loop_iterations: 3, models: vec![] }).

status: proposed

patterns: []

build_config:
  max_inner_loop_iterations: 3
  models: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_build_config_empty_models_is_valid.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Load this fixture and observe Some(BuildConfig { ..., models: [] }).

guidance: |
  Fixture authored inline for the empty-models test. Not a real story.

depends_on: []
"#;

/// Fixture whose `build_config:` block omits the `models:` key entirely.
/// Expected: same typed value as the explicit-empty case — `models`
/// defaults to `Vec::new()`, NOT a schema violation.
const OMIT_MODELS_YAML: &str = r#"id: 1707
title: "Fixture with build_config but no models key"

outcome: |
  A developer loads this fixture and observes build_config =
  Some(BuildConfig { max_inner_loop_iterations: 3, models: vec![] })
  even though the YAML has no `models:` key.

status: proposed

patterns: []

build_config:
  max_inner_loop_iterations: 3

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_build_config_empty_models_is_valid.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Load this fixture and observe Some(BuildConfig { ..., models: [] }).

guidance: |
  Fixture authored inline for the omit-models-key test. Not a real story.

depends_on: []
"#;

#[test]
fn load_build_config_with_explicit_empty_models_is_some_with_empty_vec() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("1706.yml");
    fs::write(&path, EXPLICIT_EMPTY_YAML).expect("write fixture");

    let story: Story = Story::load(&path).expect(
        "a story whose build_config.models is an explicit empty array must load",
    );

    // The outcome: Some(...) with empty models, distinct from None.
    let bc = story.build_config.as_ref().unwrap_or_else(|| {
        panic!(
            "build_config with `models: []` must parse as Some, NOT None; \
             got {:?}",
            story.build_config
        )
    });
    assert_eq!(
        bc.max_inner_loop_iterations, 3,
        "max_inner_loop_iterations must round-trip as 3; got {}",
        bc.max_inner_loop_iterations
    );
    assert!(
        bc.models.is_empty(),
        "explicit `models: []` must parse as an empty Vec; got {:?}",
        bc.models
    );
    assert_eq!(
        bc,
        &BuildConfig {
            max_inner_loop_iterations: 3,
            models: Vec::new(),
        },
        "explicit empty models must round-trip as BuildConfig {{ \
         max_inner_loop_iterations: 3, models: vec![] }}; got {bc:?}"
    );
}

#[test]
fn load_build_config_omitting_models_key_defaults_to_empty_vec() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("1707.yml");
    fs::write(&path, OMIT_MODELS_YAML).expect("write fixture");

    let story: Story = Story::load(&path).expect(
        "a story whose build_config omits `models:` must load (models \
         defaults to Vec::new())",
    );

    // Omitting the models key is semantically identical to an explicit
    // empty array at this layer — the schema's `"default": []` applies.
    let bc = story.build_config.as_ref().unwrap_or_else(|| {
        panic!(
            "build_config present but with no `models:` key must parse as \
             Some (not None); got {:?}",
            story.build_config
        )
    });
    assert_eq!(
        bc.max_inner_loop_iterations, 3,
        "max_inner_loop_iterations must round-trip as 3 when models is \
         omitted; got {}",
        bc.max_inner_loop_iterations
    );
    assert!(
        bc.models.is_empty(),
        "omitting `models:` under build_config must default to an empty \
         Vec; got {:?}",
        bc.models
    );
}

#[test]
fn load_build_config_with_empty_models_is_distinct_from_absent_build_config() {
    // Cross-case: the present-empty case must be observationally
    // distinct from the absent case. This is the observable that pins
    // "author has no opinion on models" as separate from "author has
    // no opinion on any of this."
    let dir = TempDir::new().expect("create temp dir");

    let present_path = dir.path().join("1706.yml");
    fs::write(&present_path, EXPLICIT_EMPTY_YAML).expect("write present fixture");
    let present: Story = Story::load(&present_path).expect("present fixture must load");

    // Absent fixture: no build_config: block at all.
    let absent_yaml = r#"id: 1708
title: "Fixture omitting build_config entirely"

outcome: |
  A developer loads this fixture and observes build_config = None.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_build_config_empty_models_is_valid.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Load this fixture and observe None.

guidance: |
  Fixture authored inline for the cross-case test. Not a real story.

depends_on: []
"#;
    let absent_path = dir.path().join("1708.yml");
    fs::write(&absent_path, absent_yaml).expect("write absent fixture");
    let absent: Story = Story::load(&absent_path).expect("absent fixture must load");

    assert!(
        present.build_config.is_some(),
        "`build_config: {{ models: [] }}` must round-trip as Some(...), \
         NOT None; got {:?}",
        present.build_config
    );
    assert!(
        absent.build_config.is_none(),
        "a fixture with no `build_config:` block must round-trip as \
         None; got {:?}",
        absent.build_config
    );
    assert_ne!(
        present.build_config, absent.build_config,
        "empty-models (Some(...)) and absent (None) must be \
         observationally distinct; got present={:?}, absent={:?}",
        present.build_config, absent.build_config
    );
}
