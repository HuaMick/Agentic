//! Story 12 acceptance test: unknown id surfaces a typed refusal and
//! touches nothing.
//!
//! Justification (from stories/12.yml): proves clean failure on bad input
//! at the library boundary — `CiRunner::run("+99999")` (where 99999 has
//! no corresponding `stories/99999.yml`) returns
//! `CiRunError::UnknownStory` naming the missing id, invokes the test
//! executor zero times, and writes zero rows to `test_runs`. Without
//! this, a typo or stale id produces either a panic or a silent
//! zero-story run indistinguishable from "the subtree happened to be
//! empty."
//!
//! Red today is compile-red: the `agentic_ci_record::{CiRunner,
//! CiRunError, TestExecutor, ExecutorOutcome}` surface does not yet
//! exist.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agentic_ci_record::{CiRunError, CiRunner, ExecutorOutcome, TestExecutor};
use agentic_store::{MemStore, Store};
use tempfile::TempDir;

const ID_EXISTS_A: u32 = 81261;
const ID_EXISTS_B: u32 = 81262;
const ID_MISSING: u32 = 99999;

#[derive(Default)]
struct Calls {
    invocations: Vec<(u32, Vec<PathBuf>)>,
}

struct StubExecutor {
    calls: Arc<Mutex<Calls>>,
}

impl TestExecutor for StubExecutor {
    fn run_tests(&self, story_id: u32, test_files: &[PathBuf]) -> ExecutorOutcome {
        self.calls
            .lock()
            .expect("calls mutex poisoned")
            .invocations
            .push((story_id, test_files.to_vec()));
        ExecutorOutcome::pass()
    }
}

fn write_fixture_story(stories_dir: &Path, id: u32, depends_on: &[u32]) {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    let test_file = format!("crates/agentic-ci-record/tests/fixture_story_{id}.rs");
    let body = format!(
        r#"id: {id}
title: "Fixture {id} for story-12 unknown-id scaffold"

outcome: |
  Fixture row for the unknown-id scaffold.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: {test_file}
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Run the runner with +99999; assert typed UnknownStory refusal.

guidance: |
  Fixture authored inline. Not a real story.

{deps_yaml}
"#
    );
    fs::write(stories_dir.join(format!("{id}.yml")), body).expect("write fixture story");
}

fn setup_fixture_corpus() -> TempDir {
    let tmp = TempDir::new().expect("corpus tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    write_fixture_story(&stories_dir, ID_EXISTS_A, &[]);
    write_fixture_story(&stories_dir, ID_EXISTS_B, &[ID_EXISTS_A]);
    // Deliberately NO file for ID_MISSING — that is the contract under test.
    tmp
}

#[test]
fn unknown_id_in_selector_returns_unknown_story_error_and_touches_nothing() {
    let corpus = setup_fixture_corpus();
    let stories_dir = corpus.path().join("stories");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let calls = Arc::new(Mutex::new(Calls::default()));
    let executor = StubExecutor {
        calls: calls.clone(),
    };
    let runner = CiRunner::new(store.clone(), Box::new(executor), stories_dir);

    let selector = format!("+{ID_MISSING}");
    let result = runner.run(&selector);

    // Typed refusal — not a panic, not a generic error.
    let err = result.expect_err(
        "runner must REFUSE when the selector names a missing story id; \
         got a successful run instead",
    );

    match err {
        CiRunError::UnknownStory { id } => {
            assert_eq!(
                id, ID_MISSING,
                "UnknownStory must name the missing id {ID_MISSING}; got {id}"
            );
        }
        other => panic!("expected CiRunError::UnknownStory {{ id: {ID_MISSING} }}; got {other:?}"),
    }

    // Zero executor invocations — the runner must refuse BEFORE touching
    // the executor.
    let invocations = &calls.lock().expect("calls mutex poisoned").invocations;
    assert!(
        invocations.is_empty(),
        "UnknownStory refusal must not invoke the executor; got {} invocations: {invocations:?}",
        invocations.len()
    );

    // Zero `test_runs` rows — no partial write on refusal.
    let rows = store
        .query("test_runs", &|_| true)
        .expect("test_runs query must succeed");
    assert!(
        rows.is_empty(),
        "UnknownStory refusal must leave test_runs empty; got {} rows: {rows:?}",
        rows.len()
    );
}
