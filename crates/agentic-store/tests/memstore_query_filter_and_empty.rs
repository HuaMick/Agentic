//! Story 4 acceptance test: query-by-filter on populated and empty cases.
//!
//! Justification (from stories/4.yml): a filter that matches N appended
//! documents returns exactly those N documents, and a filter that matches
//! nothing returns an empty collection — NOT an error, NOT a typed absence,
//! just an empty collection. Consumers must not be forced to treat "no
//! matches" as an error path.
//!
//! Written against `dyn Store` deliberately.

use agentic_store::{MemStore, Store};
use serde_json::json;

#[test]
fn filter_matching_n_rows_returns_exactly_those_n_rows() {
    let store: Box<dyn Store> = Box::new(MemStore::new());

    store
        .append("events", json!({ "kind": "pass", "id": 1 }))
        .unwrap();
    store
        .append("events", json!({ "kind": "fail", "id": 2 }))
        .unwrap();
    store
        .append("events", json!({ "kind": "pass", "id": 3 }))
        .unwrap();
    store
        .append("events", json!({ "kind": "skip", "id": 4 }))
        .unwrap();
    store
        .append("events", json!({ "kind": "pass", "id": 5 }))
        .unwrap();

    let passes = store
        .query("events", &|doc| doc["kind"] == json!("pass"))
        .expect("query should succeed");

    assert_eq!(
        passes.len(),
        3,
        "filter matching three rows must return exactly three rows; got {passes:?}"
    );
    for row in &passes {
        assert_eq!(row["kind"], json!("pass"));
    }
}

#[test]
fn filter_matching_nothing_returns_empty_collection_not_error() {
    let store: Box<dyn Store> = Box::new(MemStore::new());

    store.append("events", json!({ "kind": "pass" })).unwrap();
    store.append("events", json!({ "kind": "fail" })).unwrap();

    let none = store
        .query("events", &|doc| {
            doc["kind"] == json!("this-value-never-appears")
        })
        .expect("empty-result query must NOT be an error");

    assert!(
        none.is_empty(),
        "filter matching nothing must return an empty Vec; got {none:?}"
    );
}

#[test]
fn query_on_unknown_table_returns_empty_collection_not_error() {
    // "Table does not exist" is not distinguished from "table exists but has
    // no matching rows" at the trait level — consistent with the typed
    // absence contract for get().
    let store: Box<dyn Store> = Box::new(MemStore::new());

    let none = store
        .query("table_never_written", &|_| true)
        .expect("query on unknown table must NOT be an error");

    assert!(
        none.is_empty(),
        "query on unknown table must return empty; got {none:?}"
    );
}
