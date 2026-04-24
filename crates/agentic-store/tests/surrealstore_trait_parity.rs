//! Story 5 acceptance test: SurrealStore satisfies the same trait-level
//! contract harness that pins MemStore in story 4.
//!
//! Justification (from stories/5.yml): the same trait-level harness that
//! drives `MemStore` in story 4 (upsert-replaces, append-preserves, typed-
//! absence, empty-filter, Send+Sync) is re-run against a freshly-constructed
//! `SurrealStore` backed by an embedded SurrealDB at a temp directory, and
//! every assertion that passed for `MemStore` also passes for `SurrealStore`.
//! Without this test, we have two store implementations that are only
//! "alike" by convention — at which point any consumer relying on the trait
//! will eventually discover a divergence in production.
//!
//! Written against `dyn Store` deliberately: the only line that mentions
//! `SurrealStore` is the constructor — every assertion below is the same
//! shape used in the corresponding MemStore tests.

use std::sync::Arc;
use std::thread;

use agentic_store::{Store, SurrealStore};
use serde_json::json;
use tempfile::TempDir;

fn open_store() -> (TempDir, Box<dyn Store>) {
    let dir = TempDir::new().expect("create temp dir");
    let store = SurrealStore::open(dir.path()).expect("open SurrealStore at fresh temp dir");
    (dir, Box::new(store))
}

#[test]
fn upsert_by_key_replaces_against_surrealstore() {
    let (_dir, store) = open_store();

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
    assert_eq!(got, json!({ "v": 2 }));
}

#[test]
fn append_preserves_writes_against_surrealstore() {
    let (_dir, store) = open_store();

    store
        .append("signings", json!({ "verdict": "Pass", "seq": 1 }))
        .expect("first append should succeed");
    store
        .append("signings", json!({ "verdict": "Fail", "seq": 2 }))
        .expect("second append should succeed");
    store
        .append("signings", json!({ "verdict": "Pass", "seq": 3 }))
        .expect("third append should succeed");

    let rows = store
        .query("signings", &|_| true)
        .expect("query should succeed");

    assert_eq!(
        rows.len(),
        3,
        "three appends must yield three rows; got {rows:?}"
    );
    assert_eq!(rows[0]["seq"], json!(1));
    assert_eq!(rows[1]["seq"], json!(2));
    assert_eq!(rows[2]["seq"], json!(3));
    assert_eq!(rows[0]["verdict"], json!("Pass"));
    assert_eq!(rows[1]["verdict"], json!("Fail"));
    assert_eq!(rows[2]["verdict"], json!("Pass"));
}

#[test]
fn get_missing_is_typed_absence_against_surrealstore() {
    let (_dir, store) = open_store();

    let missing_table = store
        .get("never_existed", "k")
        .expect("get on unknown table must NOT return an error");
    assert!(missing_table.is_none());

    store
        .upsert("exists", "a", json!({ "v": 1 }))
        .expect("upsert should succeed");
    let missing_key = store
        .get("exists", "b")
        .expect("get on unknown key must NOT return an error");
    assert!(missing_key.is_none());

    assert_eq!(
        missing_table, missing_key,
        "missing-table and missing-key absences must be indistinguishable at the trait level"
    );
}

#[test]
fn empty_filter_returns_empty_collection_against_surrealstore() {
    let (_dir, store) = open_store();

    store.append("events", json!({ "kind": "pass" })).unwrap();
    store.append("events", json!({ "kind": "fail" })).unwrap();

    let none = store
        .query("events", &|doc| {
            doc["kind"] == json!("this-value-never-appears")
        })
        .expect("empty-result query must NOT be an error");
    assert!(
        none.is_empty(),
        "filter matching nothing must return empty Vec; got {none:?}"
    );

    let unknown = store
        .query("table_never_written", &|_| true)
        .expect("query on unknown table must NOT be an error");
    assert!(
        unknown.is_empty(),
        "query on unknown table must return empty; got {unknown:?}"
    );
}

#[test]
fn arc_dyn_surrealstore_is_send_sync_and_shared_across_threads() {
    let dir = TempDir::new().expect("create temp dir");
    let store: Arc<dyn Store + Send + Sync> =
        Arc::new(SurrealStore::open(dir.path()).expect("open SurrealStore"));

    let s1 = Arc::clone(&store);
    let t1 = thread::spawn(move || {
        s1.append("signings", json!({ "thread": 1 }))
            .expect("thread 1 append should succeed");
    });
    let s2 = Arc::clone(&store);
    let t2 = thread::spawn(move || {
        s2.append("signings", json!({ "thread": 2 }))
            .expect("thread 2 append should succeed");
    });
    t1.join().expect("thread 1 panicked");
    t2.join().expect("thread 2 panicked");

    let rows = store
        .query("signings", &|_| true)
        .expect("query should succeed");
    assert_eq!(
        rows.len(),
        2,
        "two threads each appending once must yield two rows; got {rows:?}"
    );

    let mut thread_ids: Vec<i64> = rows
        .iter()
        .map(|r| {
            r["thread"]
                .as_i64()
                .expect("thread field must be an integer")
        })
        .collect();
    thread_ids.sort();
    assert_eq!(thread_ids, vec![1, 2]);
}
