//! Story 17 acceptance test: optionality — absent `build_config:` loads
//! as `None`, NOT a synthesised default and NOT an error.
//!
//! Justification (from stories/17.yml): proves the optionality decision —
//! a story YAML that does NOT declare `build_config:` loads cleanly into
//! a `Story` whose `build_config` field is `None` — not a typed error,
//! not a silently-substituted default struct, not a panic. The loader
//! does not synthesize a `BuildConfig` value when the block is absent;
//! consumers that need a concrete budget consult `DEFAULT_BUILD_CONFIG`
//! and make the substitution themselves. Without this, the optionality
//! is prose-only: a later refactor could silently auto-fill defaults at
//! load time, hiding the author's intent (or lack thereof) from every
//! downstream reader.
//!
//! Per the story's guidance the field is NOT in the schema's top-level
//! `required` list, and the loader's default on deserialisation is
//! `None` (not `Some(DEFAULT_BUILD_CONFIG)`). Red today is compile-red:
//! `Story::build_config` does not yet exist on the struct, and
//! `DEFAULT_BUILD_CONFIG` is not yet a public constant the scaffold can
//! take the address of for the inequality check.

use std::fs;

use agentic_story::{Story, DEFAULT_BUILD_CONFIG};
use tempfile::TempDir;

/// Fixture that OMITS `build_config:` entirely — mirrors every story on
/// disk today. Must load without error; `build_config` must be `None`,
/// not `Some(DEFAULT_BUILD_CONFIG)` or any auto-filled struct.
const ABSENT_YAML: &str = r#"id: 1702
title: "Fixture omitting the build_config field"

outcome: |
  A developer can load this fixture whose YAML has no build_config key
  and observe a Story whose build_config is None.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_absent_build_config_is_none_not_default_error.rs
      justification: |
        The very scaffold you are reading. Present so this fixture is
        itself schema-valid.
  uat: |
    Load this fixture; confirm no error; confirm build_config is None.

guidance: |
  Fixture authored inline for the absent-is-None test. Not a real story.

depends_on: []
"#;

#[test]
fn load_absent_build_config_is_none_not_synthesised_default() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("1702.yml");
    fs::write(&path, ABSENT_YAML).expect("write fixture");

    // Load must succeed — omitting build_config is the shape every
    // existing story has today and must keep working.
    let story: Story =
        Story::load(&path).expect("a story that omits `build_config` must load successfully");

    // Absent block must produce None. The distinction between "author
    // had no opinion" (None) and "author accepted the defaults"
    // (Some(DEFAULT_BUILD_CONFIG)) is the load-bearing observable this
    // test pins; silently substituting the defaults at load time would
    // erase the former.
    assert!(
        story.build_config.is_none(),
        "a story YAML that omits `build_config:` must load with \
         `story.build_config == None`; got {:?}",
        story.build_config
    );

    // Explicitly rule out the "auto-filled defaults at load time"
    // regression — if a future refactor substitutes DEFAULT_BUILD_CONFIG
    // when the block is absent, this assertion fails loud.
    assert_ne!(
        story.build_config.as_ref(),
        Some(&DEFAULT_BUILD_CONFIG),
        "when `build_config:` is omitted the loader must NOT synthesise \
         `Some(DEFAULT_BUILD_CONFIG)` — consumers apply the default at the \
         point of consumption, so that 'author thought about it and \
         accepted the default' stays distinguishable from 'author never \
         thought about it'. Got {:?}",
        story.build_config
    );
}
