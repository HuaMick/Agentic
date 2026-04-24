//! Story 11 acceptance test: asymmetry — `--verdict fail` bypasses the
//! ancestor gate.
//!
//! Justification (from stories/11.yml):
//! Proves the asymmetry: given a story whose ancestors are NOT all
//! healthy, `Uat::run` with a `--verdict fail` still writes the Fail
//! row to `uat_signings` as usual and does not return
//! `AncestorNotHealthy`. Without this, a story whose tests genuinely
//! regressed could not have its Fail recorded while an ancestor is
//! also broken — which would cascade a real negative signal into
//! silence and make the dashboard's "fell from grace" detection
//! unreachable for whole subtrees.
//!
//! Red today is compile-red via the test's match arm against the
//! missing `UatError::AncestorNotHealthy` variant.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_uat::{StubExecutor, Uat, UatError, Verdict};
use tempfile::TempDir;

const LEAF_ID: u32 = 11401;
const ANCESTOR_ID: u32 = 11402;

const LEAF_YAML: &str = r#"id: 11401
title: "Leaf whose Fail verdict must be recorded even with unhealthy ancestry"

outcome: |
  A fixture leaf used to exercise the Fail-verdict-bypasses-gate
  asymmetry. Fail records a real regression signal regardless of
  ancestor health.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_fail_verdict_ignores_ancestor_health.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Run the stub executor (always-fail); expect Fail recorded.

guidance: |
  Fixture authored inline for the story-11 fail-bypass scaffold.
  Not a real story.

depends_on:
  - 11402
"#;

const ANCESTOR_YAML: &str = r#"id: 11402
title: "Ancestor whose on-disk status is under_construction"

outcome: |
  Ancestor fixture whose status is deliberately not healthy.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_fail_verdict_ignores_ancestor_health.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the unhealthy ancestor.

depends_on: []
"#;

#[test]
fn uat_run_fail_verdict_bypasses_ancestor_gate_and_records_the_fail_row() {
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
    let executor = StubExecutor::always_fail();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    // Fail must NOT be gated by ancestor health — the gate is
    // asymmetric by design.
    let verdict = match uat.run(LEAF_ID) {
        Ok(v) => v,
        Err(UatError::AncestorNotHealthy { ancestor_id, .. }) => panic!(
            "asymmetry violated: Fail verdict was refused with \
             AncestorNotHealthy naming ancestor_id={ancestor_id}; \
             Fail must bypass the ancestor gate"
        ),
        Err(other) => panic!(
            "asymmetry violated: Fail verdict must proceed to Ok(Fail) \
             even when ancestors are unhealthy; got {other:?}"
        ),
    };
    assert!(
        matches!(verdict, Verdict::Fail),
        "stub-always-fail must yield a Fail verdict; got {verdict:?}"
    );

    // Exactly one signing row for the leaf with verdict=fail.
    let leaf_rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(LEAF_ID as u64)
        })
        .expect("store query should succeed");
    assert_eq!(
        leaf_rows.len(),
        1,
        "Fail must write exactly one uat_signings row for the leaf; got {} rows: {leaf_rows:?}",
        leaf_rows.len()
    );
    assert_eq!(
        leaf_rows[0].get("verdict").and_then(|v| v.as_str()),
        Some("fail"),
        "leaf signing row must carry verdict=\"fail\"; got {}",
        leaf_rows[0]
    );
    assert_eq!(
        leaf_rows[0].get("commit").and_then(|v| v.as_str()),
        Some(head_sha.as_str()),
        "leaf signing row must carry the HEAD SHA; got {}",
        leaf_rows[0]
    );

    // Leaf YAML is byte-for-byte unchanged — Fail never promotes.
    let leaf_bytes_after = fs::read(&leaf_path).expect("read leaf after run");
    assert_eq!(
        leaf_bytes_after, leaf_bytes_before,
        "Fail must not rewrite the target story YAML"
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
