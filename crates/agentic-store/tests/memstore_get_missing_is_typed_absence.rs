//! Story 4 acceptance test: get-missing returns typed absence.
//!
//! Justification (from stories/4.yml): `get` against a (table, key) that was
//! never written returns the library's absence sentinel (`Option::None`),
//! does not panic, does not error, and does not distinguish "table does not
//! exist" from "key does not exist" at the trait level. Without this,
//! consumers have to wrap every read in panic-handlers or invent their own
//! absence encoding.
//!
//! Written against `dyn Store` deliberately.

use agentic_store::{MemStore, Store};

#[test]
fn get_on_unknown_table_returns_none_not_error() {
    let store: Box<dyn Store> = Box::new(MemStore::new());

    // Table has never been written to. Absence must be typed, not an error.
    let got = store
        .get("table_never_written", "any-key")
        .expect("get on unknown table must NOT return an error");

    assert!(
        got.is_none(),
        "get on unknown table must return None; got {got:?}"
    );
}

#[test]
fn get_on_unknown_key_in_known_table_returns_none_not_error() {
    let store: Box<dyn Store> = Box::new(MemStore::new());

    // Make the table exist by writing something else to it.
    store
        .upsert("notes", "a", serde_json::json!({ "v": 1 }))
        .expect("upsert should succeed");

    // Now read a key that was never written.
    let got = store
        .get("notes", "zzz-missing")
        .expect("get on unknown key must NOT return an error");

    assert!(
        got.is_none(),
        "get on unknown key must return None; got {got:?}"
    );
}

#[test]
fn get_does_not_distinguish_missing_table_from_missing_key() {
    // Both cases must return the same absence shape. This is the contract:
    // consumers cannot branch on "is the table missing" vs "is the key
    // missing" at the trait level — both look like `Ok(None)`.
    let store: Box<dyn Store> = Box::new(MemStore::new());

    let missing_table = store
        .get("never_existed", "k")
        .expect("must not error on missing table");
    let missing_key_in_existing_table = {
        store
            .upsert("exists", "a", serde_json::json!({ "v": 1 }))
            .expect("upsert should succeed");
        store
            .get("exists", "b")
            .expect("must not error on missing key")
    };

    assert_eq!(
        missing_table, missing_key_in_existing_table,
        "missing-table and missing-key absences must be indistinguishable at the trait level"
    );
    assert!(missing_table.is_none());
}
