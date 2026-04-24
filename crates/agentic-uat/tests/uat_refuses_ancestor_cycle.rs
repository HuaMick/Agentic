//! Story 11 acceptance test: cycle defence at the UAT boundary.
//!
//! Justification (from stories/11.yml):
//! Proves the invariant defence at the UAT boundary: given a
//! `stories/` directory the loader accepted but where a cycle somehow
//! reached the UAT call (e.g. a regression in the loader's DAG check),
//! `Uat::run` returns `UatError::Cycle` naming the offending edge,
//! writes no row, and does not rewrite any YAML. Without this, a
//! cycle reaching the UAT path would either loop forever computing
//! "is every ancestor healthy" or panic — both strictly worse than a
//! loud refusal.
//!
//! The cycle in this fixture is constructed deliberately on disk
//! (11501 depends_on 11502 depends_on 11501) so the UAT path sees a
//! cycle even though a well-behaved loader would reject it earlier.
//! Red today is compile-red via the missing `UatError::Cycle` variant.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_uat::{StubExecutor, Uat, UatError};
use tempfile::TempDir;

const A_ID: u32 = 11501;
const B_ID: u32 = 11502;

const A_YAML: &str = r#"id: 11501
title: "Cycle participant A that depends on B"

outcome: |
  Fixture story whose depends_on names B, completing a cycle with B's
  depends_on.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_ancestor_cycle.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Run the stub executor; expect cycle refusal.

guidance: |
  Fixture authored inline for the story-11 cycle-defence scaffold.
  Not a real story.

depends_on:
  - 11502
"#;

const B_YAML: &str = r#"id: 11502
title: "Cycle participant B that depends on A"

outcome: |
  Fixture story whose depends_on names A, completing a cycle with A's
  depends_on.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_ancestor_cycle.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the other cycle participant.

depends_on:
  - 11501
"#;

#[test]
fn uat_run_refuses_with_typed_cycle_error_when_depends_on_graph_cycles() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let a_path = stories_dir.join(format!("{A_ID}.yml"));
    let b_path = stories_dir.join(format!("{B_ID}.yml"));
    fs::write(&a_path, A_YAML).expect("write a fixture");
    fs::write(&b_path, B_YAML).expect("write b fixture");

    init_repo_and_commit_seed(repo_root);
    let a_bytes_before = fs::read(&a_path).expect("read a before run");
    let b_bytes_before = fs::read(&b_path).expect("read b before run");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    let err = uat
        .run(A_ID)
        .expect_err("Pass on a story whose ancestry graph cycles must be a typed refusal");

    match err {
        UatError::Cycle { edge } => {
            let (from, to) = edge;
            // The edge must name one of the two cycle participants on
            // each side — either A -> B or B -> A is a valid naming.
            let participants = [A_ID, B_ID];
            assert!(
                participants.contains(&from) && participants.contains(&to),
                "cycle edge must name the two cycle participants \
                 ({A_ID}, {B_ID}); got ({from}, {to})"
            );
            assert_ne!(
                from, to,
                "cycle edge must name two distinct participants; got self-loop ({from}, {to})"
            );
        }
        other => panic!(
            "cyclic depends_on graph must return UatError::Cycle naming the \
             offending edge; got {other:?}"
        ),
    }

    // Neither YAML was rewritten; no rows were written.
    let a_bytes_after = fs::read(&a_path).expect("read a after run");
    let b_bytes_after = fs::read(&b_path).expect("read b after run");
    assert_eq!(
        a_bytes_after, a_bytes_before,
        "cycle refusal must not rewrite participant A's YAML"
    );
    assert_eq!(
        b_bytes_after, b_bytes_before,
        "cycle refusal must not rewrite participant B's YAML"
    );

    let rows = store
        .query("uat_signings", &|_| true)
        .expect("store query should succeed");
    assert!(
        rows.is_empty(),
        "cycle refusal must write zero uat_signings rows; got {rows:?}"
    );
}

fn init_repo_and_commit_seed(root: &Path) -> String {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder")
            .expect("set user.name");
        cfg.set_str("user.email", "test@agentic.local")
            .expect("set user.email");
    }
    let mut index = repo.index().expect("repo index");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("stage all");
    index.write().expect("write index");
    let tree_oid = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_oid).expect("find tree");
    let sig = repo.signature().expect("repo signature");
    let commit_oid = repo
        .commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
    commit_oid.to_string()
}
