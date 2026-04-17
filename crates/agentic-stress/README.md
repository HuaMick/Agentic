# agentic-stress

## What this crate is

⭐ **Chaos + property test harness, first-class from day one.** Generates synthetic load, kills processes mid-operation, corrupts partial writes, drives property tests via `proptest`. The orchestrator cannot merge without passing a minimum stress suite.

## Why it's a separate crate

The legacy system failed because it was never pressure-tested — it worked on the happy path and crashed under iteration. This crate is the structural answer: stress testing is not a `tests/` afterthought, it's a named dependency that other crates must satisfy.

Having it as a dedicated crate means:

1. It has its own binary (`cargo run -p agentic-stress -- scenario <name>`), invokable manually or from CI.
2. It defines scenarios as versioned, named artifacts — not ad-hoc test files.
3. It owns the chaos primitives (random kills, partial writes, clock skew, concurrent writers) so they're reusable across scenarios.

## Public API sketch

```rust
// Library side — chaos primitives
pub fn kill_mid_operation<F: FnOnce() -> R>(f: F, kill_at: Duration) -> Option<R>;
pub fn corrupt_write(path: &Path, strategy: CorruptionStrategy);
pub struct ClockSkew(Duration);

// Scenario runner
pub struct Scenario {
    pub name: &'static str,
    pub setup: fn() -> World,
    pub run: fn(World) -> Result<()>,
    pub invariants: Vec<fn(&World) -> Result<()>>,
}

pub fn run_scenario(s: &Scenario) -> ScenarioResult;
```

## Dependencies

- Depends on: every domain crate (it needs to pressure-test them). Uses `agentic-testkit` for fixtures.
- Depended on by: CI only.

## Design decisions

- **Scenarios are named and versioned.** `story-verify-under-concurrent-writes`, `orchestrator-recovery-after-kill`, `store-resilient-to-corrupt-db-file`. You can point to them in conversation.
- **`proptest` for property tests, hand-written for chaos.** Property tests find invariant violations; chaos finds concurrency and resilience bugs.
- **Invariants as first-class data.** After every scenario run, invariants are checked: "no evidence file is partial," "no story is in an invalid state transition," "no orphaned processes remain."
- **Minimum bar per crate.** Each other crate's README lists its "stress/verify requirements." This crate implements those requirements.

## Open questions

- How do we run stress in CI without flaky timeouts? Leaning: fixed seeds for determinism, real clock skew simulated via `tokio::time::pause()`.
- Do we fuzz the YAML parsers? Likely yes — `cargo-fuzz` with corpora checked in.

## Stress/verify requirements

- Self-hosting: this crate's own tests must pass (including under random seeds).
- CI runs a named minimum scenario set on every PR. Adding a new orchestrator feature requires adding at least one stress scenario that would have failed without the feature's invariant check.
