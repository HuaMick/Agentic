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
//! Additionally pins the kit-vs-bespoke contract per stories/12.yml's
//! amended justification: the fixture corpus, repo seeding, and stub
//! executor MUST source from `agentic_test_support`'s shared primitives
//! (story 26), not bespoke per-file helpers. The deep-modules contract
//! is observable in this file's `use` block — a reimplementation that
//! brought back a local `fn write_fixture_story`, `fn setup_fixture_corpus`,
//! or `struct StubExecutor impl TestExecutor` would fail the contract.
//! Reference `assets/principles/deep-modules.yml`'s
//! `application_to_test_scaffolding` for the operational rationale.

use std::sync::Arc;

use agentic_ci_record::CiRunner;
use agentic_store::{MemStore, Store};
use agentic_test_support::{FixtureCorpus, RecordingExecutor};

// Fixture DAG: ANC <- TARGET <- DESC; plus UNRELATED (out of subtree).
const ID_ANC: u32 = 81241;
const ID_TARGET: u32 = 81242;
const ID_DESC: u32 = 81243;
const ID_UNRELATED: u32 = 81244;

#[test]
fn full_subtree_run_upserts_exactly_one_row_per_executed_story() {
    // Build the four-story DAG via the shared kit primitive — the
    // local `write_fixture_story` / `setup_fixture_corpus` helpers this
    // file used to carry are now sourced from `agentic_test_support`
    // per stories/12.yml's kit-vs-bespoke contract.
    let corpus = FixtureCorpus::new();
    corpus.write_story(ID_ANC, &[]);
    corpus.write_story(ID_TARGET, &[ID_ANC]);
    corpus.write_story(ID_DESC, &[ID_TARGET]);
    corpus.write_story(ID_UNRELATED, &[]);
    let stories_dir = corpus.stories_dir();

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let executor = RecordingExecutor::default();
    let runner = CiRunner::new(store.clone(), Box::new(executor.clone()), stories_dir);

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
