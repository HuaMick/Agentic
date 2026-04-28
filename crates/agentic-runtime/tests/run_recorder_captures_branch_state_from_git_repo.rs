//! Story 16 acceptance test: `branch_state` on the row is populated
//! from real git observation, not caller-supplied fiction.
//!
//! Justification (from stories/16.yml acceptance.tests[7]):
//!   Proves the `branch_state` sub-object is populated from real git
//!   observation, not caller-supplied fiction: given a tempdir git
//!   repo with a baseline commit on `main`, a recorder
//!   `start_branch(repo_path, "run/16-abc123")` that cuts a branch
//!   from HEAD, two commits made on that branch, and a
//!   `finish_branch(merged=false)` call, the `runs` row's
//!   `branch_state` has `start_sha` equal to the pre-branch HEAD
//!   SHA, `end_sha` equal to the second commit's SHA, `commits` as
//!   a two-element array each carrying `{sha, author, subject}`,
//!   `merged: false`, and `merge_shas: []`.
//!
//! Red today: natural. `RunRecorder::start_branch` and
//! `finish_branch` do not yet exist; `cargo check` fails on the
//! method-call lines when resolution of `RunRecorder` itself fails.

use agentic_runtime::{Outcome, RunRecorder, RunRecorderConfig};
use agentic_store::{MemStore, Store};
use git2::{IndexAddOption, Repository, Signature};
use serde_json::json;
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn branch_state_start_sha_end_sha_commits_come_from_real_git_observation() {
    // Repo: baseline commit on main.
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let repo = Repository::init(repo_root).expect("git init");
    let baseline_sha = {
        fs::write(repo_root.join("baseline.txt"), b"baseline\n").expect("write baseline");
        commit_all(&repo, "baseline")
    };

    // Runs root + store.
    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let run_id = "eeee5555-ffff-4000-8111-222233334444".to_string();
    let cfg = RunRecorderConfig {
        store: Arc::clone(&store),
        runs_root: runs_root_tmp.path().to_path_buf(),
        run_id: run_id.clone(),
        story_id: 15,
        story_yaml_bytes: b"id: 15\n".to_vec(),
        signer: "sandbox:stub@run-branch".to_string(),
        build_config: json!({}),
    };

    let recorder = RunRecorder::start(cfg).expect("start should succeed");

    // Cut the branch from the baseline HEAD.
    recorder
        .start_branch(repo_root, "run/16-abc123")
        .expect("start_branch should succeed from a clean HEAD");

    // Two commits on the sandbox branch.
    fs::write(repo_root.join("work-1.txt"), b"w1\n").expect("write w1");
    let commit_1_sha = commit_all(&repo, "work: first change");
    fs::write(repo_root.join("work-2.txt"), b"w2\n").expect("write w2");
    let commit_2_sha = commit_all(&repo, "work: second change");

    recorder
        .finish_branch(false)
        .expect("finish_branch(merged=false) should succeed");

    recorder
        .finish(Outcome::Green {
            signing_run_id: "stub-signing-1".to_string(),
        })
        .expect("finish should succeed");

    // Read the row back and cross-check against real git.
    let rows = store
        .query("runs", &|doc| doc["run_id"] == json!(run_id))
        .expect("query");
    assert_eq!(rows.len(), 1, "one row; got {rows:?}");
    let row = &rows[0];
    let bs = &row["branch_state"];

    assert_eq!(
        bs["start_sha"],
        json!(baseline_sha),
        "branch_state.start_sha must equal the pre-branch HEAD SHA"
    );
    assert_eq!(
        bs["end_sha"],
        json!(commit_2_sha),
        "branch_state.end_sha must equal the second commit's SHA"
    );

    let commits = bs["commits"]
        .as_array()
        .expect("branch_state.commits must be a JSON array");
    assert_eq!(
        commits.len(),
        2,
        "two sandbox commits must appear; got {commits:?}"
    );
    for (i, expected_sha) in [&commit_1_sha, &commit_2_sha].iter().enumerate() {
        let sha_field = commits[i]["sha"].as_str().unwrap_or_default();
        let author_field = commits[i]["author"].as_str().unwrap_or_default();
        let subject_field = commits[i]["subject"].as_str().unwrap_or_default();
        assert_eq!(
            sha_field,
            expected_sha.as_str(),
            "commits[{i}].sha mismatch"
        );
        assert!(
            !author_field.trim().is_empty(),
            "commits[{i}].author must be non-empty; got {author_field:?}"
        );
        assert!(
            !subject_field.trim().is_empty(),
            "commits[{i}].subject must be non-empty; got {subject_field:?}"
        );
    }

    assert_eq!(bs["merged"], json!(false));
    assert_eq!(bs["merge_shas"], json!([]));
}

/// Helper: stage everything and commit with message `msg`; return the
/// new HEAD SHA as a 40-char lowercase hex string.
fn commit_all(repo: &Repository, msg: &str) -> String {
    let mut index = repo.index().expect("index");
    index
        .add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
        .expect("add_all");
    index.write().expect("write index");
    let tree_oid = index.write_tree().expect("write_tree");
    let tree = repo.find_tree(tree_oid).expect("find tree");
    let sig = Signature::now("test-builder", "test@agentic.local").expect("sig");

    let parent = repo
        .head()
        .ok()
        .and_then(|h| h.target())
        .and_then(|oid| repo.find_commit(oid).ok());
    let parents: Vec<&git2::Commit> = parent.iter().collect();

    let new_oid = repo
        .commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
        .expect("commit");
    new_oid.to_string()
}
