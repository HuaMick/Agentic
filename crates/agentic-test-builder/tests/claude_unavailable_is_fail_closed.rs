//! Story 14 acceptance test: a missing `claude` on PATH fails closed.
//!
//! Justification (from stories/14.yml): Proves the fail-closed
//! contract on an unavailable `claude` binary: given a fixture where
//! `claude` is not on `PATH` (or the wrapper exit-codes in a way
//! that indicates auth failure — no `~/.claude/.credentials.json`,
//! or the subprocess exits non-zero before emitting output),
//! `TestBuilder::run` returns `TestBuilderError::ClaudeUnavailable`
//! with a typed reason, writes zero scaffolds (even for cache hits
//! that could have been served — one fail-closed axis governs the
//! whole run), writes zero evidence, and leaves the tree in its
//! pre-run state. Without this, a CI runner without `claude` auth
//! would silently skip real scaffolding and produce evidence rows
//! claiming red against empty or partial files.
//!
//! The scaffold sets `PATH` to a tempdir that CONTAINS NO `claude`
//! executable (the directory is intentionally empty). Any child
//! process that tries to spawn `claude` then fails with
//! ENOENT-equivalent. `TestBuilder::run` must surface that as
//! `TestBuilderError::ClaudeUnavailable` — NOT as
//! `TestBuilderError::Other(...)`. Red today is compile-red via the
//! missing `TestBuilderError::ClaudeUnavailable` variant.

use std::fs;
use std::path::Path;

use agentic_test_builder::{TestBuilder, TestBuilderError};
use tempfile::TempDir;

const STORY_ID: u32 = 14006;

const FIXTURE_STORY_YAML: &str = r#"id: 14006
title: "Claude-unavailable fixture: PATH has no claude binary"

outcome: |
  A fixture that exercises the ClaudeUnavailable refusal when
  `claude` is not on PATH.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/unavailable-fixture/tests/would_be_scaffolded.rs
      justification: |
        A substantive justification; the only reason this scaffold is
        not written is that claude is unavailable on PATH, and
        test-builder must fail closed with ClaudeUnavailable.
  uat: |
    Set PATH to an empty tempdir; run test-builder; observe
    ClaudeUnavailable and zero side effects.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

#[test]
fn claude_unavailable_is_fail_closed_returns_typed_error_and_writes_zero_side_effects() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{STORY_ID}.yml")),
        FIXTURE_STORY_YAML,
    )
    .expect("write fixture");

    materialise_fixture_crate(repo_root);

    // PATH is set to an EMPTY directory so any `claude` spawn fails
    // with ENOENT. We deliberately do NOT install a shim here — the
    // fail-closed contract fires precisely because no `claude` is
    // resolvable.
    let empty_bin = repo_root.join(".empty-bin");
    fs::create_dir_all(&empty_bin).expect("empty bin dir");
    std::env::set_var("PATH", &empty_bin);
    std::env::set_var("AGENTIC_CACHE", repo_root.join(".agentic-cache"));

    init_repo_and_commit_seed(repo_root);

    let scaffold_path = repo_root
        .join("crates/unavailable-fixture/tests/would_be_scaffolded.rs");
    let evidence_dir = repo_root.join("evidence/runs").join(STORY_ID.to_string());

    let builder = TestBuilder::new(repo_root);
    let err = builder
        .run(STORY_ID)
        .expect_err("unavailable claude must surface as Err, not Ok");

    assert!(
        matches!(err, TestBuilderError::ClaudeUnavailable),
        "missing claude on PATH must surface as TestBuilderError::ClaudeUnavailable; \
         got {err:?} (a plain Other(..) is not typed enough)"
    );

    // Zero scaffolds — fail-closed governs the whole story.
    assert!(
        !scaffold_path.exists(),
        "ClaudeUnavailable must write zero scaffolds; found {}",
        scaffold_path.display()
    );

    // Zero evidence.
    if evidence_dir.exists() {
        let any_jsonl = fs::read_dir(&evidence_dir)
            .expect("read evidence dir")
            .filter_map(|e| e.ok())
            .any(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"));
        assert!(
            !any_jsonl,
            "ClaudeUnavailable must write zero evidence files"
        );
    }
}

fn materialise_fixture_crate(repo_root: &Path) {
    let crate_root = repo_root.join("crates/unavailable-fixture");
    fs::create_dir_all(crate_root.join("src")).expect("fixture src dir");
    fs::create_dir_all(crate_root.join("tests")).expect("fixture tests dir");
    fs::write(
        crate_root.join("Cargo.toml"),
        r#"[package]
name = "unavailable-fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write fixture Cargo.toml");
    fs::write(crate_root.join("src/lib.rs"), b"").expect("write fixture lib.rs");
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
