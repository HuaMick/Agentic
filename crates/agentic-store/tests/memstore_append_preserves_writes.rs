//! Story 4 acceptance test: append-to-collection preserves writes.
//!
//! Justification (from stories/4.yml): three successive appends to the same
//! table yield three retrievable documents, none of which replace or mutate
//! the others, and their relative order is preserved on read-back. Without
//! this, append-only tables (uat_signings, evidence runs) have no
//! trait-level guarantee that a later write cannot silently overwrite an
//! earlier one.
//!
//! Written against `dyn Store` deliberately.

use agentic_store::{MemStore, Store};
use serde_json::json;

#[test]
fn three_appends_yield_three_rows_in_insertion_order() {
    let store: Box<dyn Store> = Box::new(MemStore::new());

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

    // Insertion order preserved.
    assert_eq!(
        rows[0]["seq"],
        json!(1),
        "first row must be the first append"
    );
    assert_eq!(
        rows[1]["seq"],
        json!(2),
        "second row must be the second append"
    );
    assert_eq!(
        rows[2]["seq"],
        json!(3),
        "third row must be the third append"
    );

    // And none of them has been mutated by later writes.
    assert_eq!(rows[0]["verdict"], json!("Pass"));
    assert_eq!(rows[1]["verdict"], json!("Fail"));
    assert_eq!(rows[2]["verdict"], json!("Pass"));
}
