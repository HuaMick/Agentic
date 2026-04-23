//! Story 18 acceptance test: UAT Pass wires the resolved signer onto
//! the `uat_signings` row at the library boundary.
//!
//! Justification (from stories/18.yml acceptance.tests[5]):
//!   Proves the wire from resolver to `uat_signings` at the
//!   library boundary: given a clean tempdir repo whose
//!   `git config user.email` is `dev@example.com`, a
//!   fixture proposed story, and no `AGENTIC_SIGNER` env
//!   var, a `Uat::run(..., SignerSource::Resolve)` call
//!   issuing a Pass verdict writes exactly one row to
//!   `uat_signings` whose `signer` field equals
//!   `dev@example.com`. The same test, re-run with the
//!   env var set to `env-person@example.com`, produces a
//!   row whose `signer` is `env-person@example.com`. Both
//!   rows also carry the commit hash per story 1's
//!   existing contract — the signer field is ADDITIVE, not
//!   replacing any existing field. Without this, the
//!   resolver is proven in isolation but story 1's row
//!   shape does not actually gain the field, and the
//!   outcome's "written onto the row it emits" clause is
//!   unproven for the signing path.
//!
//! Red today: compile-red via the missing `SignerSource` symbol in
//! `agentic_uat` (and the missing `Uat::run` overload that accepts
//! it) — the test `use`s `agentic_uat::SignerSource` which does not
//! exist yet.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_uat::{SignerSource, StubExecutor, Uat, Verdict};
use tempfile::TempDir;

const STORY_ID: u32 = 77701;

const FIXTURE_YAML: &str = r#"id: 77701
title: "Fixture story for story 18 signer-on-signing-row"

outcome: |
  A fixture that the UAT gate drives to Pass so the signing row's
  signer field can be asserted.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_pass_writes_resolved_signer_onto_signing_row.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Run the stub executor; it returns Pass; assert signer field.

guidance: |
  Fixture authored inline for the story-18 signer-on-signing-row
  scaffold. Not a real story.

depends_on: []
"#;

#[test]
fn uat_pass_writes_resolved_signer_onto_signing_row_tier_3_then_tier_2() {
    // --- Subtest 1: tier-3 fallback to git config user.email. ---
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    init_repo_with_email(repo_root, "dev@example.com");

    // No env var for tier-3 subtest.
    std::env::remove_var("AGENTIC_SIGNER");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    let verdict = uat
        .run_with_signer(STORY_ID, SignerSource::Resolve)
        .expect("Pass path must not error");
    assert!(matches!(verdict, Verdict::Pass));

    let rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64)
        })
        .expect("store query");
    assert_eq!(rows.len(), 1, "exactly one signing row; got {rows:?}");
    let row = &rows[0];
    assert_eq!(
        row.get("signer").and_then(|v| v.as_str()),
        Some("dev@example.com"),
        "tier-3 resolution must stamp `signer` = git config user.email; got row={row}"
    );
    // Signer is ADDITIVE — the pre-existing `commit` field still present.
    let commit = row
        .get("commit")
        .and_then(|v| v.as_str())
        .expect("signing row must still carry a string `commit` field");
    assert_eq!(commit.len(), 40, "commit must be a full SHA; got {commit:?}");

    // --- Subtest 2: tier-2 env var beats git. ---
    // Fresh fixture, fresh store, fresh repo — re-use a separate story
    // id so the same in-memory store is not read for stale rows.
    const STORY_ID_2: u32 = 77702;
    let tmp2 = TempDir::new().expect("tempdir2");
    let repo_root_2 = tmp2.path();
    let stories_dir_2 = repo_root_2.join("stories");
    fs::create_dir_all(&stories_dir_2).expect("stories dir 2");
    let fixture2 = FIXTURE_YAML
        .replace("77701", &STORY_ID_2.to_string())
        .replace(
            "Fixture story for story 18 signer-on-signing-row",
            "Fixture story for story 18 env-tier",
        );
    fs::write(stories_dir_2.join(format!("{STORY_ID_2}.yml")), &fixture2)
        .expect("write fixture 2");
    init_repo_with_email(repo_root_2, "git-person@example.com");

    std::env::set_var("AGENTIC_SIGNER", "env-person@example.com");

    let store2: Arc<dyn Store> = Arc::new(MemStore::new());
    let uat2 = Uat::new(store2.clone(), StubExecutor::always_pass(), stories_dir_2);
    let _ = uat2
        .run_with_signer(STORY_ID_2, SignerSource::Resolve)
        .expect("Pass path (tier-2) must not error");

    let rows2 = store2
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID_2 as u64)
        })
        .expect("store query 2");
    assert_eq!(rows2.len(), 1);
    assert_eq!(
        rows2[0].get("signer").and_then(|v| v.as_str()),
        Some("env-person@example.com"),
        "tier-2 resolution must stamp `signer` = AGENTIC_SIGNER; got row={row}",
        row = rows2[0]
    );

    // Cleanup — don't leak into sibling tests.
    std::env::remove_var("AGENTIC_SIGNER");
}

fn init_repo_with_email(root: &Path, email: &str) {
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
    let _ = repo
        .commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
}
