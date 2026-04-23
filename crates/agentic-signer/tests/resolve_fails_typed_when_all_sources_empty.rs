//! Story 18 acceptance test: tier 4 of the four-tier signer-resolution
//! chain — typed `SignerMissing` error when every source is empty.
//!
//! Justification (from stories/18.yml acceptance.tests[3]):
//!   Proves the strict floor: with no flag, no env var, and
//!   either no git config or a git config whose
//!   `user.email` is unset, `Signer::resolve` returns
//!   `SignerError::SignerMissing` naming which sources it
//!   consulted and found empty. The function does NOT panic,
//!   does NOT fall back to the current unix user, hostname,
//!   or any other guess, and does NOT return an empty
//!   `Signer`. Without this, the exit-2-and-no-store-write
//!   contract at the CLI boundary has no library-level
//!   proof, and a regression that quietly defaulted the
//!   signer to `""` or `"unknown"` would pass every other
//!   test in this file.
//!
//! Red today: compile-red via the missing `agentic_signer`
//! public surface (`Resolver`, `Signer`, `SignerError`,
//! `Source`).

use agentic_signer::{Resolver, Signer, SignerError, Source};
use std::path::Path;
use tempfile::TempDir;

#[test]
fn resolve_fails_typed_when_all_sources_empty() {
    // Init a tempdir git repo with NO user.email configured and scope
    // the resolver to it — so the git tier has nothing to read back.
    let repo_tmp = TempDir::new().expect("tempdir");
    let repo_path = repo_tmp.path();
    init_bare_repo_without_email(repo_path);

    // Hard-unset the env var for this test's duration.
    std::env::remove_var("AGENTIC_SIGNER");

    // No flag on the resolver.
    let resolver = Resolver::new().at_repo(repo_path);

    let err = Signer::resolve(resolver)
        .expect_err("all-sources-empty resolution must fail with a typed error");

    // Must be SignerMissing — NOT a panic, NOT a fallback to unix user,
    // NOT an empty `Signer`.
    match err {
        SignerError::SignerMissing { consulted } => {
            // The error must name the sources it actually consulted. All
            // three (Flag, Env, Git) are in scope for the miss.
            assert!(
                consulted.contains(&Source::Flag),
                "SignerMissing must name Flag as consulted; got {consulted:?}"
            );
            assert!(
                consulted.contains(&Source::Env),
                "SignerMissing must name Env as consulted; got {consulted:?}"
            );
            assert!(
                consulted.contains(&Source::Git),
                "SignerMissing must name Git as consulted; got {consulted:?}"
            );
        }
        other => panic!(
            "expected SignerError::SignerMissing, got {other:?} — a regression that \
             falls back to unix user or an empty string would surface here"
        ),
    }
}

/// Init a git repo with NO user.email set — the point of this test.
/// Use isolated-config scope so ambient `~/.gitconfig` does not leak in.
fn init_bare_repo_without_email(root: &Path) {
    let _repo = git2::Repository::init(root).expect("git init");
    // Deliberately NOT setting user.email — the resolver must treat
    // this as "git source empty" and fall through to SignerMissing.
}
