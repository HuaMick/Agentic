//! Story 16 acceptance test: the `RunRecorder` + `TraceTee` sub-module
//! can be driven end-to-end from a dependency floor of only
//! `agentic-runtime`, `agentic-store`, `serde`, and `tempfile`.
//!
//! Justification (from stories/16.yml acceptance.tests[9]):
//!   Proves the standalone-resilient-library claim: the
//!   `agentic-runtime` crate's `RunRecorder` + `TraceTee` types can
//!   be constructed and driven end-to-end from a test that links
//!   only against `agentic-runtime`, `agentic-store` (for
//!   `MemStore`), `serde`, and `tempfile`. No `agentic-cli`, no
//!   `agentic-orchestrator`, no `agentic-sandbox`, no LLM
//!   subprocess. The test drives a fixture NDJSON stream (a `&[u8]`
//!   slice — not `claude`), records three iterations, finishes
//!   green, and asserts the `runs` row plus the on-disk trace.
//!   Without this, the recorder's "prove-it path" posture is
//!   aspirational; the first downstream consumer would drag
//!   orchestrator dependencies back in and we'd only notice when
//!   the system was in flames.
//!
//! Red today: natural. The recorder does not yet exist, so
//! `cargo check` fails on the import. Once green, this test
//! becomes the crate's standing guarantee that the recorder does
//! not require the orchestrator to function.
//!
//! Dependency-floor discipline: this file deliberately names ONLY
//! the four allowed crates in its `use` statements. Adding an
//! orchestrator-dependent crate here — even by accident — is the
//! regression this test is here to pin.

#![allow(dead_code)]

use agentic_runtime::{IterationSummary, Outcome, RunRecorder, RunRecorderConfig, TraceTee};
use agentic_store::{MemStore, Store};
use serde::Deserialize;
use std::fs;
use std::io::Write;
use std::sync::Arc;
use tempfile::TempDir;

/// Structural assertion of the row shape, using `serde` only. If the
/// `runs` row drifts away from this shape, the test fails at
/// deserialization.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunRowView {
    run_id: String,
    story_id: i64,
    story_yaml_snapshot: String,
    signer: String,
    started_at: String,
    ended_at: String,
    build_config: serde_json::Value,
    outcome: String,
    iterations: Vec<serde_json::Value>,
    branch_state: serde_json::Value,
    trace_ndjson_path: String,
}

#[test]
fn recorder_can_be_driven_end_to_end_without_orchestrator_or_llm() {
    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let runs_root = runs_root_tmp.path().to_path_buf();
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let run_id = "ffff6666-0000-4111-8222-333344445555".to_string();
    let cfg = RunRecorderConfig {
        store: Arc::clone(&store),
        runs_root: runs_root.clone(),
        run_id: run_id.clone(),
        story_id: 15,
        story_yaml_bytes: b"id: 15\ntitle: standalone-resilience fixture\n".to_vec(),
        signer: "sandbox:stub@run-standalone".to_string(),
        build_config: serde_json::json!({ "max_inner_loop_iterations": 3 }),
    };

    let recorder = RunRecorder::start(cfg).expect("start (no orchestrator needed)");

    // Drive a canned NDJSON stream — a `&[u8]` slice, not a real
    // subprocess — through the tee.
    let mut tee: TraceTee = recorder.trace_tee();
    let canned = b"{\"kind\":\"tool_call\",\"i\":0}\n{\"kind\":\"tool_result\",\"i\":0}\n{\"kind\":\"iteration_end\",\"i\":0}\n";
    tee.write_all(canned).expect("write canned ndjson");
    tee.flush().expect("flush tee");

    // Three iterations, finishing green.
    for i in 0..3 {
        recorder
            .record_iteration(IterationSummary {
                i,
                started_at: format!("2026-04-23T00:00:0{i}Z"),
                ended_at: format!("2026-04-23T00:00:0{}Z", i + 1),
                probes: vec![],
                verdict: None,
                error: None,
            })
            .expect("record_iteration");
    }

    recorder
        .finish(Outcome::Green {
            signing_run_id: "stub-signing-1".to_string(),
        })
        .expect("finish green");

    // The row exists and deserializes into the documented shape under
    // `deny_unknown_fields` — the recorder is not smuggling extra
    // columns in.
    let rows = store
        .query("runs", &|doc| doc["run_id"] == serde_json::json!(run_id))
        .expect("query");
    assert_eq!(rows.len(), 1, "one row; got {rows:?}");
    let view: RunRowView = serde_json::from_value(rows[0].clone())
        .expect("row must deserialize into documented shape under deny_unknown_fields");

    assert_eq!(view.run_id, run_id);
    assert_eq!(view.story_id, 15);
    assert_eq!(view.outcome, "green");
    assert_eq!(view.iterations.len(), 3);

    // The on-disk trace file byte-matches the canned input.
    let trace_abs = runs_root.join(&view.trace_ndjson_path);
    let on_disk = fs::read(&trace_abs).expect("read trace");
    assert_eq!(
        on_disk, canned,
        "trace file must contain exactly the canned bytes we wrote"
    );
}
