//! Story 18 acceptance test: a `RunRecorder::start` driven through the
//! runtime writes a `runs` row whose `signer` equals the composed
//! sandbox convention `sandbox:<model>@<run_id>` — NOT the outer
//! shell's `AGENTIC_SIGNER`.
//!
//! Justification (from stories/18.yml acceptance.tests[10]):
//!   Proves the third evidence table gains the field: a
//!   `RunRecorder::start` driven through the runtime
//!   against a stub NDJSON stream writes a `runs` row
//!   whose `signer` equals the sandbox convention
//!   `sandbox:<model>@<run_id>` for that run. Story 16
//!   already requires `signer` as a non-empty string on
//!   the run row; this test proves the runtime populates
//!   it from the same composition the env-var injection
//!   test pins, rather than (say) reading the outer
//!   environment's `AGENTIC_SIGNER` and passing through
//!   whatever a dev happens to have exported. Without
//!   this, a run launched by a dev who exported their
//!   own `AGENTIC_SIGNER` would attribute the run to
//!   them rather than to the model-and-run-id, and the
//!   run row's signer would no longer identify which
//!   agent produced it.
//!
//! Red today: compile-red via the missing `ClaudeCodeRuntime` /
//! `RunConfig` symbols in `agentic_runtime`. The test `use`s them
//! explicitly; story 19 authors them.

use agentic_runtime::{
    ClaudeCodeRuntime, IterationSummary, Outcome, RunConfig, RunRecorder, RunRecorderConfig,
};
use agentic_store::{MemStore, Store};
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn runtime_driven_run_recorder_writes_row_with_sandbox_signer_composition() {
    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let runs_root = runs_root_tmp.path().to_path_buf();
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // The outer shell has its own AGENTIC_SIGNER exported. The run row
    // must NOT carry this value — the runtime's sandbox composition
    // wins.
    std::env::set_var("AGENTIC_SIGNER", "outer-shell-person@example.com");

    let run_id = "a1b2c3".to_string();
    let model = "claude-sonnet-4-6".to_string();

    // Drive the run through the runtime: the runtime composes the
    // sandbox signer and hands it into the recorder's config.
    let runtime = ClaudeCodeRuntime::new();
    let composed_signer = runtime.compose_signer(&RunConfig {
        model: model.clone(),
        run_id: run_id.clone(),
    });
    assert_eq!(
        composed_signer, "sandbox:claude-sonnet-4-6@run-a1b2c3",
        "runtime.compose_signer must produce the normative convention"
    );

    let cfg = RunRecorderConfig {
        store: Arc::clone(&store),
        runs_root: runs_root.clone(),
        run_id: run_id.clone(),
        story_id: 18,
        story_yaml_bytes: b"id: 18\n".to_vec(),
        signer: composed_signer.clone(),
        build_config: json!({ "max_inner_loop_iterations": 1 }),
    };
    let recorder = RunRecorder::start(cfg).expect("start");
    recorder
        .record_iteration(IterationSummary {
            i: 0,
            started_at: "2026-04-23T00:00:00Z".to_string(),
            ended_at: "2026-04-23T00:00:01Z".to_string(),
            probes: vec![],
            verdict: None,
            error: None,
        })
        .expect("iter");
    recorder
        .finish(Outcome::Green {
            signing_run_id: "stub-signing-1".to_string(),
        })
        .expect("finish");

    let rows = store
        .query("runs", &|doc| doc["run_id"] == json!(run_id))
        .expect("query");
    assert_eq!(rows.len(), 1, "one run row; got {rows:?}");
    let row = &rows[0];

    // The signer field is the sandbox composition — NOT the outer
    // shell's export.
    assert_eq!(
        row["signer"],
        json!("sandbox:claude-sonnet-4-6@run-a1b2c3"),
        "run row's signer must be the composed sandbox value; got {}",
        row["signer"]
    );
    assert_ne!(
        row["signer"],
        json!("outer-shell-person@example.com"),
        "run row's signer must NOT be the outer shell's AGENTIC_SIGNER export"
    );

    // Cleanup.
    std::env::remove_var("AGENTIC_SIGNER");
}
