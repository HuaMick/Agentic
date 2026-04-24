//! Story 5 acceptance test: `SurrealStore::snapshot_for_story` returns the
//! transitive-ancestor closure of `uat_signings` rows, mirroring the
//! trait-level contract story 4 pins for `MemStore`.
//!
//! Justification (from stories/5.yml acceptance.tests[4]):
//!   Proves the snapshot primitive's closure property holds for
//!   `SurrealStore` with the same semantics story 4 pins for
//!   `MemStore`: given a `SurrealStore` rooted at a temp directory
//!   and seeded with `uat_signings` rows for a three-story fixture
//!   chain (`leaf` depends_on `mid`, `mid` depends_on `root`), each
//!   ancestor carrying a valid `verdict=pass` row,
//!   `Store::snapshot_for_story(leaf_id)` returns a `StoreSnapshot`
//!   containing exactly the two ancestor signings (`root` and `mid`)
//!   and does NOT contain the leaf's own signing nor signings for
//!   unrelated stories present in the same store. Two calls against
//!   the same store state return byte-identical JSON when serialised
//!   with sorted keys. Without this, the trait-parity claim that
//!   story 5 was promoted on collapses: the snapshot method added to
//!   the trait (via story 4's amendment) would behave one way for
//!   `MemStore` and an unspecified way for `SurrealStore`, and every
//!   consumer routing through `Arc<dyn Store>` would hit backend-
//!   specific surprises at the seeding boundary. This test mirrors
//!   story 4's amendment test of the same shape against the
//!   production backend.
//!
//! Red today: compile-red. The trait does not yet expose
//! `snapshot_for_story` and the `StoreSnapshot` type is not declared;
//! story 5 inherits the trait extension landed by story 4's
//! amendment. This scaffold fails `cargo check` until both the trait
//! method and the `SurrealStore` implementation land.
//!
//! Written against `dyn Store` deliberately: the only line that
//! mentions `SurrealStore` is the constructor — every assertion below
//! is the same shape used in story 4's corresponding MemStore test.

use agentic_store::{Store, StoreSnapshot, SurrealStore};
use serde_json::json;
use tempfile::TempDir;

const ROOT_ID: i64 = 5001;
const MID_ID: i64 = 5002;
const LEAF_ID: i64 = 5003;
const UNRELATED_ID: i64 = 5999;

#[test]
fn surreal_snapshot_of_leaf_carries_mid_and_root_signings_only() {
    let dir = TempDir::new().expect("create temp dir for surrealstore");
    let store: Box<dyn Store> =
        Box::new(SurrealStore::open(dir.path()).expect("open SurrealStore at fresh temp dir"));

    // Root ancestor: pass signing.
    store
        .append(
            "uat_signings",
            json!({
                "story_id": ROOT_ID,
                "verdict": "pass",
                "signer": "alice@example.com",
                "commit": "0000000000000000000000000000000000005001",
            }),
        )
        .expect("seed root signing");

    // Mid ancestor: pass signing.
    store
        .append(
            "uat_signings",
            json!({
                "story_id": MID_ID,
                "verdict": "pass",
                "signer": "bob@example.com",
                "commit": "0000000000000000000000000000000000005002",
            }),
        )
        .expect("seed mid signing");

    // Leaf itself: has a signing from a prior attempt. MUST NOT
    // appear in the snapshot — a build is a fresh attestation, not a
    // continuation of a prior signing.
    store
        .append(
            "uat_signings",
            json!({
                "story_id": LEAF_ID,
                "verdict": "pass",
                "signer": "carol@example.com",
                "commit": "0000000000000000000000000000000000005003",
            }),
        )
        .expect("seed leaf signing (must be excluded from snapshot)");

    // Unrelated story: has a signing. MUST NOT appear — the sandbox
    // must not claim knowledge of unrelated corpus state.
    store
        .append(
            "uat_signings",
            json!({
                "story_id": UNRELATED_ID,
                "verdict": "pass",
                "signer": "dave@example.com",
                "commit": "0000000000000000000000000000000000005999",
            }),
        )
        .expect("seed unrelated signing (must be excluded from snapshot)");

    // Take the snapshot for the leaf.
    let snapshot: StoreSnapshot = store
        .snapshot_for_story(LEAF_ID)
        .expect("snapshot_for_story must succeed on a populated SurrealStore");

    // Exactly two signings: mid + root.
    assert_eq!(
        snapshot.signings.len(),
        2,
        "snapshot must carry the transitive-ancestor closure (mid + root) and no other rows; got {} rows: {:?}",
        snapshot.signings.len(),
        snapshot.signings,
    );

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

    // Schema version pinned at v1 by story 20's mount contract.
    assert_eq!(
        snapshot.schema_version, 1,
        "StoreSnapshot.schema_version must be 1 for Phase 0 (story 20's mount contract)"
    );

    // Two calls against the same store state return byte-identical
    // JSON when serialised with sorted keys. This proves
    // determinism — the sandbox mount contract requires the snapshot
    // be content-addressable.
    let second: StoreSnapshot = store
        .snapshot_for_story(LEAF_ID)
        .expect("second snapshot_for_story must succeed");
    let first_bytes =
        canonical_json(&snapshot).expect("first snapshot must serialise deterministically");
    let second_bytes =
        canonical_json(&second).expect("second snapshot must serialise deterministically");
    assert_eq!(
        first_bytes, second_bytes,
        "two snapshot_for_story calls against the same state must serialise byte-identically with sorted keys"
    );
}

/// Serialise `snap` to a JSON byte vector with deterministically-sorted
/// keys at every object level. The BTreeMap intermediate gives us the
/// sorted-keys invariant without depending on serde's `preserve_order`
/// feature, which is not enabled for this crate.
fn canonical_json(snap: &StoreSnapshot) -> Result<Vec<u8>, serde_json::Error> {
    let as_value = serde_json::to_value(snap)?;
    let sorted = sort_value(&as_value);
    serde_json::to_vec(&sorted)
}

fn sort_value(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut out: std::collections::BTreeMap<String, serde_json::Value> =
                std::collections::BTreeMap::new();
            for (k, val) in map {
                out.insert(k.clone(), sort_value(val));
            }
            serde_json::to_value(out).expect("btreemap -> value is infallible")
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(sort_value).collect())
        }
        other => other.clone(),
    }
}
