//! Story 15 acceptance test: `record` refuses when a planned scaffold
//! does not exist on disk — with a typed `ScaffoldMissing` naming the
//! absent path, exit semantics "could-not-verdict," zero evidence
//! rows, and no directory under `evidence/runs/<id>/`.
//!
//! Justification (from stories/15.yml acceptance.tests[2]): given a
//! fixture story whose plan names two scaffold paths and a working
//! tree where only one of the two files exists on disk,
//! `agentic test-build record <id>` returns
//! `TestBuilderError::ScaffoldMissing` naming the missing path,
//! writes zero evidence rows, creates no directory under
//! `evidence/runs/<id>/`, and leaves the tree byte-identical to its
//! pre-invocation state. Without this, record would silently skip
//! absent scaffolds and write a partial evidence row that attests
//! "story is red" for a file that does not exist — the
//! claude-as-component failure mode in a new skin.
//!
//! Red today is compile-red: `TestBuilder::record` and the
//! `TestBuilderError::ScaffoldMissing` variant are the new story-15
//! API surface; neither exists yet. The scaffold fails `cargo check`
//! on unresolved items.

use std::fs;
use std::path::Path;

use agentic_test_builder::{TestBuilder, TestBuilderError};
use tempfile::TempDir;

const STORY_ID: u32 = 99_015_003;

const FIXTURE_YAML: &str = r#"id: 99015003
title: "Fixture for story 15 record-refuses-when-scaffold-missing"

outcome: |
  Fixture used to prove record refuses with ScaffoldMissing when a
  planned scaffold path does not exist on disk.

status: proposed

patterns:
- standalone-resilient-library
- fail-closed-on-dirty-tree

acceptance:
  tests:
    - file: crates/fixture-crate/tests/scaffold_present.rs
      justification: |
        Proves the fixture crate's present-scaffold observable; this
        file WILL exist on disk at record time so record proceeds
        past this entry and lands on the missing one.
    - file: crates/fixture-crate/tests/scaffold_absent.rs
      justification: |
        Proves the fixture crate's absent-scaffold observable; this
        file WILL NOT exist on disk at record time, so record must
        refuse with ScaffoldMissing naming this path.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only. Two scaffold paths; only the first is written.

depends_on: []
"#;

/// A syntactically-valid Rust scaffold body. Its only job is to BE on
/// disk so the test's one-scaffold-present-one-absent shape holds.
const PRESENT_SCAFFOLD_BODY: &str = r#"#[test]
fn scaffold_present() {
    assert_eq!(fixture_crate::returns_one(), 1);
}
"#;

#[test]
fn record_refuses_with_scaffold_missing_naming_the_absent_path() {
    // Arrange: fixture repo with the story and ONE of the two scaffold
    // files on disk. The other is deliberately absent.
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    let present_path = repo_root.join("crates/fixture-crate/tests/scaffold_present.rs");
    let absent_path = repo_root.join("crates/fixture-crate/tests/scaffold_absent.rs");
    fs::create_dir_all(present_path.parent().unwrap()).expect("tests dir");
    fs::write(&present_path, PRESENT_SCAFFOLD_BODY).expect("write present scaffold");
    assert!(
        !absent_path.exists(),
        "pre-test invariant: the absent scaffold path must not exist"
    );

    init_repo_and_commit_seed(repo_root);

    let before_listing = listing(repo_root);
    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    assert!(
        !evidence_dir.exists(),
        "pre-test invariant: evidence dir must not exist yet"
    );

    // Act: call record. It must refuse with ScaffoldMissing naming
    // the absent path.
    let builder = TestBuilder::new(repo_root);
    let result = builder.record(STORY_ID);

    // Assert: the typed variant.
    match result {
        Err(TestBuilderError::ScaffoldMissing { file }) => {
            assert_eq!(
                file, absent_path,
                "ScaffoldMissing must name the absent scaffold path; got {}",
                file.display()
            );
        }
        other => panic!(
            "record must return ScaffoldMissing naming {}; got {:?}",
            absent_path.display(),
            other
        ),
    }

    // Assert: zero evidence. No evidence/runs/<id>/ directory created.
    assert!(
        !evidence_dir.exists(),
        "record refusal must not create evidence/runs/{STORY_ID}/; found {}",
        evidence_dir.display()
    );

    // Assert: tree is byte-identical to its pre-invocation state.
    let after_listing = listing(repo_root);
    assert_eq!(
        before_listing, after_listing,
        "record refusal must leave the tree byte-identical; before:\n{before_listing}\nafter:\n{after_listing}"
    );
}

/// Recursive listing of files and their byte contents, keyed by relative
/// path. Used as a proxy for "byte-identical" comparison.
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
            // Skip `.git` — libgit2 touches index timestamps on open.
            if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
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
