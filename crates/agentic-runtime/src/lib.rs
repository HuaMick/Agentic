//! # agentic-runtime
//!
//! Runs agents. Story 16 introduces the `RunRecorder` + `TraceTee` +
//! `Outcome` + `IterationSummary` + `RunRecorderError` types that record
//! one `runs` row per inner-loop invocation.
//!
//! Story 19 adds the `Runtime` trait, `ClaudeCodeRuntime`, `MockRuntime`,
//! `RunConfig`, `RunOutcome`, and `RuntimeError` for spawning agents.

pub mod run_recorder;
pub mod runtime;

pub use run_recorder::{
    IterationSummary, Outcome, RunRecorder, RunRecorderConfig, RunRecorderError, TraceTee,
};
pub use runtime::{
    ClaudeCodeRuntime, ClaudeSpawnReason, EventSink, MockRuntime, RunConfig, RunOutcome, Runtime,
    RuntimeError,
};
