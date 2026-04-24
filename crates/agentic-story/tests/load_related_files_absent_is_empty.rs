//! Story 9 acceptance test: backward compatibility of the schema change.
//!
//! Justification (from stories/9.yml): proves backward compatibility of
//! the schema change — a story YAML that omits `related_files` entirely
//! (as every existing story on disk does today) loads successfully and
//! the typed `Story`'s `related_files` is an empty collection, NOT a
//! parse error, NOT an absent-sentinel that downstream code has to
//! unwrap. Without this, shipping the schema change would break every
//! existing story at load time — the whole corpus would need
//! back-population before the dashboard could render at all.
//!
//! Per the story's guidance (schema change section) `related_files` is
//! NOT added to `required`, and the loader default on deserialisation is
//! an empty vec. The scaffold writes a fixture YAML that omits
//! `related_files` entirely and asserts the load succeeds and
//! `Story::related_files` is an empty `Vec<String>`. Red today is
//! compile-red via the missing `related_files` field on
//! `agentic_story::Story`.

use std::fs;

use agentic_story::Story;
use tempfile::TempDir;

/// Fixture that OMITS `related_files` entirely — mirrors every story on
/// disk today. Must load without error; `related_files` must default to
/// an empty vec, not `Option::None` or any absent-sentinel.
const OMIT_YAML: &str = r#"id: 92
title: "Fixture omitting the related_files field"

outcome: |
  A developer can load this fixture whose YAML has no related_files key
  and observe a Story whose related_files is an empty Vec, not a parse
  error and not an absent sentinel.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/load_related_files_absent_is_empty.rs
      justification: |
        The very scaffold you are reading. Present so this fixture is
        itself schema-valid.
  uat: |
    Load this fixture; confirm no error; confirm related_files is empty.

guidance: |
  Fixture authored inline for the absent-is-empty test. Not a real story.

depends_on: []
"#;

#[test]
fn load_related_files_absent_is_empty_vec_not_parse_error() {
    let dir = TempDir::new().expect("create temp dir");
    let path = dir.path().join("92.yml");
    fs::write(&path, OMIT_YAML).expect("write fixture");

    // Load must succeed — omitting related_files is the shape every
    // existing story has today and must keep working.
    let story: Story =
        Story::load(&path).expect("a story that omits `related_files` must load successfully");

    // `related_files` must be a Vec<String>, and it must be empty.
    // Typed as Vec<String> (not Option<Vec<String>>) so downstream code
    // does not have to unwrap — asserting `.is_empty()` on the field
    // directly enforces the non-Option shape at compile time.
    assert!(
        story.related_files.is_empty(),
        "when `related_files` is omitted from the YAML the Story's \
         related_files must be an empty Vec; got {:?}",
        story.related_files
    );
    assert_eq!(
        story.related_files.len(),
        0,
        "omitted related_files must produce a Vec of length 0; got len={}",
        story.related_files.len()
    );
}
