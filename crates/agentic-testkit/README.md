# agentic-testkit

## What this crate is

Shared test fixtures and mock implementations. In-memory store, mock runtime (replays canned NDJSON transcripts), example stories, temp-dir helpers.

## Why it's a separate crate

Every other crate needs these fixtures for tests. Keeping them in one place avoids copy-paste and makes fixtures reviewable as a design artifact.

## Public API sketch

```rust
pub struct MemStore { /* in-memory impl of Store trait */ }
pub struct MockRuntime { /* replays NDJSON transcripts */ }

pub fn example_story(id: &str) -> Story;
pub fn example_agent_def() -> AgentDef;
pub fn tmp_workspace() -> TempDir;

pub mod transcripts {
    pub const HAPPY_PATH_VERIFY: &str = include_str!("transcripts/happy_path_verify.ndjson");
    pub const FAILED_VERIFY: &str = include_str!("transcripts/failed_verify.ndjson");
}
```

## Dependencies

- Depends on: `agentic-story`, `agentic-work`, `agentic-store`, `agentic-runtime`, `agentic-agent-defs`, `agentic-events`, `tempfile`
- Depended on by: tests in every other crate, `agentic-stress`

## Design decisions

- **Mock runtime is transcript-driven.** Real `claude` output is canonicalized into NDJSON files; the mock replays them. Tests are deterministic. Updating transcripts is a deliberate act.
- **Example stories are the documentation.** A reader can open `testkit/stories/` to see what real stories look like.
- **Nothing production-facing depends on testkit.** Only `[dev-dependencies]` in other crates' `Cargo.toml`. Enforced by workspace config.

## Open questions

- Do we generate transcripts from real Claude runs periodically (golden-file style) or hand-curate them? Likely hybrid — a few hand-curated canonical ones, and a `cargo xtask capture-transcripts` to refresh from live runs on demand.

## Stress/verify requirements

- MemStore behavior matches SurrealStore for the full Store trait surface.
- Transcripts parse without errors on every build.
