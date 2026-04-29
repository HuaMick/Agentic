//! Story 3 acceptance test: `agentic stories health --all` enforces
//! the exit-code-as-gate contract at the binary boundary.
//!
//! Justification (from stories/3.yml): proves the exit-code-as-gate
//! contract — `agentic stories health --all` against a fixture
//! corpus containing at least one row whose computed health is
//! `error` (a story whose YAML says `status: healthy` but whose
//! store carries no `uat_signings.verdict=pass` row — the
//! status-evidence-mismatch shape) exits non-zero (exit code 2,
//! "could-not-attest", mirroring the fail-closed-on-dirty-tree
//! pattern's mapping); against a corpus where every row is
//! `healthy`, `proposed`, or `under_construction` the same command
//! exits 0. Without this, the dashboard's detection of forged
//! promotions remains a silent observation: rendering shows `error`
//! to a human reader, but a pre-commit hook (story 29) wrapping the
//! command sees only exit 0. The exit code is what makes this
//! command an enforceable gate rather than a cosmetic inspector.
//!
//! Today the binary exits 0 regardless of detected drift (the render
//! call returns `Ok(output)` which is printed and main falls through
//! to a 0 exit). Red today is RUNTIME-red: the scaffold compiles
//! against the existing CLI surface (`agentic stories health --all
//! --store <tempdir>`) and the assertion `status.code() == Some(2)`
//! fails on the error-fixture invocation. The clean-fixture
//! invocation already exits 0; the scaffold pins both halves of the
//! two-sided observable so an implementation that flips the wrong
//! direction (e.g. exits 2 unconditionally) is also caught.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use tempfile::TempDir;

const ID_FORGED_HEALTHY: u32 = 30401; // status: healthy on disk, NO uat_signings row → error class.
const ID_PROPOSED: u32 = 30402;
const ID_UNDER_CONSTRUCTION: u32 = 30403;

fn fixture_yaml(id: u32, status: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for exit-code-on-drift gate"

outcome: |
  Fixture row authored inline for the story-3 exit-code-on-drift
  scaffold; not a real story.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/health_exits_non_zero_on_error_or_unhealthy.rs
      justification: |
        Present so the fixture YAML is itself schema-valid; the live
        scaffold drives the agentic binary against this directory.
  uat: |
    Run `agentic stories health --all`; assert exit code matches
    the gate contract.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#
    )
}

fn init_repo_and_seed(root: &Path) {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder")
            .expect("set user.name");
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
    repo.commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
}

#[test]
fn agentic_stories_health_all_exits_two_on_error_row_and_zero_on_clean_corpus() {
    // ----- Half 1: error-row corpus must exit non-zero (specifically 2). -----
    //
    // Fixture: a single story with `status: healthy` on disk, but the
    // configured store carries NO `uat_signings.verdict=pass` row for
    // it. The dashboard's classification rules render this as
    // `error: status-evidence mismatch`. The exit-code-as-gate contract
    // says this case must exit 2 — a wrapping pre-commit hook (story
    // 29) needs the non-zero signal to refuse the commit.
    let error_repo = TempDir::new().expect("error-fixture repo tempdir");
    let error_root = error_repo.path();
    let error_stories = error_root.join("stories");
    fs::create_dir_all(&error_stories).expect("error-fixture stories dir");
    fs::write(
        error_stories.join(format!("{ID_FORGED_HEALTHY}.yml")),
        fixture_yaml(ID_FORGED_HEALTHY, "healthy"),
    )
    .expect("write FORGED_HEALTHY fixture");
    init_repo_and_seed(error_root);

    let error_store = TempDir::new().expect("error-fixture store tempdir");

    let error_assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(error_root)
        .arg("stories")
        .arg("health")
        .arg("--all")
        .arg("--store")
        .arg(error_store.path())
        .assert();

    let error_output = error_assert.get_output().clone();
    let error_stdout = String::from_utf8_lossy(&error_output.stdout).to_string();
    let error_stderr = String::from_utf8_lossy(&error_output.stderr).to_string();
    let error_status = error_output.status;

    assert_eq!(
        error_status.code(),
        Some(2),
        "`agentic stories health --all` against an error-class corpus \
         (status: healthy with no uat_signings.verdict=pass row, the \
         status-evidence-mismatch shape) must exit 2 — the \
         could-not-attest exit code that lets a pre-commit hook refuse \
         the commit. Got status={error_status:?}\n\
         stdout:\n{error_stdout}\n\
         stderr:\n{error_stderr}"
    );

    // ----- Half 2: all-clean corpus must exit 0. -----
    //
    // Fixture: stories whose statuses are all in
    // `proposed | under_construction` (no `healthy` rows are seeded
    // because reaching `healthy` requires a uat_signings row matching
    // HEAD, which a tempdir store does not have). Every row classifies
    // as either `proposed` or `under_construction` — neither is a gate
    // failure — so the command must exit 0. This is the inverse half
    // of the two-sided observable: an implementation that exits
    // non-zero unconditionally would fail this assertion even though
    // it would pass the error-fixture half.
    let clean_repo = TempDir::new().expect("clean-fixture repo tempdir");
    let clean_root = clean_repo.path();
    let clean_stories = clean_root.join("stories");
    fs::create_dir_all(&clean_stories).expect("clean-fixture stories dir");
    fs::write(
        clean_stories.join(format!("{ID_PROPOSED}.yml")),
        fixture_yaml(ID_PROPOSED, "proposed"),
    )
    .expect("write PROPOSED fixture");
    fs::write(
        clean_stories.join(format!("{ID_UNDER_CONSTRUCTION}.yml")),
        fixture_yaml(ID_UNDER_CONSTRUCTION, "under_construction"),
    )
    .expect("write UNDER_CONSTRUCTION fixture");
    init_repo_and_seed(clean_root);

    let clean_store = TempDir::new().expect("clean-fixture store tempdir");

    let clean_assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(clean_root)
        .arg("stories")
        .arg("health")
        .arg("--all")
        .arg("--store")
        .arg(clean_store.path())
        .assert();

    let clean_output = clean_assert.get_output().clone();
    let clean_stdout = String::from_utf8_lossy(&clean_output.stdout).to_string();
    let clean_stderr = String::from_utf8_lossy(&clean_output.stderr).to_string();
    let clean_status = clean_output.status;

    assert_eq!(
        clean_status.code(),
        Some(0),
        "`agentic stories health --all` against a corpus whose every \
         row classifies as `proposed`, `under_construction`, or \
         `healthy` must exit 0 — the prove-it gate already attests to \
         this state. Got status={clean_status:?}\n\
         stdout:\n{clean_stdout}\n\
         stderr:\n{clean_stderr}"
    );
}
