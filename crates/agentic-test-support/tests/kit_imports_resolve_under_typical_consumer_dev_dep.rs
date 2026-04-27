//! Story 26 acceptance test: smoke test for the kit's public-surface
//! contract from the consumer side.
//!
//! Justification (from stories/26.yml): a small integration test that
//! `use`s each of the five public names (`FixtureCorpus`, `StoryFixture`,
//! `FixtureRepo`, `RecordingExecutor`, `RecordedCall`) and constructs
//! the minimum non-trivial value of each compiles and runs green when
//! the kit crate is consumed via a normal `[dev-dependencies]`
//! declaration. Without this, the four shape-pinning tests above could
//! pass in isolation while the kit's re-exports (the `pub use` lines
//! in `lib.rs`) drift to private or to wrong-cased names that downstream
//! consumers cannot import.
//!
//! Red today is compile-red: the constructors and methods this test
//! invokes (`FixtureCorpus::new()`, `write_story()`,
//! `FixtureRepo::init_with_email(...)`, `RecordingExecutor::default()`,
//! `recorded_calls()`) are not yet declared on the unit-struct shells.

use std::path::PathBuf;

use agentic_ci_record::TestExecutor;
use agentic_test_support::{
    FixtureCorpus, FixtureRepo, RecordedCall, RecordingExecutor, StoryFixture,
};
use tempfile::TempDir;

#[test]
fn kit_imports_resolve_under_typical_consumer_dev_dep() {
    // FixtureCorpus + StoryFixture: tempdir lifecycle + fixture YAML.
    let corpus = FixtureCorpus::new();
    let story: StoryFixture = corpus.write_story(70001, &[]);
    assert!(
        story.path().is_file(),
        "StoryFixture::path() must point at the file write_story() emitted; \
         missing at {}",
        story.path().display()
    );

    // FixtureRepo: git seeding with committer email.
    let tmp = TempDir::new().expect("repo tempdir");
    let repo = FixtureRepo::init_with_email(tmp.path(), "ci@example.com");
    let head = repo.head_sha();
    assert_eq!(
        head.len(),
        40,
        "FixtureRepo::head_sha() must return 40 chars; got {}: `{head}`",
        head.len()
    );

    // RecordingExecutor + RecordedCall: drive one invocation through the
    // TestExecutor surface so the recorded call's public fields are
    // exercised by name (story_id, files).
    let executor = RecordingExecutor::default();
    let files = vec![PathBuf::from("crates/agentic-test-support/tests/smoke.rs")];
    let _ = executor.run_tests(70001, &files);
    let recorded: Vec<RecordedCall> = executor.recorded_calls();
    assert_eq!(
        recorded.len(),
        1,
        "RecordingExecutor must record exactly one call after one invocation; got {}",
        recorded.len()
    );
    assert_eq!(
        recorded[0].story_id, 70001,
        "RecordedCall.story_id must be a public field accessible to consumers; \
         got {}",
        recorded[0].story_id
    );
    assert_eq!(
        recorded[0].files, files,
        "RecordedCall.files must be a public field accessible to consumers; \
         got {:?}",
        recorded[0].files
    );
}
