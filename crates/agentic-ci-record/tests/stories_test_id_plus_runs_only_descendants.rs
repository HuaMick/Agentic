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
//! Red today is compile-red: the `agentic_ci_record::{CiRunner,
//! TestExecutor, ExecutorOutcome}` surface does not yet exist.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agentic_ci_record::{CiRunner, ExecutorOutcome, TestExecutor};
use agentic_store::{MemStore, Store};
use tempfile::TempDir;

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

#[derive(Default)]
struct Calls {
    invocations: Vec<(u32, Vec<PathBuf>)>,
}

struct StubExecutor {
    calls: Arc<Mutex<Calls>>,
    _verdicts: HashMap<u32, ExecutorOutcome>,
}

impl StubExecutor {
    fn new(calls: Arc<Mutex<Calls>>) -> Self {
        Self {
            calls,
            _verdicts: HashMap::new(),
        }
    }
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
    let test_file = format!(
        "crates/agentic-ci-record/tests/fixture_story_{id}.rs"
    );
    let body = format!(
        r#"id: {id}
title: "Fixture {id} for story-12 descendant-selector scaffold"

outcome: |
  Fixture row for the <id>+ selector scaffold.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: {test_file}
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Run the runner with <id>+; assert invocation set.

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
    write_fixture_story(&stories_dir, ID_DESC_MID, &[ID_TARGET]);
    write_fixture_story(&stories_dir, ID_DESC_LEAF, &[ID_DESC_MID]);
    write_fixture_story(&stories_dir, ID_UNRELATED, &[]);

    tmp
}

#[test]
fn id_plus_selector_invokes_executor_only_for_target_and_transitive_descendants() {
    let corpus = setup_fixture_corpus();
    let stories_dir = corpus.path().join("stories");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let calls = Arc::new(Mutex::new(Calls::default()));
    let executor = StubExecutor::new(calls.clone());

    let runner = CiRunner::new(store.clone(), Box::new(executor), stories_dir);

    let selector = format!("{ID_TARGET}+");
    runner
        .run(&selector)
        .expect("runner must succeed across descendant set on a clean stub corpus");

    let invocations = &calls
        .lock()
        .expect("calls mutex poisoned")
        .invocations;
    let invoked: Vec<u32> = invocations.iter().map(|(id, _)| *id).collect();

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
        invocations.len(),
        3,
        "{ID_TARGET}+ must trigger exactly 3 executor calls (TARGET, DESC_MID, DESC_LEAF); got {}: {:?}",
        invocations.len(),
        invoked
    );
}
