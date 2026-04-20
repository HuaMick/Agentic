//! Story 15 acceptance test: `record` refuses when a scaffold parses
//! as valid Rust but passes when probed — `cargo check` succeeds AND
//! the isolated `cargo test` exits 0. The refusal is typed
//! `ScaffoldNotRed` naming the offending file and the probe that came
//! back green.
//!
//! Justification (from stories/15.yml acceptance.tests[3]): without
//! this, a user-authored `assert!(true)` or `#[test] fn _() {}`
//! placeholder would be accepted by record and stamped into the
//! evidence as "red" — the exact theatre ADR-0005 names.
//!
//! Red today is compile-red: `TestBuilder::record` and the
//! `TestBuilderError::ScaffoldNotRed` variant are story-15 additions
//! that do not exist yet; `cargo check` fails on the unresolved items.
//!
//! NOTE: this scaffold deliberately sets up a real cargo workspace
//! with a fixture crate inside a `TempDir` so that when record's probe
//! hits `cargo check --package <fixture-crate> --test <name>` the
//! build succeeds and `cargo test` exits 0. The scaffold body is
//! `assert!(true)` — the canonical placeholder the justification
//! names. Record must recognise that the probe came back green and
//! refuse.

use std::fs;
use std::path::{Path, PathBuf};

use agentic_test_builder::{TestBuilder, TestBuilderError};
use tempfile::TempDir;

const STORY_ID: u32 = 99_015_004;

const FIXTURE_STORY_YAML: &str = r#"id: 99015004
title: "Fixture for story 15 record-refuses-when-scaffold-parses-green"

outcome: |
  Fixture used to prove record refuses with ScaffoldNotRed when a
  scaffold parses as Rust and probes green.

status: proposed

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-green-crate/tests/always_green.rs
      justification: |
        Proves that when a user authors a scaffold that parses fine
        and compiles fine and passes (e.g. `assert!(true)`), record
        refuses with ScaffoldNotRed rather than stamping a fake red
        verdict into the evidence row.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

const FIXTURE_WORKSPACE_CARGO: &str = r#"[workspace]
resolver = "2"
members = ["crates/fixture-green-crate"]
"#;

const FIXTURE_CRATE_CARGO: &str = r#"[package]
name = "fixture-green-crate"
version = "0.0.1"
edition = "2021"

[lib]
path = "src/lib.rs"
"#;

const FIXTURE_CRATE_LIB: &str = r#"pub fn identity() -> u32 {
    1
}
"#;

/// The canonical "green scaffold" the justification names:
/// `#[test] fn _() { assert!(true); }`. Parses fine, compiles fine,
/// passes. Record must refuse.
const GREEN_SCAFFOLD_BODY: &str = r#"#[test]
fn always_green() {
    assert!(true);
}
"#;

#[test]
fn record_refuses_with_scaffold_not_red_when_probe_exits_zero() {
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();

    // Workspace + crate shell so `cargo check --package <name> --test <name>`
    // can actually build the test target.
    fs::write(repo_root.join("Cargo.toml"), FIXTURE_WORKSPACE_CARGO).expect("ws cargo");
    let crate_root = repo_root.join("crates/fixture-green-crate");
    fs::create_dir_all(crate_root.join("src")).expect("crate src");
    fs::create_dir_all(crate_root.join("tests")).expect("crate tests");
    fs::write(crate_root.join("Cargo.toml"), FIXTURE_CRATE_CARGO).expect("crate cargo");
    fs::write(crate_root.join("src/lib.rs"), FIXTURE_CRATE_LIB).expect("crate lib");

    let scaffold_path = crate_root.join("tests/always_green.rs");
    fs::write(&scaffold_path, GREEN_SCAFFOLD_BODY).expect("write green scaffold");

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    init_repo_and_commit_seed(repo_root);

    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    let before_listing = listing(repo_root);

    // Act
    let builder = TestBuilder::new(repo_root);
    let result = builder.record(STORY_ID);

    match result {
        Err(TestBuilderError::ScaffoldNotRed { file, probe }) => {
            assert_eq!(
                file, scaffold_path,
                "ScaffoldNotRed must name the green scaffold path; got {}",
                file.display()
            );
            // `probe` names which probe came back green: compile or
            // runtime. For `assert!(true)` the cargo test run exits
            // 0 — runtime-green.
            let probe_str: &str = probe.as_ref();
            assert!(
                matches!(probe_str, "compile" | "runtime"),
                "ScaffoldNotRed.probe must be 'compile' or 'runtime'; got {probe_str:?}"
            );
        }
        other => panic!(
            "record must return ScaffoldNotRed naming {}; got {:?}",
            scaffold_path.display(),
            other
        ),
    }

    assert!(
        !evidence_dir.exists(),
        "record refusal must not create evidence/runs/{STORY_ID}/"
    );
    let after_listing = listing(repo_root);
    assert_eq!(
        before_listing, after_listing,
        "record refusal must leave the tree byte-identical"
    );
}

fn listing(root: &Path) -> String {
    let mut entries: Vec<(String, u64)> = Vec::new();
    walk(root, root, &mut entries);
    entries.sort();
    entries
        .into_iter()
        .map(|(rel, size)| format!("{rel}\t{size}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, u64)>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path: PathBuf = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Skip `.git` and cargo build artefacts — their internal
            // state shifts without being semantically dirty.
            if name == ".git" || name == "target" {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else if let Ok(meta) = fs::metadata(&path) {
                let rel = path
                    .strip_prefix(root)
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                out.push((rel, meta.len()));
            }
        }
    }
}

fn init_repo_and_commit_seed(root: &Path) {
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
