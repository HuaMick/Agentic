//! Story 20 acceptance test: on budget exhaustion (inner loop never
//! declares green within `max_inner_loop_iterations`),
//! `StoryBuild::run_in_sandbox` writes zero `uat_signings` rows and
//! exactly one `runs` row with `outcome: inner_loop_exhausted`,
//! `iterations.length == budget`, `branch_state.merged == false`,
//! and a non-empty trace on disk.
//!
//! Justification (from stories/20.yml acceptance.tests[5]):
//!   Proves the exhausted outcome wiring with a stub that
//!   never greens: given the same fixture as the green test
//!   but a stub `Runtime` that emits iterations up to the
//!   budget (`max_inner_loop_iterations: 3`) without ever
//!   declaring green and without ever invoking `agentic uat
//!   --verdict pass`, the resulting store state has
//!   (a) exactly ZERO new `uat_signings` rows for the
//!   fixture story; (b) exactly one `runs` row with
//!   `outcome: "inner_loop_exhausted"`, `iterations.length
//!   == 3`, and `branch_state.merged == false`. The trace
//!   file on disk at `/output/runs/<run-id>/trace.ndjson`
//!   is non-empty and contains the full NDJSON sequence
//!   the stub emitted.
//!
//! Red today: compile-red via the missing `agentic_story_build`
//! public surface (`StoryBuild::run_in_sandbox_with_runtime`,
//! `InSandboxConfig`, `Outcome`).

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use agentic_runtime::MockRuntime;
use agentic_store::{MemStore, Store};
use agentic_story_build::{InSandboxConfig, Outcome, StoryBuild};
use serde_json::json;
use tempfile::TempDir;

#[tokio::test(flavor = "current_thread")]
async fn exhausted_inner_loop_writes_run_row_no_signing_and_non_empty_trace() {
    let work_tmp = TempDir::new().expect("work tempdir");
    let work = work_tmp.path();

    // Fixture story — same shape as the green test, depends_on
    // empty so the gate is trivially satisfied.
    let story_yaml_path = work.join("story.yml");
    fs::write(
        &story_yaml_path,
        "id: 5092\n\
         title: fixture-never-greens\n\
         outcome: never\n\
         status: proposed\n\
         patterns: []\n\
         acceptance:\n  tests:\n  - file: crates/fx/tests/t.rs\n    justification: never satisfied\n  uat: ignored\n\
         depends_on: []\n",
    )
    .expect("write fixture story");

    let snapshot_path = work.join("snapshot.json");
    fs::write(&snapshot_path, r#"{"schema_version":1,"signings":[]}"#)
        .expect("write empty snapshot");

    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let runs_root = runs_root_tmp.path().to_path_buf();

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Stub runtime: canned fixture that walks the inner loop up to
    // the budget without ever greening. Story 19 ships this fixture
    // at `mock_budget_five_pairs.ndjson` — we pass max=3 below so
    // the runtime stops at iteration 3.
    let budget_fixture =
        PathBuf::from("crates/agentic-runtime/tests/fixtures/mock_budget_five_pairs.ndjson");
    let mock =
        MockRuntime::from_fixture(&budget_fixture).expect("MockRuntime::from_fixture budget");

    let run_id = "run-exh-92".to_string();
    let signer = format!("sandbox:claude-sonnet-4-6@{run_id}");

    let cfg = InSandboxConfig {
        story_id: 5092,
        run_id: run_id.clone(),
        signer: signer.clone(),
        story_yaml_path,
        snapshot_path,
        runs_root: runs_root.clone(),
        start_sha: "a09aaed609cdab88ca8dcb0a8be5c7928befbabc".to_string(),
        max_inner_loop_iterations: 3,
        model: "claude-sonnet-4-6".to_string(),
    };

    let outcome = StoryBuild::run_in_sandbox_with_runtime(cfg, Arc::clone(&store), Arc::new(mock))
        .await
        .expect("run_in_sandbox on exhausted fixture must return Ok(Outcome::...) rather than Err");

    assert!(
        matches!(outcome, Outcome::InnerLoopExhausted { .. }),
        "run_in_sandbox on a budget-exhausted fixture must return Outcome::InnerLoopExhausted; \
         got {outcome:?}"
    );

    // Zero uat_signings rows for this story.
    let signings = store
        .query("uat_signings", &|doc| doc["story_id"] == json!(5092))
        .expect("query signings");
    assert!(
        signings.is_empty(),
        "no uat_signings row may land on an exhausted run; got {signings:?}"
    );

    // Exactly one runs row.
    let runs = store
        .query("runs", &|doc| doc["run_id"] == json!(&run_id))
        .expect("query runs");
    assert_eq!(
        runs.len(),
        1,
        "exactly one runs row must land on exhaustion; got {runs:?}"
    );
    let row = &runs[0];
    assert_eq!(row["outcome"], json!("inner_loop_exhausted"));

    let iterations = row["iterations"]
        .as_array()
        .expect("iterations must be an array");
    assert_eq!(
        iterations.len(),
        3,
        "iterations.length must equal the budget; got {iterations:?}"
    );
    assert_eq!(
        row["branch_state"]["merged"],
        json!(false),
        "exhausted run must have branch_state.merged == false; got {}",
        row["branch_state"]["merged"]
    );

    // Non-empty trace on disk under <runs_root>/<run-id>/trace.ndjson.
    let trace_path = runs_root.join(&run_id).join("trace.ndjson");
    assert!(
        trace_path.exists(),
        "trace.ndjson must exist at {trace_path:?} for an exhausted run"
    );
    let trace_body = fs::read(&trace_path).expect("read trace file");
    assert!(
        !trace_body.is_empty(),
        "trace.ndjson must contain the NDJSON the stub emitted; got empty file at {trace_path:?}"
    );
}
