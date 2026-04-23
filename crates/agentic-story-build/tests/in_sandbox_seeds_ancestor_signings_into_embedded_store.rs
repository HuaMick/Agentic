//! Story 20 acceptance test: the container-side seeding step runs
//! BEFORE claude spawns. If the mounted ancestor snapshot would not
//! satisfy story 11's gate, the sandbox exits with
//! `StoryBuildError::AncestorSnapshotInsufficient` naming the
//! ancestor and writes a `runs` row with `outcome: crashed`.
//!
//! Justification (from stories/20.yml acceptance.tests[3]):
//!   Proves the container-side seeding step runs before the
//!   inner loop: given a mounted ancestor snapshot at
//!   `/work/snapshot.json`, a mounted story at
//!   `/work/story.yml`, and the `--in-sandbox` flag,
//!   `StoryBuild::run_in_sandbox` initialises an embedded
//!   `SurrealStore`, calls `Store::restore(snapshot)`, and
//!   BEFORE spawning claude verifies
//!   `Uat::ancestor_gate(story_id)` returns `Satisfied`. If
//!   the snapshot would NOT satisfy the gate, the sandbox
//!   exits with `StoryBuildError::AncestorSnapshotInsufficient`
//!   naming the ancestor whose signing is missing, writes a
//!   `runs` row with `outcome: crashed` and a clear error
//!   phrasing the seeding failure, and DOES NOT spawn
//!   claude.
//!
//! Red today: compile-red via the missing `agentic_story_build`
//! public surface (`StoryBuild::run_in_sandbox`, `InSandboxConfig`,
//! `StoryBuildError::AncestorSnapshotInsufficient`).

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_story_build::{InSandboxConfig, StoryBuild, StoryBuildError};
use serde_json::json;
use tempfile::TempDir;

#[tokio::test(flavor = "current_thread")]
async fn seeding_refuses_and_writes_crashed_row_when_snapshot_misses_an_ancestor() {
    // A temporary work root simulating the container's view of the
    // mounted repo + snapshot + story paths.
    let work_tmp = TempDir::new().expect("work tempdir");
    let work = work_tmp.path();

    let story_yaml_path = work.join("story.yml");
    // The fixture story declares depends_on: [2042]; the snapshot
    // below deliberately does NOT include a signing for 2042, so
    // the gate must refuse.
    fs::write(
        &story_yaml_path,
        "id: 3077\n\
         title: fixture-needing-ancestor-2042\n\
         outcome: ignored\n\
         status: proposed\n\
         patterns: []\n\
         acceptance:\n  tests: []\n  uat: ignored\n\
         depends_on: [2042]\n",
    )
    .expect("write fixture story");

    let snapshot_path = work.join("snapshot.json");
    // Snapshot carries a signing for an UNRELATED story 9999, not
    // for the ancestor 2042 that the target depends on.
    fs::write(
        &snapshot_path,
        r#"{
  "schema_version": 1,
  "signings": [
    {
      "story_id": 9999,
      "verdict": "pass",
      "signer": "alice@example.com",
      "commit": "0000000000000000000000000000000000000009"
    }
  ]
}
"#,
    )
    .expect("write insufficient snapshot");

    let runs_root_tmp = TempDir::new().expect("runs root tempdir");
    let runs_root = runs_root_tmp.path().to_path_buf();

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let cfg = InSandboxConfig {
        story_id: 3077,
        run_id: "run-seed-fail".to_string(),
        signer: "sandbox:claude-sonnet-4-6@run-seed-fail".to_string(),
        story_yaml_path: story_yaml_path.clone(),
        snapshot_path: snapshot_path.clone(),
        runs_root: runs_root.clone(),
        start_sha: "a09aaed609cdab88ca8dcb0a8be5c7928befbabc".to_string(),
        max_inner_loop_iterations: 3,
        model: "claude-sonnet-4-6".to_string(),
    };

    let err = StoryBuild::run_in_sandbox(cfg, Arc::clone(&store))
        .await
        .expect_err(
            "run_in_sandbox must refuse typed when the snapshot does not satisfy the gate",
        );

    // The typed refusal names the missing ancestor so the operator
    // can chase the right signing, not a generic "gate refused" line.
    match &err {
        StoryBuildError::AncestorSnapshotInsufficient { missing_ancestor } => {
            assert_eq!(
                *missing_ancestor, 2042,
                "AncestorSnapshotInsufficient must name the ancestor whose signing is missing; \
                 got {missing_ancestor}, expected 2042"
            );
        }
        other => panic!(
            "run_in_sandbox must return StoryBuildError::AncestorSnapshotInsufficient; \
             got {other:?}"
        ),
    }

    // Despite refusing, the container writes exactly one crashed
    // `runs` row naming the seeding failure — the evidence surface
    // is non-empty even when the inner loop never spawned.
    let rows = store
        .query("runs", &|doc| doc["run_id"] == json!("run-seed-fail"))
        .expect("query runs");
    assert_eq!(
        rows.len(),
        1,
        "exactly one `runs` row for the failed seeding attempt; got {rows:?}"
    );
    let row = &rows[0];
    assert_eq!(
        row["outcome"],
        json!("crashed"),
        "seeding-refused runs row must carry outcome=\"crashed\"; got {}",
        row["outcome"]
    );
    let err_text = row["error"]
        .as_str()
        .or_else(|| row["iterations"].as_array().and_then(|a| a.last()).and_then(|i| i["error"].as_str()))
        .unwrap_or("");
    assert!(
        err_text.to_ascii_lowercase().contains("ancestor")
            || err_text.to_ascii_lowercase().contains("snapshot"),
        "seeding-refused row must carry an error field phrasing the seeding failure \
         (naming ancestor or snapshot); got {err_text:?}"
    );

    // And zero `uat_signings` rows — seeding refused before the
    // inner loop could produce a signing, and the refusal must
    // never forge one.
    let signings = store
        .query("uat_signings", &|_| true)
        .expect("query signings");
    assert!(
        signings.is_empty(),
        "no uat_signings may exist when seeding refused; got {signings:?}"
    );
}
