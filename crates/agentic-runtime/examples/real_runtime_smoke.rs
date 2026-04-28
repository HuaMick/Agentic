//! Smoke test for ClaudeCodeRuntime: constructs a real ClaudeCodeRuntime,
//! exercises the error path if the claude binary is not found.
//!
//! Usage:
//!   cargo run -p agentic-runtime --example real_runtime_smoke
//!
//! To test the error path when claude is missing:
//!   PATH="" cargo run -p agentic-runtime --example real_runtime_smoke
//!
//! Environment variables (optional):
//!   AGENTIC_RUNS_ROOT: root directory for trace files (default: /tmp/agentic-runs)
//!   AGENTIC_SIGNER: signer identity (default: sandbox:real@bf1bb09e-c0c8-4bc6-a6bd-214449e6fc5b)

use agentic_runtime::{ClaudeCodeRuntime, EventSink, RunConfig, Runtime, RuntimeError};
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
    let runs_root = std::env::var("AGENTIC_RUNS_ROOT")
        .unwrap_or_else(|_| "/tmp/agentic-runs".to_string());
    let signer = std::env::var("AGENTIC_SIGNER")
        .unwrap_or_else(|_| "sandbox:real@bf1bb09e-c0c8-4bc6-a6bd-214449e6fc5b".to_string());

    // Attempt to construct ClaudeCodeRuntime.
    // This is where the error path is exercised if claude is missing.
    match ClaudeCodeRuntime::new() {
        Ok(runtime) => {
            println!("ClaudeCodeRuntime constructed successfully");
            let runtime: Arc<dyn Runtime> = Arc::new(runtime);

            // Create an event sink.
            let sink = CollectingSink::default();

            // Create runs root directory if it doesn't exist.
            if let Err(e) = std::fs::create_dir_all(&runs_root) {
                eprintln!("Warning: failed to create runs root directory: {}", e);
            }

            // Build the RunConfig.
            let run_id = "55555555-6666-4777-8888-999900001111".to_string();
            let cfg = RunConfig {
                run_id: run_id.clone(),
                story_id: 15,
                story_yaml_bytes: b"id: 15\n".to_vec(),
                signer,
                build_config: json!({ "max_inner_loop_iterations": 1 }),
                runs_root: PathBuf::from(&runs_root),
                repo_path: None,
                branch_name: None,
                prompt: "smoke test prompt".to_string(),
                event_sink: Box::new(sink),
            };

            // Note: For Phase 0, the real runtime spawn_claude_session returns a stub outcome.
            // This example demonstrates the construction path; the subprocess driver is
            // deferred to the agentic-stream crate.
            match runtime.spawn_claude_session(cfg).await {
                Ok(outcome) => {
                    println!("Outcome: {:?}", outcome);
                    println!("Run ID: {}", outcome.run_id);
                    println!("Runs row ID: {}", outcome.runs_row_id);
                }
                Err(e) => {
                    eprintln!("spawn_claude_session failed: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            // This is the expected error path when claude is missing.
            eprintln!("ClaudeCodeRuntime construction failed: {:?}", e);
            if let RuntimeError::ClaudeSpawn { reason } = e {
                eprintln!("Reason: {:?}", reason);
            }
            std::process::exit(1);
        }
    }
}
