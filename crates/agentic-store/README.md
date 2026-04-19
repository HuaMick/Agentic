# agentic-store

## What this crate is

Persistence. Defines the `Store` trait — the single point every runtime
crate writes through — and ships both the in-memory `MemStore`
implementation (used by tests and fast-path consumers) and the
SurrealDB-backed `SurrealStore` (story 5, shipped) backed by the
`surrealkv` embedded LSM.

## Why it's a separate crate

1. **Swap-able backend.** The trait is the contract; backends are
   implementation details. Local to cloud is a config change, not a code
   change. See `memory/store_cloud_migration.md`.
2. **Testability.** `MemStore` lives in this same crate so every
   trait-level test doubles as a contract harness for story 5's
   `SurrealStore` — no circular dev-dep graph via a separate testkit
   crate.
3. **Concentrates I/O.** Domain crates stay pure (no I/O); all disk /
   network touches funnel through here.

## Current state (stories 4 + 5 shipped)

The trait, `MemStore`, and `SurrealStore` land in this crate. Five behavioural contracts
are pinned as integration tests under `tests/` (each runs against both impls):

1. **Upsert-by-key replaces.** Two upserts to the same `(table, key)`
   leave exactly one row, equal to the second write.
2. **Append-to-collection preserves.** N appends yield N rows in
   insertion order; later writes do not mutate earlier ones.
3. **Typed absence on `get`.** A read against a never-written
   `(table, key)` returns `Ok(None)` — not an error, not a panic. Missing
   tables and missing keys are indistinguishable at the trait level.
4. **Empty query is not an error.** A filter matching zero rows, or a
   query against an unknown table, returns `Ok(vec![])`.
5. **Send + Sync at the trait-object level.** Consumers hold the store as
   `Arc<dyn Store + Send + Sync>`; the trait is object-safe.

Story 5 delivered the SurrealDB impl and reuses the same five tests as its
contract harness. That reuse is what proves the trait is a real
abstraction rather than a one-impl stub.

## Public API

```rust
pub trait Store: Send + Sync {
    fn upsert(&self, table: &str, key: &str, doc: Value) -> Result<(), StoreError>;
    fn append(&self, table: &str, doc: Value) -> Result<(), StoreError>;
    fn get(&self, table: &str, key: &str) -> Result<Option<Value>, StoreError>;
    fn query(
        &self,
        table: &str,
        filter: &dyn Fn(&Value) -> bool,
    ) -> Result<Vec<Value>, StoreError>;
}

pub struct MemStore { /* ... */ }
impl Store for MemStore { /* ... */ }
```

Documents are `serde_json::Value` — schemaless by design (ADR-0002).
Consumers layer their own typed `serde` helpers on top; the trait takes
no position on the payload shape. Methods are sync; async vs sync was
explicitly left open by story 4 and we picked sync as the simpler
starting point. Later stories may revisit if a backend demands it.

A single table is either a keyed map (written by `upsert`) or an append
list (written by `append`); mixing the two on the same table is an error.
Story 4 does not demand anything stronger; later stories can tighten the
model if a consumer needs both shapes at once.

## Dependencies

- Depends on: `serde_json` (document shape).
- Depended on by: `agentic-verify`, `agentic-orchestrator`, `agentic-cli`
  and every other runtime crate that persists state.

## Design decisions

- **SurrealDB embedded** for the durable backend, chosen in ADR-0002:
  schemaless by default, per-table schema later, live queries for a
  future streaming UI, real transactions, single Rust library.
- **Schemaless by default.** We don't enforce schema in the DB layer.
  Story YAML is the source of truth for stories; the DB is a
  cache/index. Add schema per table only when a query needs it.
- **`Store` object-safe.** Methods take no type parameters so
  `Arc<dyn Store>` is a first-class consumer-facing type. Serde layering
  is a caller concern.
- **Typed absence, not error, for "not found."** Consumers never have to
  string-match "no such key" to know they got nothing.
- **Empty filter returns empty Vec.** No special case for first-run
  empty databases.

## Deliberately not pinned (yet)

- **Transactions across tables.** Single-document writes only for now.
  Story 5 will extend this when SurrealDB's transaction closure needs
  pinning.
- **Live queries / subscriptions.** Sketched below but no method yet.
  Added when a consumer demands it.
- **Durability.** `MemStore` is explicitly lossy. Story 5 pins
  durability for the SurrealDB impl.
- **Async.** Sync for now; revisit if a backend forces the issue.

## Open questions (future stories)

- Where does the SurrealDB file live? `~/.agentic/store.surreal` by
  default, env override? (Story 5.)
- Do we use SurrealDB's schemafull mode for some tables (evidence,
  verdicts) where we want strong typing? Likely yes, later.
- Backup strategy — rely on append-only evidence files and treat the DB
  as a rebuildable index?

## Stress/verify requirements (future)

- 10k+ concurrent reads without contention.
- Writes survive abrupt process kill (transaction durability).
- Live queries deliver every committed change to subscribers within
  100ms.
- `MemStore` behaves identically to `SurrealStore` for the trait-level
  test suite (this is what story 5 must prove).
