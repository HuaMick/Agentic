//! Story 1 acceptance test: the UAT library is exercised end-to-end as a
//! standalone library, with no orchestrator, runtime, sandbox, or CLI in
//! the link graph.
//!
//! Justification (from stories/1.yml): proves the
//! standalone-resilient-library claim — the UAT library is driven
//! directly with only `agentic-store`, `agentic-events`, `agentic-story`,
//! and `git2` wired up (no orchestrator, no runtime, no sandbox), and
//! produces the same verdict-and-signing shape as the CLI path. Without
//! this we cannot claim `agentic uat` is the layer that still promotes
//! stories when the rest of the system is in flames; it would be just
//! another thing that breaks together.
//!
//! Pattern: standalone-resilient-library. The dependency floor is
//! enforced by what this test file imports — ONLY `agentic_uat`,
//! `agentic_store`, `agentic_story`, and `git2` from the workspace.
//! Adding `agentic_orchestrator`, `agentic_runtime`, `agentic_sandbox`,
//! or `agentic_cli` here would be a review-time red flag and the
//! standalone-resilience claim would break. `agentic-events` is named
//! in the allowed set by the story but is not yet a published workspace
//! crate at this commit; the resilience claim is still witnessed by the
//! absence of every forbidden crate from this file's imports.
//!
//! Red today is compile-red via the missing `agentic_uat` public surface
//! — `Uat`, `Uat::run`, `Verdict`, `UatError`, `UatExecutor`,
//! `ExecutionOutcome`, `StubExecutor` do not yet exist in
//! `crates/agentic-uat/src/lib.rs`.

// Compile-time witness: the only workspace crates this test names are
// `agentic_uat`, `agentic_store`, and `agentic_story`. Anything else in
// a use statement would be a dependency-floor violation.
use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_story::Story;
use agentic_uat::{ExecutionOutcome, StubExecutor, Uat, UatError, UatExecutor, Verdict};
use tempfile::TempDir;

const STORY_ID: u32 = 4245;

const FIXTURE_YAML: &str = r#"id: 4245
title: "A fixture story for the standalone-resilience scaffold"

outcome: |
  A fixture driven through the library directly to witness the
  standalone-resilience claim.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_standalone_resilience.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Run the stub executor from this test; verify verdict and row shape.

guidance: |
  Fixture authored inline for the standalone-resilience scaffold. Not a
  real story.

depends_on: []
"#;

#[test]
fn uat_library_is_driveable_with_only_the_declared_dependency_floor() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    let head_sha = init_repo_and_commit_seed(repo_root);

    // Sanity: the same `agentic-story` loader the library uses can read
    // the fixture on its own, proving the fixture shape matches the
    // loader's schema. This also witnesses that `agentic-story` is
    // genuinely reachable at the test's dependency floor.
    let loaded = Story::load(&story_path).expect("fixture must load via StoryLoader");
    assert_eq!(
        loaded.id, STORY_ID,
        "fixture id must round-trip through StoryLoader"
    );

    // Primary entry point constructed with only the declared dependency
    // floor — a Store (MemStore), a UatExecutor (StubExecutor), and a
    // stories directory path. No orchestrator. No runtime. No sandbox.
    // No CLI.
    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    let verdict = uat.run(STORY_ID).expect("standalone uat must produce a verdict");
    assert!(
        matches!(verdict, Verdict::Pass),
        "standalone stub-always-pass path must yield a Pass verdict; got {verdict:?}"
    );

    // The row shape must match what the CLI path would produce — same
    // fields, same table — so a future CLI shim is a pure mapping, not a
    // re-implementation.
    let rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64)
        })
        .expect("store query should succeed");
    assert_eq!(
        rows.len(),
        1,
        "standalone path must record exactly one uat_signings row; got {} rows: {rows:?}",
        rows.len()
    );
    let row = &rows[0];
    assert_eq!(
        row.get("verdict").and_then(|v| v.as_str()),
        Some("pass"),
        "standalone row must carry verdict=\"pass\"; got row={row}"
    );
    assert_eq!(
        row.get("commit").and_then(|v| v.as_str()),
        Some(head_sha.as_str()),
        "standalone row must carry the full HEAD SHA; got row={row}, expected {head_sha}"
    );
    assert!(
        row.get("signed_at").and_then(|v| v.as_str()).is_some(),
        "standalone row must carry a `signed_at` field; got row={row}"
    );
    assert!(
        row.get("id").is_some(),
        "standalone row must carry an `id` (ULID or UUIDv7) field; got row={row}"
    );

    // Compile-time witnesses that the full standalone-driving surface is
    // reachable through `agentic_uat::*` alone — no transitive import
    // from orchestrator/runtime/sandbox/CLI crates is required. If any
    // of these names were secretly re-exports from a forbidden crate,
    // the `use` statements above would have pulled that crate into the
    // link graph.
    let _verdict_kind: fn(Verdict) -> &'static str = |v| v.as_str();
    let _error_is_local: fn(&UatError) -> &dyn std::error::Error = |e| e;

    // `UatExecutor` and `ExecutionOutcome` are part of the library's
    // public trait surface; a local zero-state impl must satisfy the
    // trait using only the library's own types. This is what lets a
    // future non-stub executor (a UAT agent driving a human) slot into
    // the signed-verdict contract without changing it.
    struct LocalPassExecutor;
    impl UatExecutor for LocalPassExecutor {
        fn execute(&self, _story: &Story) -> ExecutionOutcome {
            ExecutionOutcome {
                verdict: Verdict::Pass,
                transcript: String::new(),
            }
        }
    }
    let _witness: Box<dyn UatExecutor> = Box::new(LocalPassExecutor);
}

/// See uat_pass.rs for rationale. Duplicated here rather than hoisted to
/// a `tests/common/mod.rs` so each scaffold is independently readable.
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
