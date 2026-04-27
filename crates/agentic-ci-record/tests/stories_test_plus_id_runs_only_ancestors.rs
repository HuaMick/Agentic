//! Story 12 acceptance test: `+<id>` selector runs only the target plus
//! its transitive ancestors.
//!
//! Justification (from stories/12.yml): proves the `+<id>` selector at the
//! library boundary — `CiRunner::run("+<id>")` invokes the test executor
//! exactly once per acceptance-test file declared by the target story and
//! each of its transitive ancestors, and exactly zero times for any story
//! outside that ancestor set. Without this, operators cannot prove "the
//! code path leading up to my leaf still works" in isolation — which is
//! the upstream-only lane CI needs for targeted diagnostics.
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

// Fixture DAG (edges are `depends_on`, i.e. child -> parent):
//
//    ANC_ROOT
//       ^
//       |
//      ANC_MID
//       ^
//       |
//     TARGET           <-- selector `+TARGET` must cover {ANC_ROOT, ANC_MID, TARGET}
//       ^
//       |
//      DESC            <-- must NOT be covered
//
//   UNRELATED (no edge)  <-- must NOT be covered
const ID_ANC_ROOT: u32 = 81201;
const ID_ANC_MID: u32 = 81202;
const ID_TARGET: u32 = 81203;
const ID_DESC: u32 = 81204;
const ID_UNRELATED: u32 = 81205;

#[test]
fn plus_id_selector_invokes_executor_only_for_target_and_transitive_ancestors() {
    // Build the five-story DAG via the shared kit primitive — the
    // local `write_fixture_story` / `setup_fixture_corpus` helpers this
    // file used to carry are now sourced from `agentic_test_support`
    // per stories/12.yml's kit-vs-bespoke contract.
    let corpus = FixtureCorpus::new();
    corpus.write_story(ID_ANC_ROOT, &[]);
    corpus.write_story(ID_ANC_MID, &[ID_ANC_ROOT]);
    corpus.write_story(ID_TARGET, &[ID_ANC_MID]);
    corpus.write_story(ID_DESC, &[ID_TARGET]);
    corpus.write_story(ID_UNRELATED, &[]);
    let stories_dir = corpus.stories_dir();

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let executor = RecordingExecutor::default();
    let runner = CiRunner::new(store.clone(), Box::new(executor.clone()), stories_dir);

    // Selector `+TARGET` — ancestors plus target, no descendants.
    let selector = format!("+{ID_TARGET}");
    runner
        .run(&selector)
        .expect("runner must succeed across ancestor set on a clean stub corpus");

    let recorded = executor.recorded_calls();
    let invoked_story_ids: Vec<u32> = recorded.iter().map(|call| call.story_id).collect();

    // EXACTLY the ancestor set (target + transitive ancestors), once each.
    assert!(
        invoked_story_ids.contains(&ID_ANC_ROOT),
        "+<id> must cover the transitive ancestor {ID_ANC_ROOT}; invoked={invoked_story_ids:?}"
    );
    assert!(
        invoked_story_ids.contains(&ID_ANC_MID),
        "+<id> must cover the direct ancestor {ID_ANC_MID}; invoked={invoked_story_ids:?}"
    );
    assert!(
        invoked_story_ids.contains(&ID_TARGET),
        "+<id> must cover the target {ID_TARGET} itself; invoked={invoked_story_ids:?}"
    );

    // NOT descendants, NOT unrelated stories.
    assert!(
        !invoked_story_ids.contains(&ID_DESC),
        "+<id> must EXCLUDE descendant {ID_DESC}; invoked={invoked_story_ids:?}"
    );
    assert!(
        !invoked_story_ids.contains(&ID_UNRELATED),
        "+<id> must EXCLUDE unrelated story {ID_UNRELATED}; invoked={invoked_story_ids:?}"
    );

    // Exactly-once per covered story — no duplicate invocations even
    // though a diamond could be layered on later.
    for expected in [ID_ANC_ROOT, ID_ANC_MID, ID_TARGET] {
        let count = invoked_story_ids
            .iter()
            .filter(|id| **id == expected)
            .count();
        assert_eq!(
            count, 1,
            "story {expected} must be invoked exactly once under +<id>; got {count} invocations; \
             all invocations={invoked_story_ids:?}"
        );
    }

    // Exactly three invocations total — the ancestor set size, nothing more.
    assert_eq!(
        recorded.len(),
        3,
        "+{ID_TARGET} must trigger exactly 3 executor calls (ANC_ROOT, ANC_MID, TARGET); got {}: {:?}",
        recorded.len(),
        invoked_story_ids
    );
}
