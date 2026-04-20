//! Story 7 scaffold bootstrap.
//!
//! This file exists only so the crate compiles as an empty library during
//! the red-state phase of story 7 — its public surface (`TestBuilder`,
//! `TestBuilderError`, and friends) is build-rust's to author once the
//! red-state evidence under `evidence/runs/7/` is committed. Every test
//! scaffold in `tests/` that imports a symbol from this crate is
//! therefore compile-red on a fresh checkout, which is the intended red
//! path for story 7.
//!
//! Per ADR-0005 and the test-builder contract, this stub is the
//! minimal compile-anchor, not production source; build-rust will
//! overwrite it during implementation.
