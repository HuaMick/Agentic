//! Story 12 acceptance test: `<id>+` selector runs only the target plus
//! its transitive descendants.
//!
//! Justification (from stories/12.yml): proves the `<id>+` selector at
//! the library boundary — `CiRunner::run("<id>+")` invokes the test
//! executor exactly once per acceptance-test file declared by the target
//! story and each of its transitive descendants, and zero times for any
//! story outside that descendant set. Without this, an operator changing
//! an upstream contract cannot prove "nothing I broke is downstream"
//! without running the world.
//!
//! Additionally pins the kit-vs-bespoke contract per stories/12.yml's
//! amended justification: the fixture corpus, repo seeding, and stub
//! executor MUST source from `agentic_test_support`'s shared primitives
//! (story 26), not bespoke per-file helpers. The deep-modules contract
//! is observable in this file's `use` block — a reimplementation that
//! brought back a local `fn write_fixture_story`, `fn setup_fixture_corpus`,
//! or `struct StubExecutor impl TestExecutor` would fail the contract.
//! Reference `agents/assets/principles/deep-modules.yml`'s
//! `application_to_test_scaffolding` for the operational rationale.

use std::sync::Arc;

use agentic_ci_record::CiRunner;
use agentic_store::{MemStore, Store};
use agentic_test_support::{FixtureCorpus, RecordingExecutor};

// Fixture DAG (child -> parent via `depends_on`):
//
//    ANC
//     ^
//     |
//   TARGET            <-- selector `TARGET+` must cover {TARGET, DESC_MID, DESC_LEAF}
//     ^
//     |
//   DESC_MID
//     ^
//     |
//   DESC_LEAF
//
//   UNRELATED         <-- must NOT be covered
const ID_ANC: u32 = 81211;
const ID_TARGET: u32 = 81212;
const ID_DESC_MID: u32 = 81213;
const ID_DESC_LEAF: u32 = 81214;
const ID_UNRELATED: u32 = 81215;

#[test]
fn id_plus_selector_invokes_executor_only_for_target_and_transitive_descendants() {
    // Build the five-story DAG via the shared kit primitive — the
    // local `write_fixture_story` / `setup_fixture_corpus` helpers this
    // file used to carry are now sourced from `agentic_test_support`
    // per stories/12.yml's kit-vs-bespoke contract.
    let corpus = FixtureCorpus::new();
    corpus.write_story(ID_ANC, &[]);
    corpus.write_story(ID_TARGET, &[ID_ANC]);
    corpus.write_story(ID_DESC_MID, &[ID_TARGET]);
    corpus.write_story(ID_DESC_LEAF, &[ID_DESC_MID]);
    corpus.write_story(ID_UNRELATED, &[]);
    let stories_dir = corpus.stories_dir();

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let executor = RecordingExecutor::default();
    let runner = CiRunner::new(store.clone(), Box::new(executor.clone()), stories_dir);

    let selector = format!("{ID_TARGET}+");
    runner
        .run(&selector)
        .expect("runner must succeed across descendant set on a clean stub corpus");

    let recorded = executor.recorded_calls();
    let invoked: Vec<u32> = recorded.iter().map(|call| call.story_id).collect();

    // EXACTLY {target, all transitive descendants}, once each.
    for expected in [ID_TARGET, ID_DESC_MID, ID_DESC_LEAF] {
        assert!(
            invoked.contains(&expected),
            "<id>+ must cover story {expected}; invoked={invoked:?}"
        );
        let count = invoked.iter().filter(|id| **id == expected).count();
        assert_eq!(
            count, 1,
            "story {expected} must be invoked exactly once; got {count}; all invocations={invoked:?}"
        );
    }

    // Ancestors and unrelated stories are out of scope for <id>+.
    assert!(
        !invoked.contains(&ID_ANC),
        "<id>+ must EXCLUDE ancestor {ID_ANC}; invoked={invoked:?}"
    );
    assert!(
        !invoked.contains(&ID_UNRELATED),
        "<id>+ must EXCLUDE unrelated story {ID_UNRELATED}; invoked={invoked:?}"
    );

    assert_eq!(
        recorded.len(),
        3,
        "{ID_TARGET}+ must trigger exactly 3 executor calls (TARGET, DESC_MID, DESC_LEAF); got {}: {:?}",
        recorded.len(),
        invoked
    );
}
