//! Story 2 acceptance test: upsert-by-story_id replaces the prior row; it
//! does not append a new row per run.
//!
//! Justification (from stories/2.yml): proves upsert-by-story_id semantics
//! — after recording a Pass for story `<n>` and then a Fail for the same
//! story, a query of `test_runs` filtered by `story_id=<n>` returns
//! exactly one row (the Fail), not two. Without this the table grows
//! unboundedly per CI run and the dashboard would have to choose a
//! "latest" row by timestamp at read time — moving correctness from a
//! write-time invariant to a read-time computation we would get wrong
//! eventually.
//!
//! The scaffold records Pass then Fail for the same `story_id` against a
//! shared `MemStore`, queries `test_runs` filtered on `story_id`, and
//! asserts exactly one row, carrying the Fail's shape.  Red today is
//! compile-red via the missing `agentic_ci_record` public surface.

use std::sync::Arc;

use agentic_ci_record::{Recorder, RunInput};
use agentic_store::{MemStore, Store};

#[test]
fn second_record_replaces_first_for_same_story_id() {
    const STORY_ID: i64 = 42;

    // Story 18 made signer resolution mandatory on every Recorder::record
    // call; tier 2 (`AGENTIC_SIGNER` env var) is the cheapest fixture
    // setup the recorder will accept. Cleared at the end of the test.
    std::env::set_var("AGENTIC_SIGNER", "test-fixture@signer.local");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let recorder = Recorder::new(store.clone());

    // First: Pass.
    recorder
        .record(RunInput::pass(STORY_ID))
        .expect("pass record should succeed");

    // Second: Fail, same story_id.  Contract says: replace, not append.
    let failing = vec!["crates/agentic-ci-record/tests/record_fail.rs".to_string()];
    recorder
        .record(RunInput::fail(STORY_ID, failing))
        .expect("fail record should succeed");

    // Query `test_runs` filtered on this story_id.  The upsert contract
    // says exactly one row.
    let rows = store
        .query("test_runs", &|doc| {
            doc.get("story_id").and_then(|v| v.as_i64()) == Some(STORY_ID)
        })
        .expect("query should succeed");

    assert_eq!(
        rows.len(),
        1,
        "upsert-by-story_id must leave exactly one row per story; got {} rows: {rows:?}",
        rows.len()
    );

    let row = &rows[0];
    assert_eq!(
        row.get("verdict").and_then(|v| v.as_str()),
        Some("fail"),
        "the surviving row must be the Fail (the later write); got row={row}"
    );

    // The `failing_tests` on the surviving row must reflect the Fail's
    // input, not the earlier Pass's empty array.
    let failing = row
        .get("failing_tests")
        .and_then(|v| v.as_array())
        .expect("failing_tests must be an array");
    assert_eq!(
        failing.len(),
        1,
        "surviving Fail row must carry its one failing test; got {failing:?}"
    );
    assert_eq!(
        failing[0].as_str(),
        Some("record_fail.rs"),
        "surviving Fail row must name the basename of the failing file; got {failing:?}"
    );

    // Cleanup: clear the env var we set for this test so it does not
    // leak across test invocations sharing the same process.
    std::env::remove_var("AGENTIC_SIGNER");
}
