# agentic-budget (deferred)

## What it will be

Cost and quota tracking. Per-story, per-epic, per-run budgets in tokens, USD (when applicable), wall-clock time, and retry counts. Enforces halting when limits are reached.

## Why deferred

Day one, we don't care about cost. The goal is correctness. Cost becomes a problem when the system works and runs at scale.

Earns its place when:

1. We're regularly bumping against Claude Code subscription quotas.
2. Runs take long enough that wall-clock budgets would prevent runaway loops.
3. We want per-team or per-epic cost accountability.

## What it would look like

- Budget policies: `Tokens(n)`, `Duration(d)`, `Retries(n)`, composable with `any_of` / `all_of`.
- Event subscriber that tallies cost as events arrive (from `agentic-ledger`).
- Halt signals emitted back to orchestrator when policy is violated.

## Trigger to build

When a runaway orchestration burns through our quota unexpectedly. Reactive, not speculative.
