//! Story 6 acceptance test (amendment — story 17 trigger): a story YAML
//! that does NOT declare `build_config:` loads cleanly with
//! `story.build_config == None` — the loader does NOT synthesise a
//! default struct at parse time. The single public `DEFAULT_BUILD_CONFIG`
//! constant is what downstream consumers reach for at the point of
//! consumption.
//!
//! Justification (from stories/6.yml): proves the optionality decision
//! from story 17 — a story YAML that does NOT declare `build_config:`
//! loads cleanly into a `Story` whose `build_config` field is `None` —
//! not a typed error, not a silently-substituted default struct
//! synthesised at parse time, not a panic. Consumers that need a concrete
//! budget consult the public constant `DEFAULT_BUILD_CONFIG` (owned by
//! `agentic-story`; story 17 pins its value) and apply it at the point
//! of consumption, preserving the distinction between "author thought
//! about it and declined to pin" (`Some(BuildConfig {…})`) and "author
//! expressed no opinion" (`None`). Without this, a later refactor could
//! silently auto-fill defaults at load time and the author's intent (or
//! lack thereof) would become un-introspectable downstream.
//!
//! Red today is compile-red: `Story::build_config` does not yet exist,
//! nor does the `DEFAULT_BUILD_CONFIG` constant, so neither the struct
//! field access nor the constant import below resolves.

use std::fs;

use agentic_story::{BuildConfig, Story, DEFAULT_BUILD_CONFIG};
use tempfile::TempDir;

/// Fixture with no `build_config:` block at all. The loader must leave
/// `story.build_config` as `None`, not auto-fill a default struct.
const NO_BUILD_CONFIG_YAML: &str = r#"id: 42
title: "Fixture story with no build_config block"

outcome: |
  A story without a build_config declaration loads cleanly with the
  field reading back as None; no silent defaulting.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_build_config_optional_defaults_apply.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Load the fixture and confirm build_config is None.

guidance: |
  Fixture authored inline for the build_config optionality decision.

depends_on: []
"#;

#[test]
fn load_build_config_optional_defaults_apply() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("42.yml");
    fs::write(&path, NO_BUILD_CONFIG_YAML).expect("write fixture");

    let story: Story =
        Story::load(&path).expect("a story without a build_config block must still load cleanly");

    // Absent block => None. NOT a silently-substituted default struct.
    assert_eq!(
        story.build_config, None,
        "absent build_config must read back as None (author expressed no \
         opinion); got {:?}",
        story.build_config
    );

    // The defaults constant is what consumers reach for. It is the single
    // source of truth per story 17 — max_inner_loop_iterations: 5,
    // models: [] — and the loader must NOT have been the agent that
    // synthesised it into the parsed story.
    let defaults: &BuildConfig = &DEFAULT_BUILD_CONFIG;
    assert_eq!(
        defaults.max_inner_loop_iterations, 5,
        "DEFAULT_BUILD_CONFIG.max_inner_loop_iterations must be 5; got {}",
        defaults.max_inner_loop_iterations
    );
    assert!(
        defaults.models.is_empty(),
        "DEFAULT_BUILD_CONFIG.models must be empty; got {:?}",
        defaults.models
    );
}
