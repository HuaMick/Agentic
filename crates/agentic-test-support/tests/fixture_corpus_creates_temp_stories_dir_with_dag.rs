//! Story 26 acceptance test: `FixtureCorpus` creates a temp dir whose
//! root contains a `stories/` subdirectory, and a series of
//! `write_story()` calls produces a corpus with the expected file layout.
//!
//! Justification (from stories/26.yml): pins the kit's primary
//! tempdir-plus-stories primitive. Without this, every consumer would
//! re-derive the tempdir-and-stories-dir lifecycle the deep-modules
//! asset's deletion-test worked example calls out by name
//! (`write_fixture_story` repeated across 8 callers in
//! `crates/agentic-ci-record/tests/`) — exactly the shallow-module
//! replication the kit exists to retire.
//!
//! Red today is compile-red: `FixtureCorpus::new()`, `write_story()`,
//! and `stories_dir()` are not yet declared on the unit-struct shells in
//! `crates/agentic-test-support/src/lib.rs`.

use agentic_test_support::FixtureCorpus;

#[test]
fn fixture_corpus_creates_temp_stories_dir_with_dag() {
    // A freshly-constructed corpus must root itself at a temp directory
    // whose `stories/` subdir exists and is writable.
    let corpus = FixtureCorpus::new();
    let stories_dir = corpus.stories_dir();

    assert!(
        stories_dir.is_dir(),
        "FixtureCorpus::new() must create a stories/ subdirectory; \
         got non-directory at {}",
        stories_dir.display()
    );
    assert_eq!(
        stories_dir.file_name().and_then(|s| s.to_str()),
        Some("stories"),
        "the stories subdirectory must be named exactly `stories` \
         (loader contract); got {}",
        stories_dir.display()
    );

    // Author a tiny three-story DAG: anc -> target -> desc.
    let anc = corpus.write_story(81231, &[]);
    let target = corpus.write_story(81232, &[81231]);
    let desc = corpus.write_story(81233, &[81232]);

    // Each write_story() call must land a file at <stories_dir>/<id>.yml.
    for fixture_path in [anc.path(), target.path(), desc.path()] {
        assert!(
            fixture_path.is_file(),
            "write_story() must produce a file on disk; missing at {}",
            fixture_path.display()
        );
        assert_eq!(
            fixture_path.parent(),
            Some(stories_dir.as_path()),
            "fixture stories must land directly under stories_dir(); \
             got parent {:?}",
            fixture_path.parent()
        );
    }

    // The three filenames are exactly what a downstream loader expects.
    let mut entries: Vec<String> = std::fs::read_dir(&stories_dir)
        .expect("read stories_dir")
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    entries.sort();
    assert_eq!(
        entries,
        vec!["81231.yml".to_string(), "81232.yml".to_string(), "81233.yml".to_string()],
        "stories_dir() must contain exactly the three files write_story() emitted"
    );
}
