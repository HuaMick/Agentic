//! Story 28 acceptance test: idempotency-via-refusal — a second backfill
//! invocation refuses when `manual_signings` already carries a Pass row.
//!
//! Justification (from stories/28.yml acceptance.tests[6]):
//!   Proves the idempotency-via-refusal guard: given a story whose
//!   `manual_signings` table already contains a Pass row at the
//!   current HEAD (i.e. the backfill has already been run for this
//!   story-id at this commit), a second invocation of
//!   `Store::backfill_manual_signing(story_id)` returns
//!   `BackfillError::AlreadyAttested { story_id, table:
//!   "manual_signings" }` and writes ZERO rows. The append-only
//!   contract on `manual_signings` mirrors the contract on
//!   `uat_signings`: one attestation per story per commit.
//!
//! Red today is compile-red: `BackfillError::AlreadyAttested` and
//! `Store::backfill_manual_signing` do not yet exist on the
//! `agentic-store` public surface.

use std::fs;
use std::path::{Path, PathBuf};

use agentic_store::{BackfillError, MemStore, Store};
use serde_json::json;
use tempfile::TempDir;

const STORY_ID: u32 = 28_071;
const SIGNER_EMAIL: &str = "backfill-manual-already@agentic.local";

const STORY_YAML_HEALTHY: &str = r#"id: 28071
title: "Fixture for story-28 manual-row-already-present scaffold"

outcome: |
  Fixture whose YAML is healthy and history flip is clean — a
  manual_signings.verdict=pass row already exists, so the second
  backfill invocation must refuse.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-store/tests/backfill_refuses_when_manual_signings_already_present.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the library entry point against this file.
  uat: |
    Seed a manual_signings row; run backfill; assert AlreadyAttested.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const STORY_YAML_UNDER_CONSTRUCTION: &str = r#"id: 28071
title: "Fixture for story-28 manual-row-already-present scaffold"

outcome: |
  Fixture whose YAML is healthy and history flip is clean — a
  manual_signings.verdict=pass row already exists, so the second
  backfill invocation must refuse.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-store/tests/backfill_refuses_when_manual_signings_already_present.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the library entry point against this file.
  uat: |
    Seed a manual_signings row; run backfill; assert AlreadyAttested.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const GREEN_JSONL: &str = "{\"run_id\":\"aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee\",\"story_id\":28071,\"commit\":\"deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\",\"timestamp\":\"2026-04-29T00:00:00Z\",\"verdicts\":[{\"file\":\"crates/agentic-store/tests/backfill_refuses_when_manual_signings_already_present.rs\",\"verdict\":\"green\"}]}\n";

#[test]
fn second_backfill_invocation_refuses_when_manual_signings_row_already_exists_at_head() {
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

    // Seed an existing manual_signings.verdict=pass row at HEAD for the
    // story. This is the shape "the backfill has already been run."
    store
        .append(
            "manual_signings",
            json!({
                "id": "seeded-manual-signing-row",
                "story_id": STORY_ID,
                "verdict": "pass",
                "commit": head_sha,
                "signed_at": "2026-04-29T00:00:00Z",
                "signer": "first-backfill@agentic.local",
                "source": "manual-backfill",
            }),
        )
        .expect("seed manual_signings row");

    let err = store
        .backfill_manual_signing(STORY_ID, repo_root)
        .expect_err("idempotency guard must refuse when manual_signings row already exists");
    match &err {
        BackfillError::AlreadyAttested { story_id, table } => {
            assert_eq!(*story_id, STORY_ID);
            assert_eq!(
                table, "manual_signings",
                "AlreadyAttested.table must name the table that holds the existing row \
                 (manual_signings); got {table:?}"
            );
        }
        other => panic!(
            "idempotency guard must surface AlreadyAttested {{ story_id, table: \
             \"manual_signings\" }}; got {other:?}"
        ),
    }

    // Exactly one row in manual_signings — the seeded one. The refusal
    // must NOT add a duplicate.
    let manual_rows = store
        .query("manual_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64)
        })
        .expect("manual_signings query must succeed");
    assert_eq!(
        manual_rows.len(),
        1,
        "idempotency refusal must NOT add a duplicate manual_signings row; \
         got {} rows: {manual_rows:?}",
        manual_rows.len()
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
            "story(28071): UAT promotion to healthy",
            &flip_tree,
            &[&seed_commit],
        )
        .expect("commit flip");

    format!("{}", flip_commit_oid)
}
