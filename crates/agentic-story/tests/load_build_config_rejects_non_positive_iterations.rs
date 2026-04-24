//! Story 17 acceptance test: the non-positive-budget guard on
//! `build_config.max_inner_loop_iterations`.
//!
//! Justification (from stories/17.yml): proves the non-positive budget
//! guard — a story whose `build_config.max_inner_loop_iterations` is
//! `0`, a negative integer, or a non-integer (string, float, null) is
//! rejected by the loader with a typed error whose payload names the
//! `build_config.max_inner_loop_iterations` field AND the offending
//! value. The contract is "positive integer"; `0` is nonsensical and
//! negatives are clearly malformed. Without this, a typo in the story
//! YAML (`-5` pasted as a budget) would load as a signed int and the
//! runtime (story 19's scope) would either underflow, spin forever, or
//! treat negative as unbounded — each worse than failing loud at load.
//!
//! Per the story's guidance the new typed-error variants extending
//! `StoryError` are `BuildConfigInvalidIterations { value: i64 }` for
//! zero / negative / out-of-`u32`-range inputs, and
//! `BuildConfigTypeMismatch { field, expected, found }` for a string /
//! null / float where an integer is expected. Red today is compile-red:
//! neither variant yet exists on `StoryError`, so the scaffold's
//! `match` arms do not resolve.

use std::fs;

use agentic_story::{Story, StoryError};
use tempfile::TempDir;

/// Build a fixture YAML whose `build_config.max_inner_loop_iterations`
/// takes the supplied literal value. Every other field is a fixed valid
/// shell so the only reason the loader can fail is the iterations value.
fn fixture_with_iterations(iterations_literal: &str, id: u32) -> String {
    // NOTE: do NOT interpolate `iterations_literal` into the `title:`
    // line — callers pass values like `"five"` (quoted) to exercise the
    // type-mismatch path, and inlining those inside a double-quoted
    // YAML scalar would break YAML parsing BEFORE the loader gets to
    // reject the iterations value on its merits. The title is fixed;
    // only the iterations value varies.
    format!(
        r#"id: {id}
title: Fixture exercising build_config.max_inner_loop_iterations rejection

outcome: |
  A developer loads this fixture and observes a typed error naming the
  build_config.max_inner_loop_iterations field and the offending value.

status: proposed

patterns: []

build_config:
  max_inner_loop_iterations: {iterations_literal}
  models: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_build_config_rejects_non_positive_iterations.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Run the loader against this fixture and observe the typed error.

guidance: |
  Fixture authored inline for the non-positive iterations test. Not a
  real story.

depends_on: []
"#
    )
}

#[test]
fn load_build_config_rejects_zero_iterations_with_typed_error_naming_field_and_value() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("1703.yml");
    fs::write(&path, fixture_with_iterations("0", 1703)).expect("write fixture");

    let err = Story::load(&path)
        .expect_err("a build_config.max_inner_loop_iterations of 0 must be rejected");

    match err {
        StoryError::BuildConfigInvalidIterations { value } => {
            assert_eq!(
                value, 0,
                "BuildConfigInvalidIterations must carry the offending \
                 value verbatim; got value={value}"
            );
        }
        other => {
            let rendered = other.to_string();
            assert!(
                rendered.contains("build_config.max_inner_loop_iterations")
                    && rendered.contains('0'),
                "a non-BuildConfigInvalidIterations variant is acceptable only \
                 if its rendered message names both \
                 `build_config.max_inner_loop_iterations` AND the offending \
                 value `0`; got {rendered:?}"
            );
            panic!(
                "expected StoryError::BuildConfigInvalidIterations {{ value: 0 }}; \
                 got {other:?}"
            );
        }
    }
}

#[test]
fn load_build_config_rejects_negative_iterations_with_typed_error_carrying_signed_value() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("1704.yml");
    fs::write(&path, fixture_with_iterations("-5", 1704)).expect("write fixture");

    let err = Story::load(&path)
        .expect_err("a negative build_config.max_inner_loop_iterations must be rejected");

    match err {
        StoryError::BuildConfigInvalidIterations { value } => {
            assert_eq!(
                value, -5,
                "BuildConfigInvalidIterations must preserve the sign of \
                 the offending value so authors see `-5` back; got value={value}"
            );
        }
        other => {
            let rendered = other.to_string();
            assert!(
                rendered.contains("build_config.max_inner_loop_iterations")
                    && rendered.contains("-5"),
                "a non-BuildConfigInvalidIterations variant is acceptable only \
                 if its rendered message names both \
                 `build_config.max_inner_loop_iterations` AND `-5`; got \
                 {rendered:?}"
            );
            panic!(
                "expected StoryError::BuildConfigInvalidIterations {{ value: -5 }}; \
                 got {other:?}"
            );
        }
    }
}

#[test]
fn load_build_config_rejects_string_iterations_with_typed_type_mismatch() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("1705.yml");
    fs::write(&path, fixture_with_iterations("\"five\"", 1705)).expect("write fixture");

    let err = Story::load(&path)
        .expect_err("a string build_config.max_inner_loop_iterations must be rejected");

    match err {
        StoryError::BuildConfigTypeMismatch {
            field,
            expected,
            found,
        } => {
            assert!(
                field.contains("max_inner_loop_iterations"),
                "BuildConfigTypeMismatch.field must name \
                 `max_inner_loop_iterations` (ideally as \
                 `build_config.max_inner_loop_iterations`); got field={field:?}"
            );
            assert!(
                expected.to_lowercase().contains("int"),
                "BuildConfigTypeMismatch.expected must describe an integer \
                 type; got expected={expected:?}"
            );
            assert!(
                found.to_lowercase().contains("string"),
                "BuildConfigTypeMismatch.found must describe the offending \
                 `string` type; got found={found:?}"
            );
        }
        other => {
            let rendered = other.to_string();
            assert!(
                rendered.contains("max_inner_loop_iterations")
                    && (rendered.contains("string") || rendered.contains("invalid type")),
                "a non-BuildConfigTypeMismatch variant is acceptable only if \
                 its rendered message names `max_inner_loop_iterations` AND \
                 the string/type-mismatch context; got {rendered:?}"
            );
            panic!(
                "expected StoryError::BuildConfigTypeMismatch for a string \
                 iterations value; got {other:?}"
            );
        }
    }
}
