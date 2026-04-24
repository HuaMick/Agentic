//! Story 18 acceptance test: fail-closed floor at the UAT library
//! boundary — no signer resolvable → `UatError::SignerMissing`,
//! zero rows written, no YAML mutation.
//!
//! Justification (from stories/18.yml acceptance.tests[6]):
//!   Proves the fail-closed floor at the library boundary:
//!   given a clean tempdir repo whose git config has no
//!   `user.email`, no `AGENTIC_SIGNER` env var, and a
//!   `Uat::run(..., SignerSource::Resolve)` call with no
//!   flag override, the library returns
//!   `UatError::SignerMissing`, writes ZERO rows to
//!   `uat_signings`, does NOT mutate the story YAML's
//!   status, and leaves the working tree byte-identical.
//!   Without this, story 1's promotion path could quietly
//!   write a row with `signer: ""` when git config is
//!   incomplete, and the whole "every verdict is
//!   attributable" promise collapses at the first
//!   misconfigured dev machine.
//!
//! Red today: compile-red via the missing `SignerSource` symbol and
//! the missing `UatError::SignerMissing` variant on the `UatError`
//! enum.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_uat::{SignerSource, StubExecutor, Uat, UatError};
use tempfile::TempDir;

const STORY_ID: u32 = 77703;

const FIXTURE_YAML: &str = r#"id: 77703
title: "Fixture story for story 18 signer-unresolvable refusal"

outcome: |
  A fixture that the UAT gate refuses to sign because no signer can be
  resolved on this host.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_pass_refuses_when_signer_unresolvable.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Refuse; typed error; zero rows.

guidance: |
  Fixture authored inline for story-18 fail-closed-on-no-signer.
  Not a real story.

depends_on: []
"#;

#[test]
fn uat_run_refuses_when_all_signer_sources_are_empty() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    // Git repo with NO user.email set — critical for the fail-closed
    // floor to fire. Set user.name only so committer-identity for the
    // seed commit does not block `git init`.
    init_repo_without_email(repo_root);

    std::env::remove_var("AGENTIC_SIGNER");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    // Snapshot the fixture YAML bytes BEFORE the call — they must be
    // byte-identical after the refusal (no status rewrite).
    let fixture_before = fs::read_to_string(&story_path).expect("read fixture pre");

    let err = uat
        .run_with_signer(STORY_ID, SignerSource::Resolve)
        .expect_err("UAT must refuse when no signer resolves");

    // Must be the typed SignerMissing variant — not a generic Io /
    // InternalError / misrouted "story already healthy" code path.
    assert!(
        matches!(err, UatError::SignerMissing { .. }),
        "expected UatError::SignerMissing; got {err:?}"
    );

    // Zero rows in uat_signings for this story.
    let rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64)
        })
        .expect("store query");
    assert!(
        rows.is_empty(),
        "refusal must write zero signing rows; got {rows:?}"
    );

    // Fixture YAML is byte-identical — no promotion, no side-effect
    // rewrite, no trailing newline drift.
    let fixture_after = fs::read_to_string(&story_path).expect("read fixture post");
    assert_eq!(
        fixture_before, fixture_after,
        "SignerMissing refusal must leave the story YAML byte-identical"
    );
}

fn init_repo_without_email(root: &Path) {
    let repo = git2::Repository::init(root).expect("git init");
    // Set ONLY user.name (libgit2 needs some identity to let us commit,
    // but we deliberately leave user.email unset so the resolver's git
    // tier finds nothing).
    let mut cfg = repo.config().expect("repo config");
    cfg.set_str("user.name", "test-builder")
        .expect("set user.name");
    // user.email intentionally NOT set.
}
