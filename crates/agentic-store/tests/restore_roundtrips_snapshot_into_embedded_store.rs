//! Story 20 acceptance test: the restore primitive round-trips a
//! `StoreSnapshot` produced by `snapshot_for_story` into a fresh
//! destination store, satisfies the ancestor gate on the restored
//! rows, and refuses a second restore call (one-shot seeding
//! semantics).
//!
//! Justification (from stories/20.yml acceptance.tests[2]):
//!   Proves the restore primitive's idempotence and
//!   completeness: given a `StoreSnapshot` produced by
//!   `snapshot_for_story` on a seeded source store, a
//!   fresh `MemStore` whose `runs` and `uat_signings`
//!   tables are empty, and a call to
//!   `Store::restore(snapshot)`, the destination store's
//!   `uat_signings` table ends with exactly the rows the
//!   snapshot carried (same `signer`, `verdict`, `commit`,
//!   and `story_id` values), and a subsequent
//!   ancestor-gate query (e.g. `has_pass_verdict(mid_id)`)
//!   returns true. A second `restore` call with the same
//!   snapshot is rejected with `StoreError::AlreadyRestored`
//!   rather than silently double-writing — restore is a
//!   one-shot seeding operation, not an append loop.
//!
//! Red today: compile-red via the missing snapshot/restore surface on
//! the `Store` trait (`snapshot_for_story`, `restore`, the
//! `StoreSnapshot` type, and the `StoreError::AlreadyRestored`
//! variant). Story 20 depends on the story-4 + story-5 amendments
//! landing those methods; this test probes the primitive from the
//! embedded-store (MemStore) side which is sufficient for red
//! evidence on the current tree.

use agentic_store::{MemStore, Store, StoreError, StoreSnapshot};
use serde_json::json;

#[test]
fn restore_seeds_destination_store_and_second_restore_is_typed_refusal() {
    // Build the source store and seed three uat_signings rows whose
    // story_ids form a small ancestry: B (root), A (depends on B),
    // and the target (depends on A). The snapshot for the target
    // should carry the closure of A and B.
    let source: MemStore = MemStore::new();
    source
        .append(
            "uat_signings",
            json!({
                "story_id": 1001,
                "verdict": "pass",
                "signer": "alice@example.com",
                "commit": "0000000000000000000000000000000000000001",
            }),
        )
        .expect("seed ancestor B");
    source
        .append(
            "uat_signings",
            json!({
                "story_id": 1002,
                "verdict": "pass",
                "signer": "bob@example.com",
                "commit": "0000000000000000000000000000000000000002",
            }),
        )
        .expect("seed ancestor A");

    // Produce a snapshot for the target story (id 1003). The
    // snapshot closure includes A (1002) and B (1001) but NOT the
    // target itself.
    let snapshot: StoreSnapshot = source
        .snapshot_for_story(1003)
        .expect("snapshot_for_story must succeed on a seeded source store");

    // Fresh destination store, empty tables.
    let dest: MemStore = MemStore::new();
    assert!(
        dest.query("uat_signings", &|_| true)
            .expect("empty query")
            .is_empty(),
        "dest store must start empty"
    );

    dest.restore(&snapshot)
        .expect("first restore must succeed on an empty store");

    // Destination carries exactly the rows the snapshot carried.
    let restored = dest
        .query("uat_signings", &|_| true)
        .expect("query restored rows");
    assert_eq!(
        restored.len(),
        2,
        "restored uat_signings must equal the snapshot closure (A + B); got {restored:?}"
    );

    // Row-level fidelity: story_id / verdict / signer / commit all
    // round-trip byte-identically.
    let by_story: std::collections::HashMap<i64, &serde_json::Value> = restored
        .iter()
        .map(|r| (r["story_id"].as_i64().expect("story_id i64"), r))
        .collect();
    let row_b = by_story.get(&1001).expect("row for story 1001 (B)");
    assert_eq!(row_b["verdict"], json!("pass"));
    assert_eq!(row_b["signer"], json!("alice@example.com"));
    assert_eq!(
        row_b["commit"],
        json!("0000000000000000000000000000000000000001")
    );
    let row_a = by_story.get(&1002).expect("row for story 1002 (A)");
    assert_eq!(row_a["verdict"], json!("pass"));
    assert_eq!(row_a["signer"], json!("bob@example.com"));

    // Second restore: one-shot semantics refuse.
    let second = dest.restore(&snapshot);
    match second {
        Err(StoreError::AlreadyRestored) => {}
        other => panic!(
            "second restore must return StoreError::AlreadyRestored; got {other:?}"
        ),
    }

    // After the refusal, the store state must not have doubled.
    let after = dest
        .query("uat_signings", &|_| true)
        .expect("query after refusal");
    assert_eq!(
        after.len(),
        2,
        "AlreadyRestored must not append additional rows; got {after:?}"
    );
}
