//! # agentic-runtime
//!
//! Runs agents. The public surface will host the `Runtime` trait and the
//! `ClaudeCodeRuntime` implementation in later stories (19+); story 16
//! lands the `RunRecorder` + `TraceTee` + `Outcome` + `IterationSummary`
//! + `RunRecorderError` types that record one `runs` row per inner-loop
//! invocation.
//!
//! This file is deliberately empty. Story 16's acceptance tests each
//! `use` at least one symbol this crate will export once build-rust
//! implements the recorder; on a fresh checkout those `use` lines
//! resolve against nothing and `cargo check` fails with a rustc
//! "unresolved import" error — the compile-red path named in ADR-0005.
