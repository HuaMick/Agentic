//! Story 11 acceptance test: ancestor refusal reaches the operator
//! through the binary with exit code 2.
//!
//! Justification (from stories/11.yml):
//! Proves the typed refusal reaches the operator through the binary:
//! `agentic uat <id> --verdict pass` where `<id>`'s ancestors are not
//! all healthy exits with code 2 (could-not-verdict, matching the
//! fail-closed-on-dirty-tree pattern's mapping), writes zero rows to
//! `uat_signings`, leaves the fixture YAML unchanged, and emits
//! stderr naming the offending ancestor id and the refusal reason.
//! Without this, a CLI wrapper that mistranslated
//! `AncestorNotHealthy` to exit 1 would turn "retry after promoting
//! the ancestor" into "the UAT failed," which is exactly the CI
//! distinction the exit-code contract exists to preserve.
//!
//! Red today is runtime-red: the binary compiles and accepts the
//! argv, but until the library's ancestor gate is wired and the CLI
//! maps `AncestorNotHealthy` to exit 2 the process will exit 0
//! (silent promotion) or 1 (confused as a Fail) — both of which this
//! test rejects.

use std::fs;
use std::path::Path;

use agentic_store::{Store, SurrealStore};
use assert_cmd::Command;
use tempfile::TempDir;

const LEAF_ID: u32 = 88811;
const ANCESTOR_ID: u32 = 88812;

const LEAF_YAML: &str = r#"id: 88811
title: "Fixture leaf for story-11 CLI ancestor-refusal contract"

outcome: |
  A fixture leaf the CLI must refuse to Pass because its ancestor is
  not healthy.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/uat_ancestor_refusal_exits_2.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the agentic binary against this file.
  uat: |
    Drive the binary; expect exit 2.

guidance: |
  Fixture authored inline for the story-11 CLI ancestor-refusal
  scaffold. Not a real story.

depends_on:
  - 88812
"#;

const ANCESTOR_YAML: &str = r#"id: 88812
title: "Fixture ancestor whose on-disk status is under_construction"

outcome: |
  Ancestor fixture whose status is deliberately not healthy, forcing
  the CLI to refuse the leaf's Pass.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/uat_ancestor_refusal_exits_2.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the unhealthy ancestor.

depends_on: []
"#;

#[test]
fn agentic_uat_on_unhealthy_ancestor_exits_two_writes_no_rows_and_names_offender_on_stderr() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let leaf_path = stories_dir.join(format!("{LEAF_ID}.yml"));
    let ancestor_path = stories_dir.join(format!("{ANCESTOR_ID}.yml"));
    fs::write(&leaf_path, LEAF_YAML).expect("write leaf fixture");
    fs::write(&ancestor_path, ANCESTOR_YAML).expect("write ancestor fixture");

    init_repo_and_commit_seed(repo_root);
    let leaf_bytes_before = fs::read(&leaf_path).expect("read leaf before run");

    let store_tmp = TempDir::new().expect("store tempdir");
    let store_path = store_tmp.path().to_path_buf();

    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("uat")
        .arg(LEAF_ID.to_string())
        .arg("--verdict")
        .arg("pass")
        .arg("--store")
        .arg(&store_path)
        .assert();

    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let status = output.status;

    // Exit code 2 EXACTLY: the could-not-verdict contract. 0 would be
    // silent promotion (catastrophic); 1 would be confused with a
    // real Fail verdict.
    assert_eq!(
        status.code(),
        Some(2),
        "ancestor-refusal must surface as exit 2 (could-not-verdict), \
         NOT 0 (pass) or 1 (fail); got status={status:?}\n\
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // stderr names the offending ancestor id so the operator knows
    // exactly which ancestor to fix.
    let ancestor_id_str = ANCESTOR_ID.to_string();
    assert!(
        stderr.contains(&ancestor_id_str),
        "stderr must name the offending ancestor id {ancestor_id_str}; \
         got stderr:\n{stderr}"
    );
    // stderr emits a refusal-reason cue so the operator can
    // distinguish "ancestor YAML not healthy" from "no signing row."
    let lower = stderr.to_lowercase();
    assert!(
        lower.contains("ancestor"),
        "stderr must name the refusal reason as ancestor-related; \
         got stderr:\n{stderr}"
    );

    // Zero rows written to the configured store.
    let store = SurrealStore::open(&store_path)
        .expect("re-opening the configured SurrealStore must succeed");
    let rows = store
        .query("uat_signings", &|_| true)
        .expect("uat_signings query must succeed");
    assert!(
        rows.is_empty(),
        "ancestor-refusal must write zero uat_signings rows; got {} rows: {rows:?}",
        rows.len()
    );

    // Fixture leaf YAML is byte-for-byte unchanged.
    let leaf_bytes_after = fs::read(&leaf_path).expect("read leaf after run");
    assert_eq!(
        leaf_bytes_after, leaf_bytes_before,
        "ancestor-refusal must not touch the fixture YAML; file changed on disk"
    );
}

fn init_repo_and_commit_seed(root: &Path) -> String {
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
    let commit_oid = repo
        .commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
    commit_oid.to_string()
}
