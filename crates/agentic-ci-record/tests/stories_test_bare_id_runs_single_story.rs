//! Story 12 acceptance test: bare `<id>` selector runs exactly one story.
//!
//! Justification (from stories/12.yml): proves the bare-id case at the
//! library boundary — `CiRunner::run("<id>")` (no `+` on either side)
//! runs ONLY the acceptance tests declared by that exact story, same as
//! running the story's acceptance suite by hand. Without this, the
//! selector grammar is inconsistent with story 10's positional-argument
//! contract (where bare `<id>` is a specific operation) and operators
//! would have to guess whether `agentic stories test 10` runs one story
//! or pulls in the subtree.
//!
//! Additionally pins the kit-vs-bespoke contract per stories/12.yml's
//! amended justification: the fixture corpus, repo seeding, and stub
//! executor MUST source from `agentic_test_support`'s shared primitives
//! (story 26), not bespoke per-file helpers. The deep-modules contract
//! is observable in this file's `use` block — a reimplementation that
//! brought back a local `fn write_fixture_story`, `fn setup_fixture_corpus`,
//! or `struct StubExecutor impl TestExecutor` would fail the contract.
//!
//! Red today is compile-red: `agentic_test_support` is not yet declared
//! as a dev-dependency on `agentic-ci-record`. The kit imports below
//! resolve to `unresolved import` until build-rust adds the dev-dep in
//! the next commit (the kit-adoption pilot's intentional intermediate
//! red surface).

use std::sync::Arc;

use agentic_ci_record::CiRunner;
use agentic_store::{MemStore, Store};
use agentic_test_support::{FixtureCorpus, RecordingExecutor};

// Fixture DAG: target has both an ancestor and a descendant. Bare-id
// selection must exclude both sides.
const ID_ANC: u32 = 81231;
const ID_TARGET: u32 = 81232;
const ID_DESC: u32 = 81233;

#[test]
fn bare_id_selector_invokes_executor_only_for_the_exact_story() {
    // Build the three-story DAG via the shared kit primitive — the
    // local `write_fixture_story` / `setup_fixture_corpus` helpers this
    // file used to carry are now sourced from `agentic_test_support`
    // per stories/12.yml's kit-vs-bespoke contract.
    let corpus = FixtureCorpus::new();
    corpus.write_story(ID_ANC, &[]);
    corpus.write_story(ID_TARGET, &[ID_ANC]);
    corpus.write_story(ID_DESC, &[ID_TARGET]);
    let stories_dir = corpus.stories_dir();

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let executor = RecordingExecutor::default();
    let runner = CiRunner::new(store.clone(), Box::new(executor.clone()), stories_dir);

    let selector = format!("{ID_TARGET}");
    runner
        .run(&selector)
        .expect("runner must succeed on bare-id selector");

    let recorded = executor.recorded_calls();
    let invoked: Vec<u32> = recorded.iter().map(|call| call.story_id).collect();

    // Exactly one invocation, for the exact target.
    assert_eq!(
        recorded.len(),
        1,
        "bare-<id> must trigger exactly ONE executor call; got {}: {:?}",
        recorded.len(),
        invoked
    );
    assert_eq!(
        invoked,
        vec![ID_TARGET],
        "bare-<id> must cover only the target story {ID_TARGET}; got {invoked:?}"
    );

    // Explicitly assert neither side of the DAG leaked in.
    assert!(
        !invoked.contains(&ID_ANC),
        "bare-<id> must EXCLUDE ancestor {ID_ANC}; invoked={invoked:?}"
    );
    assert!(
        !invoked.contains(&ID_DESC),
        "bare-<id> must EXCLUDE descendant {ID_DESC}; invoked={invoked:?}"
    );
}
