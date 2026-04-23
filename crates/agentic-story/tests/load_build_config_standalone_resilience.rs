//! Story 17 acceptance test: the loader's dependency floor did not
//! widen when `build_config` landed.
//!
//! Justification (from stories/17.yml): proves the loader's dependency
//! floor did not widen when the field landed — the `build_config`
//! loading path is exercised end-to-end from a test that links only
//! against `agentic-story`, `serde`, `serde_yaml`, and `tempfile`. No
//! `agentic-cli`, no `agentic-runtime`, no `agentic-store`, no LLM
//! subprocess. Story 6's loader is a parse/validate step on the
//! critical path; adding a new optional field must not drag in
//! orchestration or runtime crates, because the first consumer to need
//! the field (the runtime, story 19) would then circularly depend on
//! its own configuration source. Without this, the "schema field lands
//! first, runtime consumes later" split this story explicitly draws
//! blurs at the dependency-graph level and story 19 inherits an
//! architectural papercut.
//!
//! Per the story's guidance the `agentic-story` crate stays a leaf —
//! the loader is the parse/validate step and must not depend on
//! orchestration or runtime crates. This scaffold takes two shapes:
//!
//!   1. Behavioural: exercises `build_config` loading end-to-end
//!      (round-trip of a populated block, default-to-None of an absent
//!      block) through `Story::load` alone — no CLI, no runtime, no
//!      store, no LLM subprocess.
//!   2. Structural: pins the target crate's `Cargo.toml` — the crate
//!      under test must not declare a dependency on `agentic-cli`,
//!      `agentic-runtime`, `agentic-store`, nor on any LLM / HTTP
//!      client that would widen the critical-path floor.
//!
//! Red today is compile-red: `BuildConfig` / `Story::build_config` do
//! not yet exist on `agentic-story`, so the behavioural assertion's
//! `use` does not resolve.

use std::fs;
use std::path::PathBuf;

use agentic_story::{BuildConfig, Story};
use tempfile::TempDir;

const POPULATED_YAML: &str = r#"id: 1709
title: "Standalone-resilience fixture with a populated build_config"

outcome: |
  A developer running only against agentic-story, serde, serde_yaml,
  and tempfile can load this fixture and read the build_config back.

status: proposed

patterns: []

build_config:
  max_inner_loop_iterations: 4
  models:
    - claude-haiku-4-5

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_build_config_standalone_resilience.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Load this fixture using only the standalone dep set; observe the
    build_config round-trips.

guidance: |
  Fixture authored inline for the standalone-resilience test. Not a
  real story.

depends_on: []
"#;

const ABSENT_YAML: &str = r#"id: 1710
title: "Standalone-resilience fixture with no build_config block"

outcome: |
  A developer running only against agentic-story, serde, serde_yaml,
  and tempfile can load this fixture and observe build_config = None.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_build_config_standalone_resilience.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Load this fixture using only the standalone dep set; observe None.

guidance: |
  Fixture authored inline for the standalone-resilience test. Not a
  real story.

depends_on: []
"#;

#[test]
fn build_config_loads_end_to_end_with_standalone_dep_set() {
    // Behavioural pin: the round-trip and the absence-default both fire
    // through `Story::load` only — no CLI, no runtime, no store, no
    // LLM subprocess. This function's `use` list at the top of the
    // file is the dep surface it exercises: `agentic_story`,
    // `tempfile`, and `std`. `serde` / `serde_yaml` participate
    // transitively through `Story::load` but are not named here (they
    // are `agentic-story`'s own `[dependencies]`, which is the point —
    // the test's direct dep floor is `agentic-story` + `tempfile`).
    let dir = TempDir::new().expect("create temp dir");

    // Populated: Some(BuildConfig { 4, ["claude-haiku-4-5"] }).
    let populated_path = dir.path().join("1709.yml");
    fs::write(&populated_path, POPULATED_YAML).expect("write populated fixture");
    let populated = Story::load(&populated_path).expect("populated fixture must load");
    let expected_populated = BuildConfig {
        max_inner_loop_iterations: 4,
        models: vec!["claude-haiku-4-5".to_string()],
    };
    assert_eq!(
        populated.build_config,
        Some(expected_populated),
        "populated build_config must round-trip via Story::load alone \
         (no runtime/store/cli crates in the loop); got {:?}",
        populated.build_config
    );

    // Absent: None.
    let absent_path = dir.path().join("1710.yml");
    fs::write(&absent_path, ABSENT_YAML).expect("write absent fixture");
    let absent = Story::load(&absent_path).expect("absent fixture must load");
    assert!(
        absent.build_config.is_none(),
        "absent build_config must round-trip as None via Story::load \
         alone; got {:?}",
        absent.build_config
    );
}

#[test]
fn agentic_story_cargo_toml_does_not_depend_on_orchestration_crates() {
    // Structural pin: the target crate's Cargo.toml must NOT declare a
    // dependency on any orchestration-layer crate, an LLM subprocess
    // crate, or an HTTP client. The forbidden list is derived from the
    // justification: "No agentic-cli, no agentic-runtime, no
    // agentic-store, no LLM subprocess."
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let text = fs::read_to_string(&manifest).unwrap_or_else(|e| {
        panic!(
            "must be able to read agentic-story's Cargo.toml at {}: {e}",
            manifest.display()
        )
    });

    // Split into `[dependencies]` vs `[dev-dependencies]` sections so
    // the dev-deps (tempfile) don't trip the check. `tempfile` is a
    // permitted dev-dep per the justification's dep list.
    let mut current_section = String::new();
    let mut runtime_deps_text = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            current_section = trimmed.to_string();
            continue;
        }
        if current_section == "[dependencies]" {
            runtime_deps_text.push_str(line);
            runtime_deps_text.push('\n');
        }
    }

    // Forbidden crates under `[dependencies]` of agentic-story:
    //   - agentic-cli / agentic-runtime / agentic-store — named in the
    //     justification directly.
    //   - reqwest / hyper / ureq / isahc — any HTTP client is an LLM
    //     subprocess in disguise for the story-loader.
    //   - tokio — the loader is sync; importing tokio would imply
    //     async I/O and widen the floor.
    for forbidden in [
        "agentic-cli",
        "agentic-runtime",
        "agentic-store",
        "reqwest",
        "hyper",
        "ureq",
        "isahc",
        "tokio",
    ] {
        assert!(
            !runtime_deps_text.contains(forbidden),
            "agentic-story's `[dependencies]` must NOT include `{forbidden}` \
             — the loader is a standalone parse/validate step (story 17 \
             guidance: \"loader stays a leaf\"). Offending dependencies \
             block:\n{runtime_deps_text}"
        );
    }
}
