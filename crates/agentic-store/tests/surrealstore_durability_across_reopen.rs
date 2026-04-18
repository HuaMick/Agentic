//! Story 5 acceptance test (RED scaffold): durability across reopen.
//!
//! Justification (from stories/5.yml):
//!   Proves durability across process lifetime: write N documents to a
//!   `SurrealStore` rooted at a temp directory, drop the store, construct
//!   a new `SurrealStore` pointing at the same directory, and verify all
//!   N documents are readable with identical content.
//!
//! Red state expected at scaffold-write time:
//!   - `agentic_store::SurrealStore` does not exist.
//!   - `tempfile` is not a dev-dependency of `agentic-store` yet.

use agentic_store::{Store, SurrealStore};
use serde_json::{json, Value};
use tempfile::TempDir;

#[test]
fn writes_survive_store_drop_and_fresh_open_against_same_root() {
    // Setup: one TempDir kept alive for the whole test so the two
    // SurrealStore instances share the same on-disk root.
    let root = TempDir::new().expect("tempdir should be creatable");

    // First process-lifetime: open, append N rows, drop.
    let expected: Vec<Value> = (0..5_i64)
        .map(|i| json!({ "n": i, "note": format!("row {i}") }))
        .collect();
    {
        let store = SurrealStore::open(root.path())
            .expect("first SurrealStore::open should succeed on empty temp dir");
        for row in &expected {
            store.append("durable", row.clone()).expect("append should succeed");
        }
        // Store dropped here: durability means what we wrote survives.
    }

    // Second process-lifetime: reopen at the same root, read back.
    let reopened =
        SurrealStore::open(root.path()).expect("second SurrealStore::open on same root should succeed");
    let got = reopened
        .query("durable", &|_| true)
        .expect("query after reopen should succeed");

    assert_eq!(
        got.len(),
        expected.len(),
        "durability: wrote {} rows, read back {}",
        expected.len(),
        got.len()
    );
    for (i, row) in expected.iter().enumerate() {
        assert_eq!(
            &got[i], row,
            "durability: row {i} must be byte-identical across reopen"
        );
    }

    panic!(
        "red: Proves durability across process lifetime: write N documents to a `SurrealStore` rooted at a temp directory, drop the store, construct a new `SurrealStore` pointing at the same directory, and verify all N documents are readable with identical content."
    );
}
