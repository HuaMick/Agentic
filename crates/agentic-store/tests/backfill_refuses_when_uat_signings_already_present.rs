//! Story 28 acceptance test: no-double-attestation guard — refuses
//! backfill when `uat_signings` already carries a Pass row.
//!
//! Justification (from stories/28.yml acceptance.tests[5]):
//!   Proves the no-double-attestation guard: given a story whose
//!   `uat_signings` table ALREADY contains a Pass row at the
//!   current HEAD (i.e. the story has been legitimately UAT-signed
//!   by `agentic uat` and the prove-it gate is already satisfied),
//!   `Store::backfill_manual_signing(story_id)` returns
//!   `BackfillError::AlreadyAttested { story_id, table:
//!   "uat_signings" }` and writes ZERO rows. The backfill's job is
//!   to bridge stories that LACK a real signing row; backfilling on
//!   top of an existing one would muddy the audit trail and offer
//!   no benefit to the gate.
//!
//! Red today is compile-red: `BackfillError::AlreadyAttested` and
//! `Store::backfill_manual_signing` do not yet exist on the
//! `agentic-store` public surface.

use std::fs;
use std::path::{Path, PathBuf};

use agentic_store::{BackfillError, MemStore, Store};
use serde_json::json;
use tempfile::TempDir;

const STORY_ID: u32 = 28_061;
const SIGNER_EMAIL: &str = "backfill-uat-already@agentic.local";

const STORY_YAML_HEALTHY: &str = r#"id: 28061
title: "Fixture for story-28 uat-row-already-present scaffold"

outcome: |
  Fixture whose YAML is healthy and history flip is clean — but a
  uat_signings.verdict=pass row already exists, so backfill must
  refuse.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-store/tests/backfill_refuses_when_uat_signings_already_present.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the library entry point against this file.
  uat: |
    Seed a uat_signings row; run backfill; assert AlreadyAttested.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const STORY_YAML_UNDER_CONSTRUCTION: &str = r#"id: 28061
title: "Fixture for story-28 uat-row-already-present scaffold"

outcome: |
  Fixture whose YAML is healthy and history flip is clean — but a
  uat_signings.verdict=pass row already exists, so backfill must
  refuse.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-store/tests/backfill_refuses_when_uat_signings_already_present.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the library entry point against this file.
  uat: |
    Seed a uat_signings row; run backfill; assert AlreadyAttested.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const GREEN_JSONL: &str = "{\"run_id\":\"aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee\",\"story_id\":28061,\"commit\":\"deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\",\"timestamp\":\"2026-04-29T00:00:00Z\",\"verdicts\":[{\"file\":\"crates/agentic-store/tests/backfill_refuses_when_uat_signings_already_present.rs\",\"verdict\":\"green\"}]}\n";

#[test]
fn backfill_refuses_when_uat_signings_already_carries_a_pass_row_for_this_story() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, STORY_YAML_UNDER_CONSTRUCTION).expect("write under_construction yaml");

    let evidence_dir: PathBuf = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    fs::create_dir_all(&evidence_dir).expect("evidence dir");

    let head_sha = init_repo_seed_then_flip(
        repo_root,
        SIGNER_EMAIL,
        &story_path,
        STORY_YAML_HEALTHY,
        &evidence_dir,
    );

    let store = MemStore::new();

    // Seed a `uat_signings.verdict=pass` row at HEAD for this story —
    // the precondition the guard refuses on.
    store
        .append(
            "uat_signings",
            json!({
                "id": "seeded-uat-signing-row",
                "story_id": STORY_ID,
                "verdict": "pass",
                "commit": head_sha,
                "signed_at": "2026-04-29T00:00:00Z",
                "signer": "real-uat-pass@agentic.local",
            }),
        )
        .expect("seed uat_signings row");

    let err = store
        .backfill_manual_signing(STORY_ID, repo_root)
        .expect_err("guard must refuse when uat_signings row already exists");
    match &err {
        BackfillError::AlreadyAttested { story_id, table } => {
            assert_eq!(*story_id, STORY_ID);
            assert_eq!(
                table, "uat_signings",
                "AlreadyAttested.table must name the table that holds the existing row \
                 (uat_signings); got {table:?}"
            );
        }
        other => panic!(
            "guard must surface AlreadyAttested {{ story_id, table: \"uat_signings\" }}; \
             got {other:?}"
        ),
    }

    // ZERO new rows in manual_signings.
    let manual_rows = store
        .query("manual_signings", &|_| true)
        .expect("manual_signings query must succeed");
    assert!(
        manual_rows.is_empty(),
        "no-double-attestation guard must write zero manual_signings rows; got {manual_rows:?}"
    );
}

fn init_repo_seed_then_flip(
    root: &Path,
    email: &str,
    story_path: &Path,
    healthy_yaml: &str,
    evidence_dir: &Path,
) -> String {
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
    let seed_tree_oid = index.write_tree().expect("write seed tree");
    let seed_tree = repo.find_tree(seed_tree_oid).expect("find seed tree");
    let sig = repo.signature().expect("repo signature");
    let seed_commit_oid = repo
        .commit(Some("HEAD"), &sig, &sig, "seed: under_construction", &seed_tree, &[])
        .expect("commit seed");
    let seed_commit = repo.find_commit(seed_commit_oid).expect("find seed commit");

    fs::write(story_path, healthy_yaml).expect("flip yaml to healthy");
    fs::write(
        evidence_dir.join("2026-04-29T00-00-00Z-green.jsonl"),
        GREEN_JSONL,
    )
    .expect("write green evidence");

    let mut index = repo.index().expect("repo index 2");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("stage all 2");
    index.write().expect("write index 2");
    let flip_tree_oid = index.write_tree().expect("write flip tree");
    let flip_tree = repo.find_tree(flip_tree_oid).expect("find flip tree");
    let flip_commit_oid = repo
        .commit(
            Some("HEAD"),
            &sig,
            &sig,
            "story(28061): UAT promotion to healthy",
            &flip_tree,
            &[&seed_commit],
        )
        .expect("commit flip");

    format!("{}", flip_commit_oid)
}
