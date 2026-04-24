//! Story 18 acceptance test: tier 1 of the four-tier signer-resolution
//! chain — `--signer` flag wins over env var and git config.
//!
//! Justification (from stories/18.yml acceptance.tests[0]):
//!   Proves tier 1 of the resolution chain: given all three
//!   lower-tier sources populated with different values
//!   (`AGENTIC_SIGNER=env-person@example.com`,
//!   `git config user.email=git-person@example.com`) and a
//!   caller passing `Signer::resolve(Resolver::with_flag(
//!   "flag-person@example.com"))`, the returned `Signer`
//!   carries the flag value byte-identical. The env var and
//!   git config are NOT consulted (observable by swapping
//!   both to invalid values partway through the test — the
//!   resolver still returns the flag value). Without this,
//!   the documented precedence order collapses into "whatever
//!   the implementation reads first that day," and an
//!   operator passing `--signer` cannot be confident their
//!   override landed.
//!
//! Red today: compile-red via the missing `agentic_signer`
//! public surface (`Resolver`, `Signer`).

use agentic_signer::{Resolver, Signer};
use std::path::Path;
use tempfile::TempDir;

#[test]
fn resolve_prefers_cli_flag_over_env_and_git() {
    // Build a tempdir git repo with an email configured — a value that
    // must NOT be returned when a flag is present.
    let repo_tmp = TempDir::new().expect("tempdir");
    let repo_path = repo_tmp.path();
    init_repo_with_email(repo_path, "git-person@example.com");

    // Populate the env var with yet another value — also must NOT win.
    std::env::set_var("AGENTIC_SIGNER", "env-person@example.com");

    // Explicit flag carries priority over both.
    let resolver = Resolver::with_flag("flag-person@example.com").at_repo(repo_path);
    let signer = Signer::resolve(resolver).expect("flag-present resolution must succeed");

    assert_eq!(
        signer.as_str(),
        "flag-person@example.com",
        "tier 1 (flag) must win; env+git must not be consulted"
    );

    // Swap env and git to invalid values; the resolver is then asked
    // again. If the resolver consulted env or git it would fail
    // validation or return a different string. It must STILL return
    // the flag value — flag is the single authoritative source.
    std::env::set_var("AGENTIC_SIGNER", ""); // empty → invalid per whitespace rule
    set_repo_email(repo_path, ""); // empty → invalid per whitespace rule

    let resolver2 = Resolver::with_flag("flag-person@example.com").at_repo(repo_path);
    let signer2 = Signer::resolve(resolver2)
        .expect("flag-only resolution must still succeed with invalid env/git");
    assert_eq!(
        signer2.as_str(),
        "flag-person@example.com",
        "tier 1 (flag) must still win even when env+git are invalid"
    );

    // Cleanup — don't leak the env var into sibling tests.
    std::env::remove_var("AGENTIC_SIGNER");
}

fn init_repo_with_email(root: &Path, email: &str) {
    let repo = git2::Repository::init(root).expect("git init");
    let mut cfg = repo.config().expect("repo config");
    cfg.set_str("user.name", "test-builder")
        .expect("set user.name");
    cfg.set_str("user.email", email).expect("set user.email");
}

fn set_repo_email(root: &Path, email: &str) {
    let repo = git2::Repository::open(root).expect("git open");
    let mut cfg = repo.config().expect("repo config");
    cfg.set_str("user.email", email).expect("set user.email");
}
