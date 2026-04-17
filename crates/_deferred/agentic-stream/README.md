# agentic-stream (deferred)

## What it will be

Real-time agent monitoring UI. Streams thinking, tool calls, and progress for every running agent. Web-based or TUI, TBD.

## Why deferred

This is the aspirational future you've mentioned — watching agents think in real time. A great goal, but day one we can inspect evidence files and log output manually. The full UI is a significant build.

Earns its place when:

1. We have multiple concurrent agent runs where log-tailing becomes impractical.
2. We want non-technical stakeholders to observe work in progress.
3. Debugging subtle agent loops requires watching the full sequence live.

## What it would look like

- Subscribes to `_deferred/agentic-ledger/` via SurrealDB live queries.
- Renders a timeline per agent: thoughts → tool calls → observations → decisions.
- Filters by story, epic, agent, time range.
- Likely web UI using `axum` + `htmx` or `leptos`. TUI alternative with `ratatui` for terminal-only ops.

## Prerequisites

- `_deferred/agentic-ledger/` must be built first (the data source).
- `agentic-runtime` must emit rich streaming events (partial thoughts, tool calls). Currently it does — we just need to wire them through the ledger.

## Trigger to build

User demand. Or when the log-tailing workflow visibly slows down development.
