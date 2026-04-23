//! Story 19 acceptance test: the runtime passes `RunConfig.signer`
//! through to the recorder-written `runs` row verbatim. It does NOT
//! generate the signer itself. A nil or whitespace-only signer passed
//! to `spawn_claude_session` returns
//! `RuntimeError::InvalidConfig` before any subprocess is spawned.
//!
//! Justification (from stories/19.yml acceptance.tests[7]):
//!   Proves the signer composition with story 18: given a
//!   `RunConfig` whose `signer` resolves to
//!   `sandbox:claude-sonnet-4-6@run-abc123`, the `runs` row
//!   the runtime writes via `RunRecorder::start(...)`
//!   carries that exact string in its `signer` field. The
//!   runtime does NOT generate the signer itself — it
//!   accepts it in `RunConfig` and passes it through — so
//!   story 18's resolution chain (CLI flag → env var →
//!   fallback) remains the sole authority. A nil or
//!   whitespace-only signer passed to
//!   `spawn_claude_session` returns
//!   `RuntimeError::InvalidConfig` before any subprocess
//!   is spawned. Without this, either the runtime forges
//!   its own signer (breaking story 18's resolution
//!   contract) or downstream `runs` rows carry empty
//!   signers (breaking story 16's non-empty-signer
//!   invariant).
//!
//! Red today: compile-red. `MockRuntime`, `RunConfig`,
//! `RuntimeError::InvalidConfig`, and the `Runtime` trait do not yet
//! exist.

use agentic_runtime::{EventSink, MockRuntime, RunConfig, Runtime, RuntimeError};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

struct NullSink;
impl EventSink for NullSink {
    fn emit(&mut self, _line: &str) {}
}

fn cfg_with_signer(runs_root: PathBuf, signer: &str) -> RunConfig {
    RunConfig {
        run_id: "77777777-8888-4999-aaaa-bbbb00001111".to_string(),
        story_id: 19,
        story_yaml_bytes: b"id: 19\n".to_vec(),
        signer: signer.to_string(),
        build_config: json!({ "max_inner_loop_iterations": 5 }),
        runs_root,
        repo_path: None,
        branch_name: None,
        prompt: "hello".to_string(),
        event_sink: Box::new(NullSink),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn signer_is_passed_through_verbatim_to_runs_row() {
    let fixture =
        PathBuf::from("crates/agentic-runtime/tests/fixtures/mock_green_three_pairs.ndjson");
    let mock = MockRuntime::from_fixture(&fixture).expect("MockRuntime::from_fixture");
    let runtime: Arc<dyn Runtime> = Arc::new(mock);

    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let expected_signer = "sandbox:claude-sonnet-4-6@run-abc123";
    let cfg = cfg_with_signer(runs_root_tmp.path().to_path_buf(), expected_signer);

    let outcome = runtime
        .spawn_claude_session(cfg)
        .await
        .expect("spawn should succeed on green fixture with valid signer");

    // The MockRuntime exposes the store it wrote through so tests
    // can inspect the `runs` row. The load-bearing assertion is
    // that the signer field on the row equals the string we passed
    // in — no forging, no override, no mutation.
    let store = runtime
        .mock_store()
        .expect("MockRuntime must expose the backing store for test inspection");
    let rows = store
        .query("runs", &|doc| {
            doc["run_id"] == json!("77777777-8888-4999-aaaa-bbbb00001111")
        })
        .expect("query");
    assert_eq!(
        rows.len(),
        1,
        "exactly one runs row must exist; got {rows:?}"
    );
    assert_eq!(
        rows[0]["signer"],
        json!(expected_signer),
        "runs.signer must equal the RunConfig.signer we passed; got {:?}",
        rows[0]["signer"]
    );

    // Outcome's run_id also round-trips, as a sanity check that the
    // test is probing the right row.
    assert_eq!(
        outcome.run_id, "77777777-8888-4999-aaaa-bbbb00001111",
        "RunOutcome.run_id must echo RunConfig.run_id"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn empty_or_whitespace_signer_returns_invalid_config_before_spawn() {
    let fixture =
        PathBuf::from("crates/agentic-runtime/tests/fixtures/mock_green_three_pairs.ndjson");

    for bad in &["", "   ", "\t\n"] {
        let mock = MockRuntime::from_fixture(&fixture).expect("MockRuntime::from_fixture");
        let runtime: Arc<dyn Runtime> = Arc::new(mock);
        let runs_root_tmp = TempDir::new().expect("runs root tempdir");
        let cfg = cfg_with_signer(runs_root_tmp.path().to_path_buf(), bad);

        let result = runtime.spawn_claude_session(cfg).await;
        match result {
            Err(RuntimeError::InvalidConfig { field }) => {
                assert_eq!(
                    field, "signer",
                    "InvalidConfig must name the `signer` field; got {field:?} for input {bad:?}"
                );
            }
            other => panic!(
                "nil/whitespace signer {bad:?} must produce RuntimeError::InvalidConfig {{ field: \"signer\" }} before any subprocess is spawned; got {other:?}"
            ),
        }

        // No runs directory was created — the error fired before any
        // side effect.
        let any_files = std::fs::read_dir(runs_root_tmp.path())
            .map(|it| it.count())
            .unwrap_or(0);
        assert_eq!(
            any_files, 0,
            "no files must be created under runs_root when InvalidConfig fires pre-spawn; found {any_files}"
        );
    }
}
