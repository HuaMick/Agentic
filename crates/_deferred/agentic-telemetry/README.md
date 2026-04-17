# agentic-telemetry (deferred)

## What it will be

Structured tracing and metrics layer. OpenTelemetry exporters, custom spans around orchestration operations, Prometheus-style metrics.

## Why deferred

Day one, we use the `tracing` crate directly in each crate — standard Rust logging. That's enough for developer debugging.

A dedicated telemetry crate earns its place when:

1. We run the system in production (multi-user, long-running).
2. We want distributed traces across spawned agent processes.
3. We're debugging latency regressions and need span-level data.

## What it would look like

- Initialization helpers (hook up `tracing_subscriber`, OTel exporter, log rotation).
- Typed span helpers for common operations (`verify_span`, `phase_span`, `agent_span`).
- Metric definitions shared across crates.

## Trigger to build

When we deploy the system somewhere other than a developer laptop. Or when a performance investigation demands it.
