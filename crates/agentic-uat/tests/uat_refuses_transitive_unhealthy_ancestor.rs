//! Story 11 acceptance test: transitive reach — the gate follows
//! `depends_on` past the direct ancestor to find the first unhealthy
//! link.
//!
//! Justification (from stories/11.yml):
//! Proves the transitive reach: given `<id>` depends_on `<A>`, `<A>`
//! depends_on `<B>`, `<A>` is `healthy` with a valid
//! `uat_signings.verdict=pass` row, but `<B>` is `under_construction`,
//! `Uat::run` on `<id>` with a Pass verdict returns
//! `UatError::AncestorNotHealthy` naming `<B>` (not `<A>`) and writes
//! no row. Without this the gate only enforces direct ancestors — a
//! pattern the legacy system's "just check depends_on" shortcut would
//! produce — and a leaf sitting two hops above a broken foundation
//! would promote.
//!
//! Red today is compile-red via the missing `UatError::AncestorNotHealthy`
//! variant.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_uat::{StubExecutor, Uat, UatError};
use serde_json::json;
use tempfile::TempDir;

const LEAF_ID: u32 = 11301; // depends on MID
const MID_ID: u32 = 11302; // healthy with valid signing; depends on ROOT
const ROOT_ID: u32 = 11303; // under_construction — the first unhealthy link

const LEAF_YAML: &str = r#"id: 11301
title: "Leaf depending on a healthy mid that depends on an unhealthy root"

outcome: |
  A fixture leaf used to exercise the transitive-ancestor refusal.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_transitive_unhealthy_ancestor.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Run the stub executor; expect transitive refusal naming ROOT.

guidance: |
  Fixture authored inline for the story-11 transitive-ancestor scaffold.
  Not a real story.

depends_on:
  - 11302
"#;

const MID_YAML: &str = r#"id: 11302
title: "Healthy mid story with a valid signing row"

outcome: |
  Mid ancestor fixture: YAML says healthy AND a valid signing row
  exists; depends_on a root that is itself under_construction.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_transitive_unhealthy_ancestor.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the valid-healthy mid link.

depends_on:
  - 11303
"#;

const ROOT_YAML: &str = r#"id: 11303
title: "Root story whose on-disk status is under_construction"

outcome: |
  Root ancestor fixture whose status is deliberately not healthy so
  the transitive gate names it as the offending link.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_transitive_unhealthy_ancestor.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the transitive unhealthy root.

depends_on: []
"#;

#[test]
fn uat_run_pass_refuses_when_transitive_ancestor_two_hops_up_is_not_healthy() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let leaf_path = stories_dir.join(format!("{LEAF_ID}.yml"));
    let mid_path = stories_dir.join(format!("{MID_ID}.yml"));
    let root_path = stories_dir.join(format!("{ROOT_ID}.yml"));
    fs::write(&leaf_path, LEAF_YAML).expect("write leaf fixture");
    fs::write(&mid_path, MID_YAML).expect("write mid fixture");
    fs::write(&root_path, ROOT_YAML).expect("write root fixture");

    let head_sha = init_repo_and_commit_seed(repo_root);
    let leaf_bytes_before = fs::read(&leaf_path).expect("read leaf before run");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed a valid signing row for MID so the gate does not refuse on
    // the direct ancestor — it must reach past MID to ROOT.
    store
        .append(
            "uat_signings",
            json!({
                "id": "seeded-mid-signing-row",
                "story_id": MID_ID,
                "verdict": "pass",
                "commit": head_sha,
                "signed_at": "2026-04-20T00:00:00Z",
            }),
        )
        .expect("seed mid signing row");

    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    let err = uat
        .run(LEAF_ID)
        .expect_err("Pass on a leaf whose transitive ancestor is unhealthy must be refused");

    match err {
        UatError::AncestorNotHealthy { ancestor_id, .. } => {
            assert_eq!(
                ancestor_id, ROOT_ID,
                "refusal must name the FIRST unhealthy link in the chain \
                 ({ROOT_ID}), not the direct healthy ancestor ({MID_ID}); \
                 got {ancestor_id}"
            );
            assert_ne!(
                ancestor_id, MID_ID,
                "refusal must NOT name the direct healthy ancestor ({MID_ID})"
            );
        }
        other => panic!(
            "Pass with a transitive unhealthy ancestor must return \
             UatError::AncestorNotHealthy naming {ROOT_ID}; got {other:?}"
        ),
    }

    // No new rows written for the leaf (the seeded MID row still exists).
    let leaf_rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(LEAF_ID as u64)
        })
        .expect("store query should succeed");
    assert!(
        leaf_rows.is_empty(),
        "transitive-ancestor refusal must write zero leaf uat_signings rows; got {leaf_rows:?}"
    );

    let leaf_bytes_after = fs::read(&leaf_path).expect("read leaf after run");
    assert_eq!(
        leaf_bytes_after, leaf_bytes_before,
        "transitive-ancestor refusal must not rewrite the target story YAML"
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
