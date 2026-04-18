//! Story 4 acceptance test: upsert-by-key replaces.
//!
//! Justification (from stories/4.yml): performing two upserts to the same
//! (table, key) pair against `MemStore` leaves exactly one document in that
//! table whose content equals the second write. Without this invariant, any
//! "one row per key" table across the workspace has undefined trait-level
//! semantics.
//!
//! Written against `dyn Store` deliberately: the only line that mentions
//! `MemStore` is the constructor. Story 5 reuses this same assertion with a
//! `SurrealStore` constructor.

use agentic_store::{MemStore, Store};
use serde_json::json;

#[test]
fn two_upserts_to_same_key_leaves_one_row_equal_to_second_write() {
    let store: Box<dyn Store> = Box::new(MemStore::new());

    store
        .upsert("notes", "a", json!({ "v": 1 }))
        .expect("first upsert should succeed");
    store
        .upsert("notes", "a", json!({ "v": 2 }))
        .expect("second upsert should succeed");

    let rows = store
        .query("notes", &|_| true)
        .expect("query should succeed");

    assert_eq!(
        rows.len(),
        1,
        "upsert to same (table, key) must not create a second row; got {rows:?}"
    );
    assert_eq!(
        rows[0],
        json!({ "v": 2 }),
        "surviving row must equal the second write, not the first"
    );

    let got = store
        .get("notes", "a")
        .expect("get should succeed")
        .expect("get should find the upserted row");
    assert_eq!(got, json!({ "v": 2 }), "get must return the second write");
}
