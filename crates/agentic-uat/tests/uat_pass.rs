//! Story 1 acceptance test: the Pass-and-promote happy path.
//!
//! Justification (from stories/1.yml): proves the happy path — given a
//! story whose UAT walkthrough executes successfully (driven by a stub
//! `UatExecutor` that returns Pass) on a clean working tree, `Uat::run`
//! returns a Pass verdict, writes exactly one row to `uat_signings` with
//! verdict=pass and the current commit hash, and rewrites the story YAML
//! so `status: healthy`. Without this the gate cannot promote a story at
//! all — and promotion-to-healthy is the whole point of the command.
//!
//! The scaffold builds a `TempDir` containing a fresh git repo with one
//! seed commit and a `stories/<id>.yml` fixture in `status:
//! under_construction`, constructs the `Uat` gate against a `MemStore`
//! and a `StubExecutor::always_pass()`, invokes `Uat::run(<id>)`, and
//! asserts the three observables named in the justification. Red today
//! is compile-red via the missing `agentic_uat` public surface
//! (`Uat`, `Verdict`, `StubExecutor`, `UatError`).

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_uat::{StubExecutor, Uat, Verdict};
use tempfile::TempDir;

const STORY_ID: u32 = 4242;

const FIXTURE_YAML: &str = r#"id: 4242
title: "A fixture story the stub executor drives to Pass"

outcome: |
  A fixture that is promoted to healthy when the stub executor returns
  Pass on a clean tree.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_pass.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Run the stub executor; it returns Pass; verify status flips.

guidance: |
  Fixture authored inline for the Pass-and-promote scaffold. Not a real
  story.

depends_on: []
"#;

#[test]
fn uat_run_returns_pass_writes_signing_row_and_promotes_story_to_healthy() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    // Initialise a fresh repo and commit the seed so the working tree is
    // clean when Uat::run checks it. The commit SHA of HEAD after this
    // step is what the signing row must carry.
    let head_sha = init_repo_and_commit_seed(repo_root);

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    let verdict = uat.run(STORY_ID).expect("Pass path must not error");
    assert!(
        matches!(verdict, Verdict::Pass),
        "stub-always-pass must yield a Pass verdict; got {verdict:?}"
    );
    assert_eq!(
        verdict.as_str(),
        "pass",
        "Pass verdict must serialise as the lowercase string \"pass\""
    );

    // Exactly one row in `uat_signings`, carrying verdict=pass and the
    // full HEAD SHA at signing time.
    let rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64)
        })
        .expect("store query should succeed");
    assert_eq!(
        rows.len(),
        1,
        "exactly one uat_signings row must be written on Pass; got {} rows: {rows:?}",
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
        "signing row must carry the full HEAD SHA; got {commit:?}, expected {head_sha:?}"
    );
    assert_eq!(
        commit.len(),
        40,
        "signing row must carry a full 40-char SHA; got {commit:?}"
    );
    assert!(
        commit.chars().all(|c| c.is_ascii_hexdigit()),
        "signing row commit must be all hex; got {commit:?}"
    );

    // The story YAML on disk was rewritten to status: healthy.
    let rewritten = fs::read_to_string(&story_path).expect("re-read story file");
    assert!(
        rewritten.contains("status: healthy"),
        "Pass promotion must rewrite status to healthy; got file body:\n{rewritten}"
    );
    assert!(
        !rewritten.contains("status: under_construction"),
        "Pass promotion must replace the prior status, not append; got file body:\n{rewritten}"
    );
}

/// Initialise a git repo rooted at `root`, stage every file currently in
/// the working tree (including the story fixture), commit it, and return
/// the full SHA of HEAD. Uses `git2` directly so the test has no external
/// `git` binary dependency.
fn init_repo_and_commit_seed(root: &Path) -> String {
    let repo = git2::Repository::init(root).expect("git init");

    // Configure a committer identity local to this repo so commits do not
    // depend on the ambient git config.
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
