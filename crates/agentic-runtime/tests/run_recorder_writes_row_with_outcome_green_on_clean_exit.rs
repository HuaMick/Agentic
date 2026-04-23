//! Story 16 acceptance test: the recorder writes a `runs` row with
//! `outcome: green` when driven through a clean start→iterate→finish
//! flow.
//!
//! Justification (from stories/16.yml acceptance.tests[3]):
//!   Proves the green outcome wiring: given a
//!   `RunRecorder::start(story_id, signer, build_config)` call
//!   followed by one or more `record_iteration(summary)` calls and
//!   a final `finish(Outcome::Green { signing_run_id })` call
//!   against a `MemStore`, a single row appears in the `runs`
//!   table whose `outcome` is `green`, whose `iterations` array
//!   length equals the number of `record_iteration` calls in
//!   insertion order, whose `ended_at` is strictly after
//!   `started_at`, and whose `trace_ndjson_path` points at a file
//!   whose first line is a valid JSON object.
//!
//! Red today: natural. The recorder types do not yet exist;
//! `cargo check` fails on the `use agentic_runtime::*` line.

use agentic_runtime::{IterationSummary, Outcome, RunRecorder, RunRecorderConfig, TraceTee};
use agentic_store::{MemStore, Store};
use serde_json::json;
use std::fs;
use std::io::Write;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn recorder_writes_single_green_row_with_iterations_and_trace_first_line() {
    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let runs_root = runs_root_tmp.path().to_path_buf();
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let run_id = "aaaa1111-bbbb-4ccc-8ddd-eeee22223333".to_string();

    let cfg = RunRecorderConfig {
        store: Arc::clone(&store),
        runs_root: runs_root.clone(),
        run_id: run_id.clone(),
        story_id: 15,
        story_yaml_bytes: b"id: 15\n".to_vec(),
        signer: "sandbox:stub@run-green".to_string(),
        build_config: json!({ "max_inner_loop_iterations": 3 }),
    };

    let recorder = RunRecorder::start(cfg).expect("start should succeed");

    // Write one valid JSON line to the trace so the "first line is a
    // valid JSON object" invariant has something to parse.
    let mut tee: TraceTee = recorder.trace_tee();
    tee.write_all(b"{\"kind\":\"tool_call\",\"i\":0}\n")
        .expect("write tee");
    tee.flush().expect("flush tee");

    // Two record_iteration calls — the row must reflect both in
    // insertion order.
    for i in 0..2 {
        recorder
            .record_iteration(IterationSummary {
                i,
                started_at: format!("2026-04-23T00:00:0{i}Z"),
                ended_at: format!("2026-04-23T00:00:0{}Z", i + 1),
                probes: vec![],
                verdict: None,
                error: None,
            })
            .expect("record_iteration should succeed");
    }

    recorder
        .finish(Outcome::Green {
            signing_run_id: "stub-signing-1".to_string(),
        })
        .expect("finish should succeed on clean wiring");

    // Exactly one row on the runs table for this run_id.
    let rows = store
        .query("runs", &|doc| doc["run_id"] == json!(run_id))
        .expect("query should succeed");
    assert_eq!(
        rows.len(),
        1,
        "finish() must write exactly one runs row; got {rows:?}"
    );
    let row = &rows[0];

    // outcome == green.
    assert_eq!(
        row["outcome"],
        json!("green"),
        "outcome must be the literal string \"green\"; got {}",
        row["outcome"]
    );

    // iterations.len() == number of record_iteration calls, in order.
    let iterations = row["iterations"]
        .as_array()
        .expect("iterations must be a JSON array");
    assert_eq!(
        iterations.len(),
        2,
        "iterations must reflect the 2 record_iteration calls; got {iterations:?}"
    );
    assert_eq!(iterations[0]["i"], json!(0));
    assert_eq!(iterations[1]["i"], json!(1));

    // ended_at is strictly after started_at (RFC3339 UTC lex order
    // works for this).
    let started = row["started_at"].as_str().expect("started_at str");
    let ended = row["ended_at"].as_str().expect("ended_at str");
    assert!(
        ended > started,
        "ended_at ({ended:?}) must be strictly greater than started_at ({started:?})"
    );

    // trace_ndjson_path points at a file whose first line is a valid
    // JSON object.
    let trace_rel = row["trace_ndjson_path"]
        .as_str()
        .expect("trace_ndjson_path must be a string");
    let trace_abs = runs_root.join(trace_rel);
    assert!(
        trace_abs.exists(),
        "trace file at {trace_abs:?} must exist on disk"
    );
    let trace_body = fs::read_to_string(&trace_abs).expect("read trace file");
    let first_line = trace_body
        .lines()
        .next()
        .expect("trace file must have at least one line");
    let parsed: serde_json::Value =
        serde_json::from_str(first_line).expect("first line of trace must be valid JSON");
    assert!(
        parsed.is_object(),
        "first line of trace must be a JSON object; got {parsed}"
    );
}
