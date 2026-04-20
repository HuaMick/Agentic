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
//! The scaffold drives the library directly per the story's
//! "Test file locations" guidance: a fixture `stories/` directory under a
//! `TempDir`, a `StubExecutor` that counts and records invocations per
//! story, and a `MemStore` for `test_runs`. Red today is compile-red: the
//! `agentic_ci_record::{CiRunner, TestExecutor, ExecutorOutcome}` surface
//! does not yet exist in `src/lib.rs`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agentic_ci_record::{CiRunner, ExecutorOutcome, TestExecutor};
use agentic_store::{MemStore, Store};
use tempfile::TempDir;

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

/// Records which test files the runner asked the executor to run, keyed
/// by the story whose acceptance.tests[] the files came from. The runner
/// supplies the story id via its invocation contract; if it does not,
/// this stub groups by the test-file path's parent-crate hint (not
/// authoritative — story id is what matters for the ancestor-set check).
#[derive(Default)]
struct Calls {
    /// One entry per executor invocation: (story_id, test_files).
    invocations: Vec<(u32, Vec<PathBuf>)>,
}

struct StubExecutor {
    calls: Arc<Mutex<Calls>>,
    /// Per-story verdict the stub returns. Defaults to Pass for every
    /// story; failing_tests always empty.
    verdicts: HashMap<u32, ExecutorOutcome>,
}

impl StubExecutor {
    fn new(calls: Arc<Mutex<Calls>>) -> Self {
        Self {
            calls,
            verdicts: HashMap::new(),
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
        self.verdicts
            .get(&story_id)
            .cloned()
            .unwrap_or_else(ExecutorOutcome::pass)
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
title: "Fixture {id} for story-12 ancestor-selector scaffold"

outcome: |
  Fixture row for the +<id> selector scaffold.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: {test_file}
      justification: |
        Present so the fixture is schema-valid. The live scaffold
        drives the runner against this file via its declared path.
  uat: |
    Run the runner with +<id>; assert invocation set.

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

    write_fixture_story(&stories_dir, ID_ANC_ROOT, &[]);
    write_fixture_story(&stories_dir, ID_ANC_MID, &[ID_ANC_ROOT]);
    write_fixture_story(&stories_dir, ID_TARGET, &[ID_ANC_MID]);
    write_fixture_story(&stories_dir, ID_DESC, &[ID_TARGET]);
    write_fixture_story(&stories_dir, ID_UNRELATED, &[]);

    tmp
}

#[test]
fn plus_id_selector_invokes_executor_only_for_target_and_transitive_ancestors() {
    let corpus = setup_fixture_corpus();
    let stories_dir = corpus.path().join("stories");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let calls = Arc::new(Mutex::new(Calls::default()));
    let executor = StubExecutor::new(calls.clone());

    let runner = CiRunner::new(store.clone(), Box::new(executor), stories_dir);

    // Selector `+TARGET` — ancestors plus target, no descendants.
    let selector = format!("+{ID_TARGET}");
    runner
        .run(&selector)
        .expect("runner must succeed across ancestor set on a clean stub corpus");

    let invocations = &calls
        .lock()
        .expect("calls mutex poisoned")
        .invocations;

    let invoked_story_ids: Vec<u32> = invocations.iter().map(|(id, _)| *id).collect();

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
        let count = invoked_story_ids.iter().filter(|id| **id == expected).count();
        assert_eq!(
            count, 1,
            "story {expected} must be invoked exactly once under +<id>; got {count} invocations; \
             all invocations={invoked_story_ids:?}"
        );
    }

    // Exactly three invocations total — the ancestor set size, nothing more.
    assert_eq!(
        invocations.len(),
        3,
        "+{ID_TARGET} must trigger exactly 3 executor calls (ANC_ROOT, ANC_MID, TARGET); got {}: {:?}",
        invocations.len(),
        invoked_story_ids
    );
}
