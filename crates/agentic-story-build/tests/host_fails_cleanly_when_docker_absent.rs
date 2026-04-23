//! Story 20 acceptance test: on a host whose `$PATH` does not contain
//! a `docker` binary (simulated via a stubbed resolver pointing at a
//! non-existent path), `StoryBuild::run` returns
//! `StoryBuildError::DockerUnavailable` without side-effects — no
//! directories under the runs root, no rows in the store, no changes
//! to the working tree.
//!
//! Justification (from stories/20.yml acceptance.tests[1]):
//!   Proves the host refuses before side-effects when docker
//!   is missing: given a `$PATH` that does not contain a
//!   `docker` binary (simulated by pointing `DOCKER_BINARY`
//!   at a non-existent path or constructing `StoryBuild`
//!   with a stubbed resolver), `StoryBuild::run` returns
//!   `StoryBuildError::DockerUnavailable` naming the
//!   looked-up binary, exits the CLI with code 2
//!   (could-not-verdict), and creates ZERO directories
//!   under the runs root, writes ZERO rows to the host's
//!   Store, and leaves `git status --porcelain`
//!   byte-identical to its pre-invocation output.
//!
//! Red today: compile-red via the missing `agentic_story_build`
//! public surface (`StoryBuild`, `BuildConfig`, `StoryBuildError`).

use std::path::PathBuf;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_story_build::{BuildConfig, StoryBuild, StoryBuildError};
use tempfile::TempDir;

#[test]
fn run_refuses_typed_when_docker_binary_is_unresolvable() {
    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let runs_root = runs_root_tmp.path().to_path_buf();

    let missing_docker = PathBuf::from("/definitely/not/on/path/docker-does-not-exist");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let cfg = BuildConfig {
        story_id: 20,
        run_id: "run-docker-missing".to_string(),
        model: "claude-sonnet-4-6".to_string(),
        image_tag: "agentic-sandbox:deadbeef".to_string(),
        docker_binary: missing_docker.clone(),
        runs_root: runs_root.clone(),
        story_yaml_path: PathBuf::from("/tmp/fixture/stories/20.yml"),
        snapshot_path: PathBuf::from("/tmp/fixture/snapshot.json"),
        credentials_path: PathBuf::from("/tmp/fixture/.credentials.json"),
        max_inner_loop_iterations: 3,
        start_sha: "a09aaed609cdab88ca8dcb0a8be5c7928befbabc".to_string(),
    };

    let build =
        StoryBuild::from_config(cfg).expect("StoryBuild::from_config must succeed; the refusal is a run-time check, not a construction-time one");

    let err = build
        .run(Arc::clone(&store))
        .expect_err("run must return Err when docker binary is unresolvable");

    // Typed refusal, named binary. The test pins the `DockerUnavailable`
    // variant carrying the looked-up binary so a later refactor cannot
    // quietly lose the operator-facing detail.
    match &err {
        StoryBuildError::DockerUnavailable { binary } => {
            assert_eq!(
                binary, &missing_docker,
                "DockerUnavailable must name the binary path the resolver looked up; \
                 got {binary:?}, expected {missing_docker:?}"
            );
        }
        other => panic!(
            "run must return StoryBuildError::DockerUnavailable when docker is absent; \
             got {other:?}"
        ),
    }

    // Zero directories under the runs root. The refusal fires before
    // the runs_root/<run-id>/ path is created.
    let entries: Vec<_> = std::fs::read_dir(&runs_root)
        .expect("runs root must exist")
        .flatten()
        .collect();
    assert!(
        entries.is_empty(),
        "runs root must contain zero entries after a DockerUnavailable refusal; got {:?}",
        entries.iter().map(|e| e.path()).collect::<Vec<_>>()
    );

    // Zero rows in the host store. `runs` and `uat_signings` are the
    // two tables this crate could conceivably write to.
    for table in ["runs", "uat_signings"] {
        let rows = store.query(table, &|_| true).expect("store query");
        assert!(
            rows.is_empty(),
            "host store table {table:?} must be empty after a DockerUnavailable refusal; \
             got {rows:?}"
        );
    }
}
