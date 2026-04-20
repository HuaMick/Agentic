//! Story 14 acceptance test: a `claude` subprocess exceeding budget fails closed.
//!
//! Justification (from stories/14.yml): Proves the fail-closed
//! contract on a `claude` subprocess that exceeds the per-scaffold
//! wall-clock budget (documented in guidance; default 120 seconds).
//! The subprocess is SIGTERMed, `TestBuilder::run` returns
//! `TestBuilderError::ClaudeTimeout` naming the offending
//! justification index, writes zero scaffolds for the story, writes
//! zero evidence, and leaves the tree in its pre-run state. The
//! already-elapsed scaffolds for sibling justifications in the same
//! story are rolled back (any file written before the timeout is
//! deleted). Without this, a hung `claude` would either block the
//! CI runner indefinitely or leave a partially-scaffolded story
//! whose evidence row disagrees with the files on disk.
//!
//! The scaffold installs a `claude` shim that `sleep`s far longer
//! than the configured budget for the second sibling entry but
//! responds promptly for the first. We set
//! `AGENTIC_TEST_BUILD_CLAUDE_TIMEOUT=200ms` so the second spawn is
//! guaranteed to exceed the budget. `TestBuilder::run` must return
//! `TestBuilderError::ClaudeTimeout { index: 1 }`, roll back the
//! first sibling that was already written, and leave no evidence on
//! disk. Red today is compile-red via the missing
//! `TestBuilderError::ClaudeTimeout` variant and the missing
//! `AGENTIC_TEST_BUILD_CLAUDE_TIMEOUT` env-var handling.

use std::fs;
use std::path::Path;

use agentic_test_builder::{TestBuilder, TestBuilderError};
use tempfile::TempDir;

const STORY_ID: u32 = 14007;

const FIXTURE_STORY_YAML: &str = r#"id: 14007
title: "Claude-timeout fixture: second sibling exceeds budget"

outcome: |
  A fixture whose second claude spawn exceeds the wall-clock budget;
  test-builder must roll back the first (already-written) scaffold
  and return TestBuilderError::ClaudeTimeout naming index 1.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/timeout-fixture/tests/first_fast_sibling.rs
      justification: |
        Substantive justification for the first sibling; its claude
        spawn responds promptly and the scaffold is written. On the
        timeout of the second sibling, test-builder must DELETE this
        file (roll-back), because story 14 names the all-or-nothing
        atomicity contract.
    - file: crates/timeout-fixture/tests/second_slow_sibling.rs
      justification: |
        Substantive justification for the second sibling; its claude
        spawn sleeps past the AGENTIC_TEST_BUILD_CLAUDE_TIMEOUT
        budget. TestBuilder::run must return
        TestBuilderError::ClaudeTimeout { index: 1 }.
  uat: |
    Set a tiny timeout; run; observe ClaudeTimeout naming index 1,
    the fast-sibling scaffold removed, no evidence.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const FAST_STDOUT: &str = r#"//! Fast-sibling scaffold.
use timeout_fixture::noop;

#[test]
fn fast_sibling() { noop(); }
"#;

#[test]
fn claude_timeout_is_fail_closed_rolls_back_earlier_scaffolds_and_returns_typed_index() {
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

    // Dispatching shim: first invocation emits FAST_STDOUT
    // immediately; second invocation sleeps 10 seconds (far past
    // our 200ms budget) so the timeout fires deterministically.
    let path_override = install_fast_then_slow_shim(repo_root, FAST_STDOUT, 10);
    std::env::set_var("PATH", &path_override);
    std::env::set_var("AGENTIC_CACHE", repo_root.join(".agentic-cache"));
    // Tiny budget so the second spawn is GUARANTEED to exceed it.
    std::env::set_var("AGENTIC_TEST_BUILD_CLAUDE_TIMEOUT", "200ms");

    init_repo_and_commit_seed(repo_root);

    let first_path = repo_root.join("crates/timeout-fixture/tests/first_fast_sibling.rs");
    let second_path = repo_root.join("crates/timeout-fixture/tests/second_slow_sibling.rs");
    let evidence_dir = repo_root.join("evidence/runs").join(STORY_ID.to_string());

    let builder = TestBuilder::new(repo_root);
    let err = builder
        .run(STORY_ID)
        .expect_err("timeout must surface as Err, not Ok");

    match &err {
        TestBuilderError::ClaudeTimeout { index } => {
            assert_eq!(
                *index, 1,
                "ClaudeTimeout.index must name the offending (second) justification; got {index}"
            );
        }
        other => panic!(
            "timeout must surface as TestBuilderError::ClaudeTimeout {{ index }}; got {other:?}"
        ),
    }

    // Roll-back: BOTH scaffolds absent on disk — the first sibling
    // that was successfully written before the timeout fired must be
    // deleted to preserve story-atomicity.
    assert!(
        !first_path.exists(),
        "first (fast) sibling must be rolled back on timeout; found {}",
        first_path.display()
    );
    assert!(
        !second_path.exists(),
        "second (slow) sibling must not be on disk"
    );

    // Zero evidence.
    if evidence_dir.exists() {
        let any_jsonl = fs::read_dir(&evidence_dir)
            .expect("read evidence dir")
            .filter_map(|e| e.ok())
            .any(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"));
        assert!(
            !any_jsonl,
            "ClaudeTimeout must write zero evidence"
        );
    }
}

fn materialise_fixture_crate(repo_root: &Path) {
    let crate_root = repo_root.join("crates/timeout-fixture");
    fs::create_dir_all(crate_root.join("src")).expect("fixture src dir");
    fs::create_dir_all(crate_root.join("tests")).expect("fixture tests dir");
    fs::write(
        crate_root.join("Cargo.toml"),
        r#"[package]
name = "timeout-fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write fixture Cargo.toml");
    fs::write(crate_root.join("src/lib.rs"), "pub fn noop() {}\n")
        .expect("write fixture lib.rs");
}

/// Install a shim whose first invocation emits `fast_stdout`
/// promptly and whose second invocation sleeps `slow_seconds`
/// seconds before emitting anything — exceeding any small timeout.
fn install_fast_then_slow_shim(
    repo_root: &Path,
    fast_stdout: &str,
    slow_seconds: u32,
) -> String {
    let shim_dir = repo_root.join(".bin");
    fs::create_dir_all(&shim_dir).expect("shim dir");
    let counter_path = shim_dir.join("counter");
    fs::write(&counter_path, "0").expect("init counter");
    let shim_path = shim_dir.join("claude");
    let script = format!(
        "#!/bin/sh\nCOUNTER_PATH='{counter}'\nN=$(cat \"$COUNTER_PATH\")\nN_NEXT=$((N + 1))\necho \"$N_NEXT\" > \"$COUNTER_PATH\"\nif [ \"$N\" = \"0\" ]; then\ncat <<'__AGENTIC_EOF__'\n{fast}__AGENTIC_EOF__\nelse\nsleep {slow}\nfi\n",
        counter = counter_path.display(),
        fast = fast_stdout,
        slow = slow_seconds
    );
    fs::write(&shim_path, script).expect("write shim");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&shim_path).expect("shim metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&shim_path, perms).expect("chmod shim");
    }
    let old_path = std::env::var("PATH").unwrap_or_default();
    format!("{}:{}", shim_dir.display(), old_path)
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
