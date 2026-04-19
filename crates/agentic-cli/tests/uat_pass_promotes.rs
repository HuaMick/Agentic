//! Story 1 acceptance test: Pass-and-promote end-to-end through the binary.
//!
//! Justification (from stories/1.yml): proves the happy path end-to-end
//! through the binary: `agentic uat <id> --verdict pass` on a clean
//! fixture repo with a valid story returns exit 0, writes exactly one
//! row to the configured store's `uat_signings` table with
//! `verdict=pass` and the fixture HEAD SHA, and rewrites the fixture's
//! `stories/<id>.yml` to `status: healthy`. Without this we cannot
//! claim the CLI is a real path to `healthy` — the library-level
//! scaffolds prove the library does it, but the binary could still
//! fail to construct `Uat` correctly or drop the promotion on the floor.
//!
//! The scaffold builds a fresh fixture repo + stories dir + SurrealStore
//! tempdir, invokes the compiled `agentic` binary, then re-opens the
//! SurrealStore from the SAME tempdir path (proving the binary wrote
//! to the configured location) and asserts on the row shape. The
//! YAML on disk is re-read and checked for `status: healthy`.

use std::fs;
use std::path::Path;

use agentic_store::{Store, SurrealStore};
use assert_cmd::Command;
use tempfile::TempDir;

const STORY_ID: u32 = 88802;

const FIXTURE_YAML: &str = r#"id: 88802
title: "Fixture story for story 1 CLI pass-and-promote"

outcome: |
  A fixture that the CLI uat subcommand promotes to healthy when
  --verdict pass is passed.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/uat_pass_promotes.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the agentic binary against this file.
  uat: |
    Run `agentic uat <id> --verdict pass`; assert promotion.

guidance: |
  Fixture authored inline for the story-1 pass-and-promote scaffold.
  Not a real story.

depends_on: []
"#;

#[test]
fn agentic_uat_verdict_pass_exits_zero_writes_signing_row_and_promotes_yaml_to_healthy() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    let head_sha = init_repo_and_commit_seed(repo_root);

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

    assert_eq!(
        status.code(),
        Some(0),
        "`agentic uat <id> --verdict pass` on a clean tree with a valid \
         fixture must exit 0; got status={status:?}\nstdout:\n{stdout}\n\
         stderr:\n{stderr}"
    );

    // Re-open the SurrealStore at the same tempdir path — this is
    // the read-side proof the binary actually wrote to the store the
    // user configured, not to some default location.
    let store = SurrealStore::open(&store_path)
        .expect("re-opening the configured SurrealStore must succeed");
    let rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64)
        })
        .expect("uat_signings query must succeed");

    assert_eq!(
        rows.len(),
        1,
        "Pass through the binary must write exactly one uat_signings row \
         for story {STORY_ID}; got {} rows: {rows:?}",
        rows.len()
    );

    let row = &rows[0];
    assert_eq!(
        row.get("verdict").and_then(|v| v.as_str()),
        Some("pass"),
        "signing row must carry verdict=\"pass\"; got row={row}"
    );
    let commit = row
        .get("commit")
        .and_then(|v| v.as_str())
        .expect("signing row must carry a string `commit` field");
    assert_eq!(
        commit, head_sha,
        "signing row must carry the fixture HEAD SHA {head_sha:?}; got {commit:?}"
    );
    assert_eq!(
        commit.len(),
        40,
        "signing row must carry a full 40-char SHA; got {commit:?}"
    );

    // The fixture YAML on disk must now say status: healthy.
    let rewritten = fs::read_to_string(&story_path).expect("re-read fixture");
    assert!(
        rewritten.contains("status: healthy"),
        "Pass through the binary must rewrite status to `healthy`; \
         got fixture body:\n{rewritten}"
    );
    assert!(
        !rewritten.contains("status: under_construction"),
        "Pass through the binary must replace the prior status, not append; \
         got fixture body:\n{rewritten}"
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
