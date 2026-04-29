//! Story 11 acceptance test: `uat_signings UNION manual_signings`
//! composition — a Pass row in `manual_signings` (the table written by
//! story 28's `agentic store backfill <id>`) satisfies the ancestor gate
//! equivalently to a Pass row in `uat_signings`.
//!
//! Justification (from stories/11.yml):
//! Proves the ancestor-gate's `uat_signings UNION manual_signings`
//! composition: given a descendant `<leaf>` whose `depends_on` names
//! `<A>`, where `<A>`'s YAML has `status: healthy`, `<A>` carries ZERO
//! `uat_signings.verdict=pass` rows, AND `<A>` carries exactly one
//! `manual_signings.verdict=pass` row at HEAD (the row shape story 28's
//! `agentic store backfill <A>` writes), `Uat::run` with a Pass verdict
//! on `<leaf>` proceeds through its usual path, writes exactly one
//! `uat_signings.verdict=pass` row for `<leaf>`, and rewrites `<leaf>`'s
//! YAML to `status: healthy`. The gate evaluates
//! `is_ancestor_satisfied(A)` by querying BOTH tables and treating
//! presence in either as "satisfied" — a `manual_signings` row
//! backfilled after a complete manual ritual is equivalent to a
//! `uat_signings` row from a real `agentic uat` invocation,
//! semantically. The audit-trail distinction (real vs backfilled) is
//! preserved at the table level so an auditor can still tell which
//! stories live under which attestation source; the ancestor-gate's
//! pass-fail decision is union-shaped because the prove-it gate is
//! satisfied either way.
//!
//! Red today is runtime-red: the ancestor gate currently queries only
//! `uat_signings`, so the seeded `manual_signings.verdict=pass` row is
//! invisible to it; with no `uat_signings.verdict=pass` row for the
//! ancestor, the gate today refuses Pass on the leaf with
//! `UatError::AncestorNotHealthy { reason: NoSigningRow }`. The
//! assertions below expect Pass to proceed, so the test fails on the
//! `match uat.run(LEAF_ID)` arm. Build-rust closes the gap by extending
//! `has_valid_signing` to consult both tables.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_uat::{StubExecutor, Uat, UatError, Verdict};
use serde_json::json;
use tempfile::TempDir;

const LEAF_ID: u32 = 11_281;
const ANCESTOR_ID: u32 = 11_282;

const LEAF_YAML: &str = r#"id: 11281
title: "Leaf whose ancestor was promoted via manual_signings backfill"

outcome: |
  A fixture leaf used to exercise the union-table semantics of the
  ancestor gate: the leaf's only ancestor stands healthy because
  `agentic store backfill` wrote a manual_signings.verdict=pass row,
  not because `agentic uat` wrote a uat_signings row.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_permits_ancestor_with_manual_signing_row.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Run the stub executor; expect Pass to proceed because the ancestor
    has a manual_signings Pass row at HEAD.

guidance: |
  Fixture authored inline for the story-11 union-table scaffold. Not a
  real story.

depends_on:
  - 11282
"#;

const ANCESTOR_YAML: &str = r#"id: 11282
title: "Ancestor promoted via agentic store backfill (manual_signings row)"

outcome: |
  An ancestor fixture whose `status: healthy` is backed by a
  manual_signings.verdict=pass row at HEAD — the row shape story 28's
  backfill writes after a complete manual ritual. There is no
  uat_signings row for this story.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_permits_ancestor_with_manual_signing_row.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; stands in for a story whose health
  attestation lives in manual_signings rather than uat_signings.

depends_on: []
"#;

#[test]
fn uat_run_pass_permits_when_ancestors_only_signing_row_lives_in_manual_signings() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let leaf_path = stories_dir.join(format!("{LEAF_ID}.yml"));
    let ancestor_path = stories_dir.join(format!("{ANCESTOR_ID}.yml"));
    fs::write(&leaf_path, LEAF_YAML).expect("write leaf fixture");
    fs::write(&ancestor_path, ANCESTOR_YAML).expect("write ancestor fixture");

    let head_sha = init_repo_and_commit_seed(repo_root);

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed exactly one manual_signings.verdict=pass row at HEAD for the
    // ancestor. Deliberately do NOT seed any uat_signings row — the
    // entire point of this test is that a manual_signings Pass row
    // satisfies the gate equivalently to a uat_signings Pass row.
    store
        .append(
            "manual_signings",
            json!({
                "id": "seeded-ancestor-manual-signing-row",
                "story_id": ANCESTOR_ID,
                "verdict": "pass",
                "commit": head_sha,
                "signed_at": "2026-04-29T00:00:00Z",
                "signer": "test-builder@agentic.local",
            }),
        )
        .expect("seed ancestor manual_signings row");

    // Sanity-check: zero uat_signings rows for the ancestor.
    let uat_rows_for_ancestor = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(ANCESTOR_ID as u64)
        })
        .expect("store query should succeed");
    assert!(
        uat_rows_for_ancestor.is_empty(),
        "fixture precondition: ancestor must carry zero uat_signings rows; \
         got {uat_rows_for_ancestor:?}"
    );

    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    // The gate must NOT refuse — the ancestor has a manual_signings
    // Pass row at HEAD, which is structurally equivalent to a
    // uat_signings Pass row at HEAD per story 11's union-table contract.
    let verdict = match uat.run(LEAF_ID) {
        Ok(v) => v,
        Err(UatError::AncestorNotHealthy {
            ancestor_id,
            reason,
        }) => panic!(
            "union-table semantics violated: Pass was refused with \
             AncestorNotHealthy {{ ancestor_id: {ancestor_id}, reason: {reason:?} }}, \
             but the ancestor carries a manual_signings.verdict=pass row at HEAD. \
             The gate must compose `uat_signings UNION manual_signings` and treat a \
             Pass row in either table as satisfaction"
        ),
        Err(other) => panic!(
            "union-table semantics violated: Pass must proceed to a Pass verdict \
             when the ancestor's only signing row lives in manual_signings; got \
             unexpected error {other:?}"
        ),
    };
    assert!(
        matches!(verdict, Verdict::Pass),
        "stub-always-pass with a manual_signings-backed ancestor must yield Pass; got {verdict:?}"
    );

    // Exactly one NEW signing row for the leaf, written to uat_signings
    // (not manual_signings — the leaf went through the real Uat::run
    // path, not backfill).
    let leaf_uat_rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(LEAF_ID as u64)
        })
        .expect("store query should succeed");
    assert_eq!(
        leaf_uat_rows.len(),
        1,
        "Pass must write exactly one uat_signings row for the leaf; got {} rows: {leaf_uat_rows:?}",
        leaf_uat_rows.len()
    );
    assert_eq!(
        leaf_uat_rows[0].get("verdict").and_then(|v| v.as_str()),
        Some("pass"),
        "leaf uat_signings row must carry verdict=\"pass\"; got {}",
        leaf_uat_rows[0]
    );

    // No new manual_signings rows were written (the leaf did not go
    // through backfill); the ancestor's seeded row is the only one.
    let leaf_manual_rows = store
        .query("manual_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(LEAF_ID as u64)
        })
        .expect("store query should succeed");
    assert!(
        leaf_manual_rows.is_empty(),
        "Uat::run must not write to manual_signings; got {leaf_manual_rows:?}"
    );

    // Leaf YAML on disk was rewritten to status: healthy.
    let rewritten = fs::read_to_string(&leaf_path).expect("re-read leaf");
    assert!(
        rewritten.contains("status: healthy"),
        "Pass promotion must rewrite leaf status to healthy; got body:\n{rewritten}"
    );
    assert!(
        !rewritten.contains("status: under_construction"),
        "Pass promotion must replace prior status, not append; got body:\n{rewritten}"
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
