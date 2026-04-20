//! Story 7 acceptance test: existing tests are bytes-immutable.
//!
//! Justification (from stories/7.yml): Proves preservation: given a story
//! where one acceptance.tests[] entry points at a file that already
//! exists with non-empty content, `TestBuilder::run` does not open the
//! existing file, does not edit it, and does not overwrite it — the
//! file's bytes on disk are byte-identical before and after the run.
//! The preserved entry still appears in the red-state JSONL row with
//! `verdict: preserved`. Without this, a second test-builder run against
//! a story mid-implementation would silently re-redden a test the
//! implementer has already turned green, which is the exact legacy
//! failure mode this ADR is designed to prevent.
//!
//! The scaffold creates a fixture story with two acceptance.tests[]
//! entries: one pointing at a pre-existing file with non-empty
//! hand-authored content, the other pointing at a missing file. After
//! `TestBuilder::run`, the pre-existing file's bytes must match exactly
//! (hashed byte-for-byte), the missing file must have been scaffolded,
//! and the evidence row must carry `verdict: preserved` for the existing
//! file (no `red_path`, no `diagnostic`) and `verdict: red` for the
//! newly-scaffolded one. Red today is compile-red via the missing
//! `agentic_test_builder` public surface (`TestBuilder`).

use std::fs;
use std::path::Path;

use agentic_test_builder::TestBuilder;
use tempfile::TempDir;

const STORY_ID: u32 = 7003;

const EXISTING_TEST_BYTES: &[u8] = b"//! Hand-authored before test-builder ran. Its bytes must survive.\n\n#[test]\nfn already_written_by_human() {\n    panic!(\"human-authored\");\n}\n";

const FIXTURE_STORY_YAML: &str = r#"id: 7003
title: "Preservation fixture: one existing file, one missing file"

outcome: |
  A fixture story whose first acceptance test already exists on disk
  (with hand-authored content) and whose second is missing — the
  test-builder preserves the first and scaffolds the second.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/preservation-fixture/tests/already_written_by_human.rs
      justification: |
        This file already exists with non-empty content; test-builder must
        leave it bytes-identical and report `verdict: preserved` in the
        evidence row.
    - file: crates/preservation-fixture/tests/freshly_scaffolded.rs
      justification: |
        This file does not yet exist; test-builder writes it as a failing
        scaffold and records a `verdict: red` row.
  uat: |
    Drive `TestBuilder::run` against this fixture; observe the first file
    unchanged and the second created.

guidance: |
  Fixture authored inline for the preservation scaffold. Not a real
  story.

depends_on: []
"#;

#[test]
fn preserves_existing_tests_file_bytes_unchanged_and_verdict_is_preserved() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    // Fixture crate with ONE pre-existing test file.
    let fixture_root = repo_root.join("crates/preservation-fixture");
    fs::create_dir_all(fixture_root.join("src")).expect("fixture src");
    fs::create_dir_all(fixture_root.join("tests")).expect("fixture tests");
    fs::write(
        fixture_root.join("Cargo.toml"),
        r#"[package]
name = "preservation-fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write fixture Cargo.toml");
    fs::write(fixture_root.join("src/lib.rs"), b"").expect("write fixture lib.rs");

    let existing_path = fixture_root.join("tests/already_written_by_human.rs");
    fs::write(&existing_path, EXISTING_TEST_BYTES).expect("write pre-existing test");

    // Commit everything so the tree is clean.
    init_repo_and_commit_seed(repo_root);

    let before_bytes = fs::read(&existing_path).expect("read existing before run");

    let builder = TestBuilder::new(repo_root);
    builder.run(STORY_ID).expect("preservation run must succeed");

    // The existing file's bytes on disk are unchanged.
    let after_bytes = fs::read(&existing_path).expect("read existing after run");
    assert_eq!(
        after_bytes, before_bytes,
        "existing test file must be byte-identical before and after the run"
    );

    // The missing file was scaffolded.
    let fresh_path = fixture_root.join("tests/freshly_scaffolded.rs");
    assert!(
        fresh_path.exists(),
        "missing acceptance.tests[] file must be scaffolded"
    );

    // Evidence row carries one `preserved` verdict and one `red` verdict.
    let evidence_dir = repo_root.join("evidence/runs").join(STORY_ID.to_string());
    let rows = collect_jsonl_rows(&evidence_dir);
    assert_eq!(rows.len(), 1, "exactly one evidence file expected");
    let verdicts = rows[0]["verdicts"].as_array().expect("verdicts array");
    assert_eq!(verdicts.len(), 2);

    let preserved = verdicts
        .iter()
        .find(|v| v["verdict"].as_str() == Some("preserved"))
        .expect("one preserved verdict");
    assert!(
        preserved.get("red_path").is_none(),
        "preserved rows carry no red_path field"
    );
    assert!(
        preserved.get("diagnostic").is_none(),
        "preserved rows carry no diagnostic field"
    );
    assert_eq!(
        preserved["file"].as_str(),
        Some("crates/preservation-fixture/tests/already_written_by_human.rs"),
        "preserved verdict names the existing file"
    );

    let red = verdicts
        .iter()
        .find(|v| v["verdict"].as_str() == Some("red"))
        .expect("one red verdict");
    assert!(red["red_path"].is_string());
    assert!(red["diagnostic"].is_string());
}

fn collect_jsonl_rows(dir: &Path) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    if !dir.exists() {
        panic!("evidence directory missing: {}", dir.display());
    }
    for entry in fs::read_dir(dir).expect("read evidence dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let Some(ext) = path.extension() else { continue };
        if ext != "jsonl" {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read jsonl");
        for line in content.lines().filter(|l| !l.trim().is_empty()) {
            let v: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            out.push(v);
        }
    }
    out
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
