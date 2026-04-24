//! # agentic-runtime
//!
//! Runs agents. Story 16 introduces the `RunRecorder` + `TraceTee` +
//! `Outcome` + `IterationSummary` + `RunRecorderError` types that record
//! one `runs` row per inner-loop invocation.

pub mod run_recorder;

pub use run_recorder::{
    IterationSummary, Outcome, RunRecorder, RunRecorderConfig, RunRecorderError, TraceTee,
};
