//! Story 20 acceptance test: `StoryBuild::merge_run_if_green` applies
//! the sandbox branch's diff to main as a SINGLE squash commit
//! authored by the dev's git identity, whose subject is
//! `story <id>: <title>`, whose body names the run-id, signer,
//! start-sha, and runs-path, and whose sha lands on the run row's
//! `branch_state.merge_shas`.
//!
//! Justification (from stories/20.yml acceptance.tests[7]):
//!   Proves the host-side auto-merge contract on green: given
//!   a completed sandbox run whose `runs` row is
//!   `outcome: green` with `branch_state` carrying
//!   `start_sha == main_tip_at_launch`, `end_sha`, and two
//!   `commits[]` entries, `StoryBuild::merge_run_if_green`
//!   applies the branch's diff to main as a SINGLE squash
//!   commit, whose author identity is `git config
//!   user.email`, whose subject is `story <id>: <title>`,
//!   whose body contains `run-id:`, `signer: sandbox:...`,
//!   `start-sha:`, `runs-path: runs/<run-id>/run.json`.
//!   After the merge, the run row is updated:
//!   `branch_state.merged == true` and
//!   `branch_state.merge_shas` contains exactly one sha
//!   (the squash commit's).
//!
//! Red today: compile-red via the missing `agentic_story_build`
//! public surface (`StoryBuild::merge_run_if_green`, `MergeReport`).

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_story_build::StoryBuild;
use serde_json::json;
use tempfile::TempDir;

#[test]
fn merge_run_if_green_squashes_branch_diff_onto_main_with_documented_commit_body() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_path = repo_tmp.path();

    // Init a git repo with `main` as the only branch. Seed it with
    // a baseline commit.
    let repo = init_repo(repo_path, "dev@example.com", "Dev Op");
    let start_sha = commit_file(&repo, "README.md", b"baseline\n", "seed main");

    // Cut a `run/4081-abcd` branch off `main` and make two commits
    // on it — these are the agent's iteration commits.
    let run_branch = "run/4081-abcd";
    checkout_new_branch(&repo, run_branch, &start_sha);
    let _c1 = commit_file(&repo, "src/one.rs", b"// one\n", "iter 1");
    let end_sha = commit_file(&repo, "src/two.rs", b"// two\n", "iter 2");

    // Switch back to main so the merge target is unambiguous.
    checkout_branch(&repo, "main");

    // A completed runs row: outcome green, branch_state populated.
    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let run_id = "run-4081-green".to_string();
    let signer = "sandbox:claude-sonnet-4-6@run-4081-green";
    store
        .upsert(
            "runs",
            &run_id,
            json!({
                "run_id": &run_id,
                "story_id": 4081,
                "outcome": "green",
                "signer": signer,
                "branch_state": {
                    "branch": run_branch,
                    "start_sha": start_sha,
                    "end_sha": end_sha,
                    "commits": ["iter 1", "iter 2"],
                    "merged": false,
                }
            }),
        )
        .expect("seed runs row");

    // Invoke the merge step. Story title is passed in from the
    // host's story-loader; the test provides a fixture title.
    let report = StoryBuild::merge_run_if_green(
        repo_path,
        Arc::clone(&store),
        &run_id,
        "fixture-green-auto-merge",
    )
    .expect("merge_run_if_green on a green run against a matching main tip must succeed");

    // `report` carries the squash commit sha. Exactly one sha.
    assert_eq!(
        report.merge_shas.len(),
        1,
        "merge must produce exactly one squash commit; got {:?}",
        report.merge_shas
    );
    let squash_sha = &report.merge_shas[0];

    // Verify on the repo side: main has advanced by exactly one
    // commit, whose tree contains both `src/one.rs` and
    // `src/two.rs` (proof it is a squash, not a cherry-pick).
    let head_sha = head_sha(&repo);
    assert_eq!(
        &head_sha, squash_sha,
        "main's HEAD must equal the reported squash sha; got HEAD={head_sha}, reported={squash_sha}"
    );
    assert!(
        repo_path.join("src/one.rs").exists() && repo_path.join("src/two.rs").exists(),
        "squash commit must include both iteration files in the tree"
    );

    // Subject line: `story <id>: <title>`.
    let (subject, body) = commit_message(&repo, &head_sha);
    assert_eq!(
        subject, "story 4081: fixture-green-auto-merge",
        "squash commit subject must be `story <id>: <title>`; got {subject:?}"
    );

    // Body contains the four documented fields.
    assert!(
        body.contains(&format!("run-id: {run_id}")),
        "commit body must name the run-id; got {body:?}"
    );
    assert!(
        body.contains(&format!("signer: {signer}")),
        "commit body must name the sandbox signer; got {body:?}"
    );
    assert!(
        body.contains(&format!("start-sha: {start_sha}")),
        "commit body must name the start-sha; got {body:?}"
    );
    assert!(
        body.contains(&format!("runs-path: runs/{run_id}/run.json")),
        "commit body must carry the runs-path pointer; got {body:?}"
    );

    // Author identity matches `git config user.email`.
    let author_email = commit_author_email(&repo, &head_sha);
    assert_eq!(
        author_email, "dev@example.com",
        "squash commit author email must be the dev's git identity; got {author_email:?}"
    );

    // Run row updated: merged=true, merge_shas=[<squash_sha>].
    let rows = store
        .query("runs", &|doc| doc["run_id"] == json!(&run_id))
        .expect("query runs after merge");
    assert_eq!(rows.len(), 1, "upsert preserves one row; got {rows:?}");
    assert_eq!(
        rows[0]["branch_state"]["merged"],
        json!(true),
        "runs.branch_state.merged must be flipped to true after the merge lands"
    );
    let shas = rows[0]["branch_state"]["merge_shas"]
        .as_array()
        .expect("merge_shas must be an array");
    assert_eq!(shas.len(), 1, "merge_shas must contain exactly one sha; got {shas:?}");
    assert_eq!(shas[0], json!(squash_sha));
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
        cfg.set_str("user.email", email).expect("set user.email");
        cfg.set_str("user.name", name).expect("set user.name");
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

fn commit_message(repo: &git2::Repository, sha: &str) -> (String, String) {
    let oid = git2::Oid::from_str(sha).expect("oid");
    let c = repo.find_commit(oid).expect("find commit");
    let full = c.message().unwrap_or("").to_string();
    let (subj, body) = match full.split_once('\n') {
        Some((s, rest)) => (s.to_string(), rest.trim_start_matches('\n').to_string()),
        None => (full, String::new()),
    };
    (subj, body)
}

fn commit_author_email(repo: &git2::Repository, sha: &str) -> String {
    let oid = git2::Oid::from_str(sha).expect("oid");
    let c = repo.find_commit(oid).expect("find commit");
    c.author().email().unwrap_or("").to_string()
}
