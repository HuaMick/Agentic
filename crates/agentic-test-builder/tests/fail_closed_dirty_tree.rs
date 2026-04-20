//! Story 7 acceptance test: dirty-tree refusal.
//!
//! Justification (from stories/7.yml): Proves the fail-closed contract on
//! a dirty working tree: with any uncommitted change present (including
//! an untracked file), `TestBuilder::run` returns
//! `TestBuilderError::DirtyTree`, writes zero scaffolds, and writes zero
//! evidence. Without this the red-state record can be forged — someone
//! could edit code into the tree, then run test-builder, and the
//! resulting evidence would claim those edits were red when they are
//! not.
//!
//! The scaffold initialises a fresh git repo in a `TempDir`, commits a
//! baseline (story fixture + an empty fixture crate), then creates an
//! untracked file so the working tree is dirty. It invokes
//! `TestBuilder::run` and asserts the typed error, no scaffold files on
//! disk, and no evidence file on disk. Red today is compile-red via the
//! missing `agentic_test_builder` public surface (`TestBuilder`,
//! `TestBuilderError`).

use std::fs;
use std::path::Path;

use agentic_test_builder::{TestBuilder, TestBuilderError};
use tempfile::TempDir;

const STORY_ID: u32 = 7004;

const FIXTURE_STORY_YAML: &str = r#"id: 7004
title: "Dirty-tree fixture: test-builder must refuse to run"

outcome: |
  A fixture story used only to exercise the dirty-tree refusal contract;
  no scaffold should be written.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/dirty-tree-fixture/tests/would_be_scaffolded.rs
      justification: |
        A substantive justification so that thin-justification refusal
        cannot be the reason the run stops — the only reason test-builder
        must refuse here is the dirty working tree.
  uat: |
    Dirty the tree; run test-builder; observe TestBuilderError::DirtyTree
    and zero side effects on disk.

guidance: |
  Fixture authored inline for the dirty-tree refusal scaffold. Not a
  real story.

depends_on: []
"#;

#[test]
fn fail_closed_dirty_tree_returns_dirty_tree_error_with_zero_side_effects() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    // Fixture crate so the scaffold WOULD be writable on a clean tree —
    // the only reason the run refuses here is the dirty tree.
    let fixture_root = repo_root.join("crates/dirty-tree-fixture");
    fs::create_dir_all(fixture_root.join("src")).expect("fixture src");
    fs::create_dir_all(fixture_root.join("tests")).expect("fixture tests");
    fs::write(
        fixture_root.join("Cargo.toml"),
        r#"[package]
name = "dirty-tree-fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write fixture Cargo.toml");
    fs::write(fixture_root.join("src/lib.rs"), b"").expect("write fixture lib.rs");

    init_repo_and_commit_seed(repo_root);

    // Dirty the tree with an untracked file.
    fs::write(repo_root.join("dirty.txt"), b"uncommitted\n").expect("write dirty file");

    let scaffold_path = fixture_root.join("tests/would_be_scaffolded.rs");
    let evidence_dir = repo_root.join("evidence/runs").join(STORY_ID.to_string());

    let builder = TestBuilder::new(repo_root);
    let err = builder
        .run(STORY_ID)
        .expect_err("dirty tree must surface as Err, not Ok");
    assert!(
        matches!(err, TestBuilderError::DirtyTree),
        "dirty tree must surface as TestBuilderError::DirtyTree; got {err:?}"
    );

    // No scaffold was written.
    assert!(
        !scaffold_path.exists(),
        "dirty-tree refusal must write zero scaffolds; found {}",
        scaffold_path.display()
    );

    // No evidence directory (or if it exists, it is empty of jsonl).
    if evidence_dir.exists() {
        let any_jsonl = fs::read_dir(&evidence_dir)
            .expect("read evidence dir")
            .filter_map(|e| e.ok())
            .any(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "jsonl")
            });
        assert!(
            !any_jsonl,
            "dirty-tree refusal must write zero evidence files"
        );
    }
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
