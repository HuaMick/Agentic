//! Story 12 acceptance test: cycle in the loaded corpus surfaces a typed
//! refusal and touches nothing.
//!
//! Justification (from stories/12.yml): proves the invariant defence at
//! the CI boundary — given a `stories/` directory where the loader
//! somehow admits a cycle (story 6 regression), `CiRunner::run` returns
//! typed `CiRunError::Cycle` naming the offending edge, invokes the test
//! executor zero times, and writes zero rows. Without this, a cycle
//! would produce infinite traversal, infinite test runs, or a panic —
//! each strictly worse than a typed refusal.
//!
//! The scaffold writes two fixture stories whose `depends_on` edges form
//! a direct cycle (A -> B -> A). Story 6's loader detects this at
//! directory-load time as `StoryError::DependsOnCycle`; the runner maps
//! it to `CiRunError::Cycle` at its own boundary. If a future regression
//! weakens the loader's cycle check, this scaffold guards the CI entry
//! point from panicking or looping.
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

const ID_A: u32 = 81271;
const ID_B: u32 = 81272;

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
title: "Fixture {id} for story-12 cycle-rejection scaffold"

outcome: |
  Fixture row for the cycle-rejection scaffold; deliberately cyclical.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: {test_file}
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Build cyclic corpus; run the runner; assert typed Cycle refusal.

guidance: |
  Fixture authored inline. Not a real story.

{deps_yaml}
"#
    );
    fs::write(stories_dir.join(format!("{id}.yml")), body).expect("write fixture story");
}

fn setup_cyclic_corpus() -> TempDir {
    let tmp = TempDir::new().expect("corpus tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    // A -> B and B -> A: a direct two-node cycle.
    write_fixture_story(&stories_dir, ID_A, &[ID_B]);
    write_fixture_story(&stories_dir, ID_B, &[ID_A]);
    tmp
}

#[test]
fn cyclic_corpus_returns_cycle_error_and_touches_nothing() {
    let corpus = setup_cyclic_corpus();
    let stories_dir = corpus.path().join("stories");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let calls = Arc::new(Mutex::new(Calls::default()));
    let executor = StubExecutor {
        calls: calls.clone(),
    };
    let runner = CiRunner::new(store.clone(), Box::new(executor), stories_dir);

    // Any selector is fine — the cycle is detected at corpus-load time,
    // before selector traversal.
    let selector = format!("{ID_A}");
    let err = runner
        .run(&selector)
        .expect_err("runner must REFUSE on a cyclic corpus; got a successful run instead");

    match err {
        CiRunError::Cycle { participants } => {
            // The cycle participants must include both fixture ids A and B.
            assert!(
                participants.contains(&ID_A) && participants.contains(&ID_B),
                "CiRunError::Cycle must name both cycle participants ({ID_A}, {ID_B}); \
                 got participants={participants:?}"
            );
        }
        other => panic!("expected CiRunError::Cycle naming the offending edge; got {other:?}"),
    }

    // Zero executor invocations — cycle refusal fires before traversal.
    let invocations = &calls.lock().expect("calls mutex poisoned").invocations;
    assert!(
        invocations.is_empty(),
        "Cycle refusal must not invoke the executor; got {} invocations: {invocations:?}",
        invocations.len()
    );

    // Zero `test_runs` rows — no partial write on refusal.
    let rows = store
        .query("test_runs", &|_| true)
        .expect("test_runs query must succeed");
    assert!(
        rows.is_empty(),
        "Cycle refusal must leave test_runs empty; got {} rows: {rows:?}",
        rows.len()
    );
}
