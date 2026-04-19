//! Story 3 acceptance test: the `--json` flag is plumbed end-to-end.
//!
//! Justification (from stories/3.yml): proves the `--json` flag is
//! plumbed: running `agentic stories health --json` against the same
//! empty fixture emits stdout that `serde_json::from_str` parses into
//! a value with `stories` (array) and `summary` (object) keys, and
//! exits 0. Without this, JSON-mode consumers (future CI status
//! checks, dashboards reading the binary's output) would have to
//! discover via runtime failure whether the flag reached the
//! `Dashboard::render_json` call at all.
//!
//! The scaffold builds an empty fixture `stories/`, a fresh git repo,
//! and an empty tempdir store, invokes `agentic stories health --json
//! --store <tempdir>`, parses stdout as JSON, and asserts the two
//! top-level keys named in the justification.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn stories_health_json_flag_emits_parseable_object_with_stories_and_summary_keys() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    init_repo_and_commit_seed(repo_root);

    let store_tmp = TempDir::new().expect("store tempdir");

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
    let status = output.status;

    assert!(
        status.success(),
        "`agentic stories health --json --store <tempdir>` must exit 0; \
         got status={status:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // stdout must parse as JSON — a single top-level value, not a
    // concatenation of the table + the JSON dump.
    let parsed: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "stdout from `--json` must be parseable via `serde_json::from_str`; \
             parse error: {e}\nraw stdout:\n{stdout}"
        )
    });

    let obj = parsed.as_object().unwrap_or_else(|| {
        panic!("top-level JSON must be an object; got: {parsed}")
    });

    let stories = obj
        .get("stories")
        .unwrap_or_else(|| panic!("JSON must have a `stories` key; got: {parsed}"));
    assert!(
        stories.is_array(),
        "`stories` must be an array; got: {stories}"
    );

    let summary = obj
        .get("summary")
        .unwrap_or_else(|| panic!("JSON must have a `summary` key; got: {parsed}"));
    assert!(
        summary.is_object(),
        "`summary` must be an object; got: {summary}"
    );

    // Empty fixture — the stories array must be empty, confirming
    // the binary actually read the fixture stories directory and did
    // not accidentally read the repo's real stories/ instead.
    assert_eq!(
        stories.as_array().unwrap().len(),
        0,
        "fixture stories dir is empty — `stories[]` must be empty; \
         got: {stories}\nfull parsed JSON: {parsed}"
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
