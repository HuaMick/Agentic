//! Story 26 acceptance test: `RecordingExecutor` invoked N times records
//! N `RecordedCall` entries, each exposing the `(story_id, files)` tuple
//! the invocation observed; order is preserved; reading recorded calls
//! back is a non-destructive observation.
//!
//! Justification (from stories/26.yml): pins the stub-executor
//! primitive's per-call recording shape. Without this, every consumer
//! that needs a `TestExecutor` / `UatExecutor` stub would either re-roll
//! a bespoke `Arc<Mutex<Vec<...>>>` pattern (the shape currently visible
//! in `crates/agentic-ci-record/tests/stories_test_bare_id_runs_single_story.rs`'s
//! local `StubExecutor`) or settle for a coarser test that only asserts
//! total invocation count. The kit's contract is that per-invocation
//! arguments are inspectable; the test pins that.
//!
//! Red today is compile-red: `RecordingExecutor::default()`, its
//! `TestExecutor for RecordingExecutor` impl, the `recorded_calls()`
//! accessor, and the `RecordedCall::story_id` / `RecordedCall::files`
//! public fields are not yet declared on the unit-struct shells.

use std::path::PathBuf;

use agentic_ci_record::{ExecutorOutcome, TestExecutor};
use agentic_test_support::{RecordedCall, RecordingExecutor};

#[test]
fn recording_executor_captures_per_invocation_args() {
    let executor = RecordingExecutor::default();

    // Drive two invocations through the TestExecutor trait surface —
    // the same surface CiRunner uses in production. Each invocation
    // hands a distinct (story_id, files) tuple.
    let files_a: Vec<PathBuf> =
        vec![PathBuf::from("crates/agentic-ci-record/tests/a.rs")];
    let files_b: Vec<PathBuf> = vec![
        PathBuf::from("crates/agentic-uat/tests/b.rs"),
        PathBuf::from("crates/agentic-uat/tests/c.rs"),
    ];

    let _outcome_a: ExecutorOutcome = executor.run_tests(101, &files_a);
    let _outcome_b: ExecutorOutcome = executor.run_tests(202, &files_b);

    // Reading the recorded calls back is non-destructive: we read
    // twice and assert the same shape both times.
    let first_read: Vec<RecordedCall> = executor.recorded_calls();
    let second_read: Vec<RecordedCall> = executor.recorded_calls();

    assert_eq!(
        first_read.len(),
        2,
        "RecordingExecutor must record exactly one entry per run_tests call; \
         got {} entries after 2 invocations",
        first_read.len()
    );
    assert_eq!(
        first_read.len(),
        second_read.len(),
        "recorded_calls() must be non-destructive — repeated reads see the \
         same N entries; first read had {} entries, second had {}",
        first_read.len(),
        second_read.len()
    );

    // Order is preserved: invocation N -> recorded[N].
    assert_eq!(
        first_read[0].story_id, 101,
        "first recorded call's story_id must match first invocation; got {}",
        first_read[0].story_id
    );
    assert_eq!(
        first_read[0].files, files_a,
        "first recorded call's files must match first invocation's files; \
         got {:?}",
        first_read[0].files
    );
    assert_eq!(
        first_read[1].story_id, 202,
        "second recorded call's story_id must match second invocation; got {}",
        first_read[1].story_id
    );
    assert_eq!(
        first_read[1].files, files_b,
        "second recorded call's files must match second invocation's files; \
         got {:?}",
        first_read[1].files
    );
}
