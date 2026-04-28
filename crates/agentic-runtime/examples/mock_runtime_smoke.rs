//! Smoke test for MockRuntime: wraps a fixture NDJSON file,
//! builds a RunConfig, calls spawn_claude_session, and prints the outcome.
//!
//! Usage:
//!   cargo run -p agentic-runtime --example mock_runtime_smoke
//!
//! Environment variables (optional):
//!   AGENTIC_FIXTURE: path to fixture NDJSON file (default: /tmp/mock-green.ndjson)
//!   AGENTIC_RUNS_ROOT: root directory for trace files (default: /tmp/agentic-runs)
//!   AGENTIC_SIGNER: signer identity (default: sandbox:mock@bf1bb09e-c0c8-4bc6-a6bd-214449e6fc5b)

use agentic_runtime::{EventSink, MockRuntime, RunConfig, Runtime};
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// A simple event sink that collects emitted lines.
#[derive(Default, Clone)]
struct CollectingSink {
    lines: Arc<Mutex<Vec<String>>>,
}

impl EventSink for CollectingSink {
    fn emit(&mut self, line: &str) {
        self.lines.lock().unwrap().push(line.to_string());
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Read environment variable overrides.
    let fixture_path = std::env::var("AGENTIC_FIXTURE")
        .unwrap_or_else(|_| "/tmp/mock-green.ndjson".to_string());
    let runs_root = std::env::var("AGENTIC_RUNS_ROOT")
        .unwrap_or_else(|_| "/tmp/agentic-runs".to_string());
    let signer = std::env::var("AGENTIC_SIGNER")
        .unwrap_or_else(|_| "sandbox:mock@bf1bb09e-c0c8-4bc6-a6bd-214449e6fc5b".to_string());

    // Create the mock runtime from the fixture.
    let fixture_path_buf = PathBuf::from(&fixture_path);
    let mock = match MockRuntime::from_fixture(&fixture_path_buf) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load fixture: {:?}", e);
            std::process::exit(1);
        }
    };
    let runtime: Arc<dyn Runtime> = Arc::new(mock);

    // Create an event sink to collect output.
    let sink = CollectingSink::default();
    let sink_lines = Arc::clone(&sink.lines);

    // Create runs root directory if it doesn't exist.
    if let Err(e) = std::fs::create_dir_all(&runs_root) {
        eprintln!("Failed to create runs root directory: {}", e);
        std::process::exit(1);
    }

    // Build the RunConfig.
    let run_id = "44444444-5555-4666-8777-888899990000".to_string();
    let cfg = RunConfig {
        run_id: run_id.clone(),
        story_id: 15,
        story_yaml_bytes: b"id: 15\n".to_vec(),
        signer,
        build_config: json!({ "max_inner_loop_iterations": 3 }),
        runs_root: PathBuf::from(&runs_root),
        repo_path: None,
        branch_name: None,
        prompt: "smoke test prompt".to_string(),
        event_sink: Box::new(sink),
    };

    // Spawn the session.
    println!("Spawning MockRuntime session with fixture: {}", fixture_path);
    let outcome = match runtime.spawn_claude_session(cfg).await {
        Ok(o) => o,
        Err(e) => {
            eprintln!("spawn_claude_session failed: {:?}", e);
            std::process::exit(1);
        }
    };

    // Print the outcome.
    println!("Outcome: {:?}", outcome);
    println!("Run ID: {}", outcome.run_id);
    println!("Runs row ID: {}", outcome.runs_row_id);
    println!("Outcome variant: {:?}", outcome.outcome);

    // Print emitted lines.
    let lines = sink_lines.lock().unwrap();
    println!("Emitted {} lines from fixture", lines.len());
    for (i, line) in lines.iter().enumerate() {
        println!("  [{}]: {}", i, line);
    }

    // Print trace file path.
    let trace_path = format!("{}/{}/trace.ndjson", runs_root, run_id);
    if std::path::Path::new(&trace_path).exists() {
        match std::fs::read_to_string(&trace_path) {
            Ok(trace_body) => {
                println!("Trace file ({}): {} bytes, {} lines", trace_path, trace_body.len(), trace_body.lines().count());
            }
            Err(e) => {
                eprintln!("Failed to read trace file: {}", e);
            }
        }
    }
}
