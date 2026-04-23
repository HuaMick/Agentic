//! Story 16 acceptance test: the recorder writes a row with
//! `outcome: inner_loop_exhausted` when driven to budget and finished
//! with `Outcome::InnerLoopExhausted`.
//!
//! Justification (from stories/16.yml acceptance.tests[4]):
//!   Proves the exhausted outcome wiring: given a recorder started
//!   with a budget of N iterations and fed exactly N
//!   `record_iteration` calls followed by a
//!   `finish(Outcome::InnerLoopExhausted)`, the resulting `runs`
//!   row has `outcome: inner_loop_exhausted`, exactly N entries in
//!   `iterations`, and NO pointer to any `uat_signings` row.
//!   Without this, the signal that drives the outer loop
//!   (human-in-the-loop amendment on exhaustion, per ADR-0006 §7)
//!   is indistinguishable from a crash.
//!
//! Red today: natural. The `Outcome::InnerLoopExhausted` variant
//! does not yet exist; `cargo check` fails on the import.

use agentic_runtime::{IterationSummary, Outcome, RunRecorder, RunRecorderConfig};
use agentic_store::{MemStore, Store};
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn recorder_writes_exhausted_row_with_n_iterations_and_no_signing_pointer() {
    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let run_id = "bbbb2222-cccc-4ddd-8eee-ffff33334444".to_string();
    let budget: usize = 3;

    let cfg = RunRecorderConfig {
        store: Arc::clone(&store),
        runs_root: runs_root_tmp.path().to_path_buf(),
        run_id: run_id.clone(),
        story_id: 15,
        story_yaml_bytes: b"id: 15\n".to_vec(),
        signer: "sandbox:stub@run-exh".to_string(),
        build_config: json!({ "max_inner_loop_iterations": budget }),
    };

    let recorder = RunRecorder::start(cfg).expect("start should succeed");

    for i in 0..budget {
        recorder
            .record_iteration(IterationSummary {
                i: i as u32,
                started_at: format!("2026-04-23T00:00:0{i}Z"),
                ended_at: format!("2026-04-23T00:00:0{}Z", i + 1),
                probes: vec![],
                verdict: None,
                error: None,
            })
            .expect("record_iteration");
    }

    recorder
        .finish(Outcome::InnerLoopExhausted)
        .expect("finish should succeed with exhausted outcome");

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
        json!("inner_loop_exhausted"),
        "outcome must be the literal string \"inner_loop_exhausted\"; got {}",
        row["outcome"]
    );

    let iterations = row["iterations"]
        .as_array()
        .expect("iterations must be a JSON array");
    assert_eq!(
        iterations.len(),
        budget,
        "iterations must reflect exactly the {budget} record_iteration calls; got {iterations:?}"
    );

    // No pointer to any `uat_signings` row. The exhausted variant
    // carries no payload, so there must not be a signing_run_id on
    // the row (that field is the green variant's exclusive payload).
    assert!(
        row.get("signing_run_id").is_none()
            && !row["outcome"].as_str().unwrap_or("").contains("signing"),
        "exhausted row must NOT carry a signing_run_id pointer; got row {row}"
    );
}
