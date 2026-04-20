//! Story 10 acceptance test: the `+<id>`, `<id>+`, `+<id>+` selector
//! argv forms reach the library through the compiled binary, and
//! an unknown id in any form exits non-zero naming the id.
//!
//! Justification (from stories/10.yml): proves the selector argv
//! reaches the library through the binary — `agentic stories health
//! +<id>`, `agentic stories health <id>+`, and `agentic stories
//! health +<id>+` each emit the row set the matching library-level
//! selector test pins down, and each exits 0. `agentic stories
//! health +99999` (unknown id) exits non-zero with a named error.
//! Without this, the dbt-style grammar is a Rust API only and the
//! CLI surface the epic commits to is unshipped.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

const ID_ANC: u32 = 92701;
const ID_TARGET: u32 = 92702;
const ID_DESC: u32 = 92703;
const ID_MISSING: u32 = 99999;

fn fixture_yaml(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for selectors-via-binary"

outcome: |
  Fixture row for the selectors-via-binary scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/health_selectors_via_binary.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Run the binary with each selector form; assert exit 0 + row set.

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

    // ANC -> TARGET -> DESC.
    fs::write(
        stories_dir.join(format!("{ID_ANC}.yml")),
        fixture_yaml(ID_ANC, "under_construction", &[]),
    )
    .expect("write ANC");
    fs::write(
        stories_dir.join(format!("{ID_TARGET}.yml")),
        fixture_yaml(ID_TARGET, "under_construction", &[ID_ANC]),
    )
    .expect("write TARGET depends_on=[ANC]");
    fs::write(
        stories_dir.join(format!("{ID_DESC}.yml")),
        fixture_yaml(ID_DESC, "under_construction", &[ID_TARGET]),
    )
    .expect("write DESC depends_on=[TARGET]");

    init_repo_and_seed(repo_root);

    let store_tmp = TempDir::new().expect("store tempdir");
    (repo_tmp, store_tmp)
}

fn run(selector: &str, repo_root: &Path, store_path: &Path) -> (bool, Vec<u64>, String, i32) {
    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("stories")
        .arg("health")
        .arg(selector)
        .arg("--store")
        .arg(store_path)
        .assert();

    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let success = output.status.success();
    let code = output.status.code().unwrap_or(-1);

    // Only parse JSON from stdout on success; on unknown-id path
    // stdout will not be JSON.
    let ids = if success {
        let parsed: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
            panic!(
                "stdout must parse as JSON for selector `{selector}`: {e}; stdout:\n{stdout}\nstderr:\n{stderr}"
            )
        });
        parsed
            .get("stories")
            .and_then(|v| v.as_array())
            .unwrap_or_else(|| {
                panic!("top-level `stories` must be an array for selector `{selector}`; got: {parsed}")
            })
            .iter()
            .filter_map(|s| s.get("id").and_then(|v| v.as_u64()))
            .collect()
    } else {
        vec![]
    };

    (success, ids, stderr, code)
}

#[test]
fn plus_id_selector_via_binary_emits_target_plus_ancestors_and_exits_0() {
    let (repo_tmp, store_tmp) = setup();
    let (ok, ids, stderr, _code) = run(
        &format!("+{ID_TARGET}"),
        repo_tmp.path(),
        store_tmp.path(),
    );
    assert!(ok, "`+{ID_TARGET}` must exit 0; stderr:\n{stderr}");
    assert!(
        ids.contains(&(ID_TARGET as u64)),
        "+<id> must include target; got {ids:?}"
    );
    assert!(
        ids.contains(&(ID_ANC as u64)),
        "+<id> must include the ancestor ANC (id {ID_ANC}); got {ids:?}"
    );
    assert!(
        !ids.contains(&(ID_DESC as u64)),
        "+<id> must EXCLUDE descendants; got {ids:?}"
    );
}

#[test]
fn id_plus_selector_via_binary_emits_target_plus_descendants_and_exits_0() {
    let (repo_tmp, store_tmp) = setup();
    let (ok, ids, stderr, _code) = run(
        &format!("{ID_TARGET}+"),
        repo_tmp.path(),
        store_tmp.path(),
    );
    assert!(ok, "`{ID_TARGET}+` must exit 0; stderr:\n{stderr}");
    assert!(
        ids.contains(&(ID_TARGET as u64)),
        "<id>+ must include target; got {ids:?}"
    );
    assert!(
        ids.contains(&(ID_DESC as u64)),
        "<id>+ must include the descendant DESC (id {ID_DESC}); got {ids:?}"
    );
    assert!(
        !ids.contains(&(ID_ANC as u64)),
        "<id>+ must EXCLUDE ancestors; got {ids:?}"
    );
}

#[test]
fn plus_id_plus_selector_via_binary_emits_full_subtree_and_exits_0() {
    let (repo_tmp, store_tmp) = setup();
    let (ok, ids, stderr, _code) = run(
        &format!("+{ID_TARGET}+"),
        repo_tmp.path(),
        store_tmp.path(),
    );
    assert!(ok, "`+{ID_TARGET}+` must exit 0; stderr:\n{stderr}");
    for expected in [ID_ANC, ID_TARGET, ID_DESC] {
        assert!(
            ids.contains(&(expected as u64)),
            "+<id>+ must include id {expected}; got {ids:?}"
        );
    }
}

#[test]
fn plus_unknown_id_selector_via_binary_exits_nonzero_naming_the_missing_id() {
    let (repo_tmp, store_tmp) = setup();
    let (ok, _ids, stderr, code) = run(
        &format!("+{ID_MISSING}"),
        repo_tmp.path(),
        store_tmp.path(),
    );
    assert!(
        !ok,
        "`+{ID_MISSING}` (unknown id) must exit non-zero; got code={code}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains(&ID_MISSING.to_string()),
        "stderr must name the missing id {ID_MISSING}; got stderr:\n{stderr}"
    );
}
