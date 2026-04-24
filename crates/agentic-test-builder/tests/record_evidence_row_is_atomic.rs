//! Story 15 acceptance test: evidence-write atomicity on success.
//! `record` on a clean tree whose scaffolds are all present, parse,
//! and probe red writes exactly one JSONL file under
//! `evidence/runs/<id>/<timestamp>-red.jsonl` with exactly the
//! top-level keys `run_id`, `story_id`, `commit`, `timestamp`,
//! `verdicts`. Each verdict carries `file`, `verdict: red`,
//! `red_path in {compile, runtime}`, and `diagnostic`.
//!
//! Justification (from stories/15.yml acceptance.tests[5]): without
//! this, story 7's evidence-atomicity contract degrades from
//! "committable atomic" to "best-effort record," the legacy failure
//! mode ADR-0005 was written to prevent.
//!
//! Red today is compile-red: `TestBuilder::record` is the new API
//! surface; `cargo check` fails on the unresolved item. Build-rust
//! adds the function and the atomic write, and this test runs green.
//!
//! Scaffold setup: a minimal fixture workspace with a real crate
//! whose scaffold file, when probed, fails at `cargo check` (the
//! scaffold `use`s a symbol the fixture crate does not declare —
//! a natural compile-red path).

use std::fs;
use std::path::{Path, PathBuf};

use agentic_test_builder::TestBuilder;
use tempfile::TempDir;

const STORY_ID: u32 = 99_015_006;

const FIXTURE_STORY_YAML: &str = r#"id: 99015006
title: "Fixture for story 15 record-evidence-row-is-atomic"

outcome: |
  Fixture used to prove record writes an atomic evidence row on
  success, with exactly the five documented top-level keys.

status: proposed

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-red-crate/tests/scaffold_a.rs
      justification: |
        Proves the red-probe path fires when the scaffold references
        a symbol the fixture crate does not declare; cargo check
        exits non-zero and the evidence row captures compile-red.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

const FIXTURE_WORKSPACE_CARGO: &str = r#"[workspace]
resolver = "2"
members = ["crates/fixture-red-crate"]
"#;

const FIXTURE_CRATE_CARGO: &str = r#"[package]
name = "fixture-red-crate"
version = "0.0.1"
edition = "2021"

[lib]
path = "src/lib.rs"
"#;

const FIXTURE_CRATE_LIB: &str = r#"// Deliberately empty: the scaffold references a symbol this crate
// does not declare, so `cargo check` fails compile-red.
"#;

/// Scaffold whose `use` names a symbol the fixture crate does not
/// declare — natural compile-red path.
const RED_SCAFFOLD_BODY: &str = r#"use fixture_red_crate::does_not_exist;

#[test]
fn scaffold_a() {
    assert_eq!(does_not_exist(), 0);
}
"#;

#[test]
fn record_writes_exactly_one_jsonl_with_five_top_level_keys_on_success() {
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();

    fs::write(repo_root.join("Cargo.toml"), FIXTURE_WORKSPACE_CARGO).expect("ws cargo");
    let crate_root = repo_root.join("crates/fixture-red-crate");
    fs::create_dir_all(crate_root.join("src")).expect("crate src");
    fs::create_dir_all(crate_root.join("tests")).expect("crate tests");
    fs::write(crate_root.join("Cargo.toml"), FIXTURE_CRATE_CARGO).expect("crate cargo");
    fs::write(crate_root.join("src/lib.rs"), FIXTURE_CRATE_LIB).expect("crate lib");

    let scaffold_path = crate_root.join("tests/scaffold_a.rs");
    fs::write(&scaffold_path, RED_SCAFFOLD_BODY).expect("write red scaffold");

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    init_repo_and_commit_seed(repo_root);

    // Act: record. Every scaffold is present, parses, and probes red
    // (compile-red) — record must succeed and write atomically.
    let builder = TestBuilder::new(repo_root);
    let _outcome = builder
        .record(STORY_ID)
        .expect("record must succeed on a clean tree with a red scaffold");

    // Assert: exactly one JSONL file under evidence/runs/<id>/.
    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    assert!(
        evidence_dir.exists(),
        "record must create evidence/runs/{STORY_ID}/"
    );
    let files: Vec<PathBuf> = fs::read_dir(&evidence_dir)
        .expect("read evidence dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()) == Some("jsonl")
                || p.to_string_lossy().ends_with(".jsonl")
        })
        .collect();
    assert_eq!(
        files.len(),
        1,
        "record must write exactly one *.jsonl file on success; got {files:?}"
    );
    let evidence_file = &files[0];
    let name = evidence_file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    assert!(
        name.ends_with("-red.jsonl"),
        "evidence file name must end with -red.jsonl; got {name}"
    );

    // Parse the JSONL row.
    let body = fs::read_to_string(evidence_file).expect("read evidence file");
    let row: serde_json::Value =
        serde_json::from_str(body.trim()).expect("evidence row must be valid JSON");

    // Exactly the documented top-level keys, nothing more, nothing
    // less.
    let obj = row.as_object().expect("evidence row must be a JSON object");
    let mut keys: Vec<&str> = obj.keys().map(|s| s.as_str()).collect();
    keys.sort();
    assert_eq!(
        keys,
        vec!["commit", "run_id", "story_id", "timestamp", "verdicts"],
        "evidence row top-level keys must be exactly {{run_id, story_id, commit, timestamp, verdicts}}; got {keys:?}"
    );

    // Per-verdict shape: file, verdict=red, red_path in {compile,
    // runtime}, diagnostic (non-empty).
    let verdicts = obj
        .get("verdicts")
        .and_then(|v| v.as_array())
        .expect("verdicts must be a JSON array");
    assert_eq!(
        verdicts.len(),
        1,
        "verdicts must have one entry per acceptance test; got {}",
        verdicts.len()
    );
    let verdict = verdicts[0]
        .as_object()
        .expect("verdict entry must be an object");
    assert_eq!(
        verdict.get("verdict").and_then(|v| v.as_str()),
        Some("red"),
        "per-verdict `verdict` must be \"red\" on success; got {:?}",
        verdict.get("verdict")
    );
    let red_path = verdict
        .get("red_path")
        .and_then(|v| v.as_str())
        .expect("per-verdict `red_path` must be a string");
    assert!(
        matches!(red_path, "compile" | "runtime"),
        "per-verdict `red_path` must be 'compile' or 'runtime'; got {red_path:?}"
    );
    let diagnostic = verdict
        .get("diagnostic")
        .and_then(|v| v.as_str())
        .expect("per-verdict `diagnostic` must be a string");
    assert!(
        !diagnostic.trim().is_empty(),
        "per-verdict `diagnostic` must be the first line of a real probe error, not empty"
    );
    let file_in_verdict = verdict
        .get("file")
        .and_then(|v| v.as_str())
        .expect("per-verdict `file` must be a string");
    assert!(
        file_in_verdict.ends_with("scaffold_a.rs"),
        "per-verdict `file` must name the scaffold path; got {file_in_verdict:?}"
    );
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
