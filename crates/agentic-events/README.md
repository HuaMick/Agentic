# agentic-events

## What this crate is

The typed event vocabulary shared across the system. Small, stable crate — just enum variants for every event the orchestrator, verifier, runtime, or store might emit.

Examples: `StoryProposed`, `StoryPromoted`, `VerifyStarted`, `VerifyPassed`, `VerifyFailed`, `PhaseStarted`, `PhaseCompleted`, `AgentSpawned`, `AgentExited`.

## Why it's a separate crate

1. **Avoid circular dependencies.** Many crates need to emit events; many crates need to subscribe. A shared vocabulary in a leaf crate avoids everyone depending on everyone.
2. **Enforce a stable event contract.** Events get recorded to the ledger and consumed by future UIs. Breaking an event shape breaks downstream consumers. Keeping them in one small crate makes breakage visible in review.
3. **Tiny compile surface.** Downstream crates depending only on the event types pay almost no compile cost.

## Public API sketch

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Event {
    Story(StoryEvent),
    Phase(PhaseEvent),
    Verify(VerifyEvent),
    Agent(AgentEvent),
    Runtime(RuntimeEvent),
}

pub struct EventEnvelope {
    pub event: Event,
    pub timestamp: DateTime<Utc>,
    pub source: String,        // e.g., "agentic-verify"
    pub correlation_id: Uuid,  // ties an event to a causing operation
}
```

## Dependencies

- Depends on: `serde`, `chrono`, `uuid`
- Depended on by: almost everything

## Design decisions

- **Events are data, not methods.** No `handle(event)` traits here — that's the consumer's job.
- **Correlation IDs are first-class.** A verify run, a phase execution, an agent spawn — each gets a UUID that threads through all events it causes. Essential for future trace UIs.
- **No event bus here.** This crate only defines the types. The bus (channel, broadcast, live query) lives in `_deferred/agentic-ledger/` when we build it.

## Open questions

- Do we version event variants? For MVP, no — we break and fix. Later, yes (additive changes only).

## Stress/verify requirements

- All events serialize and deserialize losslessly via `serde_json`.
- Event enum changes are reviewed with the same care as API breaking changes.
