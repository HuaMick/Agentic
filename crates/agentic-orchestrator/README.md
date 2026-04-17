# agentic-orchestrator

## What this crate is

The brain. Day-one loop: given a story, plan the work → execute the plan via agents → verify the result → record. One story at a time; no cross-epic DAG, no elaborate scheduler.

## Why it's a separate crate

The orchestrator is where business logic compounds. In the legacy system, this was the 1100-line file that accreted feedback triggers, quick-exit diagnostics, budget tracking, orphaned-tmux cleanup, and recovery sweeps. We want a clear boundary around this logic so we can see it growing and push back.

## Public API sketch

```rust
pub struct Orchestrator {
    store: Arc<dyn Store>,
    runtime: Arc<dyn Runtime>,
    sandbox: Arc<dyn Sandbox>,
    registry: Arc<Registry>,
    verifier: Arc<Verifier>,
}

impl Orchestrator {
    pub async fn work_story(&self, story_id: &StoryId) -> Result<Verdict>;
}
```

## Dependencies

- Depends on: `agentic-story`, `agentic-work`, `agentic-agent-defs`, `agentic-store`, `agentic-verify`, `agentic-runtime`, `agentic-sandbox`, `agentic-events`
- Depended on by: `agentic-cli`

## Design decisions

- **Thin by design.** Day one: read story → pick agent → spawn via runtime → wait → verify → write verdict. That's it. Anything more is earned.
- **No retry logic initially.** If a phase fails, it fails. Recording the failure is the feature. Retry comes only when a stress test proves it's needed.
- **No budget tracking initially.** `_deferred/agentic-budget/` holds the design for later.
- **Failure is loud, not papered over.** We surface every error with full context. Legacy swallowed errors; we do the opposite.
- **The prove-it gate is a call to `agentic-verify`, not reimplementation.** We don't reinvent acceptance checking here; we use the existing gate.

## Open questions

- Granularity — one story at a time, or batch planning over multiple stories? Start with one.
- How do phases map to agent invocations? One-to-one for MVP; fancier mappings later.
- Recovery on restart — if the orchestrator crashes mid-story, what happens? Start with: the story is not promoted (Verdict never written), so on restart the story is still `under_construction` and eligible for another run.

## Stress/verify requirements

- Given a story with a single acceptance criterion and a mock runtime, the full loop runs deterministically.
- Orchestrator can be killed at any point without leaving the system in a corrupted state (checked via `agentic-stress`).
- 10 concurrent `work_story` calls don't interfere with each other.
