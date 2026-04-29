//! Story 28 acceptance test: fail-closed-on-dirty-tree at the library
//! boundary.
//!
//! Justification (from stories/28.yml acceptance.tests[4]):
//!   Proves the fail-closed-on-dirty-tree pattern at the library
//!   boundary: given an otherwise valid backfill request (YAML
//!   healthy, evidence present, history clean) but a dirty git
//!   working tree (any uncommitted change to any tracked file or
//!   any untracked non-ignored file),
//!   `Store::backfill_manual_signing(story_id)` returns
//!   `BackfillError::DirtyTree` and writes ZERO rows. The pattern's
//!   "no escape hatch" rule applies verbatim; there is no
//!   `--allow-dirty` flag.
//!
//! Red today is compile-red: `BackfillError::DirtyTree` and
//! `Store::backfill_manual_signing` do not yet exist on the
//! `agentic-store` public surface.

use std::fs;
use std::path::{Path, PathBuf};

use agentic_store::{BackfillError, MemStore, Store};
use tempfile::TempDir;

const STORY_ID: u32 = 28_051;
const SIGNER_EMAIL: &str = "backfill-dirty-tree@agentic.local";

const STORY_YAML_HEALTHY: &str = r#"id: 28051
title: "Fixture for story-28 dirty-tree scaffold"

outcome: |
  Fixture used for the dirty-tree scaffold; the request is otherwise
  valid (YAML healthy, evidence present, flip in history) but the
  working tree is dirty.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-store/tests/backfill_refuses_with_dirty_tree.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the library entry point against this file.
  uat: |
    Dirty the tree; run the backfill; assert DirtyTree and zero rows.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const STORY_YAML_UNDER_CONSTRUCTION: &str = r#"id: 28051
title: "Fixture for story-28 dirty-tree scaffold"

outcome: |
  Fixture used for the dirty-tree scaffold; the request is otherwise
  valid (YAML healthy, evidence present, flip in history) but the
  working tree is dirty.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-store/tests/backfill_refuses_with_dirty_tree.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the library entry point against this file.
  uat: |
    Dirty the tree; run the backfill; assert DirtyTree and zero rows.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const GREEN_JSONL: &str = "{\"run_id\":\"aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee\",\"story_id\":28051,\"commit\":\"deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\",\"timestamp\":\"2026-04-29T00:00:00Z\",\"verdicts\":[{\"file\":\"crates/agentic-store/tests/backfill_refuses_with_dirty_tree.rs\",\"verdict\":\"green\"}]}\n";

#[test]
fn backfill_refuses_with_typed_dirty_tree_error_when_tree_has_untracked_file() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, STORY_YAML_UNDER_CONSTRUCTION).expect("write under_construction yaml");

    let evidence_dir: PathBuf = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    fs::create_dir_all(&evidence_dir).expect("evidence dir");

    init_repo_seed_then_flip(
        repo_root,
        SIGNER_EMAIL,
        &story_path,
        STORY_YAML_HEALTHY,
        &evidence_dir,
    );

    // Dirty the tree AFTER the flip commit by adding an untracked file.
    // `git2::Repository::statuses()` reports untracked content as dirty.
    fs::write(repo_root.join("untracked.txt"), b"uncommitted\n").expect("write untracked file");

    let store = MemStore::new();

    let err = store
        .backfill_manual_signing(STORY_ID, repo_root)
        .expect_err("dirty-tree guard must refuse the otherwise-valid request");
    match &err {
        BackfillError::DirtyTree => {}
        other => panic!(
            "dirty-tree guard must surface BackfillError::DirtyTree; got {other:?}"
        ),
    }

    // ZERO rows in manual_signings.
    let rows = store
        .query("manual_signings", &|_| true)
        .expect("manual_signings query must succeed");
    assert!(
        rows.is_empty(),
        "dirty-tree refusal must write zero manual_signings rows; got {rows:?}"
    );
}

fn init_repo_seed_then_flip(
    root: &Path,
    email: &str,
    story_path: &Path,
    healthy_yaml: &str,
    evidence_dir: &Path,
) -> String {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder")
            .expect("set user.name");
        cfg.set_str("user.email", email).expect("set user.email");
    }
    let mut index = repo.index().expect("repo index");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("stage all");
    index.write().expect("write index");
    let seed_tree_oid = index.write_tree().expect("write seed tree");
    let seed_tree = repo.find_tree(seed_tree_oid).expect("find seed tree");
    let sig = repo.signature().expect("repo signature");
    let seed_commit_oid = repo
        .commit(Some("HEAD"), &sig, &sig, "seed: under_construction", &seed_tree, &[])
        .expect("commit seed");
    let seed_commit = repo.find_commit(seed_commit_oid).expect("find seed commit");

    fs::write(story_path, healthy_yaml).expect("flip yaml to healthy");
    fs::write(
        evidence_dir.join("2026-04-29T00-00-00Z-green.jsonl"),
        GREEN_JSONL,
    )
    .expect("write green evidence");

    let mut index = repo.index().expect("repo index 2");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("stage all 2");
    index.write().expect("write index 2");
    let flip_tree_oid = index.write_tree().expect("write flip tree");
    let flip_tree = repo.find_tree(flip_tree_oid).expect("find flip tree");
    let flip_commit_oid = repo
        .commit(
            Some("HEAD"),
            &sig,
            &sig,
            "story(28051): UAT promotion to healthy",
            &flip_tree,
            &[&seed_commit],
        )
        .expect("commit flip");

    format!("{}", flip_commit_oid)
}
