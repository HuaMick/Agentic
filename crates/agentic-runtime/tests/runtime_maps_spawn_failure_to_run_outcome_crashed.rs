//! Story 19 acceptance test: a `MockRuntime` configured to inject a
//! mid-stream pipe break produces `RunOutcome { outcome: Crashed, ... }`
//! with a non-empty `error` field on the last iteration, and the trace
//! file still points at a readable-but-truncated file containing
//! whatever was captured before the break. `spawn_claude_session`
//! returns `Ok(RunOutcome)` — NOT `Err` — for a mid-session crash.
//!
//! Justification (from stories/19.yml acceptance.tests[6]):
//!   Proves the error model's `Crashed` wiring: a
//!   `MockRuntime` configured to inject a mid-stream pipe
//!   break (a simulated broken subprocess stdout) produces
//!   a `RunOutcome` whose `outcome` is `crashed`, whose
//!   recorder-written `runs` row has an `error` field on
//!   the last iteration naming the pipe-break cause, and
//!   whose `trace_ndjson_path` still points at a
//!   readable-but-possibly-truncated trace file containing
//!   whatever was captured before the break. The spawn
//!   path returns `Ok(RunOutcome)` (the crash is captured
//!   IN the outcome, not as a top-level Err); a failure
//!   BEFORE the subprocess started (e.g. `ClaudeSpawn`)
//!   does return `Err`. Without this, a subprocess crash
//!   is indistinguishable from exhaustion (the signal
//!   ADR-0006 §7 depends on to trigger human amendment)
//!   and from a runtime library bug (the signal operators
//!   need to report upstream).
//!
//! Red today: compile-red. `MockRuntime::from_fixture_with_pipe_break`,
//! `Outcome::Crashed`, and the `RunOutcome` / `Runtime` / `RunConfig`
//! surface do not yet exist.

use agentic_runtime::{EventSink, MockRuntime, Outcome, RunConfig, Runtime};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

struct NullSink;
impl EventSink for NullSink {
    fn emit(&mut self, _line: &str) {}
}

#[tokio::test(flavor = "current_thread")]
async fn crash_mid_stream_maps_to_outcome_crashed_ok_not_err() {
    let fixture = PathBuf::from("crates/agentic-runtime/tests/fixtures/mock_pipe_break.ndjson");
    let mock = MockRuntime::from_fixture_with_pipe_break(&fixture)
        .expect("MockRuntime::from_fixture_with_pipe_break");
    let runtime: Arc<dyn Runtime> = Arc::new(mock);

    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let runs_root = runs_root_tmp.path().to_path_buf();
    let run_id = "66666666-7777-4888-9999-aaaa00001111".to_string();

    let cfg = RunConfig {
        run_id: run_id.clone(),
        story_id: 19,
        story_yaml_bytes: b"id: 19\n".to_vec(),
        signer: "sandbox:mock@run-66666666".to_string(),
        build_config: json!({ "max_inner_loop_iterations": 5 }),
        runs_root: runs_root.clone(),
        repo_path: None,
        branch_name: None,
        prompt: "crash me".to_string(),
        event_sink: Box::new(NullSink),
    };

    // The load-bearing assertion: spawn_claude_session must return
    // Ok(RunOutcome{Crashed}), NOT Err. The crash is captured IN the
    // outcome because the session DID start (as opposed to a
    // pre-spawn ClaudeSpawn failure, which is Err).
    let outcome = runtime
        .spawn_claude_session(cfg)
        .await
        .expect("mid-stream crashes must surface as Ok(RunOutcome{Crashed}), not Err");

    // Outcome is Crashed and carries a non-empty error string.
    match &outcome.outcome {
        Outcome::Crashed { error } => {
            assert!(
                !error.trim().is_empty(),
                "Outcome::Crashed.error must be non-empty; got {error:?}"
            );
            assert!(
                error.to_lowercase().contains("pipe")
                    || error.to_lowercase().contains("broken")
                    || error.to_lowercase().contains("eof")
                    || error.to_lowercase().contains("unexpected"),
                "Crashed.error must name the pipe-break cause (contain `pipe`, `broken`, `eof`, or `unexpected`); \
                 got {error:?}"
            );
        }
        other => panic!(
            "outcome must be Outcome::Crashed for a mid-stream pipe break; got {other:?}"
        ),
    }

    // Trace file is readable (possibly truncated) and contains the
    // one well-formed fixture line captured before the break.
    let trace_files: Vec<std::path::PathBuf> = walk_files(&runs_root)
        .into_iter()
        .filter(|p| p.file_name().and_then(|n| n.to_str()) == Some("trace.ndjson"))
        .collect();
    assert_eq!(
        trace_files.len(),
        1,
        "expected exactly one trace.ndjson under {runs_root:?}; got {trace_files:?}"
    );
    let body = std::fs::read(&trace_files[0]).expect("read trace file");
    assert!(
        !body.is_empty(),
        "trace file must contain at least the pre-crash bytes; got empty file"
    );
    // The first fixture line (fully well-formed) must be on disk.
    let body_str = String::from_utf8_lossy(&body);
    assert!(
        body_str.contains("\"tool_call\"") && body_str.contains("\"i\":0"),
        "trace file must contain the pre-crash tool_call(i=0) line; got {body_str:?}"
    );
}

fn walk_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk_files(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}
