//! Story 18 acceptance test: tier 2 of the four-tier signer-resolution
//! chain — `AGENTIC_SIGNER` env var wins over git config when no flag
//! is passed.
//!
//! Justification (from stories/18.yml acceptance.tests[1]):
//!   Proves tier 2: with no `--signer` flag but
//!   `AGENTIC_SIGNER=env-person@example.com` set AND
//!   `git config user.email=git-person@example.com` set,
//!   `Signer::resolve` returns `env-person@example.com`.
//!   Clearing the env var on a subsequent call and leaving
//!   git config set returns the git value — proving the env
//!   tier is consulted strictly between the absent flag and
//!   the git fallback. Without this, the sandbox convention
//!   (`AGENTIC_SIGNER=sandbox:<model>@<run_id>` injected by
//!   the runtime) cannot override a human's `git config
//!   user.email` that happens to be set in the same
//!   environment, and every agent run would sign as the
//!   human running the container.
//!
//! Red today: compile-red via the missing `agentic_signer`
//! public surface (`Resolver`, `Signer`).

use agentic_signer::{Resolver, Signer};
use std::path::Path;
use tempfile::TempDir;

#[test]
fn resolve_prefers_env_over_git_when_no_flag() {
    let repo_tmp = TempDir::new().expect("tempdir");
    let repo_path = repo_tmp.path();
    init_repo_with_email(repo_path, "git-person@example.com");

    // No flag, env set, git set — env must win.
    std::env::set_var("AGENTIC_SIGNER", "env-person@example.com");
    let resolver = Resolver::new().at_repo(repo_path);
    let signer = Signer::resolve(resolver).expect("env-present resolution must succeed");
    assert_eq!(
        signer.as_str(),
        "env-person@example.com",
        "tier 2 (env) must win over tier 3 (git) when no flag is passed"
    );

    // Clear env, leave git set — git falls through.
    std::env::remove_var("AGENTIC_SIGNER");
    let resolver2 = Resolver::new().at_repo(repo_path);
    let signer2 = Signer::resolve(resolver2).expect("git-fallback resolution must succeed");
    assert_eq!(
        signer2.as_str(),
        "git-person@example.com",
        "tier 3 (git) must be consulted when tier 2 (env) is absent"
    );
}

fn init_repo_with_email(root: &Path, email: &str) {
    let repo = git2::Repository::init(root).expect("git init");
    let mut cfg = repo.config().expect("repo config");
    cfg.set_str("user.name", "test-builder").expect("set user.name");
    cfg.set_str("user.email", email).expect("set user.email");
}
