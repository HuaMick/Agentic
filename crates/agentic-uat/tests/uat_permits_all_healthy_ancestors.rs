//! Story 11 acceptance test: non-interference guarantee — all-healthy
//! ancestry permits the Pass happy path.
//!
//! Justification (from stories/11.yml):
//! Proves the non-interference guarantee: given a story `<id>` whose
//! `depends_on` lists only ancestors whose on-disk status is `healthy`
//! AND each of those ancestors has a valid `uat_signings.verdict=pass`
//! row in the store (not just the YAML claim), `Uat::run` with a Pass
//! verdict proceeds through its usual path, writes exactly one
//! `uat_signings.verdict=pass` row, and rewrites the target story YAML
//! to `status: healthy` — same as story 1's happy path. Without this,
//! the ancestor-gate could accidentally refuse every Pass (a silent
//! catastrophic tightening indistinguishable, to the operator, from
//! "UAT is broken").
//!
//! Red today is compile-red via the missing `UatError::AncestorNotHealthy`
//! public surface — this test's `match err` arm against that variant
//! refers to a type that does not yet exist. (A green `Uat::run` path
//! would still observably refuse under story 11's gate even when the
//! ancestor's signing row is valid, because the gate is not yet
//! implemented; this test pins the NON-interference case so the gate's
//! implementer proves they did not refuse every Pass.)

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_uat::{StubExecutor, Uat, UatError, Verdict};
use serde_json::json;
use tempfile::TempDir;

const LEAF_ID: u32 = 11101;
const ANCESTOR_ID: u32 = 11102;

const LEAF_YAML: &str = r#"id: 11101
title: "Leaf story whose ancestor is healthy with a valid signing row"

outcome: |
  A fixture leaf that should proceed through the Pass path because
  its ancestor is healthy with a valid signing row.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_permits_all_healthy_ancestors.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Run the stub executor; expect Pass.

guidance: |
  Fixture authored inline for the story-11 non-interference scaffold.
  Not a real story.

depends_on:
  - 11102
"#;

const ANCESTOR_YAML: &str = r#"id: 11102
title: "Healthy ancestor with a valid uat_signings row"

outcome: |
  An ancestor fixture that is healthy on disk AND has a valid signing
  row in the store, so the leaf's Pass is permitted.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_permits_all_healthy_ancestors.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the valid-healthy ancestor.

depends_on: []
"#;

#[test]
fn uat_run_pass_permits_when_every_ancestor_is_healthy_with_valid_signing_row() {
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

    // Pre-seed a valid uat_signings row for the ancestor so the gate sees
    // evidence (not just the YAML claim).
    store
        .append(
            "uat_signings",
            json!({
                "id": "seeded-ancestor-signing-row",
                "story_id": ANCESTOR_ID,
                "verdict": "pass",
                "commit": head_sha,
                "signed_at": "2026-04-20T00:00:00Z",
            }),
        )
        .expect("seed ancestor signing row");

    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    // The gate must NOT refuse — this is the non-interference guarantee.
    let verdict = match uat.run(LEAF_ID) {
        Ok(v) => v,
        // Any refusal is a failure of the non-interference guarantee.
        Err(UatError::AncestorNotHealthy { ancestor_id, .. }) => panic!(
            "non-interference violated: Pass was refused with \
             AncestorNotHealthy naming ancestor_id={ancestor_id}, but \
             the ancestor is healthy with a valid signing row"
        ),
        Err(other) => panic!(
            "non-interference violated: Pass must proceed to a Pass verdict \
             when every ancestor is healthy with a valid signing row; got \
             unexpected error {other:?}"
        ),
    };
    assert!(
        matches!(verdict, Verdict::Pass),
        "stub-always-pass with healthy ancestry must yield Pass; got {verdict:?}"
    );

    // Exactly one NEW signing row for the leaf (the seeded ancestor row
    // is separate).
    let leaf_rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(LEAF_ID as u64)
        })
        .expect("store query should succeed");
    assert_eq!(
        leaf_rows.len(),
        1,
        "Pass must write exactly one uat_signings row for the leaf; got {} rows: {leaf_rows:?}",
        leaf_rows.len()
    );
    assert_eq!(
        leaf_rows[0].get("verdict").and_then(|v| v.as_str()),
        Some("pass"),
        "leaf signing row must carry verdict=\"pass\"; got {}",
        leaf_rows[0]
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
        cfg.set_str("user.name", "test-builder").expect("set user.name");
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
