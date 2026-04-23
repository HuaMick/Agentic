//! # agentic-story-build
//!
//! Host-and-sandbox driver for `agentic story build <id>`. The public
//! surface will host:
//!
//! - `StoryBuild` — the orchestration type that composes the `docker
//!   run` argv on the host, drives the container-side seeding /
//!   gate-check / inner-loop lifecycle in `--in-sandbox` mode, and
//!   performs the post-run auto-merge on green.
//! - `BuildConfig` — the caller-owned configuration (resolved image
//!   tag, runs root, credentials path, ancestor snapshot path, docker
//!   binary resolver).
//! - `StoryBuildError` — the typed failure enum
//!   (`DockerUnavailable`, `GitIdentityMissing`, `StartShaDrift`,
//!   `AncestorSnapshotInsufficient`, `ImageTagNotFound`,
//!   `CredentialsMissing`, `RunsRootInvalid`,
//!   `InnerLoopExhausted`, `Crashed`).
//! - `RunOutcome` / `MergeReport` — the value types the CLI shim maps
//!   onto exit codes (0 on green+merged, 1 on `InnerLoopExhausted` /
//!   `Crashed`, 2 on every could-not-verdict refusal).
//!
//! This file is deliberately empty. Story 20's acceptance tests each
//! `use` at least one symbol this crate will export once build-rust
//! implements `StoryBuild`; on a fresh checkout those `use` lines
//! resolve against nothing and `cargo check` fails with a rustc
//! "unresolved import" error — the compile-red path named in
//! ADR-0005.
//!
//! See `stories/20.yml` for the full contract, `docs/decisions/
//! 0006-sandboxed-story-hardening-loop.md` for the architectural
//! framing, and `agents/test/test-builder/` for the red-state
//! evidence discipline this crate's tests participate in.
