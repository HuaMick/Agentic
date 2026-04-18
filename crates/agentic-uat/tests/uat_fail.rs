//! Story 1 acceptance test: the Fail-without-promotion path.
//!
//! Justification (from stories/1.yml): proves the failure path — given a
//! stub `UatExecutor` that returns Fail, `Uat::run` returns a Fail
//! verdict, writes a `uat_signings` row with verdict=fail and the
//! current commit, and does NOT mutate the story YAML's `status` field.
//! Without this a Fail could either silently promote (catastrophic) or
//! leave no audit trail (we'd lose the negative evidence the dashboard
//! needs to compute "fell from grace").
//!
//! The scaffold builds a clean-tree fixture identical to the Pass
//! scaffold but with `StubExecutor::always_fail()`, captures the story
//! YAML's bytes before `Uat::run`, invokes it, and asserts: a Fail
//! verdict is returned, one `uat_signings` row lands with verdict=fail
//! and the HEAD SHA, and the story file on disk is byte-for-byte
//! unchanged. Red today is compile-red via the missing `agentic_uat`
//! public surface.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_uat::{StubExecutor, Uat, Verdict};
use tempfile::TempDir;

const STORY_ID: u32 = 4243;

const FIXTURE_YAML: &str = r#"id: 4243
title: "A fixture story the stub executor drives to Fail"

outcome: |
  A fixture whose UAT the stub executor fails; its status must not
  change on disk.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_fail.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Run the stub executor; it returns Fail; verify status is unchanged.

guidance: |
  Fixture authored inline for the Fail-without-promotion scaffold. Not
  a real story.

depends_on: []
"#;

#[test]
fn uat_run_returns_fail_writes_signing_row_and_leaves_story_status_unchanged() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    let head_sha = init_repo_and_commit_seed(repo_root);
    let before_bytes = fs::read(&story_path).expect("read fixture before run");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let executor = StubExecutor::always_fail();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    let verdict = uat
        .run(STORY_ID)
        .expect("Fail is a verdict, not an error; run() must return Ok(Fail)");
    assert!(
        matches!(verdict, Verdict::Fail),
        "stub-always-fail must yield a Fail verdict; got {verdict:?}"
    );
    assert_eq!(
        verdict.as_str(),
        "fail",
        "Fail verdict must serialise as the lowercase string \"fail\""
    );

    // Exactly one signing row: verdict=fail, commit=HEAD SHA.
    let rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64)
        })
        .expect("store query should succeed");
    assert_eq!(
        rows.len(),
        1,
        "exactly one uat_signings row must be written on Fail; got {} rows: {rows:?}",
        rows.len()
    );
    let row = &rows[0];
    assert_eq!(
        row.get("verdict").and_then(|v| v.as_str()),
        Some("fail"),
        "signing row must carry verdict=\"fail\"; got row={row}"
    );
    let commit = row
        .get("commit")
        .and_then(|v| v.as_str())
        .expect("signing row must carry a string `commit` field");
    assert_eq!(
        commit, head_sha,
        "signing row must carry the full HEAD SHA; got {commit:?}, expected {head_sha:?}"
    );

    // The story YAML on disk is byte-for-byte unchanged. Fail must not
    // rewrite the file, even to the same content — the dashboard's
    // "fell from grace" computation needs the prior status preserved.
    let after_bytes = fs::read(&story_path).expect("read fixture after run");
    assert_eq!(
        after_bytes, before_bytes,
        "Fail must not mutate the story YAML; file changed on disk"
    );
}

/// See uat_pass.rs for rationale. Duplicated here rather than hoisted to
/// a `tests/common/mod.rs` so each scaffold is independently readable.
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
