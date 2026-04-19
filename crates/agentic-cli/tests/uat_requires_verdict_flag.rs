//! Story 8 acceptance test: missing `--verdict` flag exits 2 with no
//! side effects.
//!
//! Justification (from stories/8.yml): proves argv validation exits 2,
//! not 0 or 1: `agentic uat <id>` with no `--verdict` flag returns
//! exit 2 (malformed args), writes no store row, and does not touch
//! the story YAML. Without this, a user typo or scripted invocation
//! missing the flag could surface as a panic or as exit 1 (which CI
//! would treat as a real UAT failure) rather than as the documented
//! could-not-verdict code.
//!
//! The scaffold seeds the fixture the same way as the other uat
//! scaffolds, invokes `agentic uat <id>` with NO `--verdict` flag,
//! and asserts exit 2 EXACTLY, zero store rows, and byte-identical
//! fixture YAML.

use std::fs;
use std::path::Path;

use agentic_store::{Store, SurrealStore};
use assert_cmd::Command;
use tempfile::TempDir;

const STORY_ID: u32 = 88805;

const FIXTURE_YAML: &str = r#"id: 88805
title: "Fixture story for story 8 CLI missing-verdict-flag"

outcome: |
  A fixture used only to exercise the missing-verdict-flag refusal
  contract via the binary.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/uat_requires_verdict_flag.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the agentic binary against this file.
  uat: |
    Invoke `agentic uat <id>` with no verdict flag; assert exit 2.

guidance: |
  Fixture authored inline for the story-8 missing-verdict-flag
  scaffold. Not a real story.

depends_on: []
"#;

#[test]
fn agentic_uat_without_verdict_flag_exits_two_writes_no_rows_and_leaves_yaml_unchanged() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    init_repo_and_commit_seed(repo_root);
    let before_bytes = fs::read(&story_path).expect("read fixture before run");

    let store_tmp = TempDir::new().expect("store tempdir");
    let store_path = store_tmp.path().to_path_buf();

    // Invoke with NO --verdict flag. --store is still supplied so any
    // store side effect would surface in the readback.
    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("uat")
        .arg(STORY_ID.to_string())
        .arg("--store")
        .arg(&store_path)
        .assert();

    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let status = output.status;

    // Exit code 2 EXACTLY: per story 8 guidance's exit-code contract,
    // "unparseable args" / "missing --verdict flag" is a could-not-
    // verdict condition (2), not a real Fail (1) and not success (0).
    assert_eq!(
        status.code(),
        Some(2),
        "`agentic uat <id>` with no --verdict flag must exit 2 \
         (could-not-verdict / malformed args), NOT 0 (success) or 1 \
         (real fail); got status={status:?}\nstdout:\n{stdout}\n\
         stderr:\n{stderr}"
    );

    // No store row may be written — store construction, if it happens
    // at all, must not be followed by an append when args are
    // malformed. `SurrealStore::open` is safe to call on a tempdir
    // that may or may not have been touched.
    let store = SurrealStore::open(&store_path)
        .expect("SurrealStore open on the tempdir must succeed");
    let rows = store
        .query("uat_signings", &|_| true)
        .expect("uat_signings query must succeed");
    assert!(
        rows.is_empty(),
        "missing-verdict-flag refusal must write zero uat_signings rows; \
         got {} rows: {rows:?}",
        rows.len()
    );

    let after_bytes = fs::read(&story_path).expect("read fixture after run");
    assert_eq!(
        after_bytes, before_bytes,
        "missing-verdict-flag refusal must not touch the fixture YAML; \
         file changed on disk"
    );
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
