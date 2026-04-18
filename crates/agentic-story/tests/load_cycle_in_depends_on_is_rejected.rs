//! Story 6 acceptance test: a directory whose stories' `depends_on` edges
//! form a cycle is rejected by the directory-loading entry point with a
//! typed error that names at least one participating story id.
//!
//! Justification (from stories/6.yml): proves load-time cycle detection —
//! a directory containing stories whose `depends_on` edges form a cycle
//! (e.g. 10 → 11 → 10, or a self-loop 12 → 12) is rejected by the
//! directory-loading entry point with a typed error that names at least
//! one story id in the cycle. Without this, a cycle slips past parse time
//! and surfaces as an infinite loop (or stack overflow) the first time the
//! dashboard or scheduler walks the graph.
//!
//! Per the story's guidance cycle detection runs only in the directory
//! path (a single file cannot meaningfully validate its edges). The
//! loader is `Story::load_dir`; the error variant carries at least one
//! id participating in the cycle.

use std::fs;

use agentic_story::{Story, StoryError};
use tempfile::TempDir;

fn write_story(dir: &std::path::Path, id: u32, depends_on: &[u32]) {
    let depends_yaml = if depends_on.is_empty() {
        "[]".to_string()
    } else {
        let csv: Vec<String> = depends_on.iter().map(|d| d.to_string()).collect();
        format!("[{}]", csv.join(", "))
    };
    let body = format!(
        r#"id: {id}
title: "Cycle fixture story {id}"

outcome: |
  A fixture in a cycle-detection test.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_cycle_in_depends_on_is_rejected.rs
      justification: |
        Present so this fixture is schema-valid in isolation.
  uat: |
    Run the directory loader against this fixture set.

guidance: |
  Fixture authored inline for the cycle-detection test.

depends_on: {depends_yaml}
"#
    );
    fs::write(dir.join(format!("{id}.yml")), body).expect("write fixture");
}

#[test]
fn load_cycle_in_depends_on_is_rejected() {
    // Arrange: three stories forming 10 -> 11 -> 12 -> 10.
    let tmp = TempDir::new().expect("create temp dir");
    write_story(tmp.path(), 10, &[11]);
    write_story(tmp.path(), 11, &[12]);
    write_story(tmp.path(), 12, &[10]);

    // Act: load the whole directory — cycle detection is directory-scope.
    let result = Story::load_dir(tmp.path());
    let err = result.expect_err(
        "a directory whose depends_on edges form a cycle must be rejected",
    );

    // Assert: the error is the typed cycle variant, and names at least
    // one of the three participating ids.
    match err {
        StoryError::DependsOnCycle { ref participants } => {
            let hit = participants.iter().any(|id| *id == 10 || *id == 11 || *id == 12);
            assert!(
                hit,
                "DependsOnCycle must name at least one of {{10,11,12}}; got {participants:?}"
            );
        }
        other => panic!(
            "expected StoryError::DependsOnCycle naming one of 10/11/12, got {other:?}"
        ),
    }
}

#[test]
fn load_self_loop_in_depends_on_is_rejected() {
    // Arrange: a single story whose depends_on points at itself.
    let tmp = TempDir::new().expect("create temp dir");
    write_story(tmp.path(), 7, &[7]);

    let result = Story::load_dir(tmp.path());
    let err = result.expect_err(
        "a story whose depends_on points at itself must be rejected",
    );

    match err {
        StoryError::DependsOnCycle { ref participants } => {
            assert!(
                participants.contains(&7),
                "DependsOnCycle must name the self-looping id 7; got {participants:?}"
            );
        }
        other => panic!(
            "expected StoryError::DependsOnCycle naming 7, got {other:?}"
        ),
    }
}
