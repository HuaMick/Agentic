//! Story 15 acceptance test: fail-closed-on-dirty-tree contract
//! survives the realignment. On a dirty working tree (any untracked
//! file, any uncommitted change outside the scaffold paths the user
//! just authored) record returns `TestBuilderError::DirtyTree`,
//! writes zero evidence, creates no directory under
//! `evidence/runs/<id>/`, and leaves the tree byte-identical.
//!
//! Justification (from stories/15.yml acceptance.tests[6]): without
//! this, red-state evidence stops being pinnable to a commit — the
//! same forgery axis story 7 closed, now re-opened by a library that
//! did not re-assert the gate.
//!
//! Red today is compile-red: the story-15 `TestBuilder::record` API
//! does not exist yet, so `cargo check` fails on the unresolved
//! item.

use std::fs;
use std::path::Path;

use agentic_test_builder::{TestBuilder, TestBuilderError};
use tempfile::TempDir;

const STORY_ID: u32 = 99_015_007;

const FIXTURE_YAML: &str = r#"id: 99015007
title: "Fixture for story 15 record-refuses-on-dirty-tree"

outcome: |
  Fixture used to prove record refuses with DirtyTree when the
  working tree has an untracked file outside the scaffold paths.

status: proposed

patterns:
- fail-closed-on-dirty-tree

acceptance:
  tests:
    - file: crates/fixture-crate/tests/dirty_path.rs
      justification: |
        Proves the fail-closed-on-dirty-tree gate on record still
        fires after the realignment; the dirty-tree check happens
        before any probe so no evidence is written and no directory
        is created under evidence/runs/<id>/.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

const SCAFFOLD_BODY: &str = r#"use fixture_crate::does_not_exist;

#[test]
fn dirty_path() {
    assert_eq!(does_not_exist(), 0);
}
"#;

#[test]
fn record_refuses_with_dirty_tree_when_untracked_file_exists_outside_scaffold_paths() {
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    // Scaffold file exists and is a real Rust file the user "wrote" —
    // it is committed in the seed so it is NOT the source of the
    // dirt.
    let scaffold_path = repo_root.join("crates/fixture-crate/tests/dirty_path.rs");
    fs::create_dir_all(scaffold_path.parent().unwrap()).expect("tests dir");
    fs::write(&scaffold_path, SCAFFOLD_BODY).expect("write scaffold");

    init_repo_and_commit_seed(repo_root);

    // NOW, after the seed commit, dirty the tree OUTSIDE the scaffold
    // path. This is the "dirt EVERYWHERE ELSE" case the justification
    // names — record must refuse even though the scaffold itself is
    // fine.
    let dirt_path = repo_root.join("unrelated-scratch.txt");
    fs::write(&dirt_path, b"scratch\n").expect("write dirt");

    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    let before_listing = listing(repo_root);

    let builder = TestBuilder::new(repo_root);
    let result = builder.record(STORY_ID);

    match result {
        Err(TestBuilderError::DirtyTree) => {
            // Expected. Fall through to the side-effect assertions.
        }
        other => panic!(
            "record on a dirty tree must return DirtyTree; got {:?}",
            other
        ),
    }

    assert!(
        !evidence_dir.exists(),
        "record refusal must not create evidence/runs/{STORY_ID}/"
    );
    let after_listing = listing(repo_root);
    assert_eq!(
        before_listing, after_listing,
        "record refusal must leave the tree byte-identical; before:\n{before_listing}\nafter:\n{after_listing}"
    );

    // Belt-and-braces: the dirt we planted is still there untouched.
    assert!(dirt_path.exists(), "pre-existing dirt must still be present");
}

fn listing(root: &Path) -> String {
    let mut entries: Vec<(String, u64)> = Vec::new();
    walk(root, root, &mut entries);
    entries.sort();
    entries
        .into_iter()
        .map(|(rel, size)| format!("{rel}\t{size}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, u64)>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == ".git" || name == "target" {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else if let Ok(meta) = fs::metadata(&path) {
                let rel = path
                    .strip_prefix(root)
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                out.push((rel, meta.len()));
            }
        }
    }
}

fn init_repo_and_commit_seed(root: &Path) {
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
    repo.commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
}
