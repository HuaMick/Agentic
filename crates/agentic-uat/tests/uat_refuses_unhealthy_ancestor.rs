//! Story 11 acceptance test: direct-ancestor refusal at the library
//! boundary.
//!
//! Justification (from stories/11.yml):
//! Proves the core refusal at the library boundary: given a story
//! `<id>` whose `depends_on` names at least one ancestor story whose
//! current on-disk `status` is not `healthy` (i.e. one of
//! `proposed`, `under_construction`, or a status reflecting
//! historical regression), `Uat::run` with a Pass verdict returns
//! `UatError::AncestorNotHealthy` naming the offending ancestor id,
//! writes zero rows to `uat_signings`, and does NOT rewrite the
//! target story's YAML `status` field. Without this, the epic's
//! central invariant ("Pass is only signable when the whole ancestry
//! is proven") is unshipped and a leaf can claim Pass while standing
//! on an `under_construction` foundation — forging the same trust
//! shape a dirty-tree signing would.
//!
//! Red today is compile-red via the missing `UatError::AncestorNotHealthy`
//! variant (the ancestor-gate code path does not exist yet in
//! `agentic-uat`).

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_uat::{StubExecutor, Uat, UatError};
use tempfile::TempDir;

const LEAF_ID: u32 = 11001;
const ANCESTOR_ID: u32 = 11002;

const LEAF_YAML: &str = r#"id: 11001
title: "Leaf story that depends on an under_construction ancestor"

outcome: |
  A fixture leaf whose Pass must be refused because its ancestor is
  not healthy.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_unhealthy_ancestor.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Run the stub executor; expect ancestor refusal.

guidance: |
  Fixture authored inline for the story-11 direct-ancestor refusal
  scaffold. Not a real story.

depends_on:
  - 11002
"#;

const ANCESTOR_YAML: &str = r#"id: 11002
title: "Ancestor whose on-disk status is under_construction"

outcome: |
  An ancestor fixture whose status is deliberately not healthy so the
  leaf's Pass is refused.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_unhealthy_ancestor.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the unhealthy ancestor.

depends_on: []
"#;

#[test]
fn uat_run_pass_refuses_when_direct_ancestor_is_not_healthy() {
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

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    let err = uat
        .run(LEAF_ID)
        .expect_err("Pass on a leaf with an unhealthy ancestor must be a typed refusal");

    // Typed AncestorNotHealthy refusal naming the offending ancestor id.
    match err {
        UatError::AncestorNotHealthy { ancestor_id, .. } => {
            assert_eq!(
                ancestor_id, ANCESTOR_ID,
                "refusal must name the unhealthy ancestor id ({ANCESTOR_ID}); got {ancestor_id}"
            );
        }
        other => panic!(
            "Pass on a leaf with an unhealthy ancestor must return \
             UatError::AncestorNotHealthy naming ancestor {ANCESTOR_ID}; \
             got {other:?}"
        ),
    }

    // Zero rows in uat_signings.
    let rows = store
        .query("uat_signings", &|doc| {
            let sid = doc.get("story_id").and_then(|v| v.as_u64());
            sid == Some(LEAF_ID as u64) || sid == Some(ANCESTOR_ID as u64)
        })
        .expect("store query should succeed");
    assert!(
        rows.is_empty(),
        "ancestor refusal must write zero uat_signings rows; got {rows:?}"
    );

    // Leaf YAML is byte-for-byte unchanged — the refusal did not rewrite
    // the `status` field.
    let leaf_bytes_after = fs::read(&leaf_path).expect("read leaf after run");
    assert_eq!(
        leaf_bytes_after, leaf_bytes_before,
        "ancestor refusal must not rewrite the target story YAML"
    );
}

/// Initialise a git repo rooted at `root`, stage everything, commit, and
/// return the HEAD SHA. Duplicated from story 1's scaffolds rather than
/// hoisted so each test is independently readable.
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
