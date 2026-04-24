//! Story 4 acceptance test: the empty-closure edge case.
//!
//! Justification (from stories/4.yml acceptance.tests[7]):
//!   Proves the empty-closure edge case: given a fixture story seeded
//!   into the `stories` table as `{"id": T, "depends_on": []}` per the
//!   stories-table fixture mechanism (see guidance, "Ancestry fixture
//!   mechanism"), in a store that also contains `uat_signings` rows
//!   for other unrelated stories, `Store::snapshot_for_story(T)`
//!   returns an empty `StoreSnapshot` (zero signing rows), and
//!   `Store::restore(empty_snapshot)` on a fresh `MemStore` succeeds
//!   without error and without writing any rows. A subsequent
//!   ancestor-gate query against the destination store returns the
//!   empty-ancestor-set answer (the gate is trivially satisfied for a
//!   story with no declared ancestors). Without this, story 20's
//!   happy-path UAT — a fixture story with no ancestors — either
//!   crashes at snapshot time (no ancestors treated as an error) or at
//!   restore time (empty bundle treated as malformed), and the simplest
//!   possible sandbox invocation fails for reasons unrelated to the
//!   inner loop.
//!
//! Red today: compile-red. The trait does not yet expose
//! `snapshot_for_story` or `restore`, and the `StoreSnapshot` type is
//! not declared. Story 4's amendment (triggered by story 20, refined by
//! the fixture-mechanism follow-up) lands all three. Ancestry flows
//! through the `stories` table seeded inline — no filesystem, no env
//! var.

use agentic_store::{MemStore, Store, StoreSnapshot};
use serde_json::json;

const TARGET_STORY_ID: i64 = 4201; // depends_on: []
const UNRELATED_A: i64 = 4210;
const UNRELATED_B: i64 = 4211;

#[test]
fn no_ancestor_story_snapshots_to_empty_bundle_and_restores_as_noop() {
    // Seed a store with signings for UNRELATED stories only, and declare
    // the target story's empty-depends_on row in the `stories` table per
    // the fixture mechanism pinned in story 4's guidance.
    let source: Box<dyn Store> = Box::new(MemStore::new());

    source
        .append(
            "stories",
            json!({ "id": TARGET_STORY_ID, "depends_on": [] }),
        )
        .expect("seed target story row (depends_on: [])");
    // Unrelated story rows — present in the stories table but NOT part
    // of the target's closure. Proves the walker does not vacuum up
    // unrelated story rows just because they exist.
    source
        .append("stories", json!({ "id": UNRELATED_A, "depends_on": [] }))
        .expect("seed unrelated A story row");
    source
        .append("stories", json!({ "id": UNRELATED_B, "depends_on": [] }))
        .expect("seed unrelated B story row");

    source
        .append(
            "uat_signings",
            json!({
                "story_id": UNRELATED_A,
                "verdict": "pass",
                "signer": "alice@example.com",
                "commit": "3333333333333333333333333333333333334210",
            }),
        )
        .expect("seed unrelated A signing");
    source
        .append(
            "uat_signings",
            json!({
                "story_id": UNRELATED_B,
                "verdict": "pass",
                "signer": "bob@example.com",
                "commit": "4444444444444444444444444444444444444211",
            }),
        )
        .expect("seed unrelated B signing");

    // Snapshot for the no-ancestor target story. Success path: the
    // bundle is empty (zero signings), NOT an error.
    let snapshot: StoreSnapshot = source
        .snapshot_for_story(TARGET_STORY_ID)
        .expect("snapshot_for_story on a no-ancestor story must succeed");

    assert!(
        snapshot.signings.is_empty(),
        "snapshot of a no-ancestor story must carry zero signing rows; got {:?}",
        snapshot.signings
    );
    assert_eq!(
        snapshot.schema_version, 1,
        "StoreSnapshot.schema_version must be 1 even for an empty bundle"
    );

    // Restore the empty bundle into a fresh destination. Success path:
    // restore completes, the destination's uat_signings table is still
    // empty, no error.
    let dest: Box<dyn Store> = Box::new(MemStore::new());
    dest.restore(&snapshot)
        .expect("restore of an empty snapshot must succeed (not an error)");

    let restored_rows = dest
        .query("uat_signings", &|_| true)
        .expect("query on destination after empty restore must succeed");
    assert!(
        restored_rows.is_empty(),
        "destination must remain empty after restoring an empty snapshot; got {restored_rows:?}"
    );

    // Ancestor-gate query for the no-ancestor case: the gate is trivially
    // satisfied because there are no ancestors to check. Expressed here
    // as "no pass rows exist in the restored store for any ancestor",
    // which is the right empty-set answer.
    let any_pass = dest
        .query("uat_signings", &|row| row["verdict"] == json!("pass"))
        .expect("ancestor-gate-shaped query must not error on an empty store");
    assert!(
        any_pass.is_empty(),
        "no-ancestor empty-restore case must produce zero pass signings on the destination; got {any_pass:?}"
    );
}
