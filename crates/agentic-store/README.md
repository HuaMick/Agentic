# agentic-store

## What this crate is

Persistence. Wraps SurrealDB embedded behind a `Store` trait. Handles reads, writes, queries, and (eventually) live-query subscriptions.

## Why it's a separate crate

1. **Swap-able backend.** If SurrealDB disappoints, we change one crate.
2. **Testability.** The `Store` trait has a `MemStore` impl in `agentic-testkit` for fast tests.
3. **Concentrates I/O.** Domain crates stay pure (no I/O); all disk/network touches funnel through here.

## Public API sketch

```rust
pub trait Store {
    async fn get<T: DeserializeOwned>(&self, table: &str, id: &str) -> Result<Option<T>>;
    async fn put<T: Serialize>(&self, table: &str, id: &str, value: &T) -> Result<()>;
    async fn query<T: DeserializeOwned>(&self, q: Query) -> Result<Vec<T>>;
    async fn live<T: DeserializeOwned>(&self, q: Query) -> Result<LiveStream<T>>;
    async fn tx<F, R>(&self, f: F) -> Result<R>
        where F: FnOnce(&mut Tx) -> Result<R>;
}

pub struct SurrealStore { /* ... */ }
impl Store for SurrealStore { /* ... */ }
```

## Dependencies

- Depends on: `surrealdb` (embedded feature), `serde`, `tokio`, `agentic-events` (for store-emitted events)
- Depended on by: `agentic-verify`, `agentic-orchestrator`, `agentic-cli`

## Design decisions

- **SurrealDB embedded** — chosen for: schemaless by default (matches current preference), can add schema per table later without migration, built-in live queries (enables future streaming UI without refactor), real transactions (fixes legacy's concurrent-write crashes), single Rust library (no daemon).
- **Schemaless by default.** We don't enforce schema in the DB layer. Story YAML is the source of truth for stories; the DB is a cache/index. Add schema per table only when a query needs it.
- **Store trait is async.** SurrealDB is async; mocking in tests uses `tokio::test`.
- **Transactions are closure-based.** Avoids easy-to-forget commit/rollback bugs.

## Open questions

- Where does the DB file live? `~/.agentic/store.surreal` by default, with env override?
- Do we use SurrealDB's schemafull mode for some tables (evidence, verdicts) where we want strong typing? Likely yes, later.
- Backup strategy — rely on append-only evidence files and treat the DB as a rebuildable index?

## Stress/verify requirements

- 10k+ concurrent reads without contention.
- Writes survive abrupt process kill (transaction durability).
- Live queries deliver every committed change to subscribers within 100ms.
- MemStore impl behaves identically to SurrealStore for the test suite.
