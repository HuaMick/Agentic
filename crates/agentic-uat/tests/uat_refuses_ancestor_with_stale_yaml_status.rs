//! Story 11 acceptance test: evidence-over-claim rule — a hand-written
//! `status: healthy` on an ancestor with no signing row is still refused.
//!
//! Justification (from stories/11.yml):
//! Proves the evidence-over-claim rule: given a story `<id>` whose
//! direct ancestor `<A>` has `status: healthy` on disk but has NO
//! `uat_signings.verdict=pass` row in the store (a hand-edited or
//! orphan claim), `Uat::run` with a Pass verdict on `<id>` returns
//! `UatError::AncestorNotHealthy` naming `<A>` with a reason
//! distinguishable from "ancestor YAML says it isn't healthy" —
//! e.g. "no signing row." Without this, a hand-written `status:
//! healthy` on an ancestor could side-step the gate and undermine
//! the "only `agentic uat` writes healthy" invariant exactly where
//! it matters (on a story that depends on the hand-edited one).
//!
//! Red today is compile-red via the missing `UatError::AncestorNotHealthy`
//! variant and the missing `reason` sub-enum that distinguishes
//! "no signing row" from "YAML status != healthy."

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_uat::{AncestorUnhealthyReason, StubExecutor, Uat, UatError};
use tempfile::TempDir;

const LEAF_ID: u32 = 11201;
const ANCESTOR_ID: u32 = 11202;

// Note: the ancestor claims `status: healthy` on disk but no signing row
// will be written to the store — that is the whole point of this test.
const LEAF_YAML: &str = r#"id: 11201
title: "Leaf whose ancestor claims healthy but has no signing row"

outcome: |
  A fixture leaf used to exercise the evidence-over-claim refusal.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_ancestor_with_stale_yaml_status.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Run the stub executor; expect evidence-over-claim refusal.

guidance: |
  Fixture authored inline for the story-11 stale-YAML-status scaffold.
  Not a real story.

depends_on:
  - 11202
"#;

const ANCESTOR_YAML: &str = r#"id: 11202
title: "Ancestor with hand-edited status: healthy and zero signing rows"

outcome: |
  Ancestor fixture whose `status: healthy` is a claim on disk without
  corresponding evidence in the store.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_ancestor_with_stale_yaml_status.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; simulates a hand-edited healthy status with
  no signing row.

depends_on: []
"#;

#[test]
fn uat_run_pass_refuses_when_ancestor_yaml_says_healthy_but_no_signing_row_exists() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let leaf_path = stories_dir.join(format!("{LEAF_ID}.yml"));
    let ancestor_path = stories_dir.join(format!("{ANCESTOR_ID}.yml"));
    fs::write(&leaf_path, LEAF_YAML).expect("write leaf fixture");
    fs::write(&ancestor_path, ANCESTOR_YAML).expect("write ancestor fixture");

    init_repo_and_commit_seed(repo_root);
    let leaf_bytes_before = fs::read(&leaf_path).expect("read leaf before run");

    // Deliberately do NOT seed a uat_signings row for the ancestor —
    // that absence is what this test pins.
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    let err = uat.run(LEAF_ID).expect_err(
        "Pass when ancestor YAML claims healthy but has no signing row must be refused",
    );

    match err {
        UatError::AncestorNotHealthy {
            ancestor_id,
            reason,
        } => {
            assert_eq!(
                ancestor_id, ANCESTOR_ID,
                "refusal must name the offending ancestor ({ANCESTOR_ID}); got {ancestor_id}"
            );
            assert!(
                matches!(reason, AncestorUnhealthyReason::NoSigningRow),
                "refusal reason must distinguish \"no signing row\" from \
                 \"YAML status != healthy\"; got {reason:?}"
            );
        }
        other => panic!(
            "Pass with an orphan-healthy ancestor must return \
             UatError::AncestorNotHealthy {{ ancestor_id: {ANCESTOR_ID}, \
             reason: NoSigningRow }}; got {other:?}"
        ),
    }

    // No new rows written.
    let rows = store
        .query("uat_signings", &|doc| {
            let sid = doc.get("story_id").and_then(|v| v.as_u64());
            sid == Some(LEAF_ID as u64) || sid == Some(ANCESTOR_ID as u64)
        })
        .expect("store query should succeed");
    assert!(
        rows.is_empty(),
        "evidence-over-claim refusal must write zero uat_signings rows; got {rows:?}"
    );

    let leaf_bytes_after = fs::read(&leaf_path).expect("read leaf after run");
    assert_eq!(
        leaf_bytes_after, leaf_bytes_before,
        "evidence-over-claim refusal must not rewrite the target story YAML"
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
