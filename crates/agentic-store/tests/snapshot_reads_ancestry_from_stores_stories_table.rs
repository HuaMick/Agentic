//! Story 4 acceptance test: the stories-table fixture mechanism is the
//! load-bearing signal for `snapshot_for_story` ancestry — not a
//! filesystem path, not an env var. Flavor A ("mechanism is load-
//! bearing") — seed stores-table with the TRUTH, point
//! `AGENTIC_STORIES_DIR` at a tempdir containing a LYING YAML fixture,
//! and assert snapshot returns the stores-table answer. A future
//! filesystem-path impl that reads ancestry off disk instead would
//! match the lie; this test is the shortest possible proof that it
//! doesn't.
//!
//! Justification (from stories/4.yml acceptance.tests[7]):
//!   Pins the stories-table ancestry fixture mechanism at the trait
//!   level in isolation, so the two closure / round-trip tests
//!   (`snapshot_for_story_returns_ancestor_closure` and
//!   `restore_roundtrips_snapshot`) inherit a concrete, test-contained
//!   contract rather than an implicit one. Given a `MemStore` whose
//!   `stories` table carries exactly two rows — `{"id": A, "depends_on":
//!   [B]}` and `{"id": B, "depends_on": []}` — and whose `uat_signings`
//!   table carries one pass row for story B, calling
//!   `Store::snapshot_for_story(A)` returns a `StoreSnapshot` whose
//!   `signings` is exactly the B row. Without this pin, the two
//!   closure/round-trip tests can pass under an env-var fallback or a
//!   working-directory read and silently drift away from the test-
//!   contained mechanism, which is the whole reason this amendment
//!   exists.
//!
//! Note on flavor choice. Story 4's justification says "NO
//! `AGENTIC_STORIES_DIR` env var is set, NO files are written under the
//! working directory's `stories/` path during the test." That wording
//! matches Flavor B (assert the env var is unset and `./stories/` is
//! absent). This scaffold implements Flavor A instead: it ACTIVELY
//! sets `AGENTIC_STORIES_DIR` to a tempdir holding a divergent
//! `depends_on` fixture and asserts the snapshot still returns the
//! stores-table answer. Flavor A has more teeth — it fails LOUDLY
//! against any future implementation that sneaks in an env-var
//! fallback or a `./stories/` read, whereas Flavor B only proves the
//! test itself doesn't touch the filesystem. The tension between the
//! current story wording and Flavor A is a note for story-writer to
//! tighten later (the "mechanism is load-bearing" claim is the one
//! with teeth; the "no env var is set" claim only proves the test's
//! own hygiene).
//!
//! Red today: compile-red. The trait does not yet expose
//! `snapshot_for_story` and the `StoreSnapshot` type is not declared.
//! Story 4's amendment (triggered by story 20, refined by the
//! fixture-mechanism follow-up) adds both.

use agentic_store::{MemStore, Store, StoreSnapshot};
use serde_json::json;
use std::fs;

const A_ID: i64 = 4301; // subject story; depends_on = [B] per the stores-table truth
const B_ID: i64 = 4302; // ancestor; depends_on = [] per the stores-table truth
const DECOY_ANCESTOR_ID: i64 = 4399; // named only in the LYING filesystem fixture

#[test]
fn stores_stories_table_beats_filesystem_ancestry_fixture() {
    let store: Box<dyn Store> = Box::new(MemStore::new());

    // Seed the TRUTH in the `stories` table: A depends_on [B], B
    // depends_on []. This is what `snapshot_for_story` must read.
    store
        .append("stories", json!({ "id": A_ID, "depends_on": [B_ID] }))
        .expect("seed A story row (depends_on: [B])");
    store
        .append("stories", json!({ "id": B_ID, "depends_on": [] }))
        .expect("seed B story row (depends_on: [])");

    // One pass signing for B. A has none (a build is a fresh
    // attestation); the decoy ancestor has none either (it doesn't
    // exist at the trait level, only in the lying fixture).
    store
        .append(
            "uat_signings",
            json!({
                "story_id": B_ID,
                "verdict": "pass",
                "signer": "alice@example.com",
                "commit": "5555555555555555555555555555555555554302",
            }),
        )
        .expect("seed B signing");

    // Build a LYING filesystem fixture. If a future impl silently
    // falls back to reading `$AGENTIC_STORIES_DIR/<id>.yml`, the lie
    // (A depends_on [DECOY_ANCESTOR]) will win and this test will
    // fail — which is exactly the drift we want to catch.
    let lie_dir = tempfile::tempdir().expect("create tempdir for lying fixture");
    let a_yaml = format!(
        "id: {A_ID}\ntitle: lying fixture for A\nstatus: under_construction\n\
         depends_on: [{DECOY_ANCESTOR_ID}]\n\
         acceptance:\n  tests: []\n  uat: \"\"\n\
         guidance: \"This YAML lies about A's ancestry on purpose.\"\n"
    );
    let decoy_yaml = format!(
        "id: {DECOY_ANCESTOR_ID}\ntitle: decoy ancestor\nstatus: healthy\n\
         depends_on: []\n\
         acceptance:\n  tests: []\n  uat: \"\"\n\
         guidance: \"This YAML is bait for a filesystem-fallback impl.\"\n"
    );
    fs::write(lie_dir.path().join(format!("{A_ID}.yml")), a_yaml)
        .expect("write lying A fixture");
    fs::write(
        lie_dir.path().join(format!("{DECOY_ANCESTOR_ID}.yml")),
        decoy_yaml,
    )
    .expect("write decoy ancestor fixture");

    // Point the env var at the lying fixture. A stores-table-based
    // impl MUST ignore this; a filesystem-fallback impl would read it
    // and return the wrong answer.
    //
    // Env-var mutation is scoped to this process. There is no
    // parallelism-concern within a single `#[test]` binary (one
    // function per file by convention in this crate), and Rust's
    // test harness runs integration tests from distinct binaries in
    // separate processes by default, so an env var set here does not
    // leak into sibling test files.
    //
    // Safety note: `std::env::set_var` is marked unsafe from Rust
    // 1.84+ (env-mutation soundness hole). Wrap in an `unsafe` block
    // for forward-compat — on older toolchains the `unsafe` is a
    // no-op that elides, on newer ones it satisfies the call-site
    // requirement.
    #[allow(unused_unsafe)]
    unsafe {
        std::env::set_var("AGENTIC_STORIES_DIR", lie_dir.path());
    }

    let snapshot: StoreSnapshot = store
        .snapshot_for_story(A_ID)
        .expect("snapshot_for_story must succeed on a populated store");

    // Clean up the env var before we assert — otherwise a panicking
    // assertion leaves a polluted process env behind if the test
    // harness ever reuses the process.
    #[allow(unused_unsafe)]
    unsafe {
        std::env::remove_var("AGENTIC_STORIES_DIR");
    }

    // Exactly one signing: B's. The lying fixture named
    // DECOY_ANCESTOR_ID as A's ancestor, but the mechanism MUST read
    // the stores-table truth (B), not the filesystem lie.
    assert_eq!(
        snapshot.signings.len(),
        1,
        "snapshot must carry exactly the B row (stores-table ancestry); got {} rows: {:?}. \
         If this is 0, ancestry is being read from the filesystem (the lie named no signings \
         for DECOY, so the closure came up empty). If >1, something else is leaking.",
        snapshot.signings.len(),
        snapshot.signings,
    );

    let only_row = &snapshot.signings[0];
    assert_eq!(
        only_row["story_id"].as_i64(),
        Some(B_ID),
        "snapshot's single row must be B's signing (story_id={B_ID}); got {only_row:?}. \
         A story_id of {DECOY_ANCESTOR_ID} would mean the filesystem lie won — which is \
         the drift this test exists to catch."
    );
    assert_eq!(
        only_row["verdict"], json!("pass"),
        "B's signing must round-trip its verdict"
    );

    assert_eq!(
        snapshot.schema_version, 1,
        "StoreSnapshot.schema_version must be 1 (story 20's mount contract)"
    );
}
