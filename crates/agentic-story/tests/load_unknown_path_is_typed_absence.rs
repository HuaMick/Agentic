//! Story 6 acceptance test: loading a story from a path that does not exist
//! returns a typed error (or `Option`-style absence), never a panic and
//! never a raw I/O error.
//!
//! Justification (from stories/6.yml): proves clean failure on missing
//! input — loading a story from a path that does not exist returns a typed
//! error (or `Option`-style absence — impl chooses which, but must be one
//! of those two, NOT a panic and NOT an I/O error bubbled up as
//! `anyhow::Error`). Without this, a typo in a test fixture path or a
//! stale story id passed on the CLI surfaces as a panic trace that looks
//! like a loader bug when it is actually user error.
//!
//! Per the story's guidance the loader does NOT bubble raw I/O up as
//! `anyhow::Error`. Either a typed `StoryError::NotFound { path }` or a
//! conventional `Option::None` is acceptable; both are exercised below.

use std::path::PathBuf;

use agentic_story::{Story, StoryError};
use tempfile::TempDir;

#[test]
fn load_unknown_path_is_typed_absence() {
    // Arrange: build a path under a temp dir that is guaranteed NOT to
    // exist on disk. The TempDir itself exists; the nested path does not.
    let tmp = TempDir::new().expect("create temp dir");
    let missing: PathBuf = tmp.path().join("no-such-story-999.yml");
    assert!(
        !missing.exists(),
        "precondition: the fixture path must not exist"
    );

    // Act: attempt to load the missing path.
    let result = Story::load(&missing);

    // Assert: the loader must report absence as a typed outcome — never a
    // panic (guaranteed because we got here), and the Err variant (if one
    // is returned) must be the typed NotFound case, not a bubbled-up I/O
    // error. An Ok result is a contract violation because a nonexistent
    // file cannot legitimately load to a valid Story.
    match result {
        Err(StoryError::NotFound { ref path }) => {
            assert_eq!(
                path, &missing,
                "NotFound must carry the path the caller supplied; got path={path:?}"
            );
        }
        Err(other) => panic!(
            "expected StoryError::NotFound for a missing path, got {other:?} \
             (raw I/O must not surface as another variant)"
        ),
        Ok(story) => panic!(
            "loading a nonexistent path must not succeed; got Story id={}",
            story.id
        ),
    }
}

#[test]
fn load_dir_unknown_path_is_typed_absence() {
    // Directory entry-point parity: Story::load_dir must also report a
    // missing directory as a typed NotFound rather than an I/O leak.
    let tmp = TempDir::new().expect("create temp dir");
    let missing = tmp.path().join("no-such-subdir");
    assert!(
        !missing.exists(),
        "precondition: the fixture directory must not exist"
    );

    let result = Story::load_dir(&missing);
    match result {
        Err(StoryError::NotFound { ref path }) => {
            assert_eq!(
                path, &missing,
                "NotFound must carry the path the caller supplied; got path={path:?}"
            );
        }
        Err(other) => panic!(
            "expected StoryError::NotFound for a missing directory, got {other:?}"
        ),
        Ok(stories) => panic!(
            "loading a nonexistent directory must not succeed; got {} stories",
            stories.len()
        ),
    }
}
