//! Story 20 acceptance test: on green, `StoryBuild::run_in_sandbox`
//! writes exactly one `uat_signings` row (verdict=pass, sandbox
//! signer) AND exactly one `runs` row (outcome=green, matching
//! signer, non-empty branch_state.commits, merged=false). The two
//! rows agree on signer byte-for-byte — the reproducibility receipt
//! is a closed loop.
//!
//! Justification (from stories/20.yml acceptance.tests[4]):
//!   Proves the green outcome wiring end-to-end with a stub
//!   inner loop: given a fixture story with one trivial
//!   acceptance test that passes cleanly, a pre-seeded
//!   ancestor snapshot satisfying the gate, and a stub
//!   `Runtime` impl that emits a canned NDJSON trace ending
//!   with claude declaring green and exiting cleanly,
//!   `StoryBuild::run_in_sandbox` writes (a) exactly one
//!   `uat_signings` row with `verdict: pass`, `signer:
//!   "sandbox:<model>@<run_id>"`, `story_id: <id>`, and a
//!   commit hash matching the branch tip; (b) exactly one
//!   `runs` row with `outcome: "green"`, `signer` matching
//!   the signing row, and `branch_state.commits` non-empty;
//!   (c) the `runs` row's `branch_state.merged` is initially
//!   `false` (the host is what flips it to `true`). The
//!   signer on both rows is byte-identical.
//!
//! Red today: compile-red via the missing `agentic_story_build`
//! public surface (`StoryBuild::run_in_sandbox`, `InSandboxConfig`).

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use agentic_runtime::MockRuntime;
use agentic_store::{MemStore, Store};
use agentic_story_build::{InSandboxConfig, Outcome, StoryBuild};
use serde_json::json;
use tempfile::TempDir;

#[tokio::test(flavor = "current_thread")]
async fn green_inner_loop_writes_matching_signing_and_run_rows() {
    let work_tmp = TempDir::new().expect("work tempdir");
    let work = work_tmp.path();

    // Fixture story with one trivially-passable acceptance test and
    // no ancestors (depends_on: []) so the gate is satisfied by an
    // empty snapshot.
    let story_yaml_path = work.join("story.yml");
    fs::write(
        &story_yaml_path,
        "id: 4081\n\
         title: fixture-green-trivial\n\
         outcome: trivially green\n\
         status: proposed\n\
         patterns: []\n\
         acceptance:\n  tests:\n  - file: crates/fx/tests/t.rs\n    justification: trivially passable\n  uat: ignored\n\
         depends_on: []\n",
    )
    .expect("write fixture story");

    let snapshot_path = work.join("snapshot.json");
    fs::write(&snapshot_path, r#"{"schema_version":1,"signings":[]}"#)
        .expect("write empty snapshot");

    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let runs_root = runs_root_tmp.path().to_path_buf();

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Stub runtime: canned green NDJSON fixture that story 19
    // shipped; the in-sandbox driver plugs this in via the
    // `runtime_override` field below.
    let green_fixture =
        PathBuf::from("crates/agentic-runtime/tests/fixtures/mock_green_three_pairs.ndjson");
    let mock = MockRuntime::from_fixture(&green_fixture).expect("MockRuntime::from_fixture");

    let run_id = "run-green-42".to_string();
    let signer = format!("sandbox:claude-sonnet-4-6@{run_id}");

    let cfg = InSandboxConfig {
        story_id: 4081,
        run_id: run_id.clone(),
        signer: signer.clone(),
        story_yaml_path,
        snapshot_path,
        runs_root,
        start_sha: "a09aaed609cdab88ca8dcb0a8be5c7928befbabc".to_string(),
        max_inner_loop_iterations: 3,
        model: "claude-sonnet-4-6".to_string(),
    };

    let outcome = StoryBuild::run_in_sandbox_with_runtime(cfg, Arc::clone(&store), Arc::new(mock))
        .await
        .expect("run_in_sandbox on green fixture must succeed");

    // Outcome surface: green variant.
    assert!(
        matches!(outcome, Outcome::Green { .. }),
        "run_in_sandbox on a green fixture must return Outcome::Green; got {outcome:?}"
    );

    // Exactly one uat_signings row for this story.
    let signings = store
        .query("uat_signings", &|doc| doc["story_id"] == json!(4081))
        .expect("query signings");
    assert_eq!(
        signings.len(),
        1,
        "exactly one uat_signings row must land on green; got {signings:?}"
    );
    let signing = &signings[0];
    assert_eq!(signing["verdict"], json!("pass"));
    assert_eq!(signing["signer"], json!(&signer));
    assert_eq!(signing["story_id"], json!(4081));
    let signing_commit = signing["commit"]
        .as_str()
        .expect("signing commit must be a string")
        .to_string();
    assert_eq!(
        signing_commit.len(),
        40,
        "signing commit must be a 40-char hex sha; got {signing_commit:?}"
    );

    // Exactly one runs row for this run_id.
    let runs = store
        .query("runs", &|doc| doc["run_id"] == json!(&run_id))
        .expect("query runs");
    assert_eq!(
        runs.len(),
        1,
        "exactly one runs row must land on green; got {runs:?}"
    );
    let run_row = &runs[0];
    assert_eq!(run_row["outcome"], json!("green"));
    assert_eq!(
        run_row["signer"],
        json!(&signer),
        "runs.signer must be byte-identical to uat_signings.signer"
    );
    let commits = run_row["branch_state"]["commits"]
        .as_array()
        .expect("branch_state.commits must be an array");
    assert!(
        !commits.is_empty(),
        "runs.branch_state.commits must be non-empty on green; got {commits:?}"
    );
    assert_eq!(
        run_row["branch_state"]["merged"],
        json!(false),
        "runs.branch_state.merged is initially false — host flips it to true post-merge; \
         got {}",
        run_row["branch_state"]["merged"]
    );

    // Signer agreement (restate explicitly — the receipt lives on
    // both rows agreeing byte-for-byte).
    assert_eq!(
        run_row["signer"], signing["signer"],
        "runs.signer must equal uat_signings.signer byte-identically"
    );
}
