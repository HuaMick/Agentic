//! Story 19 acceptance test: `#[ignore]`-gated integration test that
//! proves the end-to-end path against real `claude` subscription auth
//! when available. Default `cargo test` does not run this; invoke with
//! `cargo test -p agentic-runtime -- --ignored`.
//!
//! Justification (from stories/19.yml acceptance.tests[9]):
//!   Proves the end-to-end path against real `claude`
//!   subscription auth when available: a `#[ignore]`-gated
//!   integration test constructs a real
//!   `ClaudeCodeRuntime`, calls
//!   `spawn_claude_session` with a trivial one-turn
//!   prompt and a budget of 1, and asserts a non-empty
//!   NDJSON trace file plus a `runs` row with
//!   `outcome: green`. The test is
//!   `#[ignore]`d so it does not run in default
//!   `cargo test` — invoking it requires
//!   `cargo test -- --ignored`. It exists so the
//!   real-claude path has a pinnable artefact, not as a
//!   CI gate; the mock path covers CI. Without this, the
//!   claim "the runtime works against real claude" has no
//!   repeatable ceremony and every manual operator has to
//!   reinvent one.
//!
//! Note on `#[ignore]`: this attribute is mandated by the story's
//! own justification. The test-builder authoring rules forbid
//! `#[ignore]` as a means of suppressing a scaffold that should fail
//! — but here the story explicitly names `#[ignore]` as the
//! opt-in ceremony for the real-claude path. `cargo check` still
//! attempts to compile the scaffold, and compile-red fires as usual
//! when `ClaudeCodeRuntime` / `RunConfig` / `Outcome` do not exist.
//!
//! Red today: compile-red. `ClaudeCodeRuntime::new`,
//! `spawn_claude_session`, `RunConfig`, `RunOutcome`, `Outcome`, and
//! the `Runtime` trait do not yet exist.

use agentic_runtime::{ClaudeCodeRuntime, EventSink, Outcome, RunConfig, Runtime};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

struct NullSink;
impl EventSink for NullSink {
    fn emit(&mut self, _line: &str) {}
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "real-claude path: run with `cargo test -p agentic-runtime -- --ignored`"]
async fn real_claude_one_turn_prompt_produces_green_outcome_and_trace() {
    let runtime = ClaudeCodeRuntime::new().expect("ClaudeCodeRuntime::new");
    let rt: Arc<dyn Runtime> = Arc::new(runtime);

    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let runs_root = runs_root_tmp.path().to_path_buf();
    let run_id = "99999999-aaaa-4bbb-cccc-ddddeeeeffff".to_string();

    let cfg = RunConfig {
        run_id: run_id.clone(),
        story_id: 19,
        story_yaml_bytes: b"id: 19\n".to_vec(),
        signer: "sandbox:claude-sonnet-4-6@run-99999999".to_string(),
        build_config: json!({ "max_inner_loop_iterations": 1 }),
        runs_root: runs_root.clone(),
        repo_path: None,
        branch_name: None,
        prompt: "Say 'hello' in one word and stop.".to_string(),
        event_sink: Box::new(NullSink),
    };

    let outcome = rt
        .spawn_claude_session(cfg)
        .await
        .expect("real claude spawn must succeed");

    assert!(
        matches!(outcome.outcome, Outcome::Green { .. }),
        "real-claude outcome must be Green; got {:?}",
        outcome.outcome
    );

    // At least one trace file under runs_root contains NDJSON.
    let traces: Vec<_> = walk_files(&runs_root)
        .into_iter()
        .filter(|p| p.file_name().and_then(|n| n.to_str()) == Some("trace.ndjson"))
        .collect();
    assert_eq!(
        traces.len(),
        1,
        "exactly one trace.ndjson must exist under {runs_root:?}; got {traces:?}"
    );
    let body = std::fs::read_to_string(&traces[0]).expect("read trace");
    assert!(
        !body.trim().is_empty(),
        "trace file must be non-empty after a real-claude run; got empty file"
    );
    // Every non-empty line must parse as JSON.
    for (i, line) in body.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let _: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("trace line {i} is not valid JSON: {line:?} ({e})"));
    }
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
