//! Story 18 acceptance test: CI-record fails closed on unresolvable
//! signer — typed `RecordError::SignerMissing` distinct from
//! `MalformedInput`, zero rows written, existing row preserved.
//!
//! Justification (from stories/18.yml acceptance.tests[8]):
//!   Proves the same fail-closed floor on the CI path:
//!   `Recorder::record` with `SignerSource::Resolve`
//!   against an environment with no flag, no env, no git
//!   `user.email` returns `RecordError::SignerMissing`,
//!   writes zero rows to `test_runs` (the existing row,
//!   if any, is untouched per story 2's malformed-input
//!   contract shape), and does not propagate the test
//!   runner's verdict to the store. The error is distinct
//!   from `RecordError::MalformedInput` so the CI wrapper
//!   can distinguish "fix your git config" from "fix your
//!   test output." Without this, a misconfigured CI
//!   runner silently writes `test_runs` rows with empty
//!   signer fields and the symmetry-with-uat claim is
//!   only aspirational.
//!
//! Red today: compile-red via the missing `SignerSource` symbol and
//! the missing `RecordError::SignerMissing` variant on the
//! `RecordError` enum.

use std::path::Path;
use std::sync::Arc;

use agentic_ci_record::{RecordError, Recorder, RunInput, SignerSource};
use agentic_store::{MemStore, Store};
use tempfile::TempDir;

#[test]
fn record_refuses_when_signer_cannot_be_resolved_and_is_distinct_from_malformed_input() {
    const STORY_ID: i64 = 99901;

    // Arrange a worldview where no signer source is reachable:
    //   - no env var
    //   - chdir into a tempdir repo whose git config has no user.email
    //
    // The recorder reads git config from the working directory
    // (mirroring story 2's commit-reading behaviour); scoping with
    // cwd is the least-invasive way to block tier 3 from finding an
    // ambient email.
    let repo_tmp = TempDir::new().expect("tempdir");
    init_repo_without_email(repo_tmp.path());
    let cwd_guard = push_cwd(repo_tmp.path());

    std::env::remove_var("AGENTIC_SIGNER");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let recorder = Recorder::new(store.clone());

    // Seed a pre-existing row on a DIFFERENT story id so we can prove
    // the refusal did not clobber it. (Seeding requires a resolvable
    // signer; set one, seed, unset again.)
    const SEED_STORY_ID: i64 = 99902;
    std::env::set_var("AGENTIC_SIGNER", "seed-signer@example.com");
    recorder
        .record_with_signer(RunInput::pass(SEED_STORY_ID), SignerSource::Resolve)
        .expect("seed row must succeed with env-set signer");
    let seed_row = store
        .get("test_runs", &SEED_STORY_ID.to_string())
        .expect("get")
        .expect("seed row must exist");
    std::env::remove_var("AGENTIC_SIGNER");

    // Now attempt the real call: no signer resolvable, so it must fail.
    let err = recorder
        .record_with_signer(RunInput::pass(STORY_ID), SignerSource::Resolve)
        .expect_err("no-signer record must fail with a typed error");

    assert!(
        matches!(err, RecordError::SignerMissing { .. }),
        "expected RecordError::SignerMissing; got {err:?}"
    );
    // SignerMissing must be DISTINCT from MalformedInput (not aliased
    // as a variant of it). A pattern match against MalformedInput must
    // NOT succeed.
    assert!(
        !matches!(err, RecordError::MalformedInput { .. }),
        "SignerMissing must NOT be routed as MalformedInput — the CI \
         wrapper needs them distinct; got {err:?}"
    );

    // Zero rows for the refused story id.
    let refused = store.get("test_runs", &STORY_ID.to_string()).expect("get");
    assert!(
        refused.is_none(),
        "refusal must write zero rows for the refused story id; got {refused:?}"
    );

    // Pre-existing seed row is untouched.
    let seed_row_after = store
        .get("test_runs", &SEED_STORY_ID.to_string())
        .expect("get")
        .expect("seed row must still exist");
    assert_eq!(
        seed_row, seed_row_after,
        "refusal must NOT mutate the pre-existing row for a different story id"
    );

    drop(cwd_guard);
}

fn init_repo_without_email(root: &Path) {
    let repo = git2::Repository::init(root).expect("git init");
    let mut cfg = repo.config().expect("repo config");
    cfg.set_str("user.name", "test-builder")
        .expect("set user.name");
    // user.email intentionally NOT set.
    let _ = cfg;
    // The recorder reads HEAD to commit-stamp its rows; an unborn-branch
    // repo errors with `UnbornBranch` before reaching the signer-resolution
    // step. Plant an empty baseline commit so HEAD resolves. Construct
    // the signature inline (libgit2's default signature lookup would fail
    // because we deliberately left user.email unset on this repo).
    let tree_oid = repo
        .treebuilder(None)
        .expect("treebuilder")
        .write()
        .expect("write empty tree");
    let tree = repo.find_tree(tree_oid).expect("find tree");
    let sig = git2::Signature::now("test-builder", "test-builder@agentic.local")
        .expect("manual signature");
    repo.commit(Some("HEAD"), &sig, &sig, "baseline", &tree, &[])
        .expect("baseline commit");
    // Re-open and clear any user.email that libgit2 may have written
    // implicitly during the commit, so tier 3 (git config user.email)
    // remains unresolvable for the test's signer-refusal observation.
    let repo = git2::Repository::open(root).expect("git open");
    let mut cfg = repo.config().expect("repo config post");
    let _ = cfg.remove("user.email");
}

/// RAII guard: push the current working dir into the tempdir and
/// restore it on drop. Uses std::env::set_current_dir which is
/// inherently process-global; the recorder's git2 repo discovery
/// honours it.
struct CwdGuard {
    prev: std::path::PathBuf,
}
impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}
fn push_cwd(into: &Path) -> CwdGuard {
    let prev = std::env::current_dir().expect("current_dir");
    std::env::set_current_dir(into).expect("set_current_dir");
    CwdGuard { prev }
}
