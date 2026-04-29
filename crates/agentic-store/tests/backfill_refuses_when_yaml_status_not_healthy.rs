//! Story 28 acceptance test: guard 1 — status guard refuses backfill when
//! the YAML on disk is not `healthy`.
//!
//! Justification (from stories/28.yml acceptance.tests[1]):
//!   Proves guard 1 (status guard) at the library boundary: given a
//!   clean working tree where `stories/<id>.yml`'s on-disk `status`
//!   is anything other than `healthy` (the test exercises both
//!   `under_construction` and `proposed`),
//!   `Store::backfill_manual_signing(story_id)` returns a typed
//!   `BackfillError::StatusNotHealthy { story_id, observed_status }`
//!   and writes ZERO rows to `manual_signings`. Without this guard,
//!   the backfill would happily attest a story that was never even
//!   claimed-healthy on disk — a forging shape strictly worse than
//!   the manual-ritual shape it exists to legitimise.
//!
//! Red today is compile-red: `BackfillError` and
//! `Store::backfill_manual_signing` do not yet exist on the
//! `agentic-store` public surface.
//!
//! Method signature pinned: see
//! `backfill_writes_one_manual_signings_row_at_head.rs` for the shape
//! all eight library tests share.

use std::fs;
use std::path::Path;

use agentic_store::{BackfillError, MemStore, Store};
use tempfile::TempDir;

const STORY_ID_UC: u32 = 28_021;
const STORY_ID_PROPOSED: u32 = 28_022;
const SIGNER_EMAIL: &str = "backfill-status-guard@agentic.local";

fn fixture_yaml(id: u32, status: &str, test_file_path: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture for story-28 status-guard scaffold ({status})"

outcome: |
  Fixture used for the status-guard scaffold; YAML on disk says
  `{status}` so the guard must refuse.

status: {status}

patterns: []

acceptance:
  tests:
    - file: {test_file_path}
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the library entry point against this file.
  uat: |
    Run the backfill; assert StatusNotHealthy and zero rows.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#
    )
}

#[test]
fn backfill_refuses_with_typed_error_when_yaml_status_is_not_healthy() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // Two fixtures: under_construction and proposed. Both must be
    // refused with the same typed error variant.
    let uc_path = stories_dir.join(format!("{STORY_ID_UC}.yml"));
    fs::write(
        &uc_path,
        fixture_yaml(
            STORY_ID_UC,
            "under_construction",
            "crates/agentic-store/tests/backfill_refuses_when_yaml_status_not_healthy.rs",
        ),
    )
    .expect("write uc fixture");
    let proposed_path = stories_dir.join(format!("{STORY_ID_PROPOSED}.yml"));
    fs::write(
        &proposed_path,
        fixture_yaml(
            STORY_ID_PROPOSED,
            "proposed",
            "crates/agentic-store/tests/backfill_refuses_when_yaml_status_not_healthy.rs",
        ),
    )
    .expect("write proposed fixture");

    init_repo_and_commit_seed(repo_root, SIGNER_EMAIL);

    let store = MemStore::new();

    // under_construction: must return StatusNotHealthy with the observed
    // status.
    let err_uc = store
        .backfill_manual_signing(STORY_ID_UC, repo_root)
        .expect_err("status guard must refuse under_construction");
    match &err_uc {
        BackfillError::StatusNotHealthy {
            story_id,
            observed_status,
        } => {
            assert_eq!(*story_id, STORY_ID_UC);
            assert_eq!(observed_status, "under_construction");
        }
        other => panic!(
            "status guard must surface StatusNotHealthy {{ story_id, observed_status }}; \
             got {other:?}"
        ),
    }

    // proposed: same variant, different observed_status.
    let err_p = store
        .backfill_manual_signing(STORY_ID_PROPOSED, repo_root)
        .expect_err("status guard must refuse proposed");
    match &err_p {
        BackfillError::StatusNotHealthy {
            story_id,
            observed_status,
        } => {
            assert_eq!(*story_id, STORY_ID_PROPOSED);
            assert_eq!(observed_status, "proposed");
        }
        other => panic!(
            "status guard must surface StatusNotHealthy {{ story_id, observed_status }}; \
             got {other:?}"
        ),
    }

    // ZERO rows in manual_signings for either story.
    let rows = store
        .query("manual_signings", &|_| true)
        .expect("manual_signings query must succeed");
    assert!(
        rows.is_empty(),
        "status-guard refusal must write zero manual_signings rows; got {} rows: {rows:?}",
        rows.len()
    );
}

fn init_repo_and_commit_seed(root: &Path, email: &str) -> String {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder")
            .expect("set user.name");
        cfg.set_str("user.email", email).expect("set user.email");
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
