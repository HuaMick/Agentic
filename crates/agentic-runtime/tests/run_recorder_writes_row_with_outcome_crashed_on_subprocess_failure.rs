//! Story 16 acceptance test: the recorder writes a `runs` row with
//! `outcome: crashed` when finished with `Outcome::Crashed { error }`.
//!
//! Justification (from stories/16.yml acceptance.tests[5]):
//!   Proves the crashed outcome wiring: given a recorder whose driven
//!   subprocess exits non-zero mid-iteration (or whose writer
//!   observes a broken pipe from the subprocess stdout), a
//!   `finish(Outcome::Crashed { error })` call yields a `runs` row
//!   whose `outcome` is `crashed`, whose final `iterations[]` entry
//!   carries a non-empty `error` field naming the failure, and whose
//!   `trace_ndjson_path` still points at a readable (possibly
//!   truncated) trace file containing whatever was captured before
//!   the crash. Without this, a subprocess crash produces either no
//!   row (silent loss of evidence) or a row mislabelled as exhausted
//!   (misdirected human attention).
//!
//! Red today: natural. `Outcome::Crashed` does not yet exist.

use agentic_runtime::{IterationSummary, Outcome, RunRecorder, RunRecorderConfig, TraceTee};
use agentic_store::{MemStore, Store};
use serde_json::json;
use std::fs;
use std::io::Write;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn recorder_writes_crashed_row_with_error_field_and_readable_trace() {
    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let runs_root = runs_root_tmp.path().to_path_buf();
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let run_id = "cccc3333-dddd-4eee-8fff-000011112222".to_string();

    let cfg = RunRecorderConfig {
        store: Arc::clone(&store),
        runs_root: runs_root.clone(),
        run_id: run_id.clone(),
        story_id: 15,
        story_yaml_bytes: b"id: 15\n".to_vec(),
        signer: "sandbox:stub@run-crash".to_string(),
        build_config: json!({}),
    };

    let recorder = RunRecorder::start(cfg).expect("start should succeed");

    // Feed a truncated line through the tee, simulating a subprocess
    // whose stdout pipe broke mid-write.
    let mut tee: TraceTee = recorder.trace_tee();
    tee.write_all(b"{\"kind\":\"tool_call\",\"i")
        .expect("partial write should succeed");
    // Intentionally no flush; the crash comes before the line
    // completes.

    // The iteration that was in progress when the crash happened:
    // record it with a non-empty `error` field naming the failure
    // (the recorder forwards this into the row verbatim).
    recorder
        .record_iteration(IterationSummary {
            i: 0,
            started_at: "2026-04-23T00:00:00Z".to_string(),
            ended_at: "2026-04-23T00:00:01Z".to_string(),
            probes: vec![],
            verdict: None,
            error: Some("stub pipe broke mid-line".to_string()),
        })
        .expect("record_iteration");

    recorder
        .finish(Outcome::Crashed {
            error: "stub pipe broke mid-line".to_string(),
        })
        .expect("finish should succeed with crashed outcome");

    let rows = store
        .query("runs", &|doc| doc["run_id"] == json!(run_id))
        .expect("query");
    assert_eq!(
        rows.len(),
        1,
        "exactly one row should exist for this run_id; got {rows:?}"
    );
    let row = &rows[0];

    assert_eq!(
        row["outcome"],
        json!("crashed"),
        "outcome must be the literal string \"crashed\"; got {}",
        row["outcome"]
    );

    // The final iterations[] entry carries a non-empty `error` field
    // naming the failure.
    let iterations = row["iterations"]
        .as_array()
        .expect("iterations must be a JSON array");
    assert!(
        !iterations.is_empty(),
        "iterations must contain at least the crashed iteration"
    );
    let final_iter = iterations.last().expect("last iteration");
    let err_field = final_iter
        .get("error")
        .and_then(|v| v.as_str())
        .expect("final iteration must carry a string `error` field on crash");
    assert!(
        !err_field.trim().is_empty(),
        "final iteration's `error` field must be non-empty on crash; got {err_field:?}"
    );

    // trace_ndjson_path still points at a readable file (possibly
    // truncated) containing whatever was captured before the crash.
    let trace_rel = row["trace_ndjson_path"]
        .as_str()
        .expect("trace_ndjson_path must be a string");
    let trace_abs = runs_root.join(trace_rel);
    assert!(
        trace_abs.exists(),
        "partial trace file must still exist at {trace_abs:?} after a crash"
    );
    let trace_bytes = fs::read(&trace_abs).expect("read partial trace");
    assert!(
        trace_bytes.starts_with(b"{\"kind\":\"tool_call\",\"i"),
        "partial trace must contain the bytes written before the crash; got {:?}",
        String::from_utf8_lossy(&trace_bytes)
    );
}
