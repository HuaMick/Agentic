//! Story 12 acceptance test: a scoped run preserves rows for stories
//! outside the selected subtree byte-identically.
//!
//! Justification (from stories/12.yml): proves the isolation contract —
//! given a prior `test_runs` row for a story `<other-id>` OUTSIDE the
//! selected subtree, after `CiRunner::run("+<id>")` completes,
//! `<other-id>`'s row is byte-identical to its pre-run state. Without
//! this, a narrow selector could silently invalidate unrelated stories'
//! health (by overwriting their rows with empty or nonsensical values) —
//! the exact opposite of what a scoped run promises.
//!
//! Red today is compile-red: the `agentic_ci_record::{CiRunner,
//! TestExecutor, ExecutorOutcome}` surface does not yet exist.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agentic_ci_record::{CiRunner, ExecutorOutcome, TestExecutor};
use agentic_store::{MemStore, Store};
use serde_json::{json, Value};
use tempfile::TempDir;

// Fixture DAG: ANC <- TARGET; OTHER is unrelated (no edges).
const ID_ANC: u32 = 81251;
const ID_TARGET: u32 = 81252;
const ID_OTHER: u32 = 81253;

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
title: "Fixture {id} for story-12 preservation scaffold"

outcome: |
  Fixture row for the preservation scaffold.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: {test_file}
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Seed OTHER's row; run +TARGET; assert OTHER's row unchanged.

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
    write_fixture_story(&stories_dir, ID_ANC, &[]);
    write_fixture_story(&stories_dir, ID_TARGET, &[ID_ANC]);
    write_fixture_story(&stories_dir, ID_OTHER, &[]);
    tmp
}

#[test]
fn scoped_run_leaves_rows_for_unselected_stories_byte_identical() {
    let corpus = setup_fixture_corpus();
    let stories_dir = corpus.path().join("stories");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed a known-good row for OTHER. The exact bytes here are what
    // the preservation invariant guards.
    let seeded: Value = json!({
        "story_id": ID_OTHER,
        "verdict": "pass",
        "commit": "deadbeef000000000000000000000000deadbeef",
        "ran_at": "2026-04-18T21:52:21Z",
        "failing_tests": [],
    });
    store
        .upsert("test_runs", &ID_OTHER.to_string(), seeded.clone())
        .expect("seed OTHER row");

    // Sanity: read back the seeded bytes.
    let before = store
        .get("test_runs", &ID_OTHER.to_string())
        .expect("store get must succeed")
        .expect("seeded row must exist before the run");
    assert_eq!(
        before, seeded,
        "seeded row must match what we just upserted; before={before}"
    );

    // Run the ancestor selector +TARGET — covers {ANC, TARGET}, NOT OTHER.
    let calls = Arc::new(Mutex::new(Calls::default()));
    let executor = StubExecutor {
        calls: calls.clone(),
    };
    let runner = CiRunner::new(store.clone(), Box::new(executor), stories_dir);
    let selector = format!("+{ID_TARGET}");
    runner
        .run(&selector)
        .expect("runner must succeed on +<id> selector");

    // Preservation: OTHER's row must be byte-identical.
    let after = store
        .get("test_runs", &ID_OTHER.to_string())
        .expect("store get must succeed")
        .expect("OTHER's row must still exist after an unrelated scoped run");
    assert_eq!(
        after, seeded,
        "unselected story's row must be byte-identical after a scoped run; \
         after={after}, expected={seeded}"
    );

    // Sanity: the selected subtree DID get rows (so the runner is not
    // a no-op — the preservation is genuine, not trivially satisfied).
    let target_row = store
        .get("test_runs", &ID_TARGET.to_string())
        .expect("store get must succeed");
    assert!(
        target_row.is_some(),
        "runner must upsert a row for the selected target {ID_TARGET}; got {target_row:?}"
    );

    // And the executor was invoked — also proves the run actually ran.
    let invocations = &calls.lock().expect("calls mutex poisoned").invocations;
    assert!(
        !invocations.is_empty(),
        "executor must be invoked at least once during a +<id> run; got zero invocations"
    );
}
