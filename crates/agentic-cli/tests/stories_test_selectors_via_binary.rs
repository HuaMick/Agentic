//! Story 12 acceptance test: the `+<id>`, `<id>+`, `+<id>+` selector
//! argv forms reach the library runner through the compiled binary, and
//! each exits 0 when the stubbed executor reports Pass across the subtree.
//!
//! Justification (from stories/12.yml): proves the selector argv reaches
//! the runner through the binary — `agentic stories test +<id>`,
//! `agentic stories test <id>+`, and `agentic stories test +<id>+` each
//! cause the test executor (stubbed in the binary test harness) to be
//! invoked exactly for the subtree the matching library-level test pins
//! down, and each exits 0 when the stub reports Pass across the subtree.
//! Without this, the library-level claim is a library-level claim only
//! — the binary's argv-to-runner wire could drop the selector on the
//! floor.
//!
//! Test harness. Per the story's "Test file locations" guidance: the
//! binary's test harness swaps the real cargo-test executor for a stub
//! via the `AGENTIC_CI_TEST_EXECUTOR=stub-pass` environment variable —
//! when set, the binary uses an always-Pass executor (no failing tests,
//! empty failing_tests array). This keeps the scaffold O(1) in DAG size
//! and avoids invoking real cargo recursively.
//!
//! Red today is compile-red: the `agentic stories test` subcommand does
//! not yet exist in the CLI's clap tree, so clap rejects the argv and
//! the `--store` flag lookup never reaches `test_runs`.

use std::fs;
use std::path::Path;

use agentic_store::{Store, SurrealStore};
use assert_cmd::Command;
use tempfile::TempDir;

// DAG: ANC <- TARGET <- DESC; plus UNRELATED (outside subtree).
const ID_ANC: u32 = 81281;
const ID_TARGET: u32 = 81282;
const ID_DESC: u32 = 81283;
const ID_UNRELATED: u32 = 81284;

fn fixture_yaml(id: u32, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    let test_file = format!("crates/agentic-ci-record/tests/fixture_story_{id}.rs");
    format!(
        r#"id: {id}
title: "Fixture {id} for story-12 CLI selectors-via-binary scaffold"

outcome: |
  Fixture row for the CLI selectors-via-binary scaffold.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: {test_file}
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Drive `agentic stories test <selector>` with a stub executor.

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

fn setup() -> (TempDir, TempDir) {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    fs::write(
        stories_dir.join(format!("{ID_ANC}.yml")),
        fixture_yaml(ID_ANC, &[]),
    )
    .expect("write ANC");
    fs::write(
        stories_dir.join(format!("{ID_TARGET}.yml")),
        fixture_yaml(ID_TARGET, &[ID_ANC]),
    )
    .expect("write TARGET");
    fs::write(
        stories_dir.join(format!("{ID_DESC}.yml")),
        fixture_yaml(ID_DESC, &[ID_TARGET]),
    )
    .expect("write DESC");
    fs::write(
        stories_dir.join(format!("{ID_UNRELATED}.yml")),
        fixture_yaml(ID_UNRELATED, &[]),
    )
    .expect("write UNRELATED");

    init_repo_and_seed(repo_root);

    let store_tmp = TempDir::new().expect("store tempdir");
    (repo_tmp, store_tmp)
}

/// Drive `agentic stories test <selector>` with the stub executor env
/// var set, capturing exit code and the set of `test_runs` story ids.
fn run_with_stub(selector: &str, repo_root: &Path, store_path: &Path) -> (i32, Vec<u32>, String) {
    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .env("AGENTIC_CI_TEST_EXECUTOR", "stub-pass")
        .arg("stories")
        .arg("test")
        .arg(selector)
        .arg("--store")
        .arg(store_path)
        .assert();

    let output = assert.get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    // Read test_runs story ids back from the store the binary was
    // pointed at — proving the wire carried the selector all the way
    // through the runner to the row-writer.
    let ids: Vec<u32> = if code == 0 {
        let store = SurrealStore::open(store_path).expect("re-open configured store must succeed");
        let rows = store
            .query("test_runs", &|_| true)
            .expect("test_runs query must succeed");
        let mut ids: Vec<u32> = rows
            .iter()
            .filter_map(|row| {
                row.get("story_id")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32)
            })
            .collect();
        ids.sort();
        ids
    } else {
        vec![]
    };

    (code, ids, stderr)
}

#[test]
fn plus_id_selector_via_binary_covers_target_plus_ancestors_and_exits_0() {
    let (repo_tmp, store_tmp) = setup();
    let (code, ids, stderr) =
        run_with_stub(&format!("+{ID_TARGET}"), repo_tmp.path(), store_tmp.path());
    assert_eq!(
        code, 0,
        "`+{ID_TARGET}` with stub-pass executor must exit 0; stderr:\n{stderr}"
    );
    assert!(
        ids.contains(&ID_ANC) && ids.contains(&ID_TARGET),
        "+<id> row set must contain ancestor+target; got {ids:?}"
    );
    assert!(
        !ids.contains(&ID_DESC),
        "+<id> row set must EXCLUDE descendants; got {ids:?}"
    );
    assert!(
        !ids.contains(&ID_UNRELATED),
        "+<id> row set must EXCLUDE unrelated stories; got {ids:?}"
    );
}

#[test]
fn id_plus_selector_via_binary_covers_target_plus_descendants_and_exits_0() {
    let (repo_tmp, store_tmp) = setup();
    let (code, ids, stderr) =
        run_with_stub(&format!("{ID_TARGET}+"), repo_tmp.path(), store_tmp.path());
    assert_eq!(
        code, 0,
        "`{ID_TARGET}+` with stub-pass executor must exit 0; stderr:\n{stderr}"
    );
    assert!(
        ids.contains(&ID_TARGET) && ids.contains(&ID_DESC),
        "<id>+ row set must contain target+descendants; got {ids:?}"
    );
    assert!(
        !ids.contains(&ID_ANC),
        "<id>+ row set must EXCLUDE ancestors; got {ids:?}"
    );
    assert!(
        !ids.contains(&ID_UNRELATED),
        "<id>+ row set must EXCLUDE unrelated stories; got {ids:?}"
    );
}

#[test]
fn plus_id_plus_selector_via_binary_covers_full_subtree_and_exits_0() {
    let (repo_tmp, store_tmp) = setup();
    let (code, ids, stderr) =
        run_with_stub(&format!("+{ID_TARGET}+"), repo_tmp.path(), store_tmp.path());
    assert_eq!(
        code, 0,
        "`+{ID_TARGET}+` with stub-pass executor must exit 0; stderr:\n{stderr}"
    );
    for expected in [ID_ANC, ID_TARGET, ID_DESC] {
        assert!(
            ids.contains(&expected),
            "+<id>+ row set must contain story {expected}; got {ids:?}"
        );
    }
    assert!(
        !ids.contains(&ID_UNRELATED),
        "+<id>+ row set must EXCLUDE unrelated story {ID_UNRELATED}; got {ids:?}"
    );
}
