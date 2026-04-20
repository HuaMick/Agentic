//! Story 10 acceptance test: `--all` is rejected when combined with
//! a positional selector OR with `--expand`.
//!
//! Justification (from stories/10.yml): proves the mutual-exclusion
//! rule — `--all` combined with any positional selector (`<id>`,
//! `+<id>`, `<id>+`, `+<id>+`) OR with `--expand` is rejected at
//! argv-parse time with a typed error and exit code 2. Selectors
//! already define the row set; `--expand` already defines a different
//! row set; layering `--all` over either makes the command's intent
//! unreadable. Without this the CLI accepts argv it cannot faithfully
//! honour.
//!
//! This is an argv-level observable, so the test drives the compiled
//! `agentic` binary via `assert_cmd` and asserts exit code 2 on each
//! of the five rejected combinations.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use tempfile::TempDir;

const KNOWN_ID: u32 = 91801;

fn fixture_yaml(id: u32) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for all-rejects-selector scaffold"

outcome: |
  Fixture whose ID the scaffold uses as the selector argument.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_all_rejects_with_selector.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Run the binary with --all plus selector or --expand; assert exit 2.

guidance: |
  Fixture authored inline for the all-rejects-selector scaffold. Not a
  real story.

depends_on: []
"#
    )
}

fn init_repo_and_seed(root: &Path) {
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
    repo.commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
}

fn setup_repo() -> (TempDir, TempDir) {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{KNOWN_ID}.yml")),
        fixture_yaml(KNOWN_ID),
    )
    .expect("write fixture");
    init_repo_and_seed(repo_root);
    let store_tmp = TempDir::new().expect("store tempdir");
    (repo_tmp, store_tmp)
}

fn assert_exit_code_2(args: &[&str]) {
    let (repo_tmp, store_tmp) = setup_repo();
    let repo_root = repo_tmp.path();

    let mut cmd = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve — add agentic-cli as dev-dep if absent");
    cmd.current_dir(repo_root).arg("stories").arg("health");
    for a in args {
        cmd.arg(a);
    }
    cmd.arg("--store").arg(store_tmp.path());

    let assert = cmd.assert();
    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code();
    assert_eq!(
        code,
        Some(2),
        "`agentic stories health {}` must exit 2; got status={:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        args.join(" "),
        output.status
    );
}

#[test]
fn all_combined_with_expand_exits_2() {
    assert_exit_code_2(&["--all", "--expand"]);
}

#[test]
fn all_combined_with_bareword_selector_exits_2() {
    let id = KNOWN_ID.to_string();
    assert_exit_code_2(&["--all", &id]);
}

#[test]
fn all_combined_with_ancestors_selector_exits_2() {
    let sel = format!("+{KNOWN_ID}");
    assert_exit_code_2(&["--all", &sel]);
}

#[test]
fn all_combined_with_descendants_selector_exits_2() {
    let sel = format!("{KNOWN_ID}+");
    assert_exit_code_2(&["--all", &sel]);
}

#[test]
fn all_combined_with_subtree_selector_exits_2() {
    let sel = format!("+{KNOWN_ID}+");
    assert_exit_code_2(&["--all", &sel]);
}
