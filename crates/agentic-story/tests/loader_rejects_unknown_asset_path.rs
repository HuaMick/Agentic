//! Story 27 acceptance test: directory-load rejects a story whose
//! `assets:` references a path that does not exist on disk.
//!
//! Justification (from stories/27.yml): proves the existence-check
//! ADR-0007 decision 3 mandates — a story YAML carrying
//! `assets: [agents/assets/nonexistent.yml]` is rejected by the
//! directory loader with a typed error variant whose payload names
//! the missing path verbatim and the source story's id, so an author
//! who mistypes an asset path (or references one that was moved or
//! retired) sees a precise diagnostic rather than a silent acceptance
//! that surfaces as a missing-file panic in some later consumer. The
//! check resolves paths relative to the repo root and runs only in
//! the directory-load path (parity with how `depends_on` and
//! `superseded_by` target-existence are validated per story 6's
//! guidance — single-file load cannot meaningfully validate
//! cross-tree references).
//!
//! Per the story's guidance build-rust surfaces a typed error variant
//! distinct enough that callers can match on it without parsing the
//! message, carrying both the offending path AND the source story id.
//! The variant name is build-rust's call; this scaffold pins on
//! `StoryError::AssetNotFound { path, source_id }` because (a) the
//! existing typed variants in the loader are flat structs with
//! field-named payloads, so this shape mirrors them; and (b) the
//! variant name reads back the contract a human asks of it ("the
//! asset was not found"). Build-rust may rename the variant if a
//! sibling-naming review demands it; the assertion below pins the
//! observable, not the lexical name's specific letters — the
//! `match` arm must carry both pieces of payload through, however
//! they are spelled.
//!
//! Red today is compile-red: `StoryError::AssetNotFound` does not
//! yet exist as a variant on the loader's error enum, so the pattern
//! match below does not resolve. Same red shape as story 21's
//! `load_superseded_by_pointing_at_unknown_id_is_rejected.rs`, which
//! pinned its own typed-variant addition the same way.

use std::fs;
use std::path::Path;

use agentic_story::{Story, StoryError};
use tempfile::TempDir;

/// Write a story whose `assets:` array references a single bogus path.
/// The path is deliberately under `agents/assets/` (so the path-shape
/// regex passes at schema layer) but names a file that does not exist
/// on disk — that is what the directory loader's existence check
/// must catch.
fn write_story_with_bad_asset(dir: &Path, id: u32, bad_asset: &str) {
    let body = format!(
        r#"id: {id}
title: "Fixture pointing at a nonexistent asset"

outcome: |
  A story whose assets array references a path that does not exist
  on disk; the directory loader must reject it with a typed error
  variant naming both the path and the source story id.

status: proposed

patterns: []

assets:
  - {bad_asset}

acceptance:
  tests:
    - file: crates/agentic-story/tests/loader_rejects_unknown_asset_path.rs
      justification: |
        Present so this fixture is otherwise schema-valid.
  uat: |
    Run the directory loader and observe the asset-not-found error.

guidance: |
  Fixture authored inline for the asset-existence-check test.

depends_on: []
"#
    );
    fs::write(dir.join(format!("{id}.yml")), body).expect("write bad-asset fixture");
}

#[test]
fn loader_rejects_unknown_asset_path_with_typed_error_naming_path_and_source_id() {
    // Arrange: one story whose assets array references a path under
    // `agents/assets/` that does not exist on disk. We use a tempdir
    // for the stories corpus; the asset path is repo-root-relative
    // by ADR-0007 contract — the loader resolves it relative to the
    // repo root, not the stories tempdir, so a path that is bogus
    // anywhere is bogus everywhere.
    let tmp = TempDir::new().expect("create temp dir");
    let bad_asset = "agents/assets/nonexistent.yml";
    write_story_with_bad_asset(tmp.path(), 273, bad_asset);

    // Act: directory loader must enforce cross-tree existence on
    // `assets:` entries (single-file load cannot, per the same rule
    // depends_on and superseded_by already use).
    let result = Story::load_dir(tmp.path());
    let err = result.expect_err(
        "a directory containing a story whose assets array names a \
         nonexistent path must be rejected at load time",
    );

    // Assert: the error is the typed AssetNotFound variant and it
    // names BOTH the offending path verbatim AND the source story
    // id, so the author can fix either side.
    match err {
        StoryError::AssetNotFound { path, source_id } => {
            assert_eq!(
                path.to_string_lossy(),
                bad_asset,
                "AssetNotFound.path must carry the offending asset path \
                 verbatim; got {path:?}"
            );
            assert_eq!(
                source_id, 273,
                "AssetNotFound.source_id must name the story whose assets \
                 array contained the bad path; got {source_id}"
            );
        }
        other => panic!(
            "expected StoryError::AssetNotFound {{ path: {bad_asset:?}, \
             source_id: 273 }}, got {other:?}"
        ),
    }
}
