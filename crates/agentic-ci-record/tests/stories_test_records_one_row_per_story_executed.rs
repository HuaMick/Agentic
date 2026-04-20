//! Story 12 acceptance test: one `test_runs` row per story the runner
//! invoked the executor for.
//!
//! Justification (from stories/12.yml): proves the bookkeeping invariant
//! — after `CiRunner::run("+<id>+")` completes, the `test_runs` table
//! contains exactly one upserted row per story whose tests the runner
//! invoked (matching story 2's row contract: one row per story,
//! `verdict`, `commit`, `failing_tests`, `ran_at`). Stories OUTSIDE the
//! selector's reach have no rows written on their behalf — their
//! existing rows (if any) are untouched. Without this, the runner could
//! run the right tests but write the wrong rows, silently misreporting
//! the dashboard's read view.
//!
//! Red today is compile-red: the `agentic_ci_record::{CiRunner,
//! TestExecutor, ExecutorOutcome}` surface does not yet exist.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agentic_ci_record::{CiRunner, ExecutorOutcome, TestExecutor};
use agentic_store::{MemStore, Store};
use tempfile::TempDir;

// Fixture DAG: ANC <- TARGET <- DESC; plus UNRELATED (out of subtree).
const ID_ANC: u32 = 81241;
const ID_TARGET: u32 = 81242;
const ID_DESC: u32 = 81243;
const ID_UNRELATED: u32 = 81244;

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
    let test_file =
        format!("crates/agentic-ci-record/tests/fixture_story_{id}.rs");
    let body = format!(
        r#"id: {id}
title: "Fixture {id} for story-12 rows-per-executed scaffold"

outcome: |
  Fixture row for the one-row-per-executed-story scaffold.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: {test_file}
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Run the runner; count rows in test_runs.

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
    write_fixture_story(&stories_dir, ID_DESC, &[ID_TARGET]);
    write_fixture_story(&stories_dir, ID_UNRELATED, &[]);
    tmp
}

#[test]
fn full_subtree_run_upserts_exactly_one_row_per_executed_story() {
    let corpus = setup_fixture_corpus();
    let stories_dir = corpus.path().join("stories");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let calls = Arc::new(Mutex::new(Calls::default()));
    let executor = StubExecutor {
        calls: calls.clone(),
    };

    let runner = CiRunner::new(store.clone(), Box::new(executor), stories_dir);

    let selector = format!("+{ID_TARGET}+");
    runner
        .run(&selector)
        .expect("runner must succeed across subtree");

    // Exactly the subtree: {ANC, TARGET, DESC}. UNRELATED is not in scope.
    let executed = [ID_ANC, ID_TARGET, ID_DESC];

    // One upserted row per executed story, keyed by story id (story 2's
    // upsert contract: the key is `story_id.to_string()`).
    for id in executed {
        let row = store
            .get("test_runs", &id.to_string())
            .expect("store get must succeed")
            .unwrap_or_else(|| {
                panic!(
                    "runner must upsert a test_runs row for executed story {id}; \
                     no row found"
                )
            });

        assert_eq!(
            row.get("story_id").and_then(|v| v.as_i64()),
            Some(id as i64),
            "row for story {id} must carry story_id={id}; got {row}"
        );
        // Story 2 row contract fields — the runner delegates to the
        // same Recorder and therefore MUST stamp all four.
        assert!(
            row.get("verdict").and_then(|v| v.as_str()).is_some(),
            "row for story {id} must carry a string `verdict`; got {row}"
        );
        assert!(
            row.get("commit").and_then(|v| v.as_str()).is_some(),
            "row for story {id} must carry a string `commit`; got {row}"
        );
        assert!(
            row.get("ran_at").and_then(|v| v.as_str()).is_some(),
            "row for story {id} must carry a string `ran_at`; got {row}"
        );
        assert!(
            row.get("failing_tests")
                .and_then(|v| v.as_array())
                .is_some(),
            "row for story {id} must carry `failing_tests` as an array; got {row}"
        );
    }

    // UNRELATED was outside the subtree and must have NO row.
    let unrelated = store
        .get("test_runs", &ID_UNRELATED.to_string())
        .expect("store get must succeed");
    assert!(
        unrelated.is_none(),
        "unrelated story {ID_UNRELATED} must have NO test_runs row; got {unrelated:?}"
    );

    // Aggregate: table cardinality over `test_runs` must equal the
    // executed-set size, not the corpus size.
    let all_rows = store
        .query("test_runs", &|_| true)
        .expect("test_runs query must succeed");
    assert_eq!(
        all_rows.len(),
        executed.len(),
        "test_runs must contain exactly {} rows after a +<id>+ run (one per executed story); \
         got {} rows: {all_rows:?}",
        executed.len(),
        all_rows.len()
    );
}
