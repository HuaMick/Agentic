# agentic-ledger (deferred)

## What it will be

An append-only event log. Durable, queryable, subscribable. Every event emitted by any crate (via `agentic-events` types) gets written here.

## Why deferred

Day one, we write verdicts and evidence to flat files. That's enough. A ledger crate becomes valuable when:

1. We have multiple consumers of events (UI, metrics, alerting, audit trail).
2. We want to replay system state from history (debugging, time travel).
3. The future streaming monitoring UI needs a subscription source.

## What it would look like

- Writer: `Ledger::append(&EventEnvelope)` — atomic, ordered, durable.
- Reader: range queries by time, correlation ID, source, event type.
- Subscribe: live stream of new events (feeds `_deferred/agentic-stream/`).
- Storage: SurrealDB table with live queries is the likely implementation — keeps everything in one store.

## Trigger to build

When we start building the streaming UI (`_deferred/agentic-stream/`), because that needs a live-query source. Or earlier if debugging distributed-agent runs demands replay.
