//! Story 16 acceptance test: `trace_ndjson_path` is `<run-id>/trace.ndjson`
//! (relative, forward slashes, no `..`) and path-injection run ids are
//! rejected with a typed error BEFORE any file is created.
//!
//! Justification (from stories/16.yml acceptance.tests[8]):
//!   Proves the trace-path discipline: `RunRecorder::start` configured
//!   with a runs root at `<root>` produces a `trace_ndjson_path`
//!   equal to `<run-id>/trace.ndjson` (relative path, forward
//!   slashes, no leading slash, no `..` components, no absolute
//!   path), and the actual file is written at
//!   `<root>/<run-id>/trace.ndjson` on disk. A `run_id` constructed
//!   to attempt path traversal (`../../etc/passwd`-shaped, or
//!   containing `\` / `/` / null bytes) is rejected by the recorder
//!   with a typed `RunRecorderError::InvalidRunId` before any file
//!   is created. Without this, the filesystem-as-trace-backing
//!   decision leaks into a path-injection vulnerability.
//!
//! Red today: natural. `RunRecorderError::InvalidRunId` does not yet
//! exist; neither does the typed validation path.

use agentic_runtime::{Outcome, RunRecorder, RunRecorderConfig, RunRecorderError};
use agentic_store::{MemStore, Store};
use serde_json::json;
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn trace_ndjson_path_is_relative_under_runs_root_with_forward_slashes() {
    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let runs_root = runs_root_tmp.path().to_path_buf();
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let run_id = "abcdef01-2345-4678-89ab-cdef01234567".to_string();
    let cfg = RunRecorderConfig {
        store: Arc::clone(&store),
        runs_root: runs_root.clone(),
        run_id: run_id.clone(),
        story_id: 15,
        story_yaml_bytes: b"id: 15\n".to_vec(),
        signer: "sandbox:stub@run-valid".to_string(),
        build_config: json!({}),
    };

    let recorder = RunRecorder::start(cfg).expect("start should succeed on a valid run_id");
    recorder
        .finish(Outcome::Green {
            signing_run_id: "stub-signing-1".to_string(),
        })
        .expect("finish should succeed");

    let rows = store
        .query("runs", &|doc| doc["run_id"] == json!(run_id))
        .expect("query");
    assert_eq!(rows.len(), 1);
    let trace_rel = rows[0]["trace_ndjson_path"]
        .as_str()
        .expect("trace_ndjson_path must be a string");

    // Exact shape: <run-id>/trace.ndjson, forward slashes, no leading
    // slash, no `..`, no backslashes.
    let expected = format!("{run_id}/trace.ndjson");
    assert_eq!(
        trace_rel, expected,
        "trace_ndjson_path must be exactly {expected:?}; got {trace_rel:?}"
    );
    assert!(
        !trace_rel.starts_with('/') && !trace_rel.starts_with('\\'),
        "trace_ndjson_path must be relative; got {trace_rel:?}"
    );
    assert!(
        !trace_rel.contains(".."),
        "trace_ndjson_path must not contain `..`; got {trace_rel:?}"
    );
    assert!(
        !trace_rel.contains('\\'),
        "trace_ndjson_path must use forward slashes; got {trace_rel:?}"
    );

    // The file exists at <root>/<run-id>/trace.ndjson on disk.
    let abs = runs_root.join(trace_rel);
    assert!(abs.exists(), "trace file must exist at {abs:?}");
}

#[test]
fn path_traversal_run_ids_are_rejected_before_any_file_is_created() {
    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let runs_root = runs_root_tmp.path().to_path_buf();
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let hostile_ids = [
        "../escape",
        "../../etc/passwd",
        "a/b",
        "a\\b",
        "contains\0null",
    ];

    for hostile in &hostile_ids {
        let cfg = RunRecorderConfig {
            store: Arc::clone(&store),
            runs_root: runs_root.clone(),
            run_id: (*hostile).to_string(),
            story_id: 15,
            story_yaml_bytes: b"id: 15\n".to_vec(),
            signer: "sandbox:stub@run-hostile".to_string(),
            build_config: json!({}),
        };

        let err = RunRecorder::start(cfg)
            .err()
            .unwrap_or_else(|| panic!("hostile run_id {hostile:?} must be rejected"));

        match err {
            RunRecorderError::InvalidRunId { value } => {
                assert_eq!(
                    value, *hostile,
                    "InvalidRunId must carry the offending id verbatim"
                );
            }
            other => panic!(
                "hostile run_id {hostile:?} must be rejected with \
                 RunRecorderError::InvalidRunId; got {other:?}"
            ),
        }

        // No row in the store.
        let rows = store
            .query("runs", &|doc| doc["run_id"] == json!(*hostile))
            .expect("query");
        assert!(
            rows.is_empty(),
            "no runs row may be written for hostile id {hostile:?}"
        );
    }

    // The <runs_root> directory is still empty — no file or
    // subdirectory was created for any hostile id.
    let entries: Vec<_> = fs::read_dir(&runs_root)
        .expect("read runs_root")
        .flatten()
        .collect();
    assert!(
        entries.is_empty(),
        "hostile run_ids must not create any filesystem entry under {runs_root:?}; got {entries:?}"
    );
}
