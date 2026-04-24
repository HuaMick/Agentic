//! Story 15 acceptance test: the plan + record contract reaches the
//! operator through the compiled `agentic` binary.
//!
//! Justification (from stories/15.yml acceptance.tests[7]):
//! `agentic test-build plan <fixture-id> --json` against a fixture
//! `stories/` directory emits valid JSON parseable by `serde_json`
//! into the documented plan shape. After the user writes scaffolds
//! that match the plan (here the test setup writes canned red
//! scaffold bodies into the planned paths and stages them via
//! `git add`), `agentic test-build record <fixture-id>` exits 0,
//! writes the evidence row with the documented shape, and names each
//! recorded file in stdout. Without this, the library-level claims
//! are library-level claims only — the argv-to-subcommand wire could
//! drop `--json`, swap plan and record's semantics, or write
//! evidence under the wrong path and the operator would never notice.
//!
//! Red today is runtime-red: the binary's argv parser does not yet
//! know the `plan` and `record` sub-subcommands of `test-build`. The
//! current `Commands::TestBuild` variant takes a bare `id` with no
//! further subcommand surface, so `agentic test-build plan <id>`
//! fails clap parsing and exits 2 with a usage error. The test
//! asserts the happy-path exit-0 contract and therefore fails on
//! that argv gap until build-rust adds `plan` and `record` to the
//! CLI shim.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use tempfile::TempDir;

const STORY_ID: u32 = 99_015_008;

const FIXTURE_STORY_YAML: &str = r#"id: 99015008
title: "Fixture for story 15 CLI plan+record roundtrip"

outcome: |
  Fixture used to exercise the plan + record roundtrip through the
  compiled agentic binary.

status: proposed

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-roundtrip-crate/tests/scaffold_a.rs
      justification: |
        Proves the plan+record contract reaches the operator via
        the `agentic test-build` subcommand; the scaffold probes
        red via a natural compile error.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

const FIXTURE_WORKSPACE_CARGO: &str = r#"[workspace]
resolver = "2"
members = ["crates/fixture-roundtrip-crate"]
"#;

const FIXTURE_CRATE_CARGO: &str = r#"[package]
name = "fixture-roundtrip-crate"
version = "0.0.1"
edition = "2021"

[lib]
path = "src/lib.rs"
"#;

const FIXTURE_CRATE_LIB: &str = r#"// Intentionally empty.
"#;

/// Canned red scaffold body: `use`s a symbol the fixture crate does
/// not declare, so `cargo check` fails compile-red — the natural
/// red path.
const RED_SCAFFOLD_BODY: &str = r#"use fixture_roundtrip_crate::does_not_exist;

#[test]
fn scaffold_a() {
    assert_eq!(does_not_exist(), 0);
}
"#;

#[test]
fn test_build_plan_emits_json_and_record_writes_evidence_through_the_binary() {
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();

    fs::write(repo_root.join("Cargo.toml"), FIXTURE_WORKSPACE_CARGO).expect("ws cargo");
    let crate_root = repo_root.join("crates/fixture-roundtrip-crate");
    fs::create_dir_all(crate_root.join("src")).expect("crate src");
    fs::create_dir_all(crate_root.join("tests")).expect("crate tests");
    fs::write(crate_root.join("Cargo.toml"), FIXTURE_CRATE_CARGO).expect("crate cargo");
    fs::write(crate_root.join("src/lib.rs"), FIXTURE_CRATE_LIB).expect("crate lib");

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    init_repo_and_commit_seed(repo_root);

    // Phase 1: plan.
    let plan_assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("test-build")
        .arg("plan")
        .arg(STORY_ID.to_string())
        .arg("--json")
        .assert();
    let plan_output = plan_assert.get_output().clone();
    let plan_stdout = String::from_utf8_lossy(&plan_output.stdout).to_string();
    let plan_stderr = String::from_utf8_lossy(&plan_output.stderr).to_string();
    assert_eq!(
        plan_output.status.code(),
        Some(0),
        "`agentic test-build plan <id> --json` must exit 0 on a fixture story; \
         got status={:?}\nstdout:\n{plan_stdout}\nstderr:\n{plan_stderr}",
        plan_output.status
    );
    let plan_json: serde_json::Value =
        serde_json::from_str(plan_stdout.trim()).unwrap_or_else(|e| {
            panic!("plan stdout must be valid JSON; err={e}; stdout:\n{plan_stdout}")
        });
    let plan_arr = plan_json
        .as_array()
        .expect("plan stdout must be a JSON array");
    assert_eq!(
        plan_arr.len(),
        1,
        "plan must emit one entry per acceptance.tests[] entry; got {}",
        plan_arr.len()
    );
    let entry = plan_arr[0]
        .as_object()
        .expect("plan entry must be a JSON object");
    let mut keys: Vec<&str> = entry.keys().map(|s| s.as_str()).collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "expected_red_path",
            "file",
            "fixture_preconditions",
            "justification",
            "target_crate",
        ],
        "plan entry must carry exactly the five documented keys; got {keys:?}"
    );

    // Phase 2: the user writes the scaffold according to the plan
    // (simulated here by the test setup) and stages it.
    let planned_file = entry
        .get("file")
        .and_then(|v| v.as_str())
        .expect("plan entry must carry a string `file`");
    let scaffold_path = repo_root.join(planned_file);
    fs::create_dir_all(scaffold_path.parent().unwrap()).expect("tests dir for scaffold");
    fs::write(&scaffold_path, RED_SCAFFOLD_BODY).expect("write red scaffold");

    // Stage the scaffold — the UAT walkthrough calls `git add
    // crates/.../tests/<name>.rs` for each. No dirt outside scaffold.
    stage_all(repo_root);

    // Phase 3: record.
    let record_assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("test-build")
        .arg("record")
        .arg(STORY_ID.to_string())
        .assert();
    let record_output = record_assert.get_output().clone();
    let record_stdout = String::from_utf8_lossy(&record_output.stdout).to_string();
    let record_stderr = String::from_utf8_lossy(&record_output.stderr).to_string();
    assert_eq!(
        record_output.status.code(),
        Some(0),
        "`agentic test-build record <id>` on a clean tree with one red scaffold \
         must exit 0; got status={:?}\nstdout:\n{record_stdout}\nstderr:\n{record_stderr}",
        record_output.status
    );

    // Stdout must name the recorded scaffold path.
    assert!(
        record_stdout.contains(planned_file),
        "record stdout must name each recorded scaffold; expected to see {planned_file:?} in:\n{record_stdout}"
    );

    // Evidence file was written with the documented shape.
    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    let files: Vec<_> = fs::read_dir(&evidence_dir)
        .expect("evidence dir must exist after record")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with("-red.jsonl"))
        .collect();
    assert_eq!(
        files.len(),
        1,
        "record must write exactly one *-red.jsonl file; got {files:?}"
    );
    let body = fs::read_to_string(&files[0]).expect("read evidence");
    let row: serde_json::Value =
        serde_json::from_str(body.trim()).expect("evidence row must be valid JSON");
    let obj = row.as_object().expect("evidence row must be an object");
    let mut top_keys: Vec<&str> = obj.keys().map(|s| s.as_str()).collect();
    top_keys.sort();
    assert_eq!(
        top_keys,
        vec!["commit", "run_id", "story_id", "timestamp", "verdicts"],
        "evidence row top-level keys must match the documented shape; got {top_keys:?}"
    );
}

fn stage_all(repo_root: &Path) {
    let repo = git2::Repository::open(repo_root).expect("open repo");
    let mut index = repo.index().expect("index");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("stage all");
    index.write().expect("write index");
}

fn init_repo_and_commit_seed(root: &Path) {
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
