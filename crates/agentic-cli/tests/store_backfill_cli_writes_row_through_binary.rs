//! Story 28 acceptance test: happy path through the binary —
//! `agentic store backfill <id>` writes one manual_signings row.
//!
//! Justification (from stories/28.yml acceptance.tests[7]):
//!   Proves the happy path through the binary: `agentic store
//!   backfill <id>` against a fixture corpus and store where the
//!   named story is in the forged-shape (YAML healthy, evidence
//!   present, history flip present, no `uat_signings` row, no
//!   `manual_signings` row, clean tree) exits 0, writes exactly
//!   one row to the configured store's `manual_signings` table
//!   with the row shape pinned by the library happy-path test,
//!   and emits stdout naming the story id and the row's commit.
//!   Without this test the binary's argv-to-library wire is
//!   unproven — the library could be correct while the CLI shim
//!   mishandled the positional argument or constructed the
//!   wrong store.
//!
//! Red today is runtime-red: the `agentic store` subcommand does
//! not yet exist on the binary, so `assert_cmd::Command::cargo_bin`
//! resolves the binary but the argv `["store", "backfill", "<id>", ...]`
//! is rejected by clap with a non-zero exit (typically 2 from clap's
//! argparse failure, NOT 0). Once build-rust wires the subcommand
//! through to `Store::backfill_manual_signing`, this test becomes the
//! contract for the argv-to-row write path.

use std::fs;
use std::path::{Path, PathBuf};

use agentic_store::{Store, SurrealStore};
use assert_cmd::Command;
use tempfile::TempDir;

const STORY_ID: u32 = 28_201;
const SIGNER_EMAIL: &str = "store-backfill-cli-pass@agentic.local";

const STORY_YAML_HEALTHY: &str = r#"id: 28201
title: "Fixture for story-28 CLI happy-path scaffold"

outcome: |
  Fixture used for the CLI happy-path scaffold; YAML on disk says
  healthy with a flip commit in history, green-jsonl evidence is
  present, and the configured store carries no signing rows for the
  story.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/store_backfill_cli_writes_row_through_binary.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the agentic binary against this file.
  uat: |
    Run `agentic store backfill <id>`; assert exit 0 and one row.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const STORY_YAML_UNDER_CONSTRUCTION: &str = r#"id: 28201
title: "Fixture for story-28 CLI happy-path scaffold"

outcome: |
  Fixture used for the CLI happy-path scaffold; YAML on disk says
  healthy with a flip commit in history, green-jsonl evidence is
  present, and the configured store carries no signing rows for the
  story.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/store_backfill_cli_writes_row_through_binary.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the agentic binary against this file.
  uat: |
    Run `agentic store backfill <id>`; assert exit 0 and one row.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const GREEN_JSONL: &str = "{\"run_id\":\"aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee\",\"story_id\":28201,\"commit\":\"deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\",\"timestamp\":\"2026-04-29T00:00:00Z\",\"verdicts\":[{\"file\":\"crates/agentic-cli/tests/store_backfill_cli_writes_row_through_binary.rs\",\"verdict\":\"green\"}]}\n";

#[test]
fn agentic_store_backfill_id_exits_zero_writes_one_manual_signings_row_and_names_id_in_stdout() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, STORY_YAML_UNDER_CONSTRUCTION).expect("write uc yaml");

    let evidence_dir: PathBuf = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    fs::create_dir_all(&evidence_dir).expect("evidence dir");

    let head_sha = init_repo_seed_then_flip(
        repo_root,
        SIGNER_EMAIL,
        &story_path,
        STORY_YAML_HEALTHY,
        &evidence_dir,
    );

    let store_tmp = TempDir::new().expect("store tempdir");
    let store_path = store_tmp.path().to_path_buf();

    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .env_remove("AGENTIC_SIGNER")
        .arg("store")
        .arg("backfill")
        .arg(STORY_ID.to_string())
        .arg("--store")
        .arg(&store_path)
        .assert();

    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let status = output.status;

    assert_eq!(
        status.code(),
        Some(0),
        "`agentic store backfill <id>` on a fully-valid fixture must exit 0; \
         got status={status:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // stdout must name the story id and the HEAD commit.
    assert!(
        stdout.contains(&STORY_ID.to_string()),
        "stdout must name the story id {STORY_ID}; got stdout:\n{stdout}"
    );
    // The full SHA or its short form is acceptable; check for the
    // 7-char prefix at minimum (story 28 guidance: "stdout includes
    // <target-id>, the resolved signer, and the HEAD short SHA").
    let short_sha = &head_sha[..7];
    assert!(
        stdout.contains(short_sha) || stdout.contains(&head_sha),
        "stdout must name HEAD's commit (full or short SHA, expected short \
         {short_sha:?}); got stdout:\n{stdout}"
    );

    // Re-open the configured SurrealStore and verify exactly one
    // manual_signings row at HEAD with the documented shape.
    let store = SurrealStore::open(&store_path)
        .expect("re-opening the configured SurrealStore must succeed");
    let manual_rows = store
        .query("manual_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64)
        })
        .expect("manual_signings query must succeed");
    assert_eq!(
        manual_rows.len(),
        1,
        "binary path must write exactly one manual_signings row for story \
         {STORY_ID}; got {} rows: {manual_rows:?}",
        manual_rows.len()
    );

    let row = &manual_rows[0];
    assert_eq!(
        row.get("verdict").and_then(|v| v.as_str()),
        Some("pass"),
        "row.verdict must equal \"pass\"; got row={row}"
    );
    let commit = row
        .get("commit")
        .and_then(|v| v.as_str())
        .expect("row.commit must be a string");
    assert_eq!(
        commit, head_sha,
        "row.commit must equal HEAD SHA {head_sha:?}; got {commit:?}"
    );
    assert_eq!(
        row.get("source").and_then(|v| v.as_str()),
        Some("manual-backfill"),
        "row.source must equal \"manual-backfill\"; got row={row}"
    );

    // ZERO uat_signings rows for the story — backfill writes only to
    // manual_signings.
    let uat_rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64)
        })
        .expect("uat_signings query must succeed");
    assert!(
        uat_rows.is_empty(),
        "backfill must write zero uat_signings rows; got {} rows: {uat_rows:?}",
        uat_rows.len()
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
            "story(28201): UAT promotion to healthy",
            &flip_tree,
            &[&seed_commit],
        )
        .expect("commit flip");

    format!("{}", flip_commit_oid)
}
