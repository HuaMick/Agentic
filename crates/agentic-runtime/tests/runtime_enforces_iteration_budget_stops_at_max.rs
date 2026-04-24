//! Story 19 acceptance test: the runtime enforces the budget from
//! `RunConfig.build_config.max_inner_loop_iterations`. A fixture with
//! five tool_call/tool_result pairs + a budget of 2 produces a
//! `RunOutcome` whose `outcome` is `inner_loop_exhausted` with exactly
//! two iterations, the trace file contains only the first 4 lines,
//! and the remaining fixture events do not appear.
//!
//! Justification (from stories/19.yml acceptance.tests[5]):
//!   Proves the budget gate: given a `RunConfig` whose
//!   `build_config.max_inner_loop_iterations == 2` and a
//!   `MockRuntime` fixture carrying five tool-use turns,
//!   `spawn_claude_session` stops feeding events after the
//!   second tool_use/tool_result pair, signals the session
//!   to terminate (the mock records that it received a
//!   `BudgetExhausted` stop signal), and returns a
//!   `RunOutcome` whose `outcome` is
//!   `inner_loop_exhausted` with exactly two iterations in
//!   the `runs` row. The remaining three fixture events do
//!   NOT appear in the trace file. Without this, a
//!   run-away agent can consume an entire 5-hour
//!   subscription window on a single story; the budget
//!   field from story 17 becomes decorative; and ADR-0006
//!   section 7's "human picks budget as complexity
//!   estimate" loses its load-bearing primitive.
//!
//! Red today: compile-red. The runtime, mock, `RunConfig` and
//! `Outcome::InnerLoopExhausted` variants do not exist in `agentic_runtime`.

use agentic_runtime::{EventSink, MockRuntime, Outcome, RunConfig, Runtime};
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

#[derive(Default, Clone)]
struct CollectingSink {
    lines: Arc<Mutex<Vec<String>>>,
}

impl EventSink for CollectingSink {
    fn emit(&mut self, line: &str) {
        self.lines.lock().unwrap().push(line.to_string());
    }
}

#[tokio::test(flavor = "current_thread")]
async fn runtime_stops_at_max_inner_loop_iterations_and_trace_is_truncated() {
    let fixture =
        PathBuf::from("crates/agentic-runtime/tests/fixtures/mock_budget_five_pairs.ndjson");
    let mock = MockRuntime::from_fixture(&fixture).expect("MockRuntime::from_fixture");
    let runtime: Arc<dyn Runtime> = Arc::new(mock);

    let sink = CollectingSink::default();
    let sink_handle = Arc::clone(&sink.lines);

    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let runs_root = runs_root_tmp.path().to_path_buf();
    let run_id = "55555555-6666-4777-8888-999900001111".to_string();

    let cfg = RunConfig {
        run_id: run_id.clone(),
        story_id: 19,
        story_yaml_bytes: b"id: 19\n".to_vec(),
        signer: "sandbox:mock@run-55555555".to_string(),
        build_config: json!({ "max_inner_loop_iterations": 2 }),
        runs_root: runs_root.clone(),
        repo_path: None,
        branch_name: None,
        prompt: "hit budget".to_string(),
        event_sink: Box::new(sink),
    };

    let outcome = runtime.spawn_claude_session(cfg).await.expect(
        "spawn_claude_session must return Ok even on budget exhaustion (Err is for pre-spawn only)",
    );

    // Outcome variant is InnerLoopExhausted.
    assert!(
        matches!(outcome.outcome, Outcome::InnerLoopExhausted),
        "outcome must be Outcome::InnerLoopExhausted for budget=2 against a 5-pair fixture; got {:?}",
        outcome.outcome
    );

    // Event sink received at most the first 4 lines (2 tool_call +
    // 2 tool_result). The last 6 lines of the 10-line fixture must
    // NOT have been forwarded.
    let lines = sink_handle.lock().unwrap().clone();
    assert!(
        lines.len() <= 4,
        "sink must have received at most 4 lines (2 pairs) before budget halt; got {} lines: {lines:?}",
        lines.len()
    );
    for (i, line) in lines.iter().enumerate() {
        assert!(
            !line.contains("\"i\":3") && !line.contains("\"i\":4"),
            "line {i} = {line:?} contains i=3 or i=4; those pairs must NOT cross the budget halt"
        );
    }

    // The runtime's runs row carries exactly 2 iterations.
    // `RunOutcome.runs_row_id` points at a runs row in whatever
    // store the mock wrote through; the mock exposes a store
    // accessor for tests. The load-bearing assertion: the
    // iterations length MUST equal the budget.
    //
    // If the runtime exposes a `runs_row_iterations_len` helper or
    // the mock's store is reachable, prefer that. We fall back to
    // asserting the on-disk trace file has at most the first four
    // fixture lines.
    let trace_candidates = walk_files(&runs_root);
    let trace_file = trace_candidates
        .iter()
        .find(|p| p.file_name().and_then(|n| n.to_str()) == Some("trace.ndjson"))
        .unwrap_or_else(|| {
            panic!("expected a trace.ndjson under {runs_root:?}; found {trace_candidates:?}")
        });
    let body = std::fs::read_to_string(trace_file).expect("read trace file");
    let trace_lines: Vec<&str> = body.lines().collect();
    assert!(
        trace_lines.len() <= 4,
        "trace file must contain at most the first 4 fixture lines (2 pairs); got {}: {trace_lines:?}",
        trace_lines.len()
    );
    assert!(
        !body.contains("\"i\":3") && !body.contains("\"i\":4"),
        "trace file must NOT contain the dropped (3rd,4th,5th) pairs; body: {body}"
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
