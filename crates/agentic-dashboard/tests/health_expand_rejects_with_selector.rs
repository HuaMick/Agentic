//! Story 10 acceptance test: `--expand` is rejected when combined
//! with any positional selector.
//!
//! Justification (from stories/10.yml): proves argv validation —
//! `--expand` combined with any positional selector (`<id>`, `+<id>`,
//! `<id>+`, `+<id>+`) is rejected at argv-parse time with a typed
//! error and exit code 2 — the two mechanisms select row sets in
//! different ways and combining them has no coherent meaning.
//! Without this, the CLI accepts argv it cannot faithfully honour
//! and the operator gets a silently surprising row set.
//!
//! This is an argv-level observable, so the test drives the compiled
//! `agentic` binary via `assert_cmd` and asserts exit code 2 on
//! each of the four selector forms combined with `--expand`.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use tempfile::TempDir;

const KNOWN_ID: u32 = 91601;

fn fixture_yaml(id: u32) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for expand-rejects-selector scaffold"

outcome: |
  Fixture whose ID the scaffold uses as the selector argument.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_expand_rejects_with_selector.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Run the binary with --expand plus selector; assert exit 2.

guidance: |
  Fixture authored inline for the expand-rejects-selector scaffold.
  Not a real story.

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

fn run_with_expand_and_selector(selector: &str) {
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

    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve — add agentic-cli as dev-dep if absent")
        .current_dir(repo_root)
        .arg("stories")
        .arg("health")
        .arg("--expand")
        .arg(selector)
        .arg("--store")
        .arg(store_tmp.path())
        .assert();

    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code();
    assert_eq!(
        code,
        Some(2),
        "`--expand {selector}` must exit with code 2 (argv-parse-time rejection); \
         got status={:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status
    );
}

#[test]
fn expand_combined_with_bareword_selector_exits_2() {
    run_with_expand_and_selector(&KNOWN_ID.to_string());
}

#[test]
fn expand_combined_with_ancestors_selector_exits_2() {
    run_with_expand_and_selector(&format!("+{KNOWN_ID}"));
}

#[test]
fn expand_combined_with_descendants_selector_exits_2() {
    run_with_expand_and_selector(&format!("{KNOWN_ID}+"));
}

#[test]
fn expand_combined_with_subtree_selector_exits_2() {
    run_with_expand_and_selector(&format!("+{KNOWN_ID}+"));
}
