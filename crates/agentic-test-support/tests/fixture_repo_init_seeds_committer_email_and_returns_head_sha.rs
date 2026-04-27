//! Story 26 acceptance test: `FixtureRepo::init_with_email(...)`
//! initialises a tempdir git repo with `committer.email` set, lands one
//! initial commit, and `head_sha()` returns a 40-character lowercase hex
//! string matching the canonical `commit` form in
//! `agents/assets/definitions/identifier-forms.yml` (declared in story
//! 26's `assets:` field).
//!
//! Justification (from stories/26.yml): pins the git-seeding primitive
//! against the canonical SHA shape. The test asserts the regex
//! `^[0-9a-f]{40}$` literally so a future drift to a 7- or 8-char
//! abbreviated SHA — exactly the failure `identifier-forms.yml`'s
//! `drift_warning` calls out for `commit` — fails red rather than
//! passing silently. Without this pin, every downstream test that
//! compares its captured SHA against an evidence row's `commit` field
//! is at the mercy of whatever shape the kit happens to return.
//!
//! Red today is compile-red: `FixtureRepo::init_with_email(...)`,
//! `head_sha()`, and the `committer_email()` accessor are not yet
//! declared on the unit-struct shell.

use std::path::Path;

use agentic_test_support::FixtureRepo;
use regex::Regex;
use tempfile::TempDir;

#[test]
fn fixture_repo_init_seeds_committer_email_and_returns_head_sha() {
    // Caller-supplied tempdir; the kit does not assume ownership of the
    // tempdir lifecycle — it seeds the path the caller hands it.
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_path: &Path = tmp.path();

    let repo = FixtureRepo::init_with_email(repo_path, "ci@example.com");

    // The committer email the caller supplied must be retrievable
    // verbatim — no munging, no lowercasing, no domain rewrite.
    assert_eq!(
        repo.committer_email(),
        "ci@example.com",
        "FixtureRepo must round-trip the committer email it was seeded with"
    );

    // head_sha() must match the canonical commit shape from
    // identifier-forms.yml: 40 lowercase hex chars, no abbreviation,
    // no leading/trailing whitespace.
    let sha = repo.head_sha();
    let canonical = Regex::new(r"^[0-9a-f]{40}$").expect("compile commit regex");
    assert!(
        canonical.is_match(&sha),
        "FixtureRepo::head_sha() must return a 40-char lowercase hex SHA \
         per agents/assets/definitions/identifier-forms.yml; got `{sha}` \
         (length {len})",
        len = sha.len()
    );

    // Belt-and-braces: explicit length check so a future regex
    // regression doesn't mask an abbreviated-SHA drift.
    assert_eq!(
        sha.len(),
        40,
        "head_sha() must be exactly 40 chars; got {} chars: `{sha}`",
        sha.len()
    );

    // The repo path on disk must contain a real .git directory — the
    // primitive cannot be a no-op shell that returns a fake SHA.
    assert!(
        repo_path.join(".git").exists(),
        "init_with_email() must create a real .git directory at {}",
        repo_path.display()
    );
}
