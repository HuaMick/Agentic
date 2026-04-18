//! Story 5 acceptance test (RED scaffold): trait-parity between `MemStore`
//! and `SurrealStore`.
//!
//! Justification (from stories/5.yml):
//!   Proves the trait-is-a-real-abstraction claim: the exact same
//!   trait-level harness that drives `MemStore` in story 4
//!   (upsert-replaces, append-preserves, typed-absence, empty-filter,
//!   Send+Sync) is re-run against a freshly-constructed `SurrealStore`
//!   backed by an embedded SurrealDB at a temp directory, and every
//!   assertion that passed for `MemStore` also passes for `SurrealStore`.
//!
//! Red state expected at scaffold-write time:
//!   - `agentic_store::SurrealStore` is not exported yet (build-rust
//!     ships it as part of story 5's implementation).
//!   - `tempfile` is not a dev-dependency of `agentic-store` yet
//!     (build-rust adds it when turning this scaffold green).
//! Either missing symbol is enough to keep this test red until
//! implementation lands.

use agentic_store::{Store, SurrealStore};
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn trait_level_harness_passes_against_surrealstore_just_as_memstore() {
    // Setup: a temp directory gives this test a fresh, isolated
    // SurrealDB data root per run.
    let root = TempDir::new().expect("tempdir should be creatable");
    let store: Arc<dyn Store + Send + Sync> =
        Arc::new(SurrealStore::open(root.path()).expect("SurrealStore::open on empty temp dir should succeed"));

    // Story 4 contract #1: upsert-by-key replaces.
    store
        .upsert("notes", "a", json!({ "v": 1 }))
        .expect("first upsert should succeed against SurrealStore");
    store
        .upsert("notes", "a", json!({ "v": 2 }))
        .expect("second upsert should succeed against SurrealStore");
    let rows = store
        .query("notes", &|_| true)
        .expect("query should succeed against SurrealStore");
    assert_eq!(rows.len(), 1, "upsert-replace parity with MemStore");
    assert_eq!(rows[0], json!({ "v": 2 }), "surviving row must equal second write");

    // Story 4 contract #2: append-to-collection preserves.
    store.append("log", json!({ "n": 1 })).expect("append 1");
    store.append("log", json!({ "n": 2 })).expect("append 2");
    store.append("log", json!({ "n": 3 })).expect("append 3");
    let log = store.query("log", &|_| true).expect("query log");
    assert_eq!(log.len(), 3, "append parity: N appends must yield N rows");

    // Story 4 contract #3: typed absence.
    assert_eq!(
        store.get("notes", "never-written").expect("get should succeed"),
        None,
        "missing key must return Ok(None) for SurrealStore too"
    );
    assert_eq!(
        store.get("unknown-table", "x").expect("get should succeed"),
        None,
        "missing table must return Ok(None) for SurrealStore too"
    );

    // Story 4 contract #4: empty filter / unknown table is Ok(vec![]).
    let empty = store
        .query("unknown-table", &|_| true)
        .expect("query on unknown table must be Ok, not Err");
    assert!(empty.is_empty(), "unknown table must return empty Vec");

    // Story 4 contract #5: Send + Sync behind Arc<dyn Store>.
    fn require_send_sync<T: Send + Sync + ?Sized>() {}
    require_send_sync::<dyn Store>();

    panic!(
        "red: Proves the trait-is-a-real-abstraction claim: the exact same trait-level harness that drives `MemStore` in story 4 (upsert-replaces, append-preserves, typed-absence, empty-filter, Send+Sync) is re-run against a freshly-constructed `SurrealStore` backed by an embedded SurrealDB at a temp directory, and every assertion that passed for `MemStore` also passes for `SurrealStore`."
    );
}
