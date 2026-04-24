//! Story 6 acceptance test (amendment — story 21 trigger): a directory
//! where one story declares `superseded_by: <id>` but no sibling with
//! that id exists is rejected with a typed
//! `StoryError::SupersededByUnknown { source_id, target_id }` naming both
//! ids.
//!
//! Justification (from stories/6.yml): proves referential integrity on
//! supersession edges at directory-load time — a `stories/` directory
//! where one story declares `superseded_by: 999` but no `stories/999.yml`
//! exists is rejected with a typed `StoryError::SupersededByUnknown {
//! source_id, target_id }` naming both ids. The check runs in the
//! directory path (not the single-file path, which cannot meaningfully
//! validate cross-file edges — same rule this story already applies to
//! `depends_on`). Without this, a retired story whose successor was
//! itself retired-and-deleted (or mistyped) would silently bind to
//! nothing, and downstream consumers — dashboard's canopy rendering,
//! ancestor gate's chain walk — would follow a dangling pointer,
//! producing either a panic or a silently-wrong answer.
//!
//! Red today is compile-red: `StoryError::SupersededByUnknown` does not
//! yet exist as a variant on the loader's error enum, so the pattern
//! match below does not resolve.

use std::fs;
use std::path::Path;

use agentic_story::{Story, StoryError};
use tempfile::TempDir;

fn write_retired_with_successor(dir: &Path, id: u32, successor: u32) {
    let body = format!(
        r#"id: {id}
title: "Retired fixture pointing at id {successor}"

outcome: |
  A retired story whose superseded_by points at a sibling id.

status: retired

superseded_by: {successor}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_superseded_by_pointing_at_unknown_id_is_rejected.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Run the directory loader and observe the referential-integrity error.

guidance: |
  Fixture authored inline for the supersession unknown-target test.

depends_on: []
"#
    );
    fs::write(dir.join(format!("{id}.yml")), body).expect("write fixture");
}

#[test]
fn load_superseded_by_pointing_at_unknown_id_is_rejected() {
    // Arrange: one retired story whose superseded_by: 999 points at a
    // sibling that does not exist in the directory.
    let tmp = TempDir::new().expect("create temp dir");
    write_retired_with_successor(tmp.path(), 200, 999);

    // Act: the directory loader must enforce cross-file referential
    // integrity on supersession edges (single-file load cannot, per the
    // same rule depends_on already uses).
    let result = Story::load_dir(tmp.path());
    let err = result.expect_err(
        "a directory where superseded_by points at a non-existent id must be \
         rejected at load time",
    );

    // Assert: the error is the typed SupersededByUnknown variant, and it
    // names BOTH the source (the retired story) and the target (the
    // missing successor), so the author can fix either side.
    match err {
        StoryError::SupersededByUnknown {
            source_id,
            target_id,
        } => {
            assert_eq!(
                source_id, 200,
                "SupersededByUnknown.source_id must name the retired story; got {source_id}"
            );
            assert_eq!(
                target_id, 999,
                "SupersededByUnknown.target_id must name the missing successor; \
                 got {target_id}"
            );
        }
        other => panic!(
            "expected StoryError::SupersededByUnknown {{ source_id: 200, \
             target_id: 999 }}, got {other:?}"
        ),
    }
}
