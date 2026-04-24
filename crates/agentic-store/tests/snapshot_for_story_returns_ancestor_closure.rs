//! Story 4 acceptance test: `snapshot_for_story` returns the transitive-
//! ancestor closure of `uat_signings` rows, exclusively.
//!
//! Justification (from stories/4.yml acceptance.tests[5]):
//!   Proves the snapshot primitive's closure property at the trait level:
//!   given a `MemStore` seeded with a three-story fixture chain
//!   (`leaf` depends_on `mid`, `mid` depends_on `root`) where each of
//!   `mid` and `root` carries a valid `uat_signings` row (and at least
//!   one unrelated story also has a signing row in the same store),
//!   `Store::snapshot_for_story(leaf_id)` returns a `StoreSnapshot`
//!   containing exactly the signings for `mid` and `root` — the
//!   transitive-ancestor closure — and excludes the leaf's own signings
//!   and the unrelated story's signings. This is the first proof point
//!   that the snapshot primitive is selective, not a wholesale dump.
//!
//! Red today: compile-red. The trait does not yet expose
//! `snapshot_for_story` and the `StoreSnapshot` type is not declared.
//! Story 4's amendment (triggered by story 20) adds both to the trait;
//! this scaffold fails `cargo check` until that lands.
//!
//! Written against `dyn Store` deliberately: the only line that mentions
//! `MemStore` is the constructor. Story 5's `SurrealStore` mirror reuses
//! the same assertions.

use agentic_store::{MemStore, Store, StoreSnapshot};
use serde_json::json;

const ROOT_ID: i64 = 4001;
const MID_ID: i64 = 4002;
const LEAF_ID: i64 = 4003;
const UNRELATED_ID: i64 = 4999;

#[test]
fn snapshot_of_leaf_carries_mid_and_root_signings_only() {
    let store: Box<dyn Store> = Box::new(MemStore::new());

    // Root ancestor: has a pass signing.
    store
        .append(
            "uat_signings",
            json!({
                "story_id": ROOT_ID,
                "verdict": "pass",
                "signer": "alice@example.com",
                "commit": "0000000000000000000000000000000000004001",
            }),
        )
        .expect("seed root signing");

    // Mid ancestor: has a pass signing.
    store
        .append(
            "uat_signings",
            json!({
                "story_id": MID_ID,
                "verdict": "pass",
                "signer": "bob@example.com",
                "commit": "0000000000000000000000000000000000004002",
            }),
        )
        .expect("seed mid signing");

    // Leaf itself: has a signing too (e.g. from a prior attempt). It MUST
    // NOT appear in the snapshot — a build is a fresh attestation, never
    // a continuation of a prior signing.
    store
        .append(
            "uat_signings",
            json!({
                "story_id": LEAF_ID,
                "verdict": "pass",
                "signer": "carol@example.com",
                "commit": "0000000000000000000000000000000000004003",
            }),
        )
        .expect("seed leaf signing (must be excluded from snapshot)");

    // Unrelated story: has a signing. It MUST NOT appear — the sandbox
    // must not claim knowledge of unrelated corpus state.
    store
        .append(
            "uat_signings",
            json!({
                "story_id": UNRELATED_ID,
                "verdict": "pass",
                "signer": "dave@example.com",
                "commit": "0000000000000000000000000000000000004999",
            }),
        )
        .expect("seed unrelated signing (must be excluded from snapshot)");

    // Take the snapshot for the leaf. The ancestry chain
    // (leaf -> mid -> root) is the one this scaffold pins; the
    // implementation is expected to walk `depends_on` the same way
    // story 11's gate does.
    let snapshot: StoreSnapshot = store
        .snapshot_for_story(LEAF_ID)
        .expect("snapshot_for_story must succeed on a populated store");

    // Exactly two signings in the bundle: mid and root.
    assert_eq!(
        snapshot.signings.len(),
        2,
        "snapshot must carry the transitive-ancestor closure (mid + root) and no other rows; got {} rows: {:?}",
        snapshot.signings.len(),
        snapshot.signings,
    );

    // Collect story ids to make the membership assertions order-insensitive.
    let story_ids: std::collections::HashSet<i64> = snapshot
        .signings
        .iter()
        .map(|row| {
            row["story_id"]
                .as_i64()
                .expect("snapshot row must carry an integer story_id")
        })
        .collect();

    assert!(
        story_ids.contains(&ROOT_ID),
        "snapshot must include the root ancestor's signing (story_id={ROOT_ID}); got {story_ids:?}"
    );
    assert!(
        story_ids.contains(&MID_ID),
        "snapshot must include the mid ancestor's signing (story_id={MID_ID}); got {story_ids:?}"
    );
    assert!(
        !story_ids.contains(&LEAF_ID),
        "snapshot MUST NOT include the subject story's own signing (story_id={LEAF_ID}); got {story_ids:?}"
    );
    assert!(
        !story_ids.contains(&UNRELATED_ID),
        "snapshot MUST NOT include unrelated stories' signings (story_id={UNRELATED_ID}); got {story_ids:?}"
    );

    // The schema version is pinned at v1 by story 20's mount contract.
    assert_eq!(
        snapshot.schema_version, 1,
        "StoreSnapshot.schema_version must be 1 for Phase 0 (story 20's mount contract)"
    );
}
