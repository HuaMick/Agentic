# agentic-runtime

## What this crate is

Runs agents. Defines the `Runtime` trait and provides day-one implementation: `ClaudeCodeRuntime` which wraps [`claude-code-rs`](https://github.com/decisiongraph/claude-code-rs) (or a fork of it) to spawn the local `claude` binary as a subprocess over stdio NDJSON.

## Why it's a separate crate

1. **Pluggable.** Trait-based so we can swap to a PTY/streaming impl later without touching callers.
2. **Isolates the auth story.** Subscription authentication via local `claude` login lives entirely here. Nothing else in the codebase knows about API keys or OAuth — because there aren't any to know about.
3. **Testability.** A `MockRuntime` (location TBD post `agentic-testkit` retirement — see story 26's `agentic-test-support` kit) replays canned NDJSON transcripts for deterministic tests.

## Public API sketch

```rust
#[async_trait]
pub trait Runtime {
    async fn spawn(&self, req: SpawnRequest) -> Result<SpawnHandle>;
}

pub struct SpawnRequest {
    pub agent: AgentDef,
    pub context: PromptContext,   // story, phase, ticket, compiled prompt
    pub timeout: Duration,
    pub max_turns: Option<u32>,
}

pub struct SpawnHandle {
    pub session_id: String,
    pub events: EventStream,      // stream of typed events as NDJSON arrives
    pub status: StatusHandle,
}

pub struct ClaudeCodeRuntime { /* ... */ }
impl Runtime for ClaudeCodeRuntime { /* spawns `claude -p` as subprocess */ }
```

## Dependencies

- Depends on: `agentic-agent-defs`, `agentic-events`, `agentic-sandbox`, `claude-code-rs` (or fork), `tokio`
- Depended on by: `agentic-orchestrator`

## Design decisions

- **Subscription auth via local `claude` binary.** The subprocess inherits OAuth credentials from `~/.claude/.credentials.json` (Linux/Windows) or macOS Keychain. No API key required. No per-token billing.
- **Never pass `--bare`.** That flag skips OAuth and demands an API key. Our invocation is plain `claude -p "<prompt>" --output-format stream-json --verbose`.
- **Subagent nesting via Rust-side fanout, not Claude-internal Task.** Claude's built-in `Task` tool is blocked for subagents. We spawn multiple top-level `claude` subprocesses from Rust, giving us native concurrency + depth control.
- **Streaming by default.** NDJSON events flow as they arrive. Callers can observe progress in real time (and subscribe a future UI).
- **Rate-limit aware.** Watches for `system/api_retry` events in the stream and backs off on 429s.

## Open questions

- Fork `claude-code-rs` or depend on it? Leaning depend + contribute upstream. Fork only if our needs diverge.
- Do we need a second `DirectApiRuntime` that uses the Anthropic API directly (for CI where interactive OAuth is hard)? Possibly later — guarded behind a cargo feature flag, off by default.
- PTY / streaming impl: when to build? Trigger is `_deferred/agentic-stream/` — when the monitoring UI work starts.

## Stress/verify requirements

- Spawning 10 concurrent agents doesn't leak processes.
- A killed `claude` subprocess is cleaned up (zombie reaper works).
- 429 responses are handled with exponential backoff, not crash.
- Stream events are delivered in order with no drops under load.
