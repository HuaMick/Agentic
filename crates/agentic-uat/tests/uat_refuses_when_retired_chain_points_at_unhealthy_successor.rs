//! Story 21 acceptance test: when the successor behind a retired
//! ancestor is not healthy, the gate's refusal names the successor,
//! not the retired intermediary.
//!
//! Justification (from stories/21.yml):
//! Proves the gate surfaces the SUCCESSOR's failure, not the
//! retired intermediary's: given a descendant `<id>` whose
//! `depends_on` names `<A>`, where `<A>` is `retired (superseded_by:
//! <B>)` and `<B>` is `under_construction` (or any non-healthy
//! state), `Uat::run` with a Pass verdict on `<id>` returns
//! `UatError::AncestorNotHealthy { ancestor_id: <B>, reason: ... }`
//! — naming `<B>`, NOT `<A>`. The stderr message reaching the
//! operator points at the successor so the fix instruction
//! ("promote `<B>`") is actionable.
//!
//! Red today is compile-red: the `Status::Retired` variant the
//! fixture YAML depends on does not yet exist on the
//! `agentic_story::Status` enum.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_story::Status;
use agentic_uat::{StubExecutor, Uat, UatError};
use tempfile::TempDir;

const LEAF_ID: u32 = 21201;
const RETIRED_ANCESTOR_ID: u32 = 21202; // retired, superseded_by UNHEALTHY_SUCCESSOR_ID
const UNHEALTHY_SUCCESSOR_ID: u32 = 21203; // under_construction

const LEAF_YAML: &str = r#"id: 21201
title: "Leaf depending on a retired ancestor whose successor is not healthy"

outcome: |
  Leaf fixture: the chain-walk must follow retirement to the
  successor, find it under_construction, and refuse NAMING the
  successor.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_when_retired_chain_points_at_unhealthy_successor.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    Run the stub executor; expect refusal naming the successor.

guidance: |
  Fixture authored inline for the story-21 chain-walk-unhealthy scaffold.
  Not a real story.

depends_on:
  - 21202
"#;

const RETIRED_ANCESTOR_YAML: &str = r#"id: 21202
title: "Retired ancestor whose successor is not healthy"

outcome: |
  Retired fixture pointing at an under_construction successor so the
  chain-walk must name the successor in the refusal.

status: retired
superseded_by: 21203
retired_reason: |
  Folded into successor 21203 which has not yet reached healthy.

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_when_retired_chain_points_at_unhealthy_successor.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the retired intermediary whose
  successor is not yet healthy.

depends_on: []
"#;

const UNHEALTHY_SUCCESSOR_YAML: &str = r#"id: 21203
title: "Under-construction successor behind a retired ancestor"

outcome: |
  Successor fixture whose status is not healthy, so the gate refuses
  and names THIS story (not the retired intermediary).

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_when_retired_chain_points_at_unhealthy_successor.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the non-healthy successor whose
  promotion is the actionable fix for the operator.

depends_on: []
"#;

#[test]
fn uat_run_pass_refuses_naming_successor_not_retired_intermediary_when_chain_ends_unhealthy() {
    // Cross-reference: Status::Retired must exist on the enum.
    let _retired_variant_exists: Status = Status::Retired;

    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let leaf_path = stories_dir.join(format!("{LEAF_ID}.yml"));
    let retired_path = stories_dir.join(format!("{RETIRED_ANCESTOR_ID}.yml"));
    let successor_path = stories_dir.join(format!("{UNHEALTHY_SUCCESSOR_ID}.yml"));
    fs::write(&leaf_path, LEAF_YAML).expect("write leaf fixture");
    fs::write(&retired_path, RETIRED_ANCESTOR_YAML).expect("write retired ancestor");
    fs::write(&successor_path, UNHEALTHY_SUCCESSOR_YAML).expect("write successor");

    let _head_sha = init_repo_and_commit_seed(repo_root);
    let leaf_bytes_before = fs::read(&leaf_path).expect("read leaf before run");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    let err = uat.run(LEAF_ID).expect_err(
        "Pass on a leaf whose retired ancestor's successor is unhealthy must be refused",
    );

    match err {
        UatError::AncestorNotHealthy { ancestor_id, .. } => {
            assert_eq!(
                ancestor_id, UNHEALTHY_SUCCESSOR_ID,
                "refusal must name the SUCCESSOR ({UNHEALTHY_SUCCESSOR_ID}) — the \
                 actionable link — and NOT the retired intermediary ({RETIRED_ANCESTOR_ID}); \
                 got ancestor_id={ancestor_id}"
            );
            assert_ne!(
                ancestor_id, RETIRED_ANCESTOR_ID,
                "retired intermediary ({RETIRED_ANCESTOR_ID}) must NEVER surface in a \
                 refusal — retired is terminal from the gate's perspective and pointing \
                 the operator at it is a dead end"
            );
        }
        other => panic!(
            "Pass through a retired-ancestor chain whose successor is not healthy \
             must return UatError::AncestorNotHealthy naming {UNHEALTHY_SUCCESSOR_ID}; \
             got {other:?}"
        ),
    }

    // No signing rows for the leaf were written.
    let leaf_rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(LEAF_ID as u64)
        })
        .expect("store query should succeed");
    assert!(
        leaf_rows.is_empty(),
        "chain-walk refusal must write zero leaf uat_signings rows; got {leaf_rows:?}"
    );

    // Leaf YAML unchanged on disk.
    let leaf_bytes_after = fs::read(&leaf_path).expect("read leaf after run");
    assert_eq!(
        leaf_bytes_after, leaf_bytes_before,
        "chain-walk refusal must not rewrite the target story YAML"
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
