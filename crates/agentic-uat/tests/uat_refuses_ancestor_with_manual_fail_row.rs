//! Story 11 acceptance test: union does NOT relax the verdict
//! requirement — a `manual_signings.verdict=fail` row is treated as
//! absence-of-Pass, NOT as satisfaction.
//!
//! Justification (from stories/11.yml):
//! Proves the union does NOT relax the verdict requirement: given a
//! descendant `<leaf>` whose `depends_on` names `<A>`, where `<A>`'s
//! YAML has `status: healthy`, `<A>` carries one `manual_signings` row
//! whose `verdict=fail` (a shape that cannot legitimately occur —
//! story 28's backfill only writes `verdict=pass` rows — but defended
//! in depth in case a future store edit or operator surgery introduces
//! it), AND `<A>` carries ZERO `uat_signings.verdict=pass` rows,
//! `Uat::run` with a Pass verdict on `<leaf>` returns
//! `UatError::AncestorNotHealthy { ancestor_id: A, reason: ... }`
//! distinguishable from the bare "no signing row" case so an operator
//! reading the refusal can tell that the latest cross-table attestation
//! is itself a Fail. The query is "does either table carry a Pass row
//! for this story?", not "does either table carry ANY row." A Fail in
//! either table is treated as absence-of-Pass for the gate's purpose.
//! Without this test, a corrupted or hand-written `manual_signings`
//! Fail row could be mistaken for satisfaction by a naive
//! `manual_signings.exists(story_id=A)` query — the same forging shape
//! the original ancestor-gate contract refused at the YAML claim level,
//! which would land at the table level as soon as `manual_signings`
//! joined the union.
//!
//! Red today is compile-red: the `AncestorUnhealthyReason` enum does
//! not yet carry a `ManualSigningLatestIsFail` variant. Story 11's
//! guidance lists the reason sub-enum as "distinguishing 'YAML status
//! != healthy' from 'no signing row' from 'signing row present but
//! latest verdict is fail'..." — the third variant is the contract this
//! test pins, named on the assertion below. Build-rust closes the gap
//! by adding the variant and routing the gate's refusal through it
//! when the latest cross-table attestation row is a Fail. Distinguishing
//! this case from the bare-NoSigningRow case is exactly what makes the
//! defense-in-depth observable: a naive `manual_signings.exists(A)`
//! implementation could not produce this variant, because it would
//! never inspect the verdict field.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_uat::{AncestorUnhealthyReason, StubExecutor, Uat, UatError};
use serde_json::json;
use tempfile::TempDir;

const LEAF_ID: u32 = 11_283;
const ANCESTOR_ID: u32 = 11_284;

const LEAF_YAML: &str = r#"id: 11283
title: "Leaf whose ancestor's only signing row is a manual_signings Fail"

outcome: |
  A fixture leaf used to exercise the union-does-not-relax-verdict
  invariant: the ancestor's only attestation row is a Fail in
  manual_signings, which must not be mistaken for satisfaction.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_ancestor_with_manual_fail_row.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Run the stub executor; expect refusal because the only cross-table
    attestation for the ancestor is a Fail.

guidance: |
  Fixture authored inline for the story-11 manual_signings-Fail
  defence-in-depth scaffold. Not a real story.

depends_on:
  - 11284
"#;

const ANCESTOR_YAML: &str = r#"id: 11284
title: "Ancestor with a manual_signings.verdict=fail row and zero uat_signings rows"

outcome: |
  An ancestor fixture whose `status: healthy` is contradicted by a
  manual_signings row that carries `verdict=fail`. No uat_signings row
  exists. The cross-table latest attestation is therefore Fail; the
  gate must refuse.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_ancestor_with_manual_fail_row.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; simulates a corrupted or hand-edited
  manual_signings row whose verdict is Fail.

depends_on: []
"#;

#[test]
fn uat_run_pass_refuses_when_ancestors_only_cross_table_signing_row_is_a_manual_fail() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let leaf_path = stories_dir.join(format!("{LEAF_ID}.yml"));
    let ancestor_path = stories_dir.join(format!("{ANCESTOR_ID}.yml"));
    fs::write(&leaf_path, LEAF_YAML).expect("write leaf fixture");
    fs::write(&ancestor_path, ANCESTOR_YAML).expect("write ancestor fixture");

    let head_sha = init_repo_and_commit_seed(repo_root);
    let leaf_bytes_before = fs::read(&leaf_path).expect("read leaf before run");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed exactly one manual_signings row whose verdict is FAIL — a
    // shape story 28's backfill cannot produce, but defended in depth
    // in case a future store edit, partial migration, or operator
    // surgery introduces it. Deliberately do NOT seed any uat_signings
    // row for the ancestor.
    store
        .append(
            "manual_signings",
            json!({
                "id": "seeded-ancestor-manual-fail-row",
                "story_id": ANCESTOR_ID,
                "verdict": "fail",
                "commit": head_sha,
                "signed_at": "2026-04-29T00:00:00Z",
                "signer": "test-builder@agentic.local",
            }),
        )
        .expect("seed ancestor manual_signings fail row");

    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    let err = uat.run(LEAF_ID).expect_err(
        "Pass when the ancestor's only cross-table attestation is a Fail must be refused",
    );

    match err {
        UatError::AncestorNotHealthy {
            ancestor_id,
            reason,
        } => {
            assert_eq!(
                ancestor_id, ANCESTOR_ID,
                "refusal must name the offending ancestor ({ANCESTOR_ID}); got {ancestor_id}"
            );
            // Story 11 guidance: the reason sub-enum distinguishes
            // "no signing row" from "signing row present but latest
            // verdict is fail". The Fail-row case warrants the latter
            // variant so an operator reading the refusal can tell the
            // gate inspected the verdict (defense in depth against a
            // naive `manual_signings.exists(A)` implementation that
            // would never read the verdict field).
            assert!(
                matches!(reason, AncestorUnhealthyReason::ManualSigningLatestIsFail),
                "refusal reason must distinguish a Fail attestation in the cross-table \
                 union from the bare NoSigningRow case; expected \
                 ManualSigningLatestIsFail, got {reason:?}"
            );
        }
        other => panic!(
            "Pass with a manual_signings.verdict=fail ancestor must return \
             UatError::AncestorNotHealthy {{ ancestor_id: {ANCESTOR_ID}, \
             reason: ManualSigningLatestIsFail }}; got {other:?}"
        ),
    }

    // No new rows written to either signing table for the leaf.
    let leaf_uat_rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(LEAF_ID as u64)
        })
        .expect("store query should succeed");
    assert!(
        leaf_uat_rows.is_empty(),
        "refusal must write zero uat_signings rows for the leaf; got {leaf_uat_rows:?}"
    );
    let leaf_manual_rows = store
        .query("manual_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(LEAF_ID as u64)
        })
        .expect("store query should succeed");
    assert!(
        leaf_manual_rows.is_empty(),
        "Uat::run must not write to manual_signings; got {leaf_manual_rows:?}"
    );

    // Leaf YAML byte-for-byte unchanged — refusal does not promote.
    let leaf_bytes_after = fs::read(&leaf_path).expect("read leaf after run");
    assert_eq!(
        leaf_bytes_after, leaf_bytes_before,
        "refusal must not rewrite the target story YAML"
    );
}

fn init_repo_and_commit_seed(root: &Path) -> String {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder")
            .expect("set user.name");
        cfg.set_str("user.email", "test@agentic.local")
            .expect("set user.email");
    }
    let mut index = repo.index().expect("repo index");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("stage all");
    index.write().expect("write index");
    let tree_oid = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_oid).expect("find tree");
    let sig = repo.signature().expect("repo signature");
    let commit_oid = repo
        .commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
    commit_oid.to_string()
}
