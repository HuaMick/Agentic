//! Story 20 acceptance test: when the sandbox's `start_sha` no longer
//! equals main's current HEAD at merge time, `merge_run_if_green`
//! returns `StoryBuildError::StartShaDrift` naming both shas,
//! performs NO merge (main is byte-identical), and updates the run
//! row with `merged: false` and a top-level `merge_error`.
//!
//! Justification (from stories/20.yml acceptance.tests[8]):
//!   Proves the drift-refusal contract at the merge step:
//!   given a completed sandbox run whose `branch_state.
//!   start_sha` no longer equals the current HEAD of `main`,
//!   `StoryBuild::merge_run_if_green` returns
//!   `StoryBuildError::StartShaDrift` naming both shas,
//!   performs NO merge (main's HEAD unchanged), updates the
//!   run row so `branch_state.merged == false` and adds an
//!   error note to the row's top-level `merge_error` field,
//!   and exits the CLI with code 2 (could-not-verdict). The
//!   sandbox's branch state and trace remain intact on disk
//!   for later manual inspection.
//!
//! Red today: compile-red via the missing `agentic_story_build`
//! public surface (`StoryBuild::merge_run_if_green`,
//! `StoryBuildError::StartShaDrift`).

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_story_build::{StoryBuild, StoryBuildError};
use serde_json::json;
use tempfile::TempDir;

#[test]
fn merge_refuses_typed_when_start_sha_has_drifted_and_main_is_untouched() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_path = repo_tmp.path();

    let repo = init_repo(repo_path, "dev@example.com", "Dev Op");
    // Baseline commit.
    let _baseline = commit_file(&repo, "README.md", b"baseline\n", "seed main");

    // The sandbox ran against a historical start_sha. We build a
    // branch off that start_sha, then advance `main` to simulate a
    // concurrent edit.
    let run_branch = "run/7104-drift";
    // Record the sha the sandbox claims as its `start_sha` — this is
    // the BASELINE sha.
    let sandbox_start_sha = head_sha(&repo);

    checkout_new_branch(&repo, run_branch, &sandbox_start_sha);
    let end_sha = commit_file(&repo, "src/sandbox.rs", b"// sandbox\n", "iter 1");

    // Now simulate a concurrent edit to main.
    checkout_branch(&repo, "main");
    let concurrent_sha = commit_file(
        &repo,
        "notes.md",
        b"concurrent edit\n",
        "concurrent edit on main",
    );

    // Main's HEAD is no longer the sandbox's start_sha.
    assert_ne!(head_sha(&repo), sandbox_start_sha);

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let run_id = "run-7104-drift".to_string();
    store
        .upsert(
            "runs",
            &run_id,
            json!({
                "run_id": &run_id,
                "story_id": 7104,
                "outcome": "green",
                "signer": "sandbox:claude-sonnet-4-6@run-7104-drift",
                "branch_state": {
                    "branch": run_branch,
                    "start_sha": &sandbox_start_sha,
                    "end_sha": &end_sha,
                    "commits": ["iter 1"],
                    "merged": false,
                }
            }),
        )
        .expect("seed drifted runs row");

    // The merge must refuse typed. Main must not move.
    let main_tip_before = head_sha(&repo);
    let err = StoryBuild::merge_run_if_green(
        repo_path,
        Arc::clone(&store),
        &run_id,
        "fixture-drift-refusal",
    )
    .expect_err("merge_run_if_green must refuse typed on start-sha drift");

    match &err {
        StoryBuildError::StartShaDrift {
            expected_start_sha,
            actual_main_sha,
        } => {
            assert_eq!(
                expected_start_sha, &sandbox_start_sha,
                "StartShaDrift must name the sandbox's recorded start_sha; got {expected_start_sha:?}"
            );
            assert_eq!(
                actual_main_sha, &concurrent_sha,
                "StartShaDrift must name main's current HEAD; got {actual_main_sha:?}"
            );
        }
        other => panic!(
            "merge_run_if_green must return StoryBuildError::StartShaDrift; got {other:?}"
        ),
    }

    // Main must be byte-identical to its pre-invocation state.
    let main_tip_after = head_sha(&repo);
    assert_eq!(
        main_tip_before, main_tip_after,
        "main's HEAD must NOT move when the merge refuses; before={main_tip_before}, after={main_tip_after}"
    );

    // Run row updated: merged=false AND a top-level merge_error note.
    let rows = store
        .query("runs", &|doc| doc["run_id"] == json!(&run_id))
        .expect("query");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0]["branch_state"]["merged"],
        json!(false),
        "merged must stay false after drift refusal"
    );
    let merge_err = rows[0]["merge_error"]
        .as_str()
        .expect("merge_error must be written as a top-level string on the runs row");
    assert!(
        merge_err.to_ascii_lowercase().contains("drift")
            || merge_err.to_ascii_lowercase().contains("start"),
        "merge_error must phrase the refusal (containing `drift` or `start`); got {merge_err:?}"
    );
}

fn init_repo(root: &Path, email: &str, name: &str) -> git2::Repository {
    let repo = git2::Repository::init_opts(
        root,
        git2::RepositoryInitOptions::new()
            .initial_head("main")
            .mkdir(false),
    )
    .expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.email", email).expect("user.email");
        cfg.set_str("user.name", name).expect("user.name");
    }
    repo
}

fn commit_file(repo: &git2::Repository, rel: &str, bytes: &[u8], msg: &str) -> String {
    let root = repo.workdir().expect("workdir").to_path_buf();
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).expect("mkparent");
    fs::write(&path, bytes).expect("write file");
    let mut index = repo.index().expect("index");
    index.add_path(Path::new(rel)).expect("add_path");
    index.write().expect("write index");
    let tree_oid = index.write_tree().expect("write_tree");
    let tree = repo.find_tree(tree_oid).expect("find_tree");
    let sig = repo.signature().expect("signature");
    let parent_oid = repo
        .head()
        .ok()
        .and_then(|r| r.target())
        .and_then(|oid| repo.find_commit(oid).ok());
    let parents: Vec<&git2::Commit> = parent_oid.iter().collect();
    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
        .expect("commit");
    oid.to_string()
}

fn checkout_new_branch(repo: &git2::Repository, name: &str, from_sha: &str) {
    let oid = git2::Oid::from_str(from_sha).expect("oid");
    let commit = repo.find_commit(oid).expect("find_commit");
    let _ = repo.branch(name, &commit, false).expect("branch");
    let refname = format!("refs/heads/{name}");
    repo.set_head(&refname).expect("set_head");
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .expect("checkout_head");
}

fn checkout_branch(repo: &git2::Repository, name: &str) {
    let refname = format!("refs/heads/{name}");
    repo.set_head(&refname).expect("set_head");
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .expect("checkout_head");
}

fn head_sha(repo: &git2::Repository) -> String {
    repo.head()
        .expect("head")
        .target()
        .expect("target")
        .to_string()
}
