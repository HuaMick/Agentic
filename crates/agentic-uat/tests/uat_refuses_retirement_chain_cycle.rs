//! Story 11 acceptance test (retirement-chain amendment): defence-
//! in-depth on supersession cycles. A cycle along `superseded_by`
//! edges that reaches the UAT call surfaces as the same
//! `UatError::Cycle` variant the existing depends_on-cycle test
//! pins, because operator-visible "cycle in the story graph" is one
//! failure, not two.
//!
//! Justification (from stories/11.yml):
//! Proves defence-in-depth on supersession cycles: given a
//! `stories/` directory the loader accepted but where a
//! supersession cycle somehow reaches the UAT call (e.g. a
//! regression in story 21's load-time `SupersededByCycle` check, or
//! an in-memory corruption of the chain), `Uat::run` on a
//! descendant whose chain-walk would traverse that cycle returns
//! `UatError::Cycle` naming a participating edge, writes no row,
//! and does not rewrite any YAML. The reused variant is
//! intentional: the gate has one cycle-refusal shape, not two,
//! because an operator-visible "cycle in the story graph" is the
//! same failure whether the cycle is along `depends_on` edges or
//! `superseded_by` edges — both collapse the invariant that
//! ancestry is a DAG.
//!
//! The cycle is constructed deliberately on disk (11951's
//! superseded_by points at 11952, and 11952's superseded_by points
//! back at 11951) so the UAT path's chain-walk sees a cycle even
//! though a well-behaved loader would reject it earlier.
//!
//! Red today is compile-red: the `Status::Retired` variant the
//! fixture YAML relies on does not yet exist on the
//! `agentic_story::Status` enum, and `Uat::run`'s chain-walk
//! cycle-defence branch is not yet implemented either.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_story::Status;
use agentic_uat::{StubExecutor, Uat, UatError};
use tempfile::TempDir;

const LEAF_ID: u32 = 11950;
const CYCLE_A_ID: u32 = 11951; // retired, superseded_by CYCLE_B_ID
const CYCLE_B_ID: u32 = 11952; // retired, superseded_by CYCLE_A_ID

const LEAF_YAML: &str = r#"id: 11950
title: "Leaf depending on a retired ancestor inside a supersession cycle"

outcome: |
  Leaf fixture used to exercise the gate's defence-in-depth against
  supersession cycles that slip past the loader.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_retirement_chain_cycle.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Run the stub executor; expect cycle refusal along the supersession
    chain.

guidance: |
  Fixture authored inline for the story-11 retirement-chain-cycle
  scaffold. Not a real story.

depends_on:
  - 11951
"#;

const CYCLE_A_YAML: &str = r#"id: 11951
title: "Retired ancestor whose superseded_by loops back through CYCLE_B"

outcome: |
  First cycle participant: retired and claims to be superseded by
  11952, which in turn claims to be superseded by 11951. A well-
  behaved loader (story 21's SupersededByCycle check) would reject
  this at load time; this test pins the gate's in-depth defence for
  the case where the loader's check regressed.

status: retired
superseded_by: 11952
retired_reason: |
  Fixture participant in a supersession cycle; the loader would
  normally reject this, but the gate defends in depth.

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_retirement_chain_cycle.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; one half of a supersession cycle.

depends_on: []
"#;

const CYCLE_B_YAML: &str = r#"id: 11952
title: "Retired ancestor whose superseded_by loops back to CYCLE_A"

outcome: |
  Second cycle participant: retired and claims to be superseded by
  11951, completing the cycle with 11951's superseded_by: 11952.

status: retired
superseded_by: 11951
retired_reason: |
  Fixture participant in a supersession cycle; the loader would
  normally reject this, but the gate defends in depth.

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_retirement_chain_cycle.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; other half of the supersession cycle.

depends_on: []
"#;

#[test]
fn uat_run_refuses_with_typed_cycle_error_when_supersession_chain_cycles() {
    // Cross-reference: Status::Retired must exist on the enum.
    let _retired_variant_exists: Status = Status::Retired;

    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let leaf_path = stories_dir.join(format!("{LEAF_ID}.yml"));
    let cycle_a_path = stories_dir.join(format!("{CYCLE_A_ID}.yml"));
    let cycle_b_path = stories_dir.join(format!("{CYCLE_B_ID}.yml"));
    fs::write(&leaf_path, LEAF_YAML).expect("write leaf fixture");
    fs::write(&cycle_a_path, CYCLE_A_YAML).expect("write cycle-a fixture");
    fs::write(&cycle_b_path, CYCLE_B_YAML).expect("write cycle-b fixture");

    init_repo_and_commit_seed(repo_root);
    let leaf_bytes_before = fs::read(&leaf_path).expect("read leaf before run");
    let a_bytes_before = fs::read(&cycle_a_path).expect("read cycle-a before run");
    let b_bytes_before = fs::read(&cycle_b_path).expect("read cycle-b before run");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    let err = uat
        .run(LEAF_ID)
        .expect_err("Pass whose chain-walk cycles along superseded_by must be a typed refusal");

    match err {
        UatError::Cycle { edge } => {
            let (from, to) = edge;
            // The edge must name the two supersession-cycle participants
            // on each side; either CYCLE_A_ID -> CYCLE_B_ID or
            // CYCLE_B_ID -> CYCLE_A_ID is a valid naming of the cycle.
            let participants = [CYCLE_A_ID, CYCLE_B_ID];
            assert!(
                participants.contains(&from) && participants.contains(&to),
                "cycle edge must name the two supersession-cycle participants \
                 ({CYCLE_A_ID}, {CYCLE_B_ID}); got ({from}, {to})"
            );
            assert_ne!(
                from, to,
                "cycle edge must name two distinct participants; got self-loop ({from}, {to})"
            );
        }
        UatError::AncestorNotHealthy { ancestor_id, .. } => panic!(
            "supersession cycle must surface as UatError::Cycle (one cycle-refusal shape, \
             not two), NOT as AncestorNotHealthy; got ancestor_id={ancestor_id} — the \
             chain-walk hit a cycle-participant and fell through to the unhealthy path \
             instead of bailing with Cycle"
        ),
        other => panic!(
            "supersession cycle must return UatError::Cycle naming a participating edge; \
             got {other:?}"
        ),
    }

    // No YAML rewritten; no rows written.
    let leaf_bytes_after = fs::read(&leaf_path).expect("read leaf after run");
    let a_bytes_after = fs::read(&cycle_a_path).expect("read cycle-a after run");
    let b_bytes_after = fs::read(&cycle_b_path).expect("read cycle-b after run");
    assert_eq!(
        leaf_bytes_after, leaf_bytes_before,
        "supersession-cycle refusal must not rewrite the leaf YAML"
    );
    assert_eq!(
        a_bytes_after, a_bytes_before,
        "supersession-cycle refusal must not rewrite cycle participant A's YAML"
    );
    assert_eq!(
        b_bytes_after, b_bytes_before,
        "supersession-cycle refusal must not rewrite cycle participant B's YAML"
    );

    let rows = store
        .query("uat_signings", &|_| true)
        .expect("store query should succeed");
    assert!(
        rows.is_empty(),
        "supersession-cycle refusal must write zero uat_signings rows; got {rows:?}"
    );
}

fn init_repo_and_commit_seed(root: &Path) -> String {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder").expect("set user.name");
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
