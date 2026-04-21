//! Story 15 acceptance test: record's compile-vs-runtime classifier
//! is robust to rustc's JSON diagnostic-renderer panicking mid-emit.
//!
//! Given a fixture story whose scaffold on disk contains three or more
//! unresolved `use foo::Bar;` imports (the pattern that trips rustc
//! 1.95's annotate-snippets ICE "slice index starts at N but ends at
//! N-1" inside `StyledBuffer::replace`), `agentic test-build record
//! <id>` must classify the verdict as `red_path: compile` for that
//! scaffold — not `runtime`. The probe MUST key on something other
//! than `error[EXXXX]` in human-rendered stderr (exit code,
//! `--message-format=short`, or `--message-format=json`) because that
//! string may never emit when the renderer panics first.
//!
//! Justification (from stories/15.yml acceptance.tests[8]): without
//! this, compile-red vs runtime-red in evidence stops being
//! trustworthy across toolchain updates, and downstream tooling (the
//! dashboard, UAT readers, future audit agents) cannot rely on
//! `red_path` as a classifier signal.
//!
//! Red today is natural: current implementation's probe greps stderr
//! for `error[E` and falls through to `cargo test` on ICE, where the
//! panic surfaces as a `runtime` classification instead of `compile`.
//! The assertion below fails because the written evidence row carries
//! `red_path: "runtime"` when the contract requires `"compile"`.

use std::fs;
use std::path::Path;

use agentic_test_builder::TestBuilder;
use tempfile::TempDir;

const STORY_ID: u32 = 99_015_009;

const FIXTURE_STORY_YAML: &str = r#"id: 99015009
title: "Fixture for story 15 compile-red-classification-across-rustc-ice"

outcome: |
  Fixture used to prove record classifies compile-red correctly
  even when rustc's diagnostic renderer panics mid-emit.

status: proposed

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-ice-crate/tests/scaffold_ice.rs
      justification: |
        Proves record keys the compile/runtime classifier on a
        signal other than human-rendered `error[EXXXX]` stderr,
        so a rustc ICE in annotate_snippets does not cause the
        probe to misclassify the verdict as runtime-red.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

const FIXTURE_WORKSPACE_CARGO: &str = r#"[workspace]
resolver = "2"
members = ["crates/fixture-ice-crate"]
"#;

const FIXTURE_CRATE_CARGO: &str = r#"[package]
name = "fixture-ice-crate"
version = "0.0.1"
edition = "2021"

[lib]
path = "src/lib.rs"
"#;

const FIXTURE_CRATE_LIB: &str = r#"// Deliberately empty — the scaffold references symbols this crate
// does not declare, so every `use` line is unresolved.
"#;

/// Scaffold body with three unresolved imports from the same path —
/// the specific pattern that triggers rustc 1.95's
/// `annotate_snippets::renderer::styled_buffer::StyledBuffer::replace`
/// ICE ("slice index starts at N but ends at N-1"). Classification
/// must still land on `compile`, not `runtime`.
const ICE_SCAFFOLD_BODY: &str = r#"use fixture_ice_crate::{alpha_missing, beta_missing, gamma_missing};

#[test]
fn tests_compile_red_classification_survives_rustc_ice() {
    let _ = alpha_missing();
    let _ = beta_missing();
    let _ = gamma_missing();
}
"#;

#[test]
fn record_classifies_compile_red_correctly_across_rustc_ice() {
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();

    fs::write(repo_root.join("Cargo.toml"), FIXTURE_WORKSPACE_CARGO).expect("ws cargo");
    let crate_root = repo_root.join("crates/fixture-ice-crate");
    fs::create_dir_all(crate_root.join("src")).expect("crate src");
    fs::create_dir_all(crate_root.join("tests")).expect("crate tests");
    fs::write(crate_root.join("Cargo.toml"), FIXTURE_CRATE_CARGO).expect("crate cargo");
    fs::write(crate_root.join("src/lib.rs"), FIXTURE_CRATE_LIB).expect("crate lib");

    let scaffold_path = crate_root.join("tests/scaffold_ice.rs");
    fs::write(&scaffold_path, ICE_SCAFFOLD_BODY).expect("write ice scaffold");

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    init_repo_and_commit_seed(repo_root);

    // Act: record. The scaffold's `cargo check` must exit non-zero
    // with an unresolved-import error; the classifier must read that
    // signal (exit code, JSON channel, etc.) and stamp
    // `red_path: "compile"`, NOT fall through to `cargo test` and
    // stamp `"runtime"`.
    let builder = TestBuilder::new(repo_root);
    let _outcome = builder
        .record(STORY_ID)
        .expect("record must succeed writing evidence for a compile-red scaffold");

    // Assert: read the single evidence JSONL and confirm the verdict
    // for this scaffold is `red_path: "compile"`.
    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    let evidence_files: Vec<_> = fs::read_dir(&evidence_dir)
        .expect("read evidence dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()) == Some("jsonl")
                || p.to_string_lossy().ends_with(".jsonl")
        })
        .collect();
    assert_eq!(
        evidence_files.len(),
        1,
        "record must write exactly one *.jsonl file; got {evidence_files:?}"
    );

    let body = fs::read_to_string(&evidence_files[0]).expect("read evidence file");
    let row: serde_json::Value =
        serde_json::from_str(body.trim()).expect("evidence row must be valid JSON");

    let verdicts = row
        .get("verdicts")
        .and_then(|v| v.as_array())
        .expect("verdicts must be a JSON array");
    assert_eq!(verdicts.len(), 1, "one verdict per acceptance test");

    let verdict = verdicts[0]
        .as_object()
        .expect("verdict must be an object");
    let red_path = verdict
        .get("red_path")
        .and_then(|v| v.as_str())
        .expect("verdict.red_path must be a string");

    assert_eq!(
        red_path, "compile",
        "a scaffold with three unresolved imports must classify as compile-red \
         even if rustc's diagnostic renderer panics mid-emit; \
         got red_path={red_path:?}. The classifier must not grep \
         human-rendered stderr for `error[E...]` — it must key on \
         `cargo check`'s exit code or a structured diagnostic channel \
         (--message-format=short / --message-format=json) so a \
         renderer ICE does not misclassify compile-red as runtime-red."
    );
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
