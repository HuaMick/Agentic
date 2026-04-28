//! Story 2 acceptance test: the failure-path row shape names exactly the
//! failing test basenames.
//!
//! Justification (from stories/2.yml): proves the failure path — given a
//! failing acceptance-test run for story id `<n>` where two test files
//! failed, `Recorder::record` upserts a row with `verdict=fail` and
//! `failing_tests` containing exactly those two test file basenames
//! (no paths, no extensions stripped — basename only, e.g.
//! `verify_pass.rs`). Without this the dashboard's `Failing tests` column
//! has nothing to render and "fell from grace" detection in story 3
//! cannot name what fell.
//!
//! The scaffold drives `Recorder::record` with a `RunInput::fail` carrying
//! two full-path failing test files; it asserts the stored row's
//! `failing_tests` array contains the basenames (with extension) of both,
//! in any order, and nothing else.  Red today is compile-red via the
//! missing `agentic_ci_record` public surface.

use std::collections::BTreeSet;
use std::sync::Arc;

use agentic_ci_record::{Recorder, RunInput};
use agentic_store::{MemStore, Store};

#[test]
fn record_fail_populates_failing_tests_with_basenames_only() {
    const STORY_ID: i64 = 42;

    // Story 18 made signer resolution mandatory on every Recorder::record
    // call; tier 2 (`AGENTIC_SIGNER` env var) is the cheapest fixture
    // setup the recorder will accept. Cleared at the end of the test.
    std::env::set_var("AGENTIC_SIGNER", "test-fixture@signer.local");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let recorder = Recorder::new(store.clone());

    // Two failing test files with full paths — the kind of shape a test
    // runner's JSON output hands the CI hook.  The recorder must collapse
    // these to basenames before writing.
    let failing_paths = vec![
        "crates/agentic-ci-record/tests/record_pass.rs".to_string(),
        "crates/agentic-ci-record/tests/record_fail.rs".to_string(),
    ];
    let input = RunInput::fail(STORY_ID, failing_paths.clone());

    recorder
        .record(input)
        .expect("record should succeed on valid fail input");

    let row = store
        .get("test_runs", &STORY_ID.to_string())
        .expect("store get should succeed")
        .expect("recorder must have upserted a row for this story_id");

    assert_eq!(
        row.get("verdict").and_then(|v| v.as_str()),
        Some("fail"),
        "Fail run must record verdict=\"fail\"; got row={row}"
    );

    let recorded: BTreeSet<String> = row
        .get("failing_tests")
        .and_then(|v| v.as_array())
        .expect("failing_tests must be an array")
        .iter()
        .map(|v| {
            v.as_str()
                .expect("each failing_tests entry must be a string")
                .to_string()
        })
        .collect();

    // Contract: basenames only, extension preserved.  No paths, no stripping.
    let expected: BTreeSet<String> = ["record_pass.rs", "record_fail.rs"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    assert_eq!(
        recorded, expected,
        "failing_tests must be exactly the basenames of the failing files; got {recorded:?}, expected {expected:?}"
    );

    // Explicit negative: none of the recorded entries should carry a path
    // separator or the original `crates/...` prefix.
    for entry in &recorded {
        assert!(
            !entry.contains('/') && !entry.contains('\\'),
            "failing_tests entry must not contain a path separator; got {entry:?}"
        );
        assert!(
            !entry.starts_with("crates"),
            "failing_tests entry must be basename only, not a repo-rooted path; got {entry:?}"
        );
    }

    // Cleanup: clear the env var we set for this test so it does not
    // leak across test invocations sharing the same process.
    std::env::remove_var("AGENTIC_SIGNER");
}
