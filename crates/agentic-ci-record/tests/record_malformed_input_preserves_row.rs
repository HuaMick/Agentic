//! Story 2 acceptance test: malformed input returns a typed error and
//! leaves any existing row untouched.
//!
//! Justification (from stories/2.yml): proves the corruption-resistance
//! contract — given an existing Pass row for story `<n>`, when
//! `Recorder::record` is invoked with malformed test-runner output
//! (e.g. truncated JSON, missing required fields, empty input), it
//! returns `RecordError::MalformedInput` and the existing row in
//! `test_runs` is unchanged. Without this, a flaky CI step that emitted
//! garbage could silently overwrite a known-good row with an empty or
//! corrupt one, and the dashboard would misreport the story's health.
//!
//! The scaffold seeds a Pass row via the normal `Recorder::record` path,
//! snapshots the row, then invokes the recorder's raw-input entry
//! (`Recorder::record_from_raw`) with empty bytes — a canonical malformed
//! shape. It asserts the returned error matches `RecordError::MalformedInput`
//! and that the stored row is byte-identical to the pre-call snapshot.
//! Red today is compile-red via the missing `agentic_ci_record` public
//! surface.

use std::sync::Arc;

use agentic_ci_record::{Recorder, RecordError, RunInput};
use agentic_store::{MemStore, Store};

#[test]
fn malformed_input_errors_without_touching_existing_row() {
    const STORY_ID: i64 = 42;

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let recorder = Recorder::new(store.clone());

    // Seed a known-good Pass row.
    recorder
        .record(RunInput::pass(STORY_ID))
        .expect("seed pass record should succeed");

    let before = store
        .get("test_runs", &STORY_ID.to_string())
        .expect("store get should succeed")
        .expect("seed row must exist");

    // Malformed input: empty bytes.  The validator must reject before
    // opening any write transaction.
    let result = recorder.record_from_raw(STORY_ID, &[]);

    match result {
        Err(RecordError::MalformedInput { .. }) => {
            // Expected.
        }
        Err(other) => panic!(
            "empty raw input must yield RecordError::MalformedInput; got {other:?}"
        ),
        Ok(()) => panic!(
            "empty raw input must not succeed — the whole corruption-resistance contract is that validation fails closed"
        ),
    }

    // Critical invariant: the pre-existing row is UNCHANGED — same
    // commit, same ran_at, same verdict.  A byte-equal comparison of the
    // full row document catches any subtle field mutation.
    let after = store
        .get("test_runs", &STORY_ID.to_string())
        .expect("store get should succeed")
        .expect("row must still exist after a rejected malformed call");

    assert_eq!(
        before, after,
        "malformed input must not alter the existing row; before={before}, after={after}"
    );
}
