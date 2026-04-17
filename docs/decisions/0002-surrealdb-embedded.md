# ADR-0002: SurrealDB embedded for persistence

**Status:** accepted
**Date:** 2026-04-17

## Context

The legacy AgenticEngineering system stored epics, tickets, phases, and stories in TinyDB — a single JSON file accessed via file locks. Crashes were frequent: concurrent writes corrupted the file, cache invalidation was manual, and "the DB got stuck" was a routine failure mode.

The rebuild needs a durable, queryable, schemaless-friendly store with these properties:

- **Transactions that actually work** under concurrent writers (fixes the legacy crash class).
- **Schemaless by default** — we don't want to lock into rigid table schemas at this stage. Stories, patterns, agents all have evolving shapes.
- **Queryability** beyond blob-by-key — we need "find all stories referencing pattern X" without loading the whole DB.
- **Live queries / change notifications** — we've flagged a future real-time monitoring UI that streams agent thinking. The store should support subscriptions without bolt-on infrastructure.
- **Embeddable as a library** — single-binary distribution, no daemon, no external server.
- **Rust-native** — avoids an FFI layer.

## Decision

Use **SurrealDB embedded** (`surrealdb` crate with the embedded feature flag) as the persistence layer. Wrapped behind a `Store` trait in `agentic-store` so the backend is swappable if we ever need it.

## Alternatives considered

**SQLite (via `rusqlite` or `sqlx`).** Rejected for now. Rock-solid, universally known, excellent tooling — but rigid schema-first model. We would end up either declaring full schemas up front (wrong for our stage) or storing everything as JSON blobs in TEXT columns, which throws away SQL's query power and becomes a worse PoloDB. Revisit if SurrealDB disappoints; the `Store` trait makes this a localized change.

**PoloDB.** Rejected. MongoDB-compatible embedded DB, closest feel to TinyDB-but-better. But: no live queries. That kills the future streaming UI without a separate pub/sub layer. If we ever needed live notifications we'd be back here.

**sled / redb.** Rejected. Excellent embedded KV stores, but we'd hand-roll every query. At our domain's complexity (stories reference patterns, stories reference stories via depends_on, epics reference stories) query-engine-free means a lot of code we don't want to maintain.

**SurrealKV.** Rejected. It's the underlying storage engine SurrealDB uses — lower level than we want. We'd be reinventing the document/query layer on top.

**Joydb.** Rejected. In-memory, positioned for prototypes. Doesn't meet durability requirements.

**Postgres (embedded via `pg_embed` or similar).** Rejected. Heavyweight, still relational, adds operational surface even when "embedded."

## Consequences

**Gained:**

- Schemaless by default. Can add optional schema per table later (SurrealDB supports both modes) without migration pain.
- Real ACID transactions. The legacy's concurrent-write corruption is structurally impossible.
- Live queries (`LIVE SELECT`) give us a streaming-UI substrate for free when we build `_deferred/agentic-stream/`.
- Multi-model: document + graph. Useful when epics/phases/patterns get genuinely relational (cross-epic dependency, pattern → story edges).
- Single Rust library. No daemon, no external server. Ships with the binary.

**Given up:**

- SurrealDB is younger than SQLite. We're accepting some risk that edge cases surface in production. Mitigation: wrap behind a trait; the worst-case fallback is rewriting one crate.
- SurrealQL is its own query language. Less universally known than SQL. Mitigation: most of our queries are simple get/put/live-subscribe; the heavy lifting is in application code.
- Embedded-mode performance is not as well-characterized as SQLite's. Mitigation: benchmark during Phase 2 stress testing; if we hit a wall, the trait lets us swap.

## Related

- ADR-0001 (Rust rebuild): SurrealDB is Rust-native, no FFI.
- `crates/agentic-store/README.md`: trait shape and the `MemStore` test impl.
- `_deferred/agentic-stream/README.md`: the future streaming UI that relies on live queries.
