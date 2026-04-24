//! Story 6 acceptance test (amendment — story 21 trigger): the loader
//! rejects cycles in the `superseded_by` edge set — both multi-hop and
//! self-loop — with a typed `StoryError::SupersededByCycle` naming at
//! least one participating id.
//!
//! Justification (from stories/6.yml): proves cycle defence on
//! supersession edges — a `stories/` directory where `A.superseded_by =
//! B`, `B.superseded_by = C`, `C.superseded_by = A` (or the degenerate
//! self-loop `A.superseded_by = A`) is rejected with a typed
//! `StoryError::SupersededByCycle` naming at least one participating id.
//! Supersession cycles are structurally distinct from `depends_on`
//! cycles (same graph shape, different edge label) and the loader
//! rejects both independently — a story may participate in one edge set
//! without the other. Without this, the ancestor gate's chain-walk
//! (story 11's amendment) would loop forever when evaluating health on
//! a descendant whose retired ancestor pointed into a cyclically-linked
//! successor set — strictly worse than a loud load-time refusal.
//!
//! Red today is compile-red: `StoryError::SupersededByCycle` does not
//! yet exist as a variant on the loader's error enum, so the pattern
//! match below does not resolve.

use std::fs;
use std::path::Path;

use agentic_story::{Story, StoryError};
use tempfile::TempDir;

fn write_retired(dir: &Path, id: u32, successor: Option<u32>) {
    let successor_line = match successor {
        Some(s) => format!("superseded_by: {s}\n\n"),
        None => String::new(),
    };
    let body = format!(
        r#"id: {id}
title: "Supersession cycle fixture {id}"

outcome: |
  A retired fixture for the supersession-cycle test.

status: retired

{successor_line}patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_superseded_by_cycle_is_rejected.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Run the directory loader and observe the cycle error.

guidance: |
  Fixture authored inline for the supersession-cycle test.

depends_on: []
"#
    );
    fs::write(dir.join(format!("{id}.yml")), body).expect("write fixture");
}

#[test]
fn load_superseded_by_multi_hop_cycle_is_rejected() {
    // Arrange: three retired stories forming A -> B -> C -> A on the
    // superseded_by edge set.
    let tmp = TempDir::new().expect("create temp dir");
    write_retired(tmp.path(), 100, Some(101));
    write_retired(tmp.path(), 101, Some(102));
    write_retired(tmp.path(), 102, Some(100));

    let result = Story::load_dir(tmp.path());
    let err = result.expect_err(
        "a superseded_by cycle across three stories must be rejected at load time",
    );

    match err {
        StoryError::SupersededByCycle { ref participants } => {
            let hit = participants
                .iter()
                .any(|id| *id == 100 || *id == 101 || *id == 102);
            assert!(
                hit,
                "SupersededByCycle must name at least one of {{100,101,102}}; \
                 got {participants:?}"
            );
        }
        other => panic!(
            "expected StoryError::SupersededByCycle naming one of 100/101/102, \
             got {other:?}"
        ),
    }
}

#[test]
fn load_superseded_by_self_loop_is_rejected() {
    // Arrange: a single retired story whose superseded_by points at
    // itself — the degenerate cycle shape named in the justification.
    let tmp = TempDir::new().expect("create temp dir");
    write_retired(tmp.path(), 200, Some(200));

    let result = Story::load_dir(tmp.path());
    let err = result.expect_err(
        "a superseded_by self-loop must be rejected at load time",
    );

    match err {
        StoryError::SupersededByCycle { ref participants } => {
            assert!(
                participants.contains(&200),
                "SupersededByCycle must name the self-looping id 200; got \
                 {participants:?}"
            );
        }
        other => panic!(
            "expected StoryError::SupersededByCycle naming 200, got {other:?}"
        ),
    }
}
