//! Shared test fixture primitives for the agentic workspace.
//!
//! This crate provides setup, fixture, and stub-executor material that
//! would otherwise be reimplemented across multiple test files. It ships
//! fixture machinery only — no assertion helpers. See the README for the
//! catalogue of available primitives.

pub struct FixtureCorpus;

pub struct StoryFixture;

pub struct FixtureRepo;

pub struct RecordingExecutor;

pub struct RecordedCall;
