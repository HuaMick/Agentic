//! Story 5 acceptance test: restored `uat_signings` rows survive a drop
//! and reopen of the `SurrealStore`.
//!
//! Justification (from stories/5.yml acceptance.tests[6]):
//!   Proves the snapshot/restore primitives compose with
//!   `SurrealStore`'s durability contract: given a fresh
//!   `SurrealStore` rooted at a temp directory, a call to
//!   `Store::restore(snapshot)` carrying three ancestor signings,
//!   followed by a drop of the store and a fresh `SurrealStore::open`
//!   against the same root, the three restored rows are still
//!   present with byte-identical content. This is the specifically-
//!   SurrealStore invariant — `MemStore` cannot exhibit it because
//!   it is in-memory by design — and it is what makes `restore`
//!   meaningful as the sandbox's seeding primitive: the container's
//!   embedded store is restored once, then operates across any
//!   number of internal process lifecycles against that seed.
//!   Without this, a sandbox that restarts its embedded store mid-
//!   run (e.g. after an internal crash the runtime recovers from)
//!   would silently lose its ancestor signings and the next
//!   ancestor-gate check would refuse as `AncestorNotHealthy`
//!   against a store that LOOKS empty but was seeded minutes
//!   earlier. Durability of the restore is not a free consequence
//!   of durability-of-writes — the restore's write path must
//!   actually commit, and this test pins that.
//!
//! Red today: compile-red. The trait's `restore` method, the
//! `StoreSnapshot` type, and any supporting public API for
//! constructing a snapshot without first going through a source
//! store are not declared; story 5 depends on story 4's amendment
//! landing the trait extension. This scaffold fails `cargo check`
//! until both the trait methods and the SurrealStore-side commit
//! semantics catch up.

use agentic_store::{Store, StoreSnapshot, SurrealStore};
use serde_json::json;
use tempfile::TempDir;

const GRAND_ID: i64 = 5201;
const PARENT_ID: i64 = 5202;
const AUNT_ID: i64 = 5203;
const LEAF_ID: i64 = 5204;

#[test]
fn restored_signings_survive_surrealstore_drop_and_reopen() {
    let root = TempDir::new().expect("create temp dir for dest SurrealStore");

    // Build a snapshot carrying three ancestor signings by going
    // through a source store on a separate temp dir — the
    // `StoreSnapshot` type does not expose a direct constructor, so
    // producing one requires seeding a source store and calling
    // `snapshot_for_story` on it.
    let snapshot: StoreSnapshot = {
        let source_dir = TempDir::new().expect("create source temp dir");
        let source: Box<dyn Store> =
            Box::new(SurrealStore::open(source_dir.path()).expect("open source SurrealStore"));

        // Seed `stories` table with ancestry graph (story 4
        // "Ancestry fixture mechanism"). Diamond: leaf depends on
        // parent+aunt; parent and aunt both depend on grand.
        source
            .append("stories", json!({"id": GRAND_ID, "depends_on": []}))
            .expect("seed grand stories row");
        source
            .append("stories", json!({"id": PARENT_ID, "depends_on": [GRAND_ID]}))
            .expect("seed parent stories row");
        source
            .append("stories", json!({"id": AUNT_ID, "depends_on": [GRAND_ID]}))
            .expect("seed aunt stories row");
        source
            .append("stories", json!({"id": LEAF_ID, "depends_on": [PARENT_ID, AUNT_ID]}))
            .expect("seed leaf stories row");

        source
            .append(
                "uat_signings",
                json!({
                    "story_id": GRAND_ID,
                    "verdict": "pass",
                    "signer": "alice@example.com",
                    "commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa5201",
                }),
            )
            .expect("seed grandparent signing");
        source
            .append(
                "uat_signings",
                json!({
                    "story_id": PARENT_ID,
                    "verdict": "pass",
                    "signer": "bob@example.com",
                    "commit": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb5202",
                }),
            )
            .expect("seed parent signing");
        source
            .append(
                "uat_signings",
                json!({
                    "story_id": AUNT_ID,
                    "verdict": "pass",
                    "signer": "carol@example.com",
                    "commit": "cccccccccccccccccccccccccccccccccccc5203",
                }),
            )
            .expect("seed aunt signing");
        source
            .snapshot_for_story(LEAF_ID)
            .expect("snapshot_for_story must succeed")
    };
    assert_eq!(
        snapshot.signings.len(),
        3,
        "source snapshot must carry three ancestor signings; got {:?}",
        snapshot.signings
    );

    // Phase 1: open the destination SurrealStore, restore the
    // snapshot, then drop the store entirely.
    {
        let dest: Box<dyn Store> =
            Box::new(SurrealStore::open(root.path()).expect("open fresh destination SurrealStore"));
        dest.restore(&snapshot)
            .expect("restore into fresh SurrealStore must succeed");
        // Sanity: pre-drop read-back sees the three rows.
        let before_drop = dest
            .query("uat_signings", &|_| true)
            .expect("pre-drop query must succeed");
        assert_eq!(
            before_drop.len(),
            3,
            "pre-drop read-back must see the three restored rows; got {before_drop:?}"
        );
        // `dest` dropped at end of block — SurrealStore's Drop impl
        // must flush any in-flight state; the test pins that this
        // flush is actually durable.
    }

    // Phase 2: open a fresh `SurrealStore` against the same root.
    // The three restored rows must still be visible with byte-
    // identical content, as if they had been written via `append` in
    // phase 1 and then read back across a reopen.
    let reopened: Box<dyn Store> = Box::new(
        SurrealStore::open(root.path()).expect("reopen SurrealStore against the same root"),
    );
    let after = reopened
        .query("uat_signings", &|_| true)
        .expect("post-reopen query must succeed");
    assert_eq!(
        after.len(),
        3,
        "three restored rows must survive drop-and-reopen of SurrealStore; got {after:?}"
    );

    let by_story: std::collections::HashMap<i64, &serde_json::Value> = after
        .iter()
        .map(|row| {
            (
                row["story_id"]
                    .as_i64()
                    .expect("post-reopen row story_id must be i64"),
                row,
            )
        })
        .collect();

    for snap_row in &snapshot.signings {
        let story_id = snap_row["story_id"]
            .as_i64()
            .expect("snapshot row story_id must be i64");
        let got = by_story
            .get(&story_id)
            .unwrap_or_else(|| panic!("post-reopen store missing row for story_id={story_id}"));
        assert_eq!(
            got["story_id"], snap_row["story_id"],
            "story_id must survive drop+reopen byte-identical for story {story_id}"
        );
        assert_eq!(
            got["verdict"], snap_row["verdict"],
            "verdict must survive drop+reopen byte-identical for story {story_id}"
        );
        assert_eq!(
            got["signer"], snap_row["signer"],
            "signer must survive drop+reopen byte-identical for story {story_id}"
        );
        assert_eq!(
            got["commit"], snap_row["commit"],
            "commit must survive drop+reopen byte-identical for story {story_id}"
        );
    }
}
