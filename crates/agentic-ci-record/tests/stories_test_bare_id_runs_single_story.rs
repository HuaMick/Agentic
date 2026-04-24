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
//! Red today is compile-red: the `agentic_ci_record::{CiRunner,
//! TestExecutor, ExecutorOutcome}` surface does not yet exist.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agentic_ci_record::{CiRunner, ExecutorOutcome, TestExecutor};
use agentic_store::{MemStore, Store};
use tempfile::TempDir;

// Fixture DAG: target has both an ancestor and a descendant. Bare-id
// selection must exclude both sides.
const ID_ANC: u32 = 81231;
const ID_TARGET: u32 = 81232;
const ID_DESC: u32 = 81233;

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
title: "Fixture {id} for story-12 bare-id scaffold"

outcome: |
  Fixture row for the bare-<id> selector scaffold.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: {test_file}
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Run the runner with <id>; assert invocation set is just {{id}}.

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

    tmp
}

#[test]
fn bare_id_selector_invokes_executor_only_for_the_exact_story() {
    let corpus = setup_fixture_corpus();
    let stories_dir = corpus.path().join("stories");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let calls = Arc::new(Mutex::new(Calls::default()));
    let executor = StubExecutor {
        calls: calls.clone(),
    };

    let runner = CiRunner::new(store.clone(), Box::new(executor), stories_dir);

    let selector = format!("{ID_TARGET}");
    runner
        .run(&selector)
        .expect("runner must succeed on bare-id selector");

    let invocations = &calls.lock().expect("calls mutex poisoned").invocations;
    let invoked: Vec<u32> = invocations.iter().map(|(id, _)| *id).collect();

    // Exactly one invocation, for the exact target.
    assert_eq!(
        invocations.len(),
        1,
        "bare-<id> must trigger exactly ONE executor call; got {}: {:?}",
        invocations.len(),
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
