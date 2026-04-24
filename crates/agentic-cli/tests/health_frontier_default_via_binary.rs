//! Story 10 acceptance test: `agentic stories health` renders the
//! frontier by default through the compiled binary.
//!
//! Justification (from stories/10.yml): proves the frontier default
//! reaches the operator through the binary — running `agentic stories
//! health` against a fixture `stories/` directory with a mixed-status
//! DAG and an empty tempdir store exits 0 and writes only the
//! frontier rows to stdout, in the documented sort order (`lvl`
//! ascending, story id ascending as tiebreaker). Without this, the
//! library-level claim is a library-level claim only — the argv-to-
//! library wire could quietly pass `all: true` (or its future
//! equivalent) and the operator would never notice.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

const ID_ROOT_DEEP: u32 = 92501; // proposed, no deps; unblocks a 3-deep path
const ID_DEEP_MID1: u32 = 92502; // under_construction, depends_on=[ROOT_DEEP]
const ID_DEEP_MID2: u32 = 92503; // under_construction, depends_on=[DEEP_MID1]
const ID_DEEP_LEAF: u32 = 92504; // under_construction, depends_on=[DEEP_MID2]

const ID_ROOT_SHALLOW: u32 = 92510; // proposed, no deps; unblocks nothing

fn fixture_yaml(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for frontier-default-via-binary"

outcome: |
  Fixture row for the frontier-default-via-binary scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/health_frontier_default_via_binary.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Run the binary; assert frontier rows and sort order.

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

#[test]
fn stories_health_default_via_binary_emits_only_frontier_rows_sorted_by_lvl_then_id() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // Frontier root at lvl=-3 (unblocks MID1 -> MID2 -> LEAF).
    fs::write(
        stories_dir.join(format!("{ID_ROOT_DEEP}.yml")),
        fixture_yaml(ID_ROOT_DEEP, "proposed", &[]),
    )
    .expect("write ROOT_DEEP");
    fs::write(
        stories_dir.join(format!("{ID_DEEP_MID1}.yml")),
        fixture_yaml(ID_DEEP_MID1, "under_construction", &[ID_ROOT_DEEP]),
    )
    .expect("write DEEP_MID1");
    fs::write(
        stories_dir.join(format!("{ID_DEEP_MID2}.yml")),
        fixture_yaml(ID_DEEP_MID2, "under_construction", &[ID_DEEP_MID1]),
    )
    .expect("write DEEP_MID2");
    fs::write(
        stories_dir.join(format!("{ID_DEEP_LEAF}.yml")),
        fixture_yaml(ID_DEEP_LEAF, "under_construction", &[ID_DEEP_MID2]),
    )
    .expect("write DEEP_LEAF");

    // Shallow frontier root (lvl=0, leaf, blocks nothing).
    fs::write(
        stories_dir.join(format!("{ID_ROOT_SHALLOW}.yml")),
        fixture_yaml(ID_ROOT_SHALLOW, "proposed", &[]),
    )
    .expect("write ROOT_SHALLOW");

    init_repo_and_seed(repo_root);

    let store_tmp = TempDir::new().expect("store tempdir");

    // Invoke with --json so we can assert row set + order structurally.
    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("stories")
        .arg("health")
        .arg("--json")
        .arg("--store")
        .arg(store_tmp.path())
        .assert();

    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert!(
        output.status.success(),
        "`agentic stories health` must exit 0; got status={:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status
    );

    // stdout contains the JSON payload. Find the last valid JSON
    // object by scanning from the last `{` — some prefix logging
    // (`store: ...` on stderr) is on stderr and must not appear on
    // stdout, but we parse stdout directly.
    let parsed: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("stdout must parse as JSON: {e}; stdout:\n{stdout}"));

    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));

    let ids_ordered: Vec<u64> = stories
        .iter()
        .filter_map(|s| s.get("id").and_then(|v| v.as_u64()))
        .collect();

    // Frontier set: {ROOT_DEEP, ROOT_SHALLOW}. Intermediate deep
    // nodes are hidden (not-healthy ancestors). Healthy hidden.
    assert!(
        ids_ordered.contains(&(ID_ROOT_DEEP as u64)),
        "frontier must include ROOT_DEEP (id {ID_ROOT_DEEP}); got ids: {ids_ordered:?}"
    );
    assert!(
        ids_ordered.contains(&(ID_ROOT_SHALLOW as u64)),
        "frontier must include ROOT_SHALLOW (id {ID_ROOT_SHALLOW}); got ids: {ids_ordered:?}"
    );
    for hidden in [ID_DEEP_MID1, ID_DEEP_MID2, ID_DEEP_LEAF] {
        assert!(
            !ids_ordered.contains(&(hidden as u64)),
            "frontier must NOT include descendant-of-not-healthy id {hidden}; got ids: {ids_ordered:?}"
        );
    }

    // Sort order: primary `lvl` ascending (most-negative first),
    // secondary `id` ascending. ROOT_DEEP has lvl=-3 (3 hops to
    // DEEP_LEAF), ROOT_SHALLOW has lvl=0. So ROOT_DEEP must come
    // FIRST.
    let pos = |id: u32| -> usize {
        ids_ordered
            .iter()
            .position(|v| *v == id as u64)
            .unwrap_or_else(|| panic!("id {id} not found in {ids_ordered:?}"))
    };
    assert!(
        pos(ID_ROOT_DEEP) < pos(ID_ROOT_SHALLOW),
        "sort order: ROOT_DEEP (lvl=-3, id {ID_ROOT_DEEP}) must precede ROOT_SHALLOW \
         (lvl=0, id {ID_ROOT_SHALLOW}) under primary-lvl-ascending sort; got {ids_ordered:?}"
    );

    // view field documents the projection.
    let view = parsed.get("view").and_then(|v| v.as_str());
    assert_eq!(
        view,
        Some("frontier"),
        "default invocation must emit view=\"frontier\"; got {view:?}"
    );
}
