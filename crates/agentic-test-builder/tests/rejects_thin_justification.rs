//! Story 7 acceptance test: thin-justification refusal.
//!
//! Justification (from stories/7.yml): Proves the thin-justification
//! refusal: given a story whose acceptance.tests[] contains one entry
//! with justification text `"TODO"` (or empty after trim, or a single
//! token), `TestBuilder::run` returns
//! `TestBuilderError::ThinJustification` naming the offending entry's
//! index, writes zero scaffolds (even for the substantive entries in the
//! same story), and writes zero evidence. Without this, story-writer
//! could offload test design by stubbing justifications, and
//! test-builder would produce scaffolds whose failure messages say
//! nothing — green evidence for a story that was never meaningfully
//! specified.
//!
//! The scaffold constructs a fixture story with TWO acceptance.tests[]
//! entries: the first has a substantive justification, the second has
//! the literal string `"TODO"`. `TestBuilder::run` must refuse the
//! whole story — no partial scaffolding — and the typed error must
//! name the offending entry's index (1 here, zero-based). Red today is
//! compile-red via the missing `agentic_test_builder` public surface
//! (`TestBuilder`, `TestBuilderError`).

use std::fs;
use std::path::Path;

use agentic_test_builder::{TestBuilder, TestBuilderError};
use tempfile::TempDir;

const STORY_ID: u32 = 7005;

const FIXTURE_STORY_YAML: &str = r#"id: 7005
title: "Thin-justification fixture: second entry's justification is TODO"

outcome: |
  A fixture story whose second acceptance.tests[] entry carries a thin
  justification; the whole story's run must be refused.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/thin-justification-fixture/tests/substantive_entry.rs
      justification: |
        A substantive justification — this entry alone would be fine, but
        the second entry's thinness halts the whole story's run so this
        file must NOT be scaffolded either.
    - file: crates/thin-justification-fixture/tests/thin_entry.rs
      justification: "TODO"
  uat: |
    Drive `TestBuilder::run` against this fixture; observe
    TestBuilderError::ThinJustification naming entry index 1, and zero
    side effects on disk.

guidance: |
  Fixture authored inline for the thin-justification refusal scaffold.
  Not a real story.

depends_on: []
"#;

#[test]
fn rejects_thin_justification_returns_typed_error_and_writes_zero_scaffolds_for_the_whole_story() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    let fixture_root = repo_root.join("crates/thin-justification-fixture");
    fs::create_dir_all(fixture_root.join("src")).expect("fixture src");
    fs::create_dir_all(fixture_root.join("tests")).expect("fixture tests");
    fs::write(
        fixture_root.join("Cargo.toml"),
        r#"[package]
name = "thin-justification-fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write fixture Cargo.toml");
    fs::write(fixture_root.join("src/lib.rs"), b"").expect("write fixture lib.rs");

    init_repo_and_commit_seed(repo_root);

    let substantive_path = fixture_root.join("tests/substantive_entry.rs");
    let thin_path = fixture_root.join("tests/thin_entry.rs");
    let evidence_dir = repo_root.join("evidence/runs").join(STORY_ID.to_string());

    let builder = TestBuilder::new(repo_root);
    let err = builder
        .run(STORY_ID)
        .expect_err("thin justification must surface as Err, not Ok");

    match &err {
        TestBuilderError::ThinJustification { index } => {
            assert_eq!(
                *index, 1,
                "the offending entry is the second entry (zero-based index 1); got {index}"
            );
        }
        other => panic!(
            "thin justification must surface as TestBuilderError::ThinJustification {{ index }}; got {other:?}"
        ),
    }

    // Neither file was scaffolded — refusal is total, not per-entry.
    assert!(
        !substantive_path.exists(),
        "thin-justification refusal must write zero scaffolds — even for substantive siblings"
    );
    assert!(!thin_path.exists(), "thin entry was not scaffolded");

    // No evidence written.
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
            "thin-justification refusal must write zero evidence files"
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
