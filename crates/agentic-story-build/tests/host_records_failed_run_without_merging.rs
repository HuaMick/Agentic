//! Story 20 acceptance test: three sequential sandbox invocations
//! for the same story — two exhausted, one green — produce exactly
//! ONE squash commit on main (from the green run) and THREE rows in
//! the `runs` table. Failed runs never touch main's history; they
//! are fully inspectable via the `runs` table.
//!
//! Justification (from stories/20.yml acceptance.tests[9]):
//!   Proves the amend-same-story semantics: given three
//!   sequential sandbox invocations for the same story id,
//!   the first two returning `inner_loop_exhausted` and the
//!   third returning `green`, the resulting main branch has
//!   exactly ONE squash commit for the story (from the third
//!   run) and the `runs` table has THREE rows. Each failed
//!   run's `branch_state.merged` is `false`; the winning
//!   run's is `true`. `main`'s git log contains zero
//!   references to the failed runs as commits, but the
//!   failed runs are fully inspectable via their `runs`
//!   rows and trace files.
//!
//! Red today: compile-red via the missing `agentic_story_build`
//! public surface (`StoryBuild::merge_run_if_green`,
//! `StoryBuild::record_failed_run`).

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_story_build::StoryBuild;
use serde_json::json;
use tempfile::TempDir;

#[test]
fn three_sequential_runs_two_exhausted_one_green_produces_one_squash_three_rows() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_path = repo_tmp.path();

    let repo = init_repo(repo_path, "dev@example.com", "Dev Op");
    let start_sha = commit_file(&repo, "README.md", b"baseline\n", "seed main");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // First failed run: exhausted. The host must record the run row
    // and NOT touch main.
    let main_tip_before_first = head_sha(&repo);
    store
        .upsert(
            "runs",
            "run-8115-a",
            json!({
                "run_id": "run-8115-a",
                "story_id": 8115,
                "outcome": "inner_loop_exhausted",
                "signer": "sandbox:claude-sonnet-4-6@run-8115-a",
                "branch_state": {
                    "branch": "run/8115-a",
                    "start_sha": start_sha,
                    "end_sha": start_sha,
                    "commits": [],
                    "merged": false,
                }
            }),
        )
        .expect("seed exhausted run a");
    StoryBuild::record_failed_run(repo_path, Arc::clone(&store), "run-8115-a")
        .expect("record_failed_run on exhausted");
    assert_eq!(
        head_sha(&repo),
        main_tip_before_first,
        "main must not move when recording a failed run"
    );

    // Second failed run: exhausted. Same discipline.
    store
        .upsert(
            "runs",
            "run-8115-b",
            json!({
                "run_id": "run-8115-b",
                "story_id": 8115,
                "outcome": "inner_loop_exhausted",
                "signer": "sandbox:claude-sonnet-4-6@run-8115-b",
                "branch_state": {
                    "branch": "run/8115-b",
                    "start_sha": start_sha,
                    "end_sha": start_sha,
                    "commits": [],
                    "merged": false,
                }
            }),
        )
        .expect("seed exhausted run b");
    StoryBuild::record_failed_run(repo_path, Arc::clone(&store), "run-8115-b")
        .expect("record_failed_run on exhausted");
    assert_eq!(
        head_sha(&repo),
        main_tip_before_first,
        "main must still not move after the second failed run"
    );

    // Third run: green. Build a branch off the unchanged start_sha,
    // commit the agent's iteration, then merge.
    let run_branch = "run/8115-c";
    checkout_new_branch(&repo, run_branch, &start_sha);
    let end_sha = commit_file(&repo, "src/green.rs", b"// green\n", "iter 1");
    checkout_branch(&repo, "main");

    store
        .upsert(
            "runs",
            "run-8115-c",
            json!({
                "run_id": "run-8115-c",
                "story_id": 8115,
                "outcome": "green",
                "signer": "sandbox:claude-sonnet-4-6@run-8115-c",
                "branch_state": {
                    "branch": run_branch,
                    "start_sha": start_sha,
                    "end_sha": end_sha,
                    "commits": ["iter 1"],
                    "merged": false,
                }
            }),
        )
        .expect("seed green run c");

    let report = StoryBuild::merge_run_if_green(
        repo_path,
        Arc::clone(&store),
        "run-8115-c",
        "fixture-amend-same-story",
    )
    .expect("merge_run_if_green on the winning run must succeed");
    assert_eq!(
        report.merge_shas.len(),
        1,
        "winning run must produce exactly one squash commit; got {:?}",
        report.merge_shas
    );
    let squash_sha = &report.merge_shas[0];

    // Main's log: baseline + one squash commit for the story.
    let log = git_log_main(&repo);
    assert_eq!(
        log.len(),
        2,
        "main must contain exactly 2 commits (baseline + squash); got {log:?}"
    );
    assert_eq!(
        &log[0], squash_sha,
        "main's tip must be the squash sha; got {:?}",
        log[0]
    );

    // Runs table has three rows for story 8115.
    let all_runs = store
        .query("runs", &|doc| doc["story_id"] == json!(8115))
        .expect("query all runs for story 8115");
    assert_eq!(
        all_runs.len(),
        3,
        "runs table must contain three rows for story 8115; got {all_runs:?}"
    );
    let mut by_id: std::collections::HashMap<String, &serde_json::Value> = all_runs
        .iter()
        .map(|r| (r["run_id"].as_str().unwrap().to_string(), r))
        .collect();
    assert_eq!(
        by_id
            .remove("run-8115-a")
            .expect("run-8115-a row")["branch_state"]["merged"],
        json!(false)
    );
    assert_eq!(
        by_id
            .remove("run-8115-b")
            .expect("run-8115-b row")["branch_state"]["merged"],
        json!(false)
    );
    assert_eq!(
        by_id
            .remove("run-8115-c")
            .expect("run-8115-c row")["branch_state"]["merged"],
        json!(true)
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

fn git_log_main(repo: &git2::Repository) -> Vec<String> {
    let mut walk = repo.revwalk().expect("revwalk");
    walk.push_ref("refs/heads/main").expect("push main");
    walk.map(|oid| oid.expect("oid").to_string()).collect()
}
