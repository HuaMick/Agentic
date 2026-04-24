//! Story 11 acceptance test (retirement-chain amendment): when the
//! successor behind a retired ancestor is not healthy, the gate's
//! refusal names the successor (the first non-transparent,
//! non-healthy link), not the retired intermediary.
//!
//! Justification (from stories/11.yml):
//! Proves the gate surfaces the TERMINAL link of the chain, not the
//! retired intermediary: given a descendant `<leaf>` whose
//! `depends_on` names `<mid>`, where `<mid>` is `retired` with
//! `superseded_by: <root>` and `<root>` is `under_construction`
//! (or any non-healthy non-retired state), `Uat::run` with a Pass
//! verdict on `<leaf>` returns `UatError::AncestorNotHealthy
//! { ancestor_id: <root>, reason: ... }`. The `ancestor_id` field
//! names `<root>` — the first non-transparent, non-healthy link in
//! the chain — NOT `<mid>`. The retired intermediary is never the
//! error's subject because retired is terminal from the gate's
//! perspective: the operator cannot un-retire or re-UAT a retired
//! story; they can only push its successor to healthy.
//!
//! Red today is compile-red: `Status::Retired` does not yet exist
//! on the `agentic_story::Status` enum, and the chain-walk code
//! path in `Uat::run` is not yet implemented either.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_story::Status;
use agentic_uat::{StubExecutor, Uat, UatError};
use tempfile::TempDir;

const LEAF_ID: u32 = 11701;
const RETIRED_MID_ID: u32 = 11702; // retired, superseded_by ROOT_ID
const ROOT_ID: u32 = 11703; // under_construction

const LEAF_YAML: &str = r#"id: 11701
title: "Leaf depending on a retired ancestor whose successor is not healthy"

outcome: |
  Leaf fixture: the chain-walk must follow retirement to the
  successor, find it under_construction, and refuse NAMING the
  successor.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_when_retired_chain_terminates_in_unhealthy.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Run the stub executor; expect refusal naming the successor.

guidance: |
  Fixture authored inline for the story-11 retirement-chain-
  terminates-in-unhealthy scaffold. Not a real story.

depends_on:
  - 11702
"#;

const RETIRED_MID_YAML: &str = r#"id: 11702
title: "Retired intermediary whose successor is not healthy"

outcome: |
  Retired fixture pointing at an under_construction successor so the
  chain-walk must name the successor (not this intermediary) in the
  refusal.

status: retired
superseded_by: 11703
retired_reason: |
  Folded into successor 11703 which has not yet reached healthy.

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_when_retired_chain_terminates_in_unhealthy.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the retired intermediary. Must
  never appear in the refusal's ancestor_id.

depends_on: []
"#;

const ROOT_YAML: &str = r#"id: 11703
title: "Under-construction terminal link of the retirement chain"

outcome: |
  Successor fixture whose status is not healthy, so the gate refuses
  and names THIS story as the ancestor_id (not the retired
  intermediary).

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_when_retired_chain_terminates_in_unhealthy.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the non-healthy terminal link
  of the retirement chain. The actionable fix for the operator is to
  promote THIS story to healthy — which is why the refusal must
  surface its id, not the retired intermediary's.

depends_on: []
"#;

#[test]
fn uat_run_pass_refuses_naming_terminal_link_not_retired_intermediary_when_chain_ends_unhealthy() {
    // Cross-reference: Status::Retired must exist on the enum.
    let _retired_variant_exists: Status = Status::Retired;

    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let leaf_path = stories_dir.join(format!("{LEAF_ID}.yml"));
    let retired_path = stories_dir.join(format!("{RETIRED_MID_ID}.yml"));
    let root_path = stories_dir.join(format!("{ROOT_ID}.yml"));
    fs::write(&leaf_path, LEAF_YAML).expect("write leaf fixture");
    fs::write(&retired_path, RETIRED_MID_YAML).expect("write retired intermediary");
    fs::write(&root_path, ROOT_YAML).expect("write root fixture");

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
                ancestor_id, ROOT_ID,
                "refusal must name the TERMINAL link ({ROOT_ID}) — the first \
                 non-transparent, non-healthy id in the chain — and NOT the \
                 retired intermediary ({RETIRED_MID_ID}); got ancestor_id={ancestor_id}"
            );
            assert_ne!(
                ancestor_id, RETIRED_MID_ID,
                "retired intermediary ({RETIRED_MID_ID}) must NEVER surface in a \
                 refusal — retired is terminal from the gate's perspective and \
                 pointing the operator at it is a dead end"
            );
        }
        other => panic!(
            "Pass through a retired-ancestor chain whose successor is not healthy \
             must return UatError::AncestorNotHealthy naming {ROOT_ID}; got {other:?}"
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
