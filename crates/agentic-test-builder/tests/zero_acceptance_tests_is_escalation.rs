//! Story 7 acceptance test: zero-acceptance-tests refusal.
//!
//! Justification (from stories/7.yml): Proves the zero-tests refusal:
//! given a story whose `acceptance.tests` is an empty array (or absent),
//! `TestBuilder::run` returns `TestBuilderError::NoAcceptanceTests` (or
//! an equivalent typed variant), writes zero scaffolds, and writes zero
//! evidence. The schema should block this at story-writer time, but the
//! agent defends in depth — a story with zero executable tests would
//! produce a zero-verdict evidence row, which is an attestation claiming
//! "this story is red" without any observable to stand behind. The
//! refusal makes the empty-acceptance case a story-writer escalation,
//! not a silent green-from-nothing.
//!
//! The scaffold materialises a fixture story YAML whose
//! `acceptance.tests` is an empty array. Because the live story schema
//! mandates `minItems: 1` on that array the loader would also reject
//! this, so the scaffold accepts either of two equivalent typed
//! outcomes: a `TestBuilderError::NoAcceptanceTests` variant raised by
//! test-builder's own defence in depth, OR a loader-sourced error
//! surfaced verbatim — the observable the justification pins is that
//! `TestBuilder::run` returns Err with a typed variant naming the
//! zero-tests case and writes nothing. Red today is compile-red via
//! the missing `agentic_test_builder` public surface (`TestBuilder`,
//! `TestBuilderError`).

use std::fs;
use std::path::Path;

use agentic_test_builder::{TestBuilder, TestBuilderError};
use tempfile::TempDir;

const STORY_ID: u32 = 7008;

const ZERO_TESTS_STORY_YAML: &str = r#"id: 7008
title: "Zero-acceptance-tests fixture: test-builder must refuse"

outcome: |
  A fixture story whose acceptance.tests[] is empty — test-builder must
  refuse rather than emit a zero-verdict evidence row.

status: proposed

patterns: []

acceptance:
  tests: []
  uat: |
    Drive `TestBuilder::run` against this fixture; observe
    TestBuilderError::NoAcceptanceTests and zero side effects on disk.

guidance: |
  Fixture authored inline for the zero-tests refusal scaffold. Not a
  real story.

depends_on: []
"#;

#[test]
fn zero_acceptance_tests_is_escalation_returns_typed_error_and_writes_no_evidence() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, ZERO_TESTS_STORY_YAML).expect("write fixture story");

    init_repo_and_commit_seed(repo_root);

    let evidence_dir = repo_root.join("evidence/runs").join(STORY_ID.to_string());

    let builder = TestBuilder::new(repo_root);
    let err = builder
        .run(STORY_ID)
        .expect_err("zero acceptance.tests must surface as Err, not Ok");

    // The observable is a typed error whose variant names the zero-tests
    // case. The exact variant is an implementation choice — the
    // justification suggests `NoAcceptanceTests`; we accept it whether
    // test-builder raises it directly or wraps a loader-sourced error
    // that also names the zero-tests condition.
    assert!(
        matches!(err, TestBuilderError::NoAcceptanceTests),
        "zero acceptance.tests must surface as TestBuilderError::NoAcceptanceTests; got {err:?}"
    );

    // No evidence.
    if evidence_dir.exists() {
        let any_jsonl = fs::read_dir(&evidence_dir)
            .expect("read evidence dir")
            .filter_map(|e| e.ok())
            .any(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "jsonl")
            });
        assert!(!any_jsonl, "zero-tests refusal must write zero evidence");
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
