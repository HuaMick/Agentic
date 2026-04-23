//! Story 16 acceptance test: the `runs` writer validates the outcome
//! enum at the library boundary.
//!
//! Justification (from stories/16.yml acceptance.tests[1]):
//!   Proves the outcome enum is validated at the library boundary,
//!   not just documented: `RunRecorder::write` called with an
//!   `outcome` string outside the set
//!   `{green, inner_loop_exhausted, crashed}` returns a typed
//!   `RunRecorderError::InvalidOutcome` naming the offending value,
//!   writes zero rows, and leaves the `runs` table unchanged.
//!   Without this, a typo in a caller — or a future outcome variant
//!   added to the schema but not to the writer — would land as a
//!   `runs` row whose `outcome` no downstream reader recognises, and
//!   the dashboard's three-way status branch degrades to "whatever
//!   string happens to be in the column."
//!
//! Red today: natural. The `RunRecorder::finish_with_outcome_string`
//! API (the stringly-typed entry point that the UAT exercises in
//! walkthrough step 17) does not yet exist. The test also imports
//! `RunRecorderError::InvalidOutcome`, which is part of the same
//! un-shipped surface, so `cargo check` fails with an unresolved-
//! import error.

use agentic_runtime::{RunRecorder, RunRecorderConfig, RunRecorderError};
use agentic_store::{MemStore, Store};
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn run_recorder_rejects_outcome_outside_documented_set() {
    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let cfg = RunRecorderConfig {
        store: Arc::clone(&store),
        runs_root: runs_root_tmp.path().to_path_buf(),
        run_id: "11111111-2222-4333-8444-555555555555".to_string(),
        story_id: 15,
        story_yaml_bytes: b"id: 15\n".to_vec(),
        signer: "sandbox:stub@run-xxx".to_string(),
        build_config: json!({}),
    };

    let recorder = RunRecorder::start(cfg).expect("recorder start should succeed");

    // Act: finish with an outcome string that is NOT in the documented
    // set {green, inner_loop_exhausted, crashed}.
    let offending = "partially-exploded";
    let err = recorder
        .finish_with_outcome_string(offending)
        .expect_err("malformed outcome must be rejected with a typed error");

    // Assert: the error is the typed InvalidOutcome variant and names
    // the offending string.
    match err {
        RunRecorderError::InvalidOutcome { value } => {
            assert_eq!(
                value, offending,
                "InvalidOutcome error must carry the offending outcome string verbatim"
            );
        }
        other => panic!(
            "expected RunRecorderError::InvalidOutcome, got {other:?}. \
             The outcome enum must be validated at the library boundary, \
             not silently passed through to the Store."
        ),
    }

    // Assert: the runs table is unchanged — zero rows written.
    let rows = store
        .query("runs", &|_| true)
        .expect("query should succeed");
    assert!(
        rows.is_empty(),
        "a rejected outcome must write zero rows; got {rows:?}. The \
         library-boundary validation must fail-closed before the \
         Store::append call, not partially apply."
    );
}
