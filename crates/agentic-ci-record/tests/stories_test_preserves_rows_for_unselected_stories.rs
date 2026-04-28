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
use serde_json::{json, Value};

// Fixture DAG: ANC <- TARGET; OTHER is unrelated (no edges).
const ID_ANC: u32 = 81251;
const ID_TARGET: u32 = 81252;
const ID_OTHER: u32 = 81253;

#[test]
fn scoped_run_leaves_rows_for_unselected_stories_byte_identical() {
    // Story 18 made signer resolution mandatory on every Recorder::record
    // call (which CiRunner delegates to per executed story); tier 2
    // (`AGENTIC_SIGNER` env var) is the cheapest fixture setup the
    // recorder will accept. Cleared at the end of the test.
    std::env::set_var("AGENTIC_SIGNER", "test-fixture@signer.local");

    // Build the three-story corpus via the shared kit primitive — the
    // local `write_fixture_story` / `setup_fixture_corpus` helpers this
    // file used to carry are now sourced from `agentic_test_support`
    // per stories/12.yml's kit-vs-bespoke contract.
    let corpus = FixtureCorpus::new();
    corpus.write_story(ID_ANC, &[]);
    corpus.write_story(ID_TARGET, &[ID_ANC]);
    corpus.write_story(ID_OTHER, &[]);
    let stories_dir = corpus.stories_dir();

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
    let executor = RecordingExecutor::default();
    let runner = CiRunner::new(store.clone(), Box::new(executor.clone()), stories_dir);
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
    let recorded = executor.recorded_calls();
    assert!(
        !recorded.is_empty(),
        "executor must be invoked at least once during a +<id> run; got zero invocations"
    );

    // Cleanup: clear the env var we set for this test so it does not
    // leak across test invocations sharing the same process.
    std::env::remove_var("AGENTIC_SIGNER");
}
