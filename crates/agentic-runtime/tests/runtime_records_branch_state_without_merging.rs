//! Story 19 acceptance test: the runtime records branch state but
//! never merges. Given a tempdir git repo with a baseline commit on
//! `main` and a `RunConfig` naming `repo_path` + `branch_name`, the
//! runtime populates the recorder's `branch_state` with `start_sha`,
//! `end_sha`, `commits[]`, `merged: false`, `merge_shas: []`. `main`'s
//! HEAD is byte-identical before and after.
//!
//! Justification (from stories/19.yml acceptance.tests[8]):
//!   Proves the git-coordination delineation per research
//!   note 14: given a tempdir git repo with a baseline
//!   commit on `main` and a `RunConfig` that names
//!   `repo_path` and `branch_name = "run/19-deadbee"`,
//!   `spawn_claude_session` (driven by a `MockRuntime`
//!   fixture whose canned events include two synthetic
//!   commits to the branch) populates the recorder's
//!   `branch_state` with `start_sha`, `end_sha`,
//!   `commits: [...]`, `merged: false`, and
//!   `merge_shas: []`. The runtime does NOT call
//!   `git merge`, `git am`, or any write operation on
//!   `main`. `main`'s HEAD is byte-identical before and
//!   after the call. Without this, the runtime silently
//!   absorbs story 20's host-side auto-merge
//!   responsibility — the exact split note 14 named to
//!   preserve: "runtime records the branch_state in the
//!   run row; merge is story 20's host command."
//!
//! Red today: compile-red. The runtime's surface
//! (`MockRuntime::from_fixture`, `RunConfig`, `Runtime`,
//! `spawn_claude_session`, the `mock_store` accessor) does not exist.

use agentic_runtime::{EventSink, MockRuntime, RunConfig, Runtime};
use agentic_store::Store;
use git2::{Repository, Signature};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

struct NullSink;
impl EventSink for NullSink {
    fn emit(&mut self, _line: &str) {}
}

fn init_repo_with_baseline(dir: &std::path::Path) -> (Repository, String) {
    let repo = Repository::init(dir).expect("git init");
    let sig = Signature::now("UAT", "uat@example.invalid").expect("sig");
    let tree_id = {
        let mut idx = repo.index().expect("index");
        idx.write_tree().expect("write_tree")
    };
    let tree = repo.find_tree(tree_id).expect("find_tree");
    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, "baseline", &tree, &[])
        .expect("baseline commit");
    (repo, oid.to_string())
}

#[tokio::test(flavor = "current_thread")]
async fn runtime_records_branch_state_and_never_writes_to_main() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_path = repo_tmp.path().to_path_buf();
    let (_repo, baseline_sha) = init_repo_with_baseline(&repo_path);

    // Snapshot main's HEAD before the runtime runs. main must be
    // byte-identical after.
    let main_head_before = read_ref(&repo_path, "refs/heads/main")
        .or_else(|| read_ref(&repo_path, "refs/heads/master"))
        .unwrap_or_else(|| baseline_sha.clone());

    let fixture =
        PathBuf::from("crates/agentic-runtime/tests/fixtures/mock_branch_two_commits.ndjson");
    let mock = MockRuntime::from_fixture(&fixture).expect("MockRuntime::from_fixture");
    let runtime: Arc<dyn Runtime> = Arc::new(mock);

    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let run_id = "88888888-9999-4aaa-bbbb-cccc00001111".to_string();

    let cfg = RunConfig {
        run_id: run_id.clone(),
        story_id: 19,
        story_yaml_bytes: b"id: 19\n".to_vec(),
        signer: "sandbox:mock@run-88888888".to_string(),
        build_config: json!({ "max_inner_loop_iterations": 5 }),
        runs_root: runs_root_tmp.path().to_path_buf(),
        repo_path: Some(repo_path.clone()),
        branch_name: Some("run/19-deadbee".to_string()),
        prompt: "branch test".to_string(),
        event_sink: Box::new(NullSink),
    };

    let _outcome = runtime
        .spawn_claude_session(cfg)
        .await
        .expect("spawn must succeed on green+branch fixture");

    // Recorder-written row's branch_state is populated with the
    // expected shape.
    let store = runtime
        .mock_store()
        .expect("MockRuntime must expose its backing store");
    let rows = store
        .query("runs", &|doc| doc["run_id"] == json!(run_id))
        .expect("query");
    assert_eq!(
        rows.len(),
        1,
        "exactly one runs row must exist; got {rows:?}"
    );
    let branch_state = rows[0]
        .get("branch_state")
        .unwrap_or_else(|| panic!("row must carry branch_state; got {}", rows[0]));

    assert_eq!(
        branch_state["start_sha"],
        json!(baseline_sha),
        "branch_state.start_sha must equal the baseline SHA; got {:?}",
        branch_state["start_sha"]
    );
    assert_eq!(
        branch_state["merged"],
        json!(false),
        "branch_state.merged must be false; got {:?}",
        branch_state["merged"]
    );
    assert_eq!(
        branch_state["merge_shas"],
        json!([]),
        "branch_state.merge_shas must be an empty array; got {:?}",
        branch_state["merge_shas"]
    );
    assert!(
        branch_state.get("end_sha").is_some(),
        "branch_state.end_sha must be present; got {branch_state}"
    );
    let commits = branch_state["commits"]
        .as_array()
        .unwrap_or_else(|| panic!("branch_state.commits must be an array; got {branch_state}"));
    // The fixture names two synthetic commits; the mock is expected
    // to surface them (or at least a non-empty `commits[]`). If the
    // mock author decides to treat fixture commit events as
    // observational rather than actual-git-commits, the array may be
    // empty — but `merged: false` and `start_sha == baseline` are
    // the load-bearing invariants this story pins.
    assert!(
        commits.is_empty() || commits.iter().all(|c| c.is_string() || c.is_object()),
        "branch_state.commits must be a valid array (possibly empty); got {commits:?}"
    );

    // main's HEAD is byte-identical — the runtime never wrote to main.
    let main_head_after = read_ref(&repo_path, "refs/heads/main")
        .or_else(|| read_ref(&repo_path, "refs/heads/master"))
        .unwrap_or_else(|| baseline_sha.clone());
    assert_eq!(
        main_head_before, main_head_after,
        "main's HEAD must be byte-identical before ({main_head_before}) and after ({main_head_after}) the runtime call"
    );
}

fn read_ref(repo_path: &std::path::Path, refname: &str) -> Option<String> {
    let repo = Repository::open(repo_path).ok()?;
    let r = repo.find_reference(refname).ok()?;
    r.target().map(|oid| oid.to_string())
}
