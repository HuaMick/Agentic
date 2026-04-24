//! Story 1 acceptance test: Fail verdict surfaces as exit 1 and does
//! not promote.
//!
//! Justification (from stories/1.yml): proves the Fail exit-code
//! contract at the binary boundary: `agentic uat <id> --verdict fail`
//! on the same clean fixture exits 1 (not 0, not 2), writes exactly
//! one row with `verdict=fail`, and leaves the fixture YAML's
//! `status:` field untouched. Without this a Fail could either
//! silently promote (catastrophic) or surface as exit 2 and be
//! confused with "the CLI itself could not run," which breaks CI's
//! distinction between a real negative verdict and a system fault.
//!
//! Post-amendment observable (per story 1's 2026-04-23 amendment's
//! `uat_signings` row contract): every row — Pass and Fail — carries
//! a non-empty `signer` field resolved through story 18's four-tier
//! chain. The Fail-path test pins the symmetry claim from the amended
//! `uat_fail.rs` justification directly ("attribution is unconditional
//! on verdict, pinning the symmetry so a red row is as attributable
//! as a green one") at the binary boundary. The fixture seeds
//! `git config user.email` so tier-3 resolution is deterministic.
//!
//! The scaffold is a twin of `uat_pass_promotes.rs`, except the
//! verdict is `fail`. Assertions are symmetric: exit 1 exactly (not
//! 0, not 2), one row with verdict=fail + a `signer` field matching
//! the resolved identity, and fixture YAML byte-identical before and
//! after.
//!
//! Red today is runtime-red: the binary currently writes a
//! `uat_signings` row but does not yet populate `signer`; the new
//! assertion below fails until the signer wire (story 18) lands.

use std::fs;
use std::path::Path;

use agentic_store::{Store, SurrealStore};
use assert_cmd::Command;
use tempfile::TempDir;

const STORY_ID: u32 = 88803;
const SIGNER_EMAIL: &str = "cli-fail@agentic.local";

const FIXTURE_YAML: &str = r#"id: 88803
title: "Fixture story for story 1 CLI fail-does-not-promote"

outcome: |
  A fixture that the CLI uat subcommand must NOT promote when
  --verdict fail is passed.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/uat_fail_does_not_promote.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the agentic binary against this file.
  uat: |
    Run `agentic uat <id> --verdict fail`; assert non-promotion.

guidance: |
  Fixture authored inline for the story-1 fail-does-not-promote
  scaffold. Not a real story.

depends_on: []
"#;

#[test]
fn agentic_uat_verdict_fail_exits_one_writes_fail_row_and_leaves_yaml_unchanged() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    init_repo_and_commit_seed(repo_root, SIGNER_EMAIL);
    let before_bytes = fs::read(&story_path).expect("read fixture before run");

    let store_tmp = TempDir::new().expect("store tempdir");
    let store_path = store_tmp.path().to_path_buf();

    // Scrub AGENTIC_SIGNER from the child env so the resolver falls
    // through to tier-3 (`git config user.email`), which the fixture
    // seeded.
    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .env_remove("AGENTIC_SIGNER")
        .arg("uat")
        .arg(STORY_ID.to_string())
        .arg("--verdict")
        .arg("fail")
        .arg("--store")
        .arg(&store_path)
        .assert();

    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let status = output.status;

    // Exit code 1 EXACTLY. Story 1 guidance is explicit: a Fail verdict
    // is a real negative result (exit 1) and must NOT collapse to
    // exit 2 (could-not-verdict) or exit 0 (success).
    assert_eq!(
        status.code(),
        Some(1),
        "`agentic uat <id> --verdict fail` must exit 1 (real negative \
         verdict), NOT 0 (success) or 2 (could-not-verdict); got \
         status={status:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let store = SurrealStore::open(&store_path)
        .expect("re-opening the configured SurrealStore must succeed");
    let rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64)
        })
        .expect("uat_signings query must succeed");

    assert_eq!(
        rows.len(),
        1,
        "Fail through the binary must write exactly one uat_signings row; \
         got {} rows: {rows:?}",
        rows.len()
    );
    assert_eq!(
        rows[0].get("verdict").and_then(|v| v.as_str()),
        Some("fail"),
        "signing row must carry verdict=\"fail\"; got row={}",
        rows[0]
    );

    // Attribution-is-unconditional-on-verdict (post-amendment): the
    // Fail row must carry a non-empty `signer` equal to the resolved
    // identity. This mirrors the Pass-path assertion from
    // `uat_pass_promotes.rs` — the symmetry is the observable story 1's
    // amended `uat_fail.rs` justification names explicitly.
    let signer = rows[0]
        .get("signer")
        .and_then(|v| v.as_str())
        .expect("signing row must carry a string `signer` field on Fail too");
    assert!(
        !signer.trim().is_empty(),
        "Fail-through-binary signing row `signer` must be non-empty; \
         got {signer:?}"
    );
    assert_eq!(
        signer, SIGNER_EMAIL,
        "tier-3 resolver must stamp `signer` = git config user.email on \
         Fail rows (same chain as Pass); got {signer:?}, expected \
         {SIGNER_EMAIL:?}"
    );

    // Fixture YAML must be byte-for-byte unchanged.
    let after_bytes = fs::read(&story_path).expect("read fixture after run");
    assert_eq!(
        after_bytes, before_bytes,
        "Fail through the binary must NOT touch the fixture YAML; \
         file changed on disk"
    );
}

fn init_repo_and_commit_seed(root: &Path, email: &str) -> String {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder").expect("set user.name");
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
