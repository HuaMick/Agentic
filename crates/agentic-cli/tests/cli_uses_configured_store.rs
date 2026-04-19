//! Story 8 acceptance test: the two subcommands share one store.
//!
//! Justification (from stories/8.yml): proves the two subcommands
//! share one store: after `agentic uat <id> --verdict pass --store
//! <tempdir>` promotes a fixture story, running `agentic stories
//! health --store <tempdir>` on the same tempdir reports that story
//! as `healthy` with a `Healthy at` cell populated from the
//! just-written `uat_signings` row. Without this end-to-end check,
//! the dashboard could read a different default store than the UAT
//! command wrote to, and the binary would look correct in isolation
//! but produce silently inconsistent state in any real session.
//!
//! The scaffold seeds the fixture, invokes `agentic uat ... --verdict
//! pass --store <X>`, then invokes `agentic stories health --store
//! <X> --json` on the SAME tempdir, parses the JSON, and asserts the
//! story's entry has `health == "healthy"` and `uat_commit` is the
//! full 40-char SHA of the fixture HEAD (matching what the UAT
//! invocation just wrote).

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

const STORY_ID: u32 = 88806;

const FIXTURE_YAML: &str = r#"id: 88806
title: "Fixture story for story 8 CLI shared-store end-to-end"

outcome: |
  A fixture that the CLI uat subcommand promotes and then the CLI
  dashboard reads back as healthy through the same store.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/cli_uses_configured_store.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the agentic binary against this file.
  uat: |
    Run uat --verdict pass --store X, then stories health --store X
    --json; assert the dashboard sees the write.

guidance: |
  Fixture authored inline for the story-8 shared-store scaffold. Not a
  real story.

depends_on: []
"#;

#[test]
fn uat_write_then_dashboard_read_on_same_store_shows_story_as_healthy_with_full_sha() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    let head_sha = init_repo_and_commit_seed(repo_root);

    let store_tmp = TempDir::new().expect("store tempdir");
    let store_path = store_tmp.path().to_path_buf();

    // Step 1: uat --verdict pass --store <X>. Must succeed so the
    // signing row exists for the dashboard to read.
    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("uat")
        .arg(STORY_ID.to_string())
        .arg("--verdict")
        .arg("pass")
        .arg("--store")
        .arg(&store_path)
        .assert();
    let uat_output = assert.get_output().clone();
    assert_eq!(
        uat_output.status.code(),
        Some(0),
        "UAT-pass step must exit 0 so the dashboard step has something to read; \
         got status={:?}\nstdout:\n{}\nstderr:\n{}",
        uat_output.status,
        String::from_utf8_lossy(&uat_output.stdout),
        String::from_utf8_lossy(&uat_output.stderr),
    );

    // Step 2: stories health --store <X> --json. Must see the story
    // as healthy with uat_commit == head_sha.
    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("stories")
        .arg("health")
        .arg("--json")
        .arg("--store")
        .arg(&store_path)
        .assert();
    let dash_output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&dash_output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&dash_output.stderr).to_string();

    assert_eq!(
        dash_output.status.code(),
        Some(0),
        "dashboard step must exit 0; got status={:?}\nstdout:\n{stdout}\n\
         stderr:\n{stderr}",
        dash_output.status
    );

    let parsed: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "dashboard `--json` stdout must parse as JSON: {e}\n\
             raw stdout:\n{stdout}"
        )
    });
    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("parsed JSON must have `stories[]`; got: {parsed}"));

    let entry = stories
        .iter()
        .find(|s| s.get("id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64))
        .unwrap_or_else(|| {
            panic!(
                "stories[] must contain an entry for id {STORY_ID} (the one \
                 the UAT step just promoted); got: {parsed}"
            )
        });

    let health = entry
        .get("health")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            panic!("entry for {STORY_ID} must have string `health`; got: {entry}")
        });
    assert_eq!(
        health, "healthy",
        "shared-store end-to-end: dashboard must see story {STORY_ID} as \
         `healthy` after UAT-pass on the same tempdir store; got health={health:?}\n\
         full entry: {entry}"
    );

    let uat_commit = entry
        .get("uat_commit")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            panic!(
                "entry for {STORY_ID} must have string `uat_commit` after \
                 the UAT-pass step; got: {entry}"
            )
        });
    assert_eq!(
        uat_commit, head_sha,
        "dashboard `uat_commit` must equal the fixture HEAD SHA the UAT \
         step just signed against; got {uat_commit:?}, expected {head_sha:?}"
    );
    assert_eq!(
        uat_commit.len(),
        40,
        "dashboard JSON mode must emit the FULL 40-char SHA (story 3 \
         contract); got {uat_commit:?}"
    );
}

fn init_repo_and_commit_seed(root: &Path) -> String {
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
    let commit_oid = repo
        .commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
    commit_oid.to_string()
}
