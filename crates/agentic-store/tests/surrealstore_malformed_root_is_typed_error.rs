//! Story 5 acceptance test (RED scaffold): malformed root returns a typed
//! error from `SurrealStore::open`, not a panic and not a partially
//! initialised store.
//!
//! Justification (from stories/5.yml):
//!   Proves clean failure on bad construction input: pointing
//!   `SurrealStore::open` at a path that exists but is not a valid
//!   SurrealDB data directory (e.g. a file, a directory containing
//!   unrelated files, a directory the process cannot write to) returns a
//!   typed `StoreError`, does not panic, and does not partially
//!   initialise.
//!
//! Red state expected at scaffold-write time:
//!   - `agentic_store::SurrealStore` does not exist.
//!   - `tempfile` is not a dev-dependency yet.

use agentic_store::{StoreError, SurrealStore};
use std::fs;
use tempfile::TempDir;

#[test]
fn open_on_a_regular_file_returns_typed_store_error_not_panic() {
    // Setup: create a regular file and try to open SurrealStore at its
    // path. A real data root must be a directory; a file is malformed.
    let dir = TempDir::new().expect("tempdir should be creatable");
    let file_path = dir.path().join("not-a-db");
    fs::write(&file_path, b"this is a regular file, not a SurrealDB root")
        .expect("writing the malformed-root fixture should succeed");

    // The call must return Err (typed StoreError), not panic, not
    // partially initialise. We catch panics explicitly so a panic is a
    // distinguishable failure mode from a returned error.
    let result = std::panic::catch_unwind(|| SurrealStore::open(&file_path));
    let Ok(inner) = result else {
        panic!("SurrealStore::open must not panic on a malformed root; got a panic");
    };
    let err: StoreError = match inner {
        Ok(_) => panic!(
            "SurrealStore::open must not succeed against a regular file; it must return a typed StoreError"
        ),
        Err(e) => e,
    };

    // The error must be Debug+Display (the StoreError contract) and must
    // mention the path so an operator can act on it without reading a
    // backtrace.
    let display = format!("{err}");
    assert!(
        display.contains("not-a-db"),
        "StoreError must name the offending path; got: {display}"
    );

    panic!(
        "red: Proves clean failure on bad construction input: pointing `SurrealStore::open` at a path that exists but is not a valid SurrealDB data directory (e.g. a file, a directory containing unrelated files, a directory the process cannot write to) returns a typed `StoreError`, does not panic, and does not partially initialise."
    );
}
