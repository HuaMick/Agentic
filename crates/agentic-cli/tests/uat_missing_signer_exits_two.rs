//! Story 18 acceptance test: `agentic uat` with no signer resolvable
//! exits 2, names `SignerMissing` on stderr, writes zero rows, and
//! does not mutate the story YAML.
//!
//! Justification (from stories/18.yml acceptance.tests[12]):
//!   Proves the no-source-exits-2 contract at the binary
//!   boundary: on a fixture repo whose git config has
//!   no `user.email`, with no `AGENTIC_SIGNER` exported
//!   and no `--signer` flag, `agentic uat <id> --verdict
//!   pass` exits 2 (could-not-verdict, consistent with
//!   story 1's dirty-tree/bad-id mapping), emits a human-
//!   readable message naming `SignerMissing` on stderr,
//!   writes zero rows, and leaves the story YAML
//!   unchanged. Exit 2 is load-bearing: story 1 already
//!   distinguishes 1 (real Fail verdict) from 2 (could-
//!   not-run); a missing signer is the latter. Without
//!   this, a misconfigured dev machine produces exit 1
//!   that CI treats as a real verdict failure, and the
//!   crate's exit-code contract degrades.
//!
//! Red today: runtime-red via the missing signer-resolution wire in
//! the compiled `agentic uat` subcommand. The binary today exits 0
//! (happily signs with an empty / default identity) rather than 2,
//! because the SignerMissing refusal has not yet been implemented.

use std::fs;
use std::path::Path;

use agentic_store::{Store, SurrealStore};
use assert_cmd::Command;
use tempfile::TempDir;

const STORY_ID: u32 = 90910;

const FIXTURE_YAML: &str = r#"id: 90910
title: "Fixture story for story 18 SignerMissing exit-2"

outcome: |
  Fixture that exercises the no-signer-resolvable refusal path at the
  binary boundary.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/uat_missing_signer_exits_two.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the agentic binary against this file.
  uat: |
    Run `agentic uat <id> --verdict pass` with no signer source; the
    binary must exit 2 and write no rows.

guidance: |
  Fixture authored inline for story-18 SignerMissing exit-2. Not a
  real story.

depends_on: []
"#;

#[test]
fn agentic_uat_with_no_signer_resolvable_exits_two_and_writes_no_rows() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    init_repo_without_email(repo_root);

    let store_tmp = TempDir::new().expect("store tempdir");
    let store_path = store_tmp.path().to_path_buf();

    // Snapshot the YAML bytes BEFORE the call for post-refusal
    // byte-identical check.
    let fixture_before = fs::read_to_string(&story_path).expect("read fixture pre");

    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic")
        .current_dir(repo_root)
        .env_remove("AGENTIC_SIGNER")
        .arg("uat")
        .arg(STORY_ID.to_string())
        .arg("--verdict")
        .arg("pass")
        .arg("--store")
        .arg(&store_path)
        .assert();

    let output = assert.get_output().clone();
    let status = output.status;
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Exit 2 — not 0 (happy path), not 1 (real Fail verdict).
    assert_eq!(
        status.code(),
        Some(2),
        "no-signer-resolvable must exit 2; got {status:?}\nstderr:\n{stderr}"
    );
    // stderr names `SignerMissing` — operator-visible diagnostic.
    assert!(
        stderr.contains("SignerMissing"),
        "stderr must name SignerMissing; got:\n{stderr}"
    );

    // Zero rows in the configured store.
    let store = SurrealStore::open(&store_path).expect("reopen store");
    let rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64)
        })
        .expect("store query");
    assert!(
        rows.is_empty(),
        "SignerMissing refusal must write zero rows; got {rows:?}"
    );

    // Story YAML is byte-identical (no `status: healthy` promotion).
    let fixture_after = fs::read_to_string(&story_path).expect("read fixture post");
    assert_eq!(
        fixture_before, fixture_after,
        "SignerMissing refusal must leave the fixture YAML byte-identical"
    );
}

fn init_repo_without_email(root: &Path) {
    let repo = git2::Repository::init(root).expect("git init");
    let mut cfg = repo.config().expect("repo config");
    cfg.set_str("user.name", "test-builder").expect("set user.name");
    // user.email intentionally NOT set.
    let _ = cfg;
    // Need a committable baseline so the dirty-tree check passes and
    // we get past it to reach the signer-resolution step. Stage and
    // commit whatever is in the working tree.
    let mut index = repo.index().expect("repo index");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("stage all");
    index.write().expect("write index");
    let tree_oid = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_oid).expect("find tree");
    // Build a signature MANUALLY because we deliberately did not set
    // user.email — libgit2's default signature lookup would fail.
    let sig = git2::Signature::now("test-builder", "test-builder@agentic.local")
        .expect("manual signature");
    let _ = repo
        .commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
    // After committing, unset user.email at repo scope in case
    // libgit2's implicit write to config happened during the commit.
    // Re-open config and ensure no user.email entry exists.
    let repo = git2::Repository::open(root).expect("git open");
    let mut cfg = repo.config().expect("repo config post");
    // Ignore error if not set.
    let _ = cfg.remove("user.email");
}
