//! Story 4 acceptance test: `Store::restore` round-trips a `StoreSnapshot`
//! produced by `snapshot_for_story` into a fresh destination store.
//!
//! Justification (from stories/4.yml acceptance.tests[6]):
//!   Proves the restore primitive's round-trip completeness: given a
//!   source `MemStore` whose `stories` table is seeded with a two-
//!   ancestor chain for the leaf per the stories-table fixture
//!   mechanism (see guidance, "Ancestry fixture mechanism"), and whose
//!   `uat_signings` table carries one row per ancestor, a
//!   `StoreSnapshot` produced by `snapshot_for_story` restores into a
//!   fresh destination `MemStore` such that the destination's ancestor-
//!   gate queries (e.g. "is there a Pass verdict for story `mid`?")
//!   return true for every row the snapshot carried, with `signer`,
//!   `verdict`, `commit`, and `story_id` preserved byte-for-byte.
//!   Without this, the snapshot could serialise selectively but restore
//!   into a shape the gate can't read — the container-side embedded
//!   Store would be seeded with rows that are present but invisible to
//!   the ancestor-gate helper, and story 11's invariant would silently
//!   break inside the sandbox.
//!
//! Red today: compile-red. The trait does not yet expose
//! `snapshot_for_story` or `restore`, and the `StoreSnapshot` type is
//! not declared. Story 4's amendment (triggered by story 20, refined by
//! the fixture-mechanism follow-up) lands all three.
//!
//! Distinct from `restore_roundtrips_snapshot_into_embedded_store.rs`
//! (story 20): that test exercises the one-shot semantics and the
//! `AlreadyRestored` refusal against an embedded store; this one pins
//! row-for-row fidelity (every `(signer, verdict, commit, story_id)`
//! survives the round-trip byte-identical) as a trait-level invariant
//! story 5's `SurrealStore` mirror will also inherit. Ancestry flows
//! through the `stories` table seeded inline — no filesystem, no env
//! var.

use agentic_store::{MemStore, Store, StoreSnapshot};
use serde_json::json;

const ROOT_ID: i64 = 4101;
const MID_ID: i64 = 4102;
const LEAF_ID: i64 = 4103;

#[test]
fn every_snapshot_row_round_trips_byte_for_byte_into_fresh_store() {
    // Source store with a small two-ancestor chain for the leaf. Ancestry
    // is declared in the `stories` table per the fixture mechanism pinned
    // in story 4's guidance.
    let source: Box<dyn Store> = Box::new(MemStore::new());

    source
        .append("stories", json!({ "id": ROOT_ID, "depends_on": [] }))
        .expect("seed root story row");
    source
        .append("stories", json!({ "id": MID_ID, "depends_on": [ROOT_ID] }))
        .expect("seed mid story row");
    source
        .append("stories", json!({ "id": LEAF_ID, "depends_on": [MID_ID] }))
        .expect("seed leaf story row");

    source
        .append(
            "uat_signings",
            json!({
                "story_id": ROOT_ID,
                "verdict": "pass",
                "signer": "alice@example.com",
                "commit": "1111111111111111111111111111111111114101",
            }),
        )
        .expect("seed root signing");
    source
        .append(
            "uat_signings",
            json!({
                "story_id": MID_ID,
                "verdict": "pass",
                "signer": "bob@example.com",
                "commit": "2222222222222222222222222222222222224102",
            }),
        )
        .expect("seed mid signing");

    let snapshot: StoreSnapshot = source
        .snapshot_for_story(LEAF_ID)
        .expect("snapshot_for_story on a seeded source store must succeed");

    // Fresh destination. Its uat_signings table must start empty.
    let dest: Box<dyn Store> = Box::new(MemStore::new());
    let pre = dest
        .query("uat_signings", &|_| true)
        .expect("dest query on empty store must not error");
    assert!(
        pre.is_empty(),
        "destination store must start with no uat_signings rows; got {pre:?}"
    );

    dest.restore(&snapshot)
        .expect("restore on an empty destination must succeed");

    // Every snapshot row must be present in the destination, with every
    // load-bearing field preserved byte-for-byte. The ancestor-gate
    // helper keys on (story_id, verdict); `signer` and `commit` are the
    // attestation surface and must not be rewritten.
    let restored_rows = dest
        .query("uat_signings", &|_| true)
        .expect("dest query after restore must succeed");

    assert_eq!(
        restored_rows.len(),
        snapshot.signings.len(),
        "destination must carry exactly the rows the snapshot shipped; snapshot={:?}, restored={:?}",
        snapshot.signings,
        restored_rows,
    );

    // Index both sides by story_id to compare row-for-row.
    let by_story_dest: std::collections::HashMap<i64, &serde_json::Value> = restored_rows
        .iter()
        .map(|row| {
            (
                row["story_id"]
                    .as_i64()
                    .expect("restored row story_id must be i64"),
                row,
            )
        })
        .collect();

    for snap_row in &snapshot.signings {
        let story_id = snap_row["story_id"]
            .as_i64()
            .expect("snapshot row story_id must be i64");
        let dest_row = by_story_dest
            .get(&story_id)
            .unwrap_or_else(|| panic!("destination missing row for story_id={story_id}"));

        assert_eq!(
            dest_row["story_id"], snap_row["story_id"],
            "story_id must round-trip byte-identical for story {story_id}"
        );
        assert_eq!(
            dest_row["verdict"], snap_row["verdict"],
            "verdict must round-trip byte-identical for story {story_id}"
        );
        assert_eq!(
            dest_row["signer"], snap_row["signer"],
            "signer must round-trip byte-identical for story {story_id}"
        );
        assert_eq!(
            dest_row["commit"], snap_row["commit"],
            "commit must round-trip byte-identical for story {story_id}"
        );
    }

    // The ancestor-gate helper queries by (story_id, verdict="pass").
    // After restore, the query must find the mid ancestor.
    let mid_pass_rows = dest
        .query("uat_signings", &|row| {
            row["story_id"].as_i64() == Some(MID_ID) && row["verdict"] == json!("pass")
        })
        .expect("ancestor-gate query must succeed");
    assert_eq!(
        mid_pass_rows.len(),
        1,
        "ancestor-gate query for story_id={MID_ID} with verdict=pass must find exactly one row post-restore; got {mid_pass_rows:?}"
    );
}
