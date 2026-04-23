//! # agentic-signer
//!
//! Resolves a signer identity string from a four-tier chain (flag → env
//! var → git config → typed error) and hands it to the evidence-writing
//! paths (`agentic-uat`, `agentic-ci-record`, `agentic-runtime`). The
//! public surface will host `Resolver`, `Signer`, `SignerError`,
//! `Source`, `InvalidReason`, and `SignerSource` once story 18's
//! build-rust pass lands them.
//!
//! This file is deliberately empty. Story 18's acceptance tests each
//! `use` at least one symbol this crate will export once build-rust
//! implements the resolver; on a fresh checkout those `use` lines
//! resolve against nothing and `cargo check` fails with a rustc
//! "unresolved import" error — the compile-red path named in
//! ADR-0005.
