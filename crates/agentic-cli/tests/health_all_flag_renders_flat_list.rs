//! Story 10 acceptance test: `agentic stories health --all` emits
//! the flat list through the compiled binary.
//!
//! Justification (from stories/10.yml): proves the `--all` flag is
//! plumbed through the binary — running `agentic stories health
//! --all` against the same fixture emits rows for every story
//! (healthy, frontier, and in-between alike), matching the row set
//! the story-3 dashboard used to emit by default. Without this the
//! escape hatch exists in the library but not at the hand operators
//! and scripts type on.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

const ID_HEALTHY: u32 = 92601; // healthy (needs evidence; none seeded — will land as error-class).
const ID_PROPOSED: u32 = 92602;
const ID_UNDER_CONSTRUCTION_DESC: u32 = 92603; // depends_on=[PROPOSED] so would be hidden by frontier.
const ID_STANDALONE_UC: u32 = 92604;

fn fixture_yaml(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for all-flag-via-binary"

outcome: |
  Fixture row for the all-flag-via-binary scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/health_all_flag_renders_flat_list.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Run the binary with --all; assert every story appears.

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
fn stories_health_all_via_binary_emits_one_row_per_story_including_frontier_descendants_and_error_class(
) {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    fs::write(
        stories_dir.join(format!("{ID_HEALTHY}.yml")),
        fixture_yaml(ID_HEALTHY, "healthy", &[]),
    )
    .expect("write HEALTHY");
    fs::write(
        stories_dir.join(format!("{ID_PROPOSED}.yml")),
        fixture_yaml(ID_PROPOSED, "proposed", &[]),
    )
    .expect("write PROPOSED");
    fs::write(
        stories_dir.join(format!("{ID_UNDER_CONSTRUCTION_DESC}.yml")),
        fixture_yaml(
            ID_UNDER_CONSTRUCTION_DESC,
            "under_construction",
            &[ID_PROPOSED],
        ),
    )
    .expect("write UC_DESC");
    fs::write(
        stories_dir.join(format!("{ID_STANDALONE_UC}.yml")),
        fixture_yaml(ID_STANDALONE_UC, "under_construction", &[]),
    )
    .expect("write STANDALONE_UC");

    init_repo_and_seed(repo_root);

    let store_tmp = TempDir::new().expect("store tempdir");

    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("stories")
        .arg("health")
        .arg("--all")
        .arg("--json")
        .arg("--store")
        .arg(store_tmp.path())
        .assert();

    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert_eq!(
        output.status.code(),
        Some(2),
        "story 3 amendment cascade: `agentic stories health --all` must exit 2 when corpus has error/unhealthy rows (this fixture has a `healthy`-claimed-but-unsigned story that classifies as error-class); rendering assertions unchanged. got status={:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status
    );

    let parsed: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("stdout must parse as JSON: {e}; stdout:\n{stdout}"));

    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));

    let ids: Vec<u64> = stories
        .iter()
        .filter_map(|s| s.get("id").and_then(|v| v.as_u64()))
        .collect();

    for expected in [
        ID_HEALTHY,
        ID_PROPOSED,
        ID_UNDER_CONSTRUCTION_DESC,
        ID_STANDALONE_UC,
    ] {
        assert!(
            ids.contains(&(expected as u64)),
            "--all must include every fixture story (id {expected} missing); got {ids:?}"
        );
    }

    assert_eq!(
        stories.len(),
        4,
        "--all must emit exactly one row per story (4 fixtures); got {}",
        stories.len()
    );

    let view = parsed.get("view").and_then(|v| v.as_str());
    assert_eq!(
        view,
        Some("all"),
        "--all invocation must emit view=\"all\"; got {view:?}"
    );
}
