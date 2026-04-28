//! Story 12 acceptance test: `+<id>+` selector runs the full subtree and
//! deduplicates stories that sit at diamond intersections.
//!
//! Justification (from stories/12.yml): proves the `+<id>+` selector at
//! the library boundary — `CiRunner::run("+<id>+")` invokes the test
//! executor exactly once per acceptance-test file declared by the target
//! plus every transitive ancestor AND every transitive descendant,
//! deduplicated (no test file is invoked twice even if it is named by
//! overlapping selector reach), and zero times for anything outside the
//! union. Without this, the CI lane that mirrors the dashboard's subtree
//! drilldown is missing — operators debugging a regression in a mid-DAG
//! story have no single command that covers both sides of the impact
//! radius.
//!
//! The fixture DAG here has a diamond — a story reachable both as an
//! ancestor and (via a different edge path) as a descendant of the
//! target's subtree — so the dedup invariant is exercised explicitly.
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

// Fixture DAG with a diamond that shares a node via two distinct
// `depends_on` edges. All edges are child -> parent:
//
//         ANC_ROOT
//           ^
//           |
//         ANC_MID
//           ^
//           |
//         TARGET          <-- selector `+TARGET+`
//         ^    ^
//         |    |
//    DESC_A   DESC_B
//         ^    ^
//         |    |
//         DESC_JOIN       (depends_on both DESC_A and DESC_B — the diamond node)
//
//   UNRELATED              <-- must be excluded
const ID_ANC_ROOT: u32 = 81221;
const ID_ANC_MID: u32 = 81222;
const ID_TARGET: u32 = 81223;
const ID_DESC_A: u32 = 81224;
const ID_DESC_B: u32 = 81225;
const ID_DESC_JOIN: u32 = 81226;
const ID_UNRELATED: u32 = 81227;

#[test]
fn plus_id_plus_selector_invokes_full_subtree_exactly_once_per_story_across_a_diamond() {
    // Story 18 made signer resolution mandatory on every Recorder::record
    // call (which CiRunner delegates to per executed story); tier 2
    // (`AGENTIC_SIGNER` env var) is the cheapest fixture setup the
    // recorder will accept. Cleared at the end of the test.
    std::env::set_var("AGENTIC_SIGNER", "test-fixture@signer.local");

    // Build the seven-story diamond DAG via the shared kit primitive —
    // the local `write_fixture_story` / `setup_fixture_corpus` helpers
    // this file used to carry are now sourced from `agentic_test_support`
    // per stories/12.yml's kit-vs-bespoke contract.
    let corpus = FixtureCorpus::new();
    corpus.write_story(ID_ANC_ROOT, &[]);
    corpus.write_story(ID_ANC_MID, &[ID_ANC_ROOT]);
    corpus.write_story(ID_TARGET, &[ID_ANC_MID]);
    corpus.write_story(ID_DESC_A, &[ID_TARGET]);
    corpus.write_story(ID_DESC_B, &[ID_TARGET]);
    // The diamond: DESC_JOIN depends on both DESC_A and DESC_B — it is
    // reachable via two paths from TARGET and must still be invoked
    // exactly ONCE under +<id>+.
    corpus.write_story(ID_DESC_JOIN, &[ID_DESC_A, ID_DESC_B]);
    corpus.write_story(ID_UNRELATED, &[]);
    let stories_dir = corpus.stories_dir();

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let executor = RecordingExecutor::default();
    let runner = CiRunner::new(store.clone(), Box::new(executor.clone()), stories_dir);

    let selector = format!("+{ID_TARGET}+");
    runner
        .run(&selector)
        .expect("runner must succeed across full subtree on a clean stub corpus");

    let recorded = executor.recorded_calls();
    let invoked: Vec<u32> = recorded.iter().map(|call| call.story_id).collect();

    // UNION of ancestor and descendant sets, target included, deduplicated.
    let expected_union = [
        ID_ANC_ROOT,
        ID_ANC_MID,
        ID_TARGET,
        ID_DESC_A,
        ID_DESC_B,
        ID_DESC_JOIN,
    ];
    for expected in expected_union {
        assert!(
            invoked.contains(&expected),
            "+<id>+ must cover story {expected}; invoked={invoked:?}"
        );
        let count = invoked.iter().filter(|id| **id == expected).count();
        // The dedup invariant: DESC_JOIN is reachable via two edges
        // (DESC_A and DESC_B) but must still be invoked exactly ONCE.
        assert_eq!(
            count, 1,
            "story {expected} must be invoked exactly once under +<id>+ (dedup across diamond); \
             got {count}; all invocations={invoked:?}"
        );
    }

    assert!(
        !invoked.contains(&ID_UNRELATED),
        "+<id>+ must EXCLUDE unrelated story {ID_UNRELATED}; invoked={invoked:?}"
    );

    assert_eq!(
        recorded.len(),
        expected_union.len(),
        "+{ID_TARGET}+ must trigger exactly {} invocations (union size, deduped); got {}: {:?}",
        expected_union.len(),
        recorded.len(),
        invoked
    );

    // Cleanup: clear the env var we set for this test so it does not
    // leak across test invocations sharing the same process.
    std::env::remove_var("AGENTIC_SIGNER");
}
