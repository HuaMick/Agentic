//! Story 16 acceptance test: the `runs` table round-trips a structured
//! run row through `Store::append` + `Store::query`.
//!
//! Justification (from stories/16.yml acceptance.tests[0]):
//!   Proves the `runs` table accepts the full documented row shape:
//!   `run_id` (uuid v4 string), `story_id` (int), `story_yaml_snapshot`
//!   (64-char lowercase hex SHA256), `signer` (non-empty string),
//!   `started_at` and `ended_at` (RFC3339 UTC), `build_config`
//!   (arbitrary JSON object), `outcome` (one of `green` /
//!   `inner_loop_exhausted` / `crashed`), `iterations` (JSON array,
//!   possibly empty), `branch_state` (JSON object with `start_sha`,
//!   `end_sha`, `commits`, `merged`, `merge_shas`), and
//!   `trace_ndjson_path` (filesystem path relative to the runs root)
//!   round-trips through `Store::append` + `Store::query(table="runs",
//!   filter={run_id=<id>})` byte-identical on the value fields a
//!   downstream reader cares about. Without this, every later
//!   consumer (a future dashboard, `agentic story build`, UAT readers)
//!   would each re-discover the row shape by observation and we'd
//!   ship twelve slightly different readers.
//!
//! Red today: natural. The `RunRecorder` type the test imports from
//! `agentic_runtime` does not yet exist, so `cargo check` fails with
//! an unresolved-import error — the compile-red path named in
//! ADR-0005. When build-rust lands the recorder, the test becomes the
//! runtime check that the row it writes is parseable back into the
//! documented shape.

use agentic_runtime::{IterationSummary, Outcome, RunRecorder, RunRecorderConfig};
use agentic_store::{MemStore, Store};
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn runs_table_round_trips_structured_run_row() {
    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let story_yaml_bytes = b"id: 15\ntitle: fixture\n".to_vec();
    let run_id = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee".to_string();

    let cfg = RunRecorderConfig {
        store: Arc::clone(&store),
        runs_root: runs_root_tmp.path().to_path_buf(),
        run_id: run_id.clone(),
        story_id: 15,
        story_yaml_bytes,
        signer: "sandbox:stub-subprocess@run-aaa111".to_string(),
        build_config: json!({ "max_inner_loop_iterations": 3 }),
    };

    let recorder = RunRecorder::start(cfg).expect("recorder should start with valid config");

    recorder
        .record_iteration(IterationSummary {
            i: 0,
            started_at: "2026-04-23T00:00:00Z".to_string(),
            ended_at: "2026-04-23T00:00:01Z".to_string(),
            probes: vec![],
            verdict: None,
            error: None,
        })
        .expect("record_iteration should succeed");

    recorder
        .finish(Outcome::Green {
            signing_run_id: "stub-signing-1".to_string(),
        })
        .expect("finish should succeed on clean wiring");

    // Round-trip: the row MUST be retrievable from the `runs` table by
    // filtering on run_id, and every documented field must survive
    // byte-identical (on the value fields a downstream reader cares
    // about — timestamps are RFC3339 strings, not parsed).
    let rows = store
        .query("runs", &|doc| doc["run_id"] == json!(run_id))
        .expect("query should succeed");

    assert_eq!(
        rows.len(),
        1,
        "exactly one row should exist for run_id={run_id}; got {rows:?}"
    );
    let row = &rows[0];

    // Required top-level keys, per the row-shape reference in story 16
    // guidance.
    for key in &[
        "run_id",
        "story_id",
        "story_yaml_snapshot",
        "signer",
        "started_at",
        "ended_at",
        "build_config",
        "outcome",
        "iterations",
        "branch_state",
        "trace_ndjson_path",
    ] {
        assert!(
            row.get(*key).is_some(),
            "runs row missing required field {key:?}; row was {row}"
        );
    }

    // Scalar fields byte-identical where a downstream reader cares.
    assert_eq!(row["run_id"], json!(run_id));
    assert_eq!(row["story_id"], json!(15));
    assert_eq!(row["signer"], json!("sandbox:stub-subprocess@run-aaa111"));
    assert_eq!(row["outcome"], json!("green"));

    // story_yaml_snapshot is a 64-char lowercase hex SHA256 string.
    let snapshot = row["story_yaml_snapshot"]
        .as_str()
        .expect("story_yaml_snapshot must be a string");
    assert_eq!(
        snapshot.len(),
        64,
        "story_yaml_snapshot must be 64 hex chars; got {snapshot:?}"
    );
    assert!(
        snapshot.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "story_yaml_snapshot must be lowercase hex; got {snapshot:?}"
    );

    // iterations is a JSON array (possibly empty). We recorded one entry.
    let iterations = row["iterations"]
        .as_array()
        .expect("iterations must be a JSON array");
    assert_eq!(iterations.len(), 1, "one recorded iteration must appear");

    // build_config round-trips byte-identical.
    assert_eq!(row["build_config"], json!({ "max_inner_loop_iterations": 3 }));

    // trace_ndjson_path is a non-empty string relative to the runs
    // root (no leading slash, no `..`).
    let trace_path = row["trace_ndjson_path"]
        .as_str()
        .expect("trace_ndjson_path must be a string");
    assert!(
        !trace_path.is_empty(),
        "trace_ndjson_path must be non-empty"
    );
    assert!(
        !trace_path.starts_with('/') && !trace_path.starts_with('\\'),
        "trace_ndjson_path must be relative (no leading slash); got {trace_path:?}"
    );
    assert!(
        !trace_path.contains(".."),
        "trace_ndjson_path must not contain `..`; got {trace_path:?}"
    );

    // branch_state is a JSON object with the five documented sub-fields.
    let branch_state: &Value = &row["branch_state"];
    assert!(
        branch_state.is_object(),
        "branch_state must be a JSON object; got {branch_state}"
    );
    for sub in &["start_sha", "end_sha", "commits", "merged", "merge_shas"] {
        assert!(
            branch_state.get(*sub).is_some(),
            "branch_state missing required sub-field {sub:?}; got {branch_state}"
        );
    }
}
