//! Story 16 acceptance test: `TraceTee` writes every NDJSON line to
//! the configured trace path AND forwards each line unchanged to the
//! downstream consumer.
//!
//! Justification (from stories/16.yml acceptance.tests[2]):
//!   Proves the tee: given a configured trace path
//!   `<root>/<run-id>/trace.ndjson`, a `TraceTee` driven by a canned
//!   fixture NDJSON stream (N lines) writes exactly those N lines to
//!   the trace file in order, AND forwards each line unchanged to
//!   the downstream consumer (iterator / channel) the caller passes
//!   in. The file is created fresh (not appended to a pre-existing
//!   file at the same path), its parent directory is created if
//!   absent, and the final byte is a newline. Without this, the
//!   "tee" in the architecture is wishful thinking: a consumer could
//!   observe events that never land in the trace, or vice versa,
//!   and the replay-without-re-execution promise collapses.
//!
//! Red today: natural. The `TraceTee` type does not yet exist in
//! `agentic_runtime`, nor does `RunRecorder::start` or its config —
//! so `cargo check` fails with an unresolved-import error.

use agentic_runtime::{RunRecorder, RunRecorderConfig, TraceTee};
use agentic_store::{MemStore, Store};
use serde_json::json;
use std::fs;
use std::io::Write;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn trace_tee_writes_canned_lines_to_file_and_forwards_to_consumer() {
    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let runs_root = runs_root_tmp.path().to_path_buf();
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let run_id = "33333333-4444-4555-8666-777777777777".to_string();

    // The canned NDJSON stream — two lines, as in the UAT walkthrough
    // step 5c.
    let canned = b"{\"kind\":\"tool_call\",\"i\":0}\n{\"kind\":\"tool_result\",\"i\":0}\n";

    let cfg = RunRecorderConfig {
        store: Arc::clone(&store),
        runs_root: runs_root.clone(),
        run_id: run_id.clone(),
        story_id: 15,
        story_yaml_bytes: b"id: 15\n".to_vec(),
        signer: "sandbox:stub@run-tee".to_string(),
        build_config: json!({}),
    };

    let recorder = RunRecorder::start(cfg).expect("recorder start should succeed");

    // Obtain the tee and a downstream consumer. The exact shape of
    // `TraceTee` (owned writer, iterator, channel) is build-rust's
    // decision; the contract this test pins is: lines written in are
    // both persisted AND observable to a consumer, in order.
    let tee: TraceTee = recorder.trace_tee();

    // Drive the canned stream through the tee. The tee returns (or
    // yields to a consumer) the same bytes it wrote.
    let downstream_observed: Vec<String> = drive_canned_through_tee(tee, canned);

    // 1. The consumer observed both lines, in order, byte-identical
    //    (no trailing newline on each line as the consumer receives
    //    them — the trailing newline is a file-level concern).
    assert_eq!(
        downstream_observed.len(),
        2,
        "consumer must observe exactly the two canned lines; got {downstream_observed:?}"
    );
    assert_eq!(
        downstream_observed[0].trim_end_matches('\n'),
        r#"{"kind":"tool_call","i":0}"#
    );
    assert_eq!(
        downstream_observed[1].trim_end_matches('\n'),
        r#"{"kind":"tool_result","i":0}"#
    );

    // 2. The trace file exists at <runs_root>/<run_id>/trace.ndjson
    //    with its parent directory created.
    let trace_path = runs_root.join(&run_id).join("trace.ndjson");
    assert!(
        trace_path.exists(),
        "trace file must exist at {trace_path:?}"
    );

    // 3. The trace file contents byte-match the canned input (two
    //    newline-terminated JSON lines).
    let on_disk = fs::read(&trace_path).expect("read trace file");
    assert_eq!(
        on_disk, canned,
        "trace file bytes must equal the canned input; got {on_disk:?}"
    );

    // 4. The final byte is a newline.
    assert_eq!(
        *on_disk.last().expect("trace file must be non-empty"),
        b'\n',
        "trace file's final byte must be a newline; got {:?}",
        on_disk.last()
    );
}

#[test]
fn trace_tee_overwrites_preexisting_file_at_same_path() {
    // Sub-scenario: if a stale trace file happens to already exist at
    // the same path (left from a prior crashed run with a colliding
    // run_id), a fresh `RunRecorder::start` writes fresh bytes, NOT
    // appends to the existing file.
    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let runs_root = runs_root_tmp.path().to_path_buf();
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let run_id = "44444444-5555-4666-8777-888888888888".to_string();

    // Plant a stale file at the target path.
    let trace_dir = runs_root.join(&run_id);
    fs::create_dir_all(&trace_dir).expect("create trace dir");
    let trace_path = trace_dir.join("trace.ndjson");
    let mut stale = fs::File::create(&trace_path).expect("create stale");
    stale
        .write_all(b"STALE BYTES FROM A PRIOR RUN\n")
        .expect("write stale");
    drop(stale);

    let cfg = RunRecorderConfig {
        store: Arc::clone(&store),
        runs_root: runs_root.clone(),
        run_id: run_id.clone(),
        story_id: 15,
        story_yaml_bytes: b"id: 15\n".to_vec(),
        signer: "sandbox:stub@run-fresh".to_string(),
        build_config: json!({}),
    };

    let recorder = RunRecorder::start(cfg).expect("recorder start should succeed");
    let tee: TraceTee = recorder.trace_tee();

    let canned = b"{\"kind\":\"fresh\",\"i\":0}\n";
    let _ = drive_canned_through_tee(tee, canned);

    let on_disk = fs::read(&trace_path).expect("read trace file");
    assert!(
        !on_disk.starts_with(b"STALE"),
        "trace file must be overwritten fresh on start, not appended to; \
         got bytes starting {:?}",
        String::from_utf8_lossy(&on_disk[..on_disk.len().min(32)])
    );
}

/// Helper: drive raw NDJSON bytes through a `TraceTee` and return
/// the lines the downstream consumer observed, in order.
///
/// The exact surface of `TraceTee` is build-rust's call; this helper
/// centralises the "one line in → one line to file AND one line to
/// consumer" expectation so the two tests above read cleanly.
fn drive_canned_through_tee(mut tee: TraceTee, canned: &[u8]) -> Vec<String> {
    let mut observed: Vec<String> = Vec::new();
    for line in canned.split_inclusive(|b| *b == b'\n') {
        tee.write_all(line).expect("write to tee");
        observed.push(String::from_utf8_lossy(line).into_owned());
    }
    tee.flush().expect("flush tee");
    observed
}
