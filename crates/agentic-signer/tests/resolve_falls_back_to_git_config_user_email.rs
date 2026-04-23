//! Story 18 acceptance test: tier 3 of the four-tier signer-resolution
//! chain — `git config user.email` is consulted when no flag and no
//! env var are present.
//!
//! Justification (from stories/18.yml acceptance.tests[2]):
//!   Proves tier 3: with no flag and no env var, but a git
//!   repository at the working directory whose `git config
//!   user.email` is `dev@example.com`,
//!   `Signer::resolve` returns `dev@example.com`. Reading
//!   goes through `git2::Config::open_default()` or a
//!   repo-scoped `Repository::config()` — whichever the
//!   implementation picks, the acceptance is that the same
//!   value `git config --get user.email` prints is the
//!   value the resolver returns. Without this, the human
//!   path (the most common one for a dev running `agentic
//!   uat` locally) requires `--signer` on every invocation,
//!   and the ergonomic default the outcome promises does
//!   not exist.
//!
//! Red today: compile-red via the missing `agentic_signer`
//! public surface (`Resolver`, `Signer`).

use agentic_signer::{Resolver, Signer};
use std::path::Path;
use tempfile::TempDir;

#[test]
fn resolve_falls_back_to_git_config_user_email() {
    let repo_tmp = TempDir::new().expect("tempdir");
    let repo_path = repo_tmp.path();
    init_repo_with_email(repo_path, "dev@example.com");

    // Make absolutely sure the env var is not set — the test asserts
    // strict tier-3 fallthrough.
    std::env::remove_var("AGENTIC_SIGNER");

    let resolver = Resolver::new().at_repo(repo_path);
    let signer = Signer::resolve(resolver).expect("git-tier resolution must succeed");

    assert_eq!(
        signer.as_str(),
        "dev@example.com",
        "tier 3 must return the git config user.email when flag and env are absent"
    );

    // The value must be byte-identical to what `git config --get
    // user.email` would print from the same repo. Read through git2 to
    // cross-check (avoids a shell dependency).
    let repo = git2::Repository::open(repo_path).expect("git open");
    let cfg = repo.config().expect("repo config");
    let email_from_git = cfg
        .get_string("user.email")
        .expect("repo must have user.email configured");
    assert_eq!(
        signer.as_str(),
        email_from_git,
        "resolved signer must be byte-identical to git config user.email"
    );
}

fn init_repo_with_email(root: &Path, email: &str) {
    let repo = git2::Repository::init(root).expect("git init");
    let mut cfg = repo.config().expect("repo config");
    cfg.set_str("user.name", "test-builder").expect("set user.name");
    cfg.set_str("user.email", email).expect("set user.email");
}
