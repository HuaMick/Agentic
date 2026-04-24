//! Story 18 acceptance test: the standalone-resilient-library claim for
//! `agentic-signer` — the crate's `Resolver` + `Signer` can be driven
//! end-to-end against a dependency floor of only `agentic-signer`,
//! `git2`, and the standard library (plus `tempfile` for an ephemeral
//! scratch dir, as allowed by the standalone-resilience pattern).
//!
//! Justification (from stories/18.yml acceptance.tests[13]):
//!   Proves the standalone-resilient-library claim for
//!   the new crate: `agentic-signer`'s `Resolver` +
//!   `Signer` can be constructed and driven end-to-end
//!   by a test that links against only `agentic-signer`,
//!   `git2`, and the standard library — no `agentic-
//!   cli`, no `agentic-uat`, no `agentic-ci-record`, no
//!   `agentic-runtime`, no `agentic-store`. The test
//!   constructs a tempdir repo, sets a git config, flips
//!   env vars, and asserts the resolved value for each
//!   tier. Without this, the resolver quietly grows a
//!   dependency on the UAT crate (because "it's
//!   convenient" to share an error type) or on the
//!   runtime (because "the sandbox convention lives
//!   there") and the first downstream crate that wants
//!   to call it end-to-end drags half the workspace in
//!   with it.
//!
//! Red today: compile-red via the missing `agentic_signer`
//! public surface (`Resolver`, `Signer`).
//!
//! Dependency-floor discipline: this file deliberately names ONLY
//! `agentic_signer`, `git2`, stdlib, and `tempfile` in its `use`
//! statements. Adding an orchestrator-dependent crate here — even by
//! accident — is the regression this test is here to pin.

use agentic_signer::{Resolver, Signer};
use std::path::Path;
use tempfile::TempDir;

#[test]
fn signer_can_be_driven_end_to_end_from_the_allowed_dependency_floor() {
    let repo_tmp = TempDir::new().expect("tempdir");
    let repo_path = repo_tmp.path();
    init_repo_with_email(repo_path, "floor-test@example.com");

    // Tier 1: flag wins.
    std::env::remove_var("AGENTIC_SIGNER");
    let s1 = Signer::resolve(Resolver::with_flag("flag-val@example.com").at_repo(repo_path))
        .expect("flag resolve");
    assert_eq!(s1.as_str(), "flag-val@example.com");

    // Tier 2: env wins when no flag.
    std::env::set_var("AGENTIC_SIGNER", "env-val@example.com");
    let s2 = Signer::resolve(Resolver::new().at_repo(repo_path)).expect("env resolve");
    assert_eq!(s2.as_str(), "env-val@example.com");

    // Tier 3: git wins when no flag and no env.
    std::env::remove_var("AGENTIC_SIGNER");
    let s3 = Signer::resolve(Resolver::new().at_repo(repo_path)).expect("git resolve");
    assert_eq!(s3.as_str(), "floor-test@example.com");
}

fn init_repo_with_email(root: &Path, email: &str) {
    let repo = git2::Repository::init(root).expect("git init");
    let mut cfg = repo.config().expect("repo config");
    cfg.set_str("user.name", "test-builder")
        .expect("set user.name");
    cfg.set_str("user.email", email).expect("set user.email");
}
