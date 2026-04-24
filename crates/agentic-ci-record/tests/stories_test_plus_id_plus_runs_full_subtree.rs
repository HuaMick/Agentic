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
//! Red today is compile-red: the `agentic_ci_record::{CiRunner,
//! TestExecutor, ExecutorOutcome}` surface does not yet exist.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agentic_ci_record::{CiRunner, ExecutorOutcome, TestExecutor};
use agentic_store::{MemStore, Store};
use tempfile::TempDir;

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
title: "Fixture {id} for story-12 full-subtree scaffold"

outcome: |
  Fixture row for the +<id>+ selector scaffold.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: {test_file}
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Run the runner with +<id>+; assert dedup'd union.

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
    write_fixture_story(&stories_dir, ID_DESC_A, &[ID_TARGET]);
    write_fixture_story(&stories_dir, ID_DESC_B, &[ID_TARGET]);
    // The diamond: DESC_JOIN depends on both DESC_A and DESC_B — it is
    // reachable via two paths from TARGET and must still be invoked
    // exactly ONCE under +<id>+.
    write_fixture_story(&stories_dir, ID_DESC_JOIN, &[ID_DESC_A, ID_DESC_B]);
    write_fixture_story(&stories_dir, ID_UNRELATED, &[]);

    tmp
}

#[test]
fn plus_id_plus_selector_invokes_full_subtree_exactly_once_per_story_across_a_diamond() {
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
        .expect("runner must succeed across full subtree on a clean stub corpus");

    let invocations = &calls.lock().expect("calls mutex poisoned").invocations;
    let invoked: Vec<u32> = invocations.iter().map(|(id, _)| *id).collect();

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
        invocations.len(),
        expected_union.len(),
        "+{ID_TARGET}+ must trigger exactly {} invocations (union size, deduped); got {}: {:?}",
        expected_union.len(),
        invocations.len(),
        invoked
    );
}
