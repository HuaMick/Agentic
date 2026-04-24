//! Story 1 acceptance test: dirty tree surfaces as exit 2 with no side
//! effects.
//!
//! Justification (from stories/1.yml): proves the fail-closed contract
//! at the binary boundary: with an uncommitted change in the fixture
//! repo, `agentic uat <id> --verdict pass` exits 2, writes zero rows
//! to `uat_signings`, and leaves the fixture YAML unchanged. Without
//! this, the dirty-tree refusal pinned at the library level is not
//! proven to reach the operator via the expected exit code — a
//! wrapper that mistranslated the error to exit 1 would silently turn
//! "could not verdict" into "real failure."
//!
//! The scaffold seeds the fixture repo and commits, then writes an
//! untracked file (so the working tree is dirty in the sense
//! `git2::statuses()` reports). It invokes `agentic uat <id> --verdict
//! pass` and asserts exit 2 EXACTLY, zero signing rows in the
//! configured store, and byte-identical fixture YAML.

use std::fs;
use std::path::Path;

use agentic_store::{Store, SurrealStore};
use assert_cmd::Command;
use tempfile::TempDir;

const STORY_ID: u32 = 88804;

const FIXTURE_YAML: &str = r#"id: 88804
title: "Fixture story for story 1 CLI dirty-tree refusal"

outcome: |
  A fixture used only to exercise the dirty-tree refusal contract via
  the binary; its status must not be rewritten.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/uat_dirty_tree_exits_2.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the agentic binary against this file.
  uat: |
    Dirty the tree; run `agentic uat <id> --verdict pass`; assert
    exit 2.

guidance: |
  Fixture authored inline for the story-1 dirty-tree scaffold. Not a
  real story.

depends_on: []
"#;

#[test]
fn agentic_uat_on_dirty_tree_exits_two_writes_no_rows_and_leaves_yaml_unchanged() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    init_repo_and_commit_seed(repo_root);

    // Dirty the tree with an untracked file AFTER the seed commit —
    // `git2::Repository::statuses()` reports untracked content as
    // dirty (matches the behaviour pinned in story 1's dirty-tree
    // scaffold).
    fs::write(repo_root.join("dirty.txt"), b"uncommitted\n").expect("write dirty file");

    let before_bytes = fs::read(&story_path).expect("read fixture before run");

    let store_tmp = TempDir::new().expect("store tempdir");
    let store_path = store_tmp.path().to_path_buf();

    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("uat")
        .arg(STORY_ID.to_string())
        .arg("--verdict")
        .arg("pass")
        .arg("--store")
        .arg(&store_path)
        .assert();

    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let status = output.status;

    // Exit code 2 EXACTLY: the could-not-verdict contract from story
    // 1 guidance. 0 would be silent promotion (catastrophic); 1 would
    // be confused with a real Fail verdict.
    assert_eq!(
        status.code(),
        Some(2),
        "dirty-tree refusal must surface as exit 2 (could-not-verdict), \
         NOT 0 (pass) or 1 (fail); got status={status:?}\n\
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let store = SurrealStore::open(&store_path)
        .expect("re-opening the configured SurrealStore must succeed");
    let rows = store
        .query("uat_signings", &|_| true)
        .expect("uat_signings query must succeed");
    assert!(
        rows.is_empty(),
        "dirty-tree refusal must write zero uat_signings rows; \
         got {} rows: {rows:?}",
        rows.len()
    );

    let after_bytes = fs::read(&story_path).expect("read fixture after run");
    assert_eq!(
        after_bytes, before_bytes,
        "dirty-tree refusal must not touch the fixture YAML; file changed on disk"
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
