//! Story 20 acceptance test: on subprocess crash (stub runtime
//! exits 137 mid-iteration), `StoryBuild::run_in_sandbox` writes
//! zero `uat_signings` rows and exactly one `runs` row with
//! `outcome: crashed`, a non-empty `error` field on the final
//! `iterations[]` entry naming the failing exit code, and a
//! readable (truncated) trace on disk.
//!
//! Justification (from stories/20.yml acceptance.tests[6]):
//!   Proves the crashed outcome wiring with a stub that exits
//!   non-zero: given a stub `Runtime` whose claude subprocess
//!   exits 137 (OOM killer shape) mid-iteration, the
//!   resulting store state has (a) zero new `uat_signings`
//!   rows; (b) exactly one `runs` row with `outcome:
//!   "crashed"`, a non-empty `error` field on the final
//!   `iterations[]` entry naming "subprocess exited with
//!   code 137" (or equivalent), and `branch_state`
//!   reflecting whatever commits the agent made before the
//!   crash (possibly empty). The trace file is readable but
//!   truncated.
//!
//! Red today: compile-red via the missing `agentic_story_build`
//! public surface (`StoryBuild::run_in_sandbox_with_runtime`,
//! `InSandboxConfig`, `Outcome::Crashed`).

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use agentic_runtime::MockRuntime;
use agentic_store::{MemStore, Store};
use agentic_story_build::{InSandboxConfig, Outcome, StoryBuild};
use serde_json::json;
use tempfile::TempDir;

#[tokio::test(flavor = "current_thread")]
async fn crashed_subprocess_writes_crashed_row_no_signing_and_truncated_trace() {
    let work_tmp = TempDir::new().expect("work tempdir");
    let work = work_tmp.path();

    let story_yaml_path = work.join("story.yml");
    fs::write(
        &story_yaml_path,
        "id: 6103\n\
         title: fixture-crashes-midway\n\
         outcome: crashed\n\
         status: proposed\n\
         patterns: []\n\
         acceptance:\n  tests:\n  - file: crates/fx/tests/t.rs\n    justification: crashes midway\n  uat: ignored\n\
         depends_on: []\n",
    )
    .expect("write fixture story");

    let snapshot_path = work.join("snapshot.json");
    fs::write(&snapshot_path, r#"{"schema_version":1,"signings":[]}"#)
        .expect("write empty snapshot");

    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let runs_root = runs_root_tmp.path().to_path_buf();

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Stub runtime: canned fixture that emits a partial NDJSON
    // sequence and then the runtime simulates the child process
    // exiting 137. Story 19 ships the `mock_pipe_break.ndjson`
    // fixture; we augment that with an explicit crash_code setter
    // on MockRuntime (if present) or rely on the fixture itself to
    // encode the crash.
    let crash_fixture =
        PathBuf::from("crates/agentic-runtime/tests/fixtures/mock_pipe_break.ndjson");
    let mock = MockRuntime::from_fixture(&crash_fixture)
        .expect("MockRuntime::from_fixture crash")
        .with_crash_exit_code(137);

    let run_id = "run-crash-03".to_string();
    let signer = format!("sandbox:claude-sonnet-4-6@{run_id}");

    let cfg = InSandboxConfig {
        story_id: 6103,
        run_id: run_id.clone(),
        signer: signer.clone(),
        story_yaml_path,
        snapshot_path,
        runs_root: runs_root.clone(),
        start_sha: "a09aaed609cdab88ca8dcb0a8be5c7928befbabc".to_string(),
        max_inner_loop_iterations: 3,
        model: "claude-sonnet-4-6".to_string(),
    };

    let outcome =
        StoryBuild::run_in_sandbox_with_runtime(cfg, Arc::clone(&store), Arc::new(mock))
            .await
            .expect("run_in_sandbox on crashed fixture must return Ok(Outcome::Crashed) rather than Err — the runs row is the evidence surface");

    assert!(
        matches!(outcome, Outcome::Crashed { .. }),
        "run_in_sandbox on a crashed fixture must return Outcome::Crashed; got {outcome:?}"
    );

    // Zero uat_signings for this story.
    let signings = store
        .query("uat_signings", &|doc| doc["story_id"] == json!(6103))
        .expect("query signings");
    assert!(
        signings.is_empty(),
        "no uat_signings may land on a crashed run; got {signings:?}"
    );

    // Exactly one runs row; outcome=crashed; final iteration has
    // a non-empty error naming exit code 137.
    let runs = store
        .query("runs", &|doc| doc["run_id"] == json!(&run_id))
        .expect("query runs");
    assert_eq!(
        runs.len(),
        1,
        "exactly one runs row must land on a crash; got {runs:?}"
    );
    let row = &runs[0];
    assert_eq!(row["outcome"], json!("crashed"));

    let iterations = row["iterations"]
        .as_array()
        .expect("iterations must be a JSON array");
    assert!(
        !iterations.is_empty(),
        "iterations must contain at least the crashed iteration; got {iterations:?}"
    );
    let final_iter = iterations.last().expect("last iteration");
    let err_text = final_iter["error"]
        .as_str()
        .expect("final iteration must carry a string `error` field on crash");
    assert!(
        err_text.contains("137"),
        "final iteration's error must name the failing exit code 137; got {err_text:?}"
    );

    // Trace file is readable but (possibly) truncated.
    let trace_path = runs_root.join(&run_id).join("trace.ndjson");
    assert!(
        trace_path.exists(),
        "trace.ndjson must be readable after a crash at {trace_path:?}"
    );
}
