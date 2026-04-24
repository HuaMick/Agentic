//! Story 5 acceptance test: `SurrealStore::restore` round-trips a
//! `StoreSnapshot` into a fresh destination store and refuses a second
//! restore call with `StoreError::AlreadyRestored` — the one-shot
//! seeding semantics the sandbox relies on, mirrored against the
//! production backend.
//!
//! Justification (from stories/5.yml acceptance.tests[5]):
//!   Proves the restore primitive's idempotence and completeness hold
//!   for `SurrealStore`: given a `StoreSnapshot` produced by
//!   `snapshot_for_story` on a seeded source store, a fresh
//!   `SurrealStore` rooted at a distinct temp directory whose `runs`
//!   and `uat_signings` tables are empty, and a call to
//!   `Store::restore(snapshot)`, the destination store's
//!   `uat_signings` table ends with exactly the rows the snapshot
//!   carried (same `signer`, `verdict`, `commit`, and `story_id`
//!   values), and a subsequent ancestor-gate query (e.g.
//!   `has_pass_verdict(mid_id)`) returns true. A second `restore`
//!   call with the same snapshot is rejected with
//!   `StoreError::AlreadyRestored` rather than silently double-
//!   writing. Without this, story 20's container-side seeding step
//!   (which constructs a `SurrealStore` and restores the host-
//!   computed snapshot into it) has no trait-level guarantee that
//!   the ancestor gate fires correctly inside the sandbox — and a
//!   backend-specific divergence would surface as either a spurious
//!   gate refusal or a silent double-seed on retry.
//!
//! Red today: compile-red. The trait's `snapshot_for_story` /
//! `restore` methods, the `StoreSnapshot` type, and the
//! `StoreError::AlreadyRestored` variant are not declared; story 5
//! depends on story 4's amendment landing them. This scaffold fails
//! `cargo check` until the SurrealStore implementation catches up.

use agentic_store::{Store, StoreError, StoreSnapshot, SurrealStore};
use serde_json::json;
use tempfile::TempDir;

const ROOT_ID: i64 = 5101;
const MID_ID: i64 = 5102;
const LEAF_ID: i64 = 5103;

#[test]
fn surreal_restore_roundtrips_signings_and_second_restore_is_already_restored() {
    // Source store: two ancestor signings on disk under a temp root.
    let source_dir = TempDir::new().expect("create source temp dir");
    let source: Box<dyn Store> = Box::new(
        SurrealStore::open(source_dir.path()).expect("open source SurrealStore at fresh temp dir"),
    );

    // Seed `stories` table with ancestry graph (story 4 "Ancestry
    // fixture mechanism").
    source
        .append("stories", json!({"id": ROOT_ID, "depends_on": []}))
        .expect("seed root stories row");
    source
        .append("stories", json!({"id": MID_ID, "depends_on": [ROOT_ID]}))
        .expect("seed mid stories row");
    source
        .append("stories", json!({"id": LEAF_ID, "depends_on": [MID_ID]}))
        .expect("seed leaf stories row");

    source
        .append(
            "uat_signings",
            json!({
                "story_id": ROOT_ID,
                "verdict": "pass",
                "signer": "alice@example.com",
                "commit": "1111111111111111111111111111111111115101",
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
                "commit": "2222222222222222222222222222222222225102",
            }),
        )
        .expect("seed mid signing");

    let snapshot: StoreSnapshot = source
        .snapshot_for_story(LEAF_ID)
        .expect("snapshot_for_story on a seeded source SurrealStore must succeed");
    assert_eq!(
        snapshot.signings.len(),
        2,
        "source snapshot must carry exactly root + mid; got {:?}",
        snapshot.signings
    );

    // Destination store: a distinct temp dir, empty tables to start.
    let dest_dir = TempDir::new().expect("create dest temp dir");
    let dest: Box<dyn Store> = Box::new(
        SurrealStore::open(dest_dir.path()).expect("open dest SurrealStore at fresh temp dir"),
    );
    let pre = dest
        .query("uat_signings", &|_| true)
        .expect("dest query on empty store must not error");
    assert!(
        pre.is_empty(),
        "destination store must start with no uat_signings rows; got {pre:?}"
    );

    // First restore: succeeds; destination now carries the ancestor
    // rows.
    dest.restore(&snapshot)
        .expect("first restore into an empty SurrealStore must succeed");

    let restored = dest
        .query("uat_signings", &|_| true)
        .expect("dest query after restore must succeed");
    assert_eq!(
        restored.len(),
        snapshot.signings.len(),
        "destination must carry exactly the rows the snapshot shipped; snapshot={:?}, restored={:?}",
        snapshot.signings,
        restored,
    );

    // Byte-for-byte fidelity on the load-bearing fields.
    let by_story: std::collections::HashMap<i64, &serde_json::Value> = restored
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
        let dest_row = by_story
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

    // Ancestor-gate-shape query: after restore, a query for
    // (story_id=MID, verdict=pass) must find exactly one row.
    let mid_pass = dest
        .query("uat_signings", &|row| {
            row["story_id"].as_i64() == Some(MID_ID) && row["verdict"] == json!("pass")
        })
        .expect("ancestor-gate query must succeed");
    assert_eq!(
        mid_pass.len(),
        1,
        "ancestor-gate query for story_id={MID_ID} verdict=pass must find exactly one row post-restore; got {mid_pass:?}"
    );

    // Second restore call with the same snapshot: one-shot semantics
    // require a typed refusal, not a silent double-write.
    let second = dest.restore(&snapshot);
    match second {
        Err(StoreError::AlreadyRestored) => {}
        other => panic!(
            "second restore must return StoreError::AlreadyRestored on a SurrealStore with existing uat_signings rows; got {other:?}"
        ),
    }

    // The refusal must NOT have appended rows.
    let after = dest
        .query("uat_signings", &|_| true)
        .expect("dest query after AlreadyRestored refusal must succeed");
    assert_eq!(
        after.len(),
        snapshot.signings.len(),
        "AlreadyRestored must not append additional rows; got {after:?}"
    );
}
