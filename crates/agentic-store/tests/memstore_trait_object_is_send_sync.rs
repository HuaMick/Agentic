//! Story 4 acceptance test: trait object is Send + Sync.
//!
//! Justification (from stories/4.yml): `MemStore` can be held behind
//! `Arc<dyn Store + Send + Sync>` and used from more than one thread without
//! introducing data races at the trait boundary. Without this being pinned
//! here, every consumer (UAT gate, CI recorder, dashboard) would make its
//! own assumption about shareability and the disagreement would only
//! surface when they were composed in the CLI.
//!
//! Written against `dyn Store` deliberately.

use agentic_store::{MemStore, Store};
use serde_json::json;
use std::sync::Arc;
use std::thread;

#[test]
fn static_assertion_memstore_is_send_and_sync() {
    fn require_send_sync<T: Send + Sync>() {}
    require_send_sync::<MemStore>();
}

#[test]
fn arc_dyn_store_compiles_and_holds_memstore() {
    // Compile-time check: the trait object form the consumers rely on must
    // actually compile. If `Store` is not object-safe, or if `MemStore` is
    // not Send + Sync, this fails to build.
    let store: Arc<dyn Store + Send + Sync> = Arc::new(MemStore::new());

    store
        .upsert("notes", "a", json!({ "v": 1 }))
        .expect("upsert through Arc<dyn Store> should succeed");
    let got = store
        .get("notes", "a")
        .expect("get through Arc<dyn Store> should succeed");
    assert_eq!(got, Some(json!({ "v": 1 })));
}

#[test]
fn two_threads_can_append_through_shared_arc() {
    let store: Arc<dyn Store + Send + Sync> = Arc::new(MemStore::new());

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

    // Both thread ids must be present; we don't care about order.
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
