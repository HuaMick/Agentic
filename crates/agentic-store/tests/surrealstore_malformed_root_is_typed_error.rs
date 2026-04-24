//! Story 5 acceptance test: SurrealStore::open returns a typed error on a
//! malformed root.
//!
//! Justification (from stories/5.yml): pointing `SurrealStore::open` at a
//! path that exists but is not a valid SurrealDB data directory (e.g. a
//! file, a directory containing unrelated files, a directory the process
//! cannot write to) returns a typed `StoreError`, does not panic, and does
//! not partially initialise. Without this, deployment-time configuration
//! mistakes — a wrong path in an env var, a stale DB from an older schema
//! — surface as panics halfway through the first write rather than as
//! clear "could not open store" errors at startup.
//!
//! The story's guidance pins the typed-error variant as `StoreError::Open`
//! carrying the offending path and underlying cause. The assertions below
//! match that contract.

use std::fs;

use agentic_store::{StoreError, SurrealStore};
use tempfile::TempDir;

#[test]
fn open_on_a_file_path_returns_typed_open_error() {
    let dir = TempDir::new().expect("create temp dir");
    let file_path = dir.path().join("not-a-directory.txt");
    fs::write(&file_path, b"this is a file, not a SurrealDB data dir").expect("write decoy file");

    let result = SurrealStore::open(&file_path);
    let err = result.expect_err("opening a file path as a store root must fail");

    match err {
        StoreError::Open { ref path, .. } => {
            assert_eq!(
                path, &file_path,
                "Open error must name the offending path; got path={path:?}"
            );
        }
        other => panic!("expected StoreError::Open for a file path, got {other:?}"),
    }
}

#[test]
fn open_on_nonexistent_parent_returns_typed_open_error_not_panic() {
    let dir = TempDir::new().expect("create temp dir");
    let nested = dir.path().join("no-such-nested").join("dir");

    let result = SurrealStore::open(&nested);
    let err = result.expect_err("opening under a nonexistent parent must fail");

    match err {
        StoreError::Open { ref path, .. } => {
            assert_eq!(
                path, &nested,
                "Open error must name the offending path; got path={path:?}"
            );
        }
        other => panic!("expected StoreError::Open for a nonexistent parent, got {other:?}"),
    }
}

#[test]
fn open_failure_does_not_partially_initialise_a_subsequent_open() {
    // After a failed open at one path, opening at a fresh, valid temp dir
    // must succeed cleanly — i.e. the failed open did not leave global
    // state behind.
    let bad_parent = TempDir::new().expect("create bad temp dir");
    let bad = bad_parent
        .path()
        .join("missing-grandparent")
        .join("missing-parent");
    let _ = SurrealStore::open(&bad).expect_err("first open must fail");

    let good_dir = TempDir::new().expect("create good temp dir");
    let _ok = SurrealStore::open(good_dir.path())
        .expect("a fresh open at a valid directory must succeed after a prior failure");
}
