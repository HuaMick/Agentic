//! Story 12 acceptance test: unknown-id selector at the binary exits 2.
//!
//! Justification (from stories/12.yml): proves typed refusal reaches the
//! operator through the binary — `agentic stories test +99999` exits
//! with code 2 (could-not-run — the argv was parseable but the target
//! did not exist), writes zero rows, and emits stderr naming the missing
//! id. Without this, a CLI wrapper that mistranslated the error to exit
//! 1 would tell CI "the suite failed" when no suite ran at all,
//! corrupting the distinction between "real fail" and "could not run."
//!
//! Red today is compile-red: the `agentic stories test` subcommand does
//! not yet exist in the CLI's clap tree.

use std::fs;
use std::path::Path;

use agentic_store::{Store, SurrealStore};
use assert_cmd::Command;
use tempfile::TempDir;

const ID_EXISTS_A: u32 = 81291;
const ID_EXISTS_B: u32 = 81292;
const ID_MISSING: u32 = 99999;

fn fixture_yaml(id: u32, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    let test_file =
        format!("crates/agentic-ci-record/tests/fixture_story_{id}.rs");
    format!(
        r#"id: {id}
title: "Fixture {id} for story-12 CLI unknown-id exit-2 scaffold"

outcome: |
  Fixture row for the CLI unknown-id scaffold.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: {test_file}
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Drive `agentic stories test +99999`; assert exit 2 + stderr id.

guidance: |
  Fixture authored inline. Not a real story.

{deps_yaml}
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

fn setup() -> (TempDir, TempDir) {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    fs::write(
        stories_dir.join(format!("{ID_EXISTS_A}.yml")),
        fixture_yaml(ID_EXISTS_A, &[]),
    )
    .expect("write A");
    fs::write(
        stories_dir.join(format!("{ID_EXISTS_B}.yml")),
        fixture_yaml(ID_EXISTS_B, &[ID_EXISTS_A]),
    )
    .expect("write B");
    // Deliberately no file for ID_MISSING.

    init_repo_and_seed(repo_root);

    let store_tmp = TempDir::new().expect("store tempdir");
    (repo_tmp, store_tmp)
}

#[test]
fn agentic_stories_test_unknown_id_exits_2_writes_no_rows_and_names_missing_id_in_stderr() {
    let (repo_tmp, store_tmp) = setup();
    let repo_root = repo_tmp.path();
    let store_path = store_tmp.path().to_path_buf();

    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        // Even with the stub-pass executor, unknown-id refusal must
        // fire BEFORE reaching the executor — so code 2 holds either
        // way. Set it to prove the env toggle does not mask the refusal.
        .env("AGENTIC_CI_TEST_EXECUTOR", "stub-pass")
        .arg("stories")
        .arg("test")
        .arg(format!("+{ID_MISSING}"))
        .arg("--store")
        .arg(&store_path)
        .assert();

    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let status = output.status;

    // Exit code 2 EXACTLY — the could-not-run code from story 12's
    // exit-code contract. 0 would be silent success (catastrophic); 1
    // would conflate this with a real Fail verdict.
    assert_eq!(
        status.code(),
        Some(2),
        "unknown-id refusal must surface as exit 2 (could-not-run), \
         NOT 0 (pass) or 1 (fail); got status={status:?}\n\
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // stderr names the missing id so an operator can read the error.
    assert!(
        stderr.contains(&ID_MISSING.to_string()),
        "stderr must name the missing id {ID_MISSING}; got stderr:\n{stderr}"
    );

    // Zero `test_runs` rows — no partial write on refusal.
    let store = SurrealStore::open(&store_path)
        .expect("re-opening the configured SurrealStore must succeed");
    let rows = store
        .query("test_runs", &|_| true)
        .expect("test_runs query must succeed");
    assert!(
        rows.is_empty(),
        "unknown-id refusal must write zero test_runs rows; got {} rows: {rows:?}",
        rows.len()
    );
}
