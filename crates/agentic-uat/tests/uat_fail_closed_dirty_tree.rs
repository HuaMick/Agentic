//! Story 1 acceptance test: the fail-closed dirty-tree contract.
//!
//! Justification (from stories/1.yml): proves the fail-closed contract
//! on a dirty git working tree at the library boundary — with
//! uncommitted changes present, `Uat::run` refuses to produce a
//! verdict, returns `UatError::DirtyTree`, writes no row to
//! `uat_signings`, and does not touch the story YAML. Without this the
//! commit-signed guarantee is forgeable — someone could run UAT, see
//! Pass, then edit code and claim the old verdict still applies to the
//! new tree.
//!
//! The scaffold builds a fresh git repo in a `TempDir`, commits a seed
//! containing the story fixture, then writes an unrelated untracked
//! file so the working tree is dirty. It invokes the amended
//! `Uat::run(<id>, SignerSource::Resolve)` — configured with
//! `StubExecutor::always_pass()` so the ONLY reason the call can fail
//! is the dirty-tree precondition, which fires BEFORE the signer
//! resolver is consulted — and asserts the typed error, zero signing
//! rows, and an unchanged story file.
//!
//! Red today is compile-red via the missing `agentic_uat::SignerSource`
//! symbol: story 1's amendment (2026-04-23) changed `Uat::run`'s
//! signature to take a `SignerSource` as its second argument; this
//! scaffold now calls that two-argument shape, and the `use` of
//! `agentic_uat::SignerSource` fails to resolve until story 18 lands
//! the signer wire. The dirty-tree observable is unchanged by the
//! amendment — only the call site's shape moves.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_uat::{SignerSource, StubExecutor, Uat, UatError};
use tempfile::TempDir;

const STORY_ID: u32 = 4244;

const FIXTURE_YAML: &str = r#"id: 4244
title: "A fixture story used to exercise the dirty-tree refusal"

outcome: |
  A fixture used only to exercise the dirty-tree refusal contract; its
  status must not be rewritten.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_fail_closed_dirty_tree.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Make the tree dirty; run the stub executor; verify refusal.

guidance: |
  Fixture authored inline for the dirty-tree-refusal scaffold. Not a
  real story.

depends_on: []
"#;

#[test]
fn uat_run_refuses_on_dirty_tree_returning_dirty_tree_error_with_no_side_effects() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    // Seed commit so there IS a clean baseline; then dirty the tree.
    init_repo_and_commit_seed(repo_root);

    // Dirty the working tree with an untracked file. Untracked content
    // counts as "dirty" per the story's guidance ("no untracked files
    // that would affect the build").
    fs::write(repo_root.join("dirty.txt"), b"uncommitted\n").expect("write dirty file");

    let before_bytes = fs::read(&story_path).expect("read fixture before run");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    // Even an always-Pass executor must not matter here; the dirty-tree
    // gate runs BEFORE the executor is consulted.
    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    let err = uat
        .run(STORY_ID, SignerSource::Resolve)
        .expect_err("dirty tree must surface as an Err, not Ok(Pass/Fail)");
    assert!(
        matches!(err, UatError::DirtyTree),
        "dirty tree must surface as UatError::DirtyTree; got {err:?}"
    );

    // No row in uat_signings — not even a "could-not-verdict" ghost row.
    let rows = store
        .query("uat_signings", &|_doc| true)
        .expect("store query should succeed");
    assert!(
        rows.is_empty(),
        "dirty-tree refusal must write zero uat_signings rows; got {} rows: {rows:?}",
        rows.len()
    );

    // The story YAML is byte-for-byte unchanged.
    let after_bytes = fs::read(&story_path).expect("read fixture after run");
    assert_eq!(
        after_bytes, before_bytes,
        "dirty-tree refusal must not touch the story YAML; file changed on disk"
    );
}

/// See uat_pass.rs for rationale.
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
