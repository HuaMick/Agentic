//! Story 2 acceptance test: the recorder drives end-to-end as a standalone
//! library, with no orchestrator, runtime, sandbox, or CLI in the link
//! graph.
//!
//! Justification (from stories/2.yml): proves the standalone-resilient-
//! library claim — the recorder library is driven directly with only
//! `agentic-store`, `agentic-events`, and `git2` wired up (no orchestrator,
//! no runtime, no sandbox), and produces the same row shape as the CLI /
//! CI hook path. Without this the recorder becomes coupled to whatever
//! invokes it; we want the CI hook, a future local pre-commit hook, and
//! the orchestrator to all be valid callers of the same library.
//!
//! Pattern: standalone-resilient-library. The dependency floor is
//! enforced by what this test imports — only `agentic_ci_record` and
//! `agentic_store` from the workspace. Adding `agentic_orchestrator`,
//! `agentic_runtime`, `agentic_sandbox`, or `agentic_cli` to this file
//! would be a review-time red flag and the standalone-resilience claim
//! would break.
//!
//! Red today is compile-red via the missing `agentic_ci_record` public
//! surface (`Recorder`, `RunInput`).

// Compile-time witness: the only workspace crates this test names are
// `agentic_ci_record` and `agentic_store`.  Anything else would be a
// dependency-floor violation.
use std::sync::Arc;

use agentic_ci_record::{Recorder, RunInput};
use agentic_store::{MemStore, Store};

#[test]
fn recorder_drives_full_happy_path_with_no_orchestrator_dependency() {
    const STORY_ID: i64 = 42;

    // Story 18 made signer resolution mandatory on every Recorder::record
    // call; tier 2 (`AGENTIC_SIGNER` env var) is the cheapest fixture
    // setup the recorder will accept and does not introduce any new
    // workspace dependency to this scaffold (the standalone-resilience
    // claim is preserved). Cleared at the end of the test.
    std::env::set_var("AGENTIC_SIGNER", "test-fixture@signer.local");

    // Primary entry point constructed with only the declared dependency
    // floor — a Store (satisfied here by MemStore).  No orchestrator.
    // No runtime.  No sandbox.  No CLI.
    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let recorder = Recorder::new(store.clone());

    // Drive the same happy path a CI hook would drive: one Pass record
    // followed by a state read from the same Store.
    recorder
        .record(RunInput::pass(STORY_ID))
        .expect("standalone recorder must record a pass");

    let row = store
        .get("test_runs", &STORY_ID.to_string())
        .expect("store get should succeed")
        .expect("standalone recorder must have upserted a row");

    // The row shape must match what the CLI / CI hook path would produce
    // — same fields in the same table.  This is the "same row shape as
    // the CLI / CI hook path" clause of the justification.
    assert_eq!(
        row.get("story_id").and_then(|v| v.as_i64()),
        Some(STORY_ID),
        "standalone path must record the same story_id as the CLI path; row={row}"
    );
    assert_eq!(
        row.get("verdict").and_then(|v| v.as_str()),
        Some("pass"),
        "standalone path must record the same verdict shape as the CLI path; row={row}"
    );
    assert!(
        row.get("failing_tests")
            .and_then(|v| v.as_array())
            .map(|a| a.is_empty())
            .unwrap_or(false),
        "standalone path must record failing_tests=[] on Pass; row={row}"
    );
    assert!(
        row.get("commit").and_then(|v| v.as_str()).is_some(),
        "standalone path must stamp a `commit` field; row={row}"
    );
    assert!(
        row.get("ran_at").and_then(|v| v.as_str()).is_some(),
        "standalone path must stamp a `ran_at` field; row={row}"
    );

    // Errors surface through the recorder's own typed enum — not anyhow,
    // not an orchestrator error type.  Keeping this function pointer
    // typed is a compile-time assertion that the error type is a local
    // `std::error::Error` impl.
    let _ensure_error_is_local: fn(&agentic_ci_record::RecordError) -> &dyn std::error::Error =
        |e| e;

    // Cleanup: clear the env var we set for this test so it does not
    // leak across test invocations sharing the same process.
    std::env::remove_var("AGENTIC_SIGNER");
}
