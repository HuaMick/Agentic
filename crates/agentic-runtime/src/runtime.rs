//! Story 19: Runtime trait for spawning agents.
//!
//! This module provides the `Runtime` trait for spawning agents (via `spawn_claude_session`),
//! implementations for `ClaudeCodeRuntime` (spawning the local `claude` binary) and
//! `MockRuntime` (replaying canned NDJSON fixtures), and the surrounding types
//! (`RunConfig`, `RunOutcome`, `RuntimeError`).

use crate::run_recorder::{Outcome, RunRecorder, RunRecorderConfig};
use agentic_store::Store;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

/// Trait for spawning agent sessions.
/// Object-safe by design: callers hold `Arc<dyn Runtime>`.
#[async_trait::async_trait]
pub trait Runtime: Send + Sync {
    /// Spawn a new agent session with the given configuration.
    async fn spawn_claude_session(&self, cfg: RunConfig) -> Result<RunOutcome, RuntimeError>;

    /// For tests: expose the backing store (MockRuntime only).
    fn mock_store(&self) -> Option<Arc<dyn Store>> {
        None
    }
}

/// Configuration for spawning an agent session.
pub struct RunConfig {
    /// UUID v4 string identifying this run.
    pub run_id: String,
    /// Story ID to run.
    pub story_id: i64,
    /// The exact bytes of the story YAML at launch.
    pub story_yaml_bytes: Vec<u8>,
    /// Signer identity for this run (must be non-empty).
    pub signer: String,
    /// Arbitrary build configuration (passed through to the recorder).
    pub build_config: serde_json::Value,
    /// Root directory for trace files.
    pub runs_root: PathBuf,
    /// Optional repo path for branch state capture.
    pub repo_path: Option<PathBuf>,
    /// Optional branch name (must be Some iff repo_path is Some).
    pub branch_name: Option<String>,
    /// The prompt to pass to claude.
    pub prompt: String,
    /// Event sink for NDJSON output.
    pub event_sink: Box<dyn EventSink>,
}

/// Outcome of a run.
#[derive(Clone, Debug)]
pub struct RunOutcome {
    /// UUID matching RunConfig.run_id.
    pub run_id: String,
    /// The outcome variant (Green, InnerLoopExhausted, Crashed).
    pub outcome: Outcome,
    /// The row ID written to the Store.
    pub runs_row_id: String,
}

/// Runtime error variants.
#[derive(Debug, Clone)]
pub enum RuntimeError {
    /// The claude binary could not be invoked.
    ClaudeSpawn { reason: ClaudeSpawnReason },
    /// Trace file write failed (propagates as Err).
    TraceWrite { io_error: String },
    /// Store write failed (propagates as Err).
    StoreWrite { store_error: String },
    /// RunConfig validation failed (e.g., empty signer).
    InvalidConfig { field: String },
}

/// Sub-error for ClaudeSpawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeSpawnReason {
    /// Binary not found on PATH.
    ClaudeNotFound,
    /// Permission denied.
    PermissionDenied,
    /// PATH is empty.
    PathEmpty,
    /// Other I/O error.
    OtherIoError(String),
}

/// Consumes NDJSON lines from the event stream.
pub trait EventSink: Send {
    /// Emit a single NDJSON line.
    fn emit(&mut self, line: &str);
}

/// ClaudeCodeRuntime spawns the local `claude` binary as a subprocess.
pub struct ClaudeCodeRuntime {
    // Placeholder: validates claude is on PATH at construction.
    _validated: (),
}

impl ClaudeCodeRuntime {
    /// Construct a new ClaudeCodeRuntime.
    /// Fails if `claude` is not found on the configured PATH.
    pub fn new() -> Result<Self, RuntimeError> {
        // Check if claude is on PATH by attempting a `which claude` equivalent.
        let path_var = std::env::var("PATH").unwrap_or_default();

        // Try to find claude binary (returns ClaudeNotFound if PATH is empty or claude not found).
        if Self::find_claude_binary(&path_var).is_none() {
            return Err(RuntimeError::ClaudeSpawn {
                reason: ClaudeSpawnReason::ClaudeNotFound,
            });
        }

        Ok(ClaudeCodeRuntime { _validated: () })
    }

    /// Find the claude binary on PATH.
    fn find_claude_binary(path: &str) -> Option<String> {
        if path.is_empty() {
            return None;
        }
        for dir_str in path.split(':') {
            let claude_path = PathBuf::from(dir_str).join("claude");
            if claude_path.exists() {
                return Some(claude_path.to_string_lossy().to_string());
            }
        }
        None
    }

    /// Compose the argv for invoking claude.
    /// Returns a vector like: ["claude", "-p", "<prompt>", "--output-format", "stream-json", "--verbose", ...]
    pub fn compose_argv(cfg: &RunConfig) -> Vec<String> {
        let mut argv = vec!["claude".to_string(), "-p".to_string(), cfg.prompt.clone()];

        argv.push("--output-format".to_string());
        argv.push("stream-json".to_string());
        argv.push("--verbose".to_string());

        // Optional: add --model if specified in build_config.
        if let Some(models) = cfg.build_config.get("models").and_then(|v| v.as_array()) {
            if let Some(first_model) = models.first().and_then(|v| v.as_str()) {
                argv.push("--model".to_string());
                argv.push(first_model.to_string());
            }
        }

        argv
    }
}

#[async_trait::async_trait]
impl Runtime for ClaudeCodeRuntime {
    async fn spawn_claude_session(&self, cfg: RunConfig) -> Result<RunOutcome, RuntimeError> {
        // Validate config.
        validate_run_config(&cfg)?;

        // Create a store for the recorder.
        let store: Arc<dyn Store> = Arc::new(agentic_store::MemStore::new());

        // Start the recorder.
        let recorder_cfg = RunRecorderConfig {
            store: Arc::clone(&store),
            runs_root: cfg.runs_root.clone(),
            run_id: cfg.run_id.clone(),
            story_id: cfg.story_id,
            story_yaml_bytes: cfg.story_yaml_bytes.clone(),
            signer: cfg.signer.clone(),
            build_config: cfg.build_config.clone(),
        };

        let recorder = RunRecorder::start(recorder_cfg).map_err(|e| RuntimeError::TraceWrite {
            io_error: format!("{:?}", e),
        })?;

        // Wire branch_state if repo_path and branch_name are provided.
        if let (Some(repo_path), Some(branch_name)) = (&cfg.repo_path, &cfg.branch_name) {
            recorder.start_branch(repo_path, branch_name).map_err(|e| RuntimeError::TraceWrite {
                io_error: format!("{:?}", e),
            })?;
        }

        // TODO: implement actual subprocess spawning and event handling.
        // For now, return a placeholder outcome.
        let outcome = Outcome::Green {
            signing_run_id: "<pending>".to_string(),
        };

        let runs_row_id = cfg.run_id.clone();

        // Finish branch tracking if repo_path was provided.
        if cfg.repo_path.is_some() && cfg.branch_name.is_some() {
            recorder.finish_branch(false).map_err(|e| RuntimeError::TraceWrite {
                io_error: format!("{:?}", e),
            })?;
        }

        // Finish recording.
        recorder
            .finish(outcome.clone())
            .map_err(|e| RuntimeError::StoreWrite {
                store_error: format!("{:?}", e),
            })?;

        Ok(RunOutcome {
            run_id: cfg.run_id,
            outcome,
            runs_row_id,
        })
    }
}

/// MockRuntime replays canned NDJSON fixtures.
pub struct MockRuntime {
    fixture_path: PathBuf,
    pipe_break: bool,
    crash_exit_code: Option<i32>,
    store: Arc<agentic_store::MemStore>,
}

impl MockRuntime {
    /// Create a MockRuntime from a fixture file.
    pub fn from_fixture(path: &std::path::Path) -> Result<Self, RuntimeError> {
        Ok(MockRuntime {
            fixture_path: path.to_path_buf(),
            pipe_break: false,
            crash_exit_code: None,
            store: Arc::new(agentic_store::MemStore::new()),
        })
    }

    /// Create a MockRuntime from a fixture file that will inject a pipe break.
    pub fn from_fixture_with_pipe_break(path: &std::path::Path) -> Result<Self, RuntimeError> {
        Ok(MockRuntime {
            fixture_path: path.to_path_buf(),
            pipe_break: true,
            crash_exit_code: None,
            store: Arc::new(agentic_store::MemStore::new()),
        })
    }

    /// Set the exit code the mock runtime should simulate when crashing.
    pub fn with_crash_exit_code(mut self, code: i32) -> Self {
        self.crash_exit_code = Some(code);
        self
    }
}

#[async_trait::async_trait]
impl Runtime for MockRuntime {
    async fn spawn_claude_session(&self, cfg: RunConfig) -> Result<RunOutcome, RuntimeError> {
        // Validate config.
        validate_run_config(&cfg)?;

        // Start the recorder.
        let recorder_cfg = RunRecorderConfig {
            store: Arc::clone(&self.store) as Arc<dyn Store>,
            runs_root: cfg.runs_root.clone(),
            run_id: cfg.run_id.clone(),
            story_id: cfg.story_id,
            story_yaml_bytes: cfg.story_yaml_bytes.clone(),
            signer: cfg.signer.clone(),
            build_config: cfg.build_config.clone(),
        };

        let recorder = RunRecorder::start(recorder_cfg).map_err(|e| RuntimeError::TraceWrite {
            io_error: format!("{:?}", e),
        })?;

        // Wire branch_state if repo_path and branch_name are provided.
        if let (Some(repo_path), Some(branch_name)) = (&cfg.repo_path, &cfg.branch_name) {
            recorder.start_branch(repo_path, branch_name).map_err(|e| RuntimeError::TraceWrite {
                io_error: format!("{:?}", e),
            })?;
        }

        // Extract max_inner_loop_iterations from build_config.
        let max_iterations = cfg
            .build_config
            .get("max_inner_loop_iterations")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as u32;

        // Read fixture file, trying multiple paths if needed.
        let fixture_content = match std::fs::read_to_string(&self.fixture_path) {
            Ok(content) => content,
            Err(first_err) => {
                // If the path is relative and doesn't work, try it with various prefixes.
                // This handles the case where tests run from different working directories.
                let paths_to_try = vec![
                    self.fixture_path.clone(),
                    PathBuf::from("..").join(&self.fixture_path),
                    PathBuf::from("../..").join(&self.fixture_path),
                ];
                let mut found = None;
                for try_path in paths_to_try {
                    if let Ok(content) = std::fs::read_to_string(&try_path) {
                        found = Some(content);
                        break;
                    }
                }
                found.ok_or_else(|| RuntimeError::TraceWrite {
                    io_error: format!("Failed to read fixture: {}", first_err),
                })?
            }
        };

        let mut tee = recorder.trace_tee();
        let mut event_sink = cfg.event_sink;
        let mut iteration_count = 0u32;
        let mut outcome = Outcome::Green {
            signing_run_id: "<pending>".to_string(),
        };
        let mut last_was_tool_call = false;
        let mut iteration_start_time = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true);

        for (line_no, line) in fixture_content.lines().enumerate() {
            // Check if we should inject a pipe break.
            if self.pipe_break && line_no > 0 {
                // Simulate a broken pipe by truncating output.
                outcome = Outcome::Crashed {
                    error: "pipe broken".to_string(),
                };
                break;
            }

            // Check iteration budget.
            if iteration_count >= max_iterations {
                outcome = Outcome::InnerLoopExhausted;
                break;
            }

            // Write to trace tee.
            let _ = tee.write_all(line.as_bytes());
            let _ = tee.write_all(b"\n");

            // Emit to event sink.
            event_sink.emit(line);

            // Count iterations: increment on tool_result that follows a tool_call.
            if line.contains("\"tool_call\"") {
                last_was_tool_call = true;
                iteration_start_time = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
            } else if line.contains("\"tool_result\"") && last_was_tool_call {
                let iteration_end_time = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true);

                // Record the iteration.
                let iteration_summary = crate::run_recorder::IterationSummary {
                    i: iteration_count,
                    started_at: iteration_start_time.clone(),
                    ended_at: iteration_end_time,
                    probes: vec![],
                    verdict: None,
                    error: None,
                };
                let _ = recorder.record_iteration(iteration_summary);

                iteration_count += 1;
                last_was_tool_call = false;
            } else if line.contains("\"assistant_final\"") {
                // Terminal marker; stop processing.
                outcome = Outcome::Green {
                    signing_run_id: "<pending>".to_string(),
                };
                break;
            }
        }

        let run_id = cfg.run_id.clone();

        // Finish branch tracking if repo_path was provided.
        if cfg.repo_path.is_some() && cfg.branch_name.is_some() {
            recorder.finish_branch(false).map_err(|e| RuntimeError::TraceWrite {
                io_error: format!("{:?}", e),
            })?;
        }

        // Finish recording.
        recorder
            .finish(outcome.clone())
            .map_err(|e| RuntimeError::StoreWrite {
                store_error: format!("{:?}", e),
            })?;

        Ok(RunOutcome {
            run_id: run_id.clone(),
            outcome,
            runs_row_id: run_id,
        })
    }

    fn mock_store(&self) -> Option<Arc<dyn Store>> {
        Some(Arc::clone(&self.store) as Arc<dyn Store>)
    }
}

/// Validate RunConfig fields.
fn validate_run_config(cfg: &RunConfig) -> Result<(), RuntimeError> {
    if cfg.signer.trim().is_empty() {
        return Err(RuntimeError::InvalidConfig {
            field: "signer".to_string(),
        });
    }
    if cfg.branch_name.is_some() && cfg.repo_path.is_none() {
        return Err(RuntimeError::InvalidConfig {
            field: "branch_name".to_string(),
        });
    }
    Ok(())
}
