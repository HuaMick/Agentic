//! Story 5 acceptance test: SurrealStore writes are durable across reopen.
//!
//! Justification (from stories/5.yml): write N documents to a `SurrealStore`
//! rooted at a temp directory, drop the store, construct a new
//! `SurrealStore` pointing at the same directory, and verify all N
//! documents are readable with identical content. Without this, SurrealDB's
//! commit-is-durable guarantee is assumed rather than enforced at the crate
//! boundary — and a misconfiguration (running an in-memory variant by
//! mistake, missing flush, wrong engine) would slip past story 4's
//! behavioural tests because those use `MemStore`.
//!
//! The temp directory outlives the first `SurrealStore` deliberately so the
//! second `open` against the same root sees the on-disk state the first
//! `SurrealStore` committed. If durability is not honoured (in-memory
//! engine, missing flush, wrong path), the second store sees zero rows and
//! the test fails on the row-count assertion.

use agentic_store::{Store, SurrealStore};
use serde_json::json;
use tempfile::TempDir;

#[test]
fn three_writes_survive_drop_and_reopen_at_same_root() {
    let dir = TempDir::new().expect("create temp dir");
    let root = dir.path().to_path_buf();

    // First lifetime: write three rows to "durable", then drop.
    {
        let store = SurrealStore::open(&root).expect("first open should succeed");
        store
            .append("durable", json!({ "seq": 1, "payload": "alpha" }))
            .expect("first append should succeed");
        store
            .append("durable", json!({ "seq": 2, "payload": "beta" }))
            .expect("second append should succeed");
        store
            .append("durable", json!({ "seq": 3, "payload": "gamma" }))
            .expect("third append should succeed");
    } // store dropped here; commit-is-durable must hold.

    // Second lifetime: reopen at the same root, read the same rows back.
    let store = SurrealStore::open(&root).expect("second open at same root should succeed");
    let rows = store
        .query("durable", &|_| true)
        .expect("query after reopen should succeed");

    assert_eq!(
        rows.len(),
        3,
        "three rows written before drop must be visible after reopen; got {rows:?}"
    );
    assert_eq!(rows[0], json!({ "seq": 1, "payload": "alpha" }));
    assert_eq!(rows[1], json!({ "seq": 2, "payload": "beta" }));
    assert_eq!(rows[2], json!({ "seq": 3, "payload": "gamma" }));
}

#[test]
fn upsert_survives_drop_and_reopen_with_latest_value() {
    let dir = TempDir::new().expect("create temp dir");
    let root = dir.path().to_path_buf();

    {
        let store = SurrealStore::open(&root).expect("first open should succeed");
        store
            .upsert("config", "active", json!({ "version": 1 }))
            .expect("first upsert should succeed");
        store
            .upsert("config", "active", json!({ "version": 2 }))
            .expect("second upsert should succeed");
    }

    let store = SurrealStore::open(&root).expect("second open should succeed");
    let got = store
        .get("config", "active")
        .expect("get after reopen should succeed")
        .expect("the upserted row should still be there after reopen");
    assert_eq!(
        got,
        json!({ "version": 2 }),
        "reopen must surface the latest committed write, not the first"
    );
}
