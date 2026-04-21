//! Story 15 acceptance test: no red verdict is ever stamped with a
//! silent, whitespace-only, or vacuous-placeholder `diagnostic`.
//!
//! Given a fixture story whose scaffolds all probe red, every
//! `verdicts[]` entry in the written JSONL whose `verdict` is `red`
//! must carry a `diagnostic` field whose trimmed length is greater
//! than zero AND which actually reports the scaffold's own failing
//! signal (first line of a rustc error or a real panic message).
//! If the probe captures nothing — because the subprocess exited
//! non-zero without emitting a panic banner, because the buffer
//! was lost, or because a refactor silently swallowed the stream
//! — record MUST fail-closed with a typed error naming the
//! scaffold, NOT write a red verdict with an empty or vacuous
//! diagnostic like the literal string "test failed".
//!
//! Justification (from stories/15.yml acceptance.tests[10]): this
//! pins a distinct failure mode from the diagnostic-aliasing
//! contract (aliasing is "wrong content"; this is "no content");
//! both can regress independently as toolchains and ICE paths
//! shift, and evidence has to survive both.
//!
//! Red today is natural: current impl's probe falls through to
//! the hard-coded fallback `("runtime", "test failed")` when
//! `cargo test` fails without emitting "panicked at" in its
//! output — for example a scaffold that calls
//! `std::process::exit(1)`. The resulting evidence row carries
//! `diagnostic: "test failed"`, which is non-empty but vacuous,
//! and the contract requires either a real captured diagnostic
//! or a typed fail-closed refusal.

use std::fs;
use std::path::{Path, PathBuf};

use agentic_test_builder::TestBuilder;
use tempfile::TempDir;

const STORY_ID: u32 = 99_015_011;

const FIXTURE_STORY_YAML: &str = r#"id: 99015011
title: "Fixture for story 15 evidence-diagnostic-is-always-non-empty"

outcome: |
  Fixture used to prove no red verdict is ever stamped with an
  empty, whitespace-only, or vacuous-placeholder diagnostic.

status: proposed

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-empty-diag-crate/tests/scaffold_silent_fail.rs
      justification: |
        Proves that when a scaffold exits non-zero without emitting a
        panic banner the probe can scrape, record does NOT stamp a
        red verdict with an empty or vacuous-placeholder diagnostic
        — it either captures a real diagnostic or fails closed.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

const FIXTURE_WORKSPACE_CARGO: &str = r#"[workspace]
resolver = "2"
members = ["crates/fixture-empty-diag-crate"]
"#;

const FIXTURE_CRATE_CARGO: &str = r#"[package]
name = "fixture-empty-diag-crate"
version = "0.0.1"
edition = "2021"

[lib]
path = "src/lib.rs"
"#;

const FIXTURE_CRATE_LIB: &str = r#"// Deliberately empty — the scaffold supplies its own failure via
// `std::process::exit(1)`.
"#;

/// Scaffold body whose test exits 1 without unwinding. cargo test
/// sees a non-zero exit with no "panicked at" banner in stdout or
/// stderr, so a probe that only scrapes panic messages captures
/// nothing meaningful. The scaffold is still red (cargo test exits
/// non-zero) — the question is whether the diagnostic the evidence
/// writer records actually names the failure or is a vacuous
/// placeholder.
const SILENT_FAIL_SCAFFOLD: &str = r#"#[test]
fn scaffold_silent_fail() {
    // Exit non-zero without unwinding. The cargo test harness
    // reports the test as failed but no "panicked at ..." line
    // appears in its output — so a probe that greps for panic
    // banners captures nothing real.
    std::process::exit(1);
}
"#;

#[test]
fn record_evidence_diagnostic_is_always_non_empty() {
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();

    fs::write(repo_root.join("Cargo.toml"), FIXTURE_WORKSPACE_CARGO).expect("ws cargo");
    let crate_root = repo_root.join("crates/fixture-empty-diag-crate");
    fs::create_dir_all(crate_root.join("src")).expect("crate src");
    fs::create_dir_all(crate_root.join("tests")).expect("crate tests");
    fs::write(crate_root.join("Cargo.toml"), FIXTURE_CRATE_CARGO).expect("crate cargo");
    fs::write(crate_root.join("src/lib.rs"), FIXTURE_CRATE_LIB).expect("crate lib");

    let scaffold_path = crate_root.join("tests/scaffold_silent_fail.rs");
    fs::write(&scaffold_path, SILENT_FAIL_SCAFFOLD).expect("write silent-fail scaffold");

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    init_repo_and_commit_seed(repo_root);

    // Act: record. If the probe scrapes real content, the evidence
    // row carries it. If the probe captures nothing real, record
    // must fail-closed with a typed error — it must NOT silently
    // stamp a vacuous placeholder like "test failed".
    let builder = TestBuilder::new(repo_root);
    let outcome = builder.record(STORY_ID);

    match outcome {
        Err(_typed_refusal) => {
            // Fail-closed path: acceptable per the contract. The
            // typed error names the scaffold. Evidence dir must
            // NOT exist.
            let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
            assert!(
                !evidence_dir.exists(),
                "record fail-closed must not create evidence/runs/{STORY_ID}/"
            );
        }
        Ok(_recorded) => {
            // Record claimed success — every red verdict's diagnostic
            // must be non-empty AND non-vacuous.
            let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
            let evidence_files: Vec<PathBuf> = fs::read_dir(&evidence_dir)
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
                "record success must write exactly one *.jsonl file; got {evidence_files:?}"
            );

            let body = fs::read_to_string(&evidence_files[0]).expect("read evidence file");
            let row: serde_json::Value =
                serde_json::from_str(body.trim()).expect("evidence row must be valid JSON");

            let verdicts = row
                .get("verdicts")
                .and_then(|v| v.as_array())
                .expect("verdicts must be a JSON array");

            for (i, v) in verdicts.iter().enumerate() {
                let obj = v.as_object().expect("verdict is object");
                let verdict_kind = obj
                    .get("verdict")
                    .and_then(|s| s.as_str())
                    .expect("verdict.verdict");
                if verdict_kind != "red" {
                    continue;
                }
                let diagnostic = obj
                    .get("diagnostic")
                    .and_then(|s| s.as_str())
                    .expect("verdict.diagnostic must be a string for red verdicts");

                assert!(
                    !diagnostic.trim().is_empty(),
                    "verdict[{i}].diagnostic must not be empty or whitespace-only; \
                     got {diagnostic:?}. Empty diagnostics on red verdicts mean the \
                     probe captured nothing and record stamped a lie — the contract \
                     is fail-closed with a typed error, not silently write an empty \
                     diagnostic."
                );

                let trimmed_lower = diagnostic.trim().to_lowercase();
                assert_ne!(
                    trimmed_lower.as_str(),
                    "test failed",
                    "verdict[{i}].diagnostic must not be the vacuous placeholder \
                     \"test failed\"; a probe that captures nothing real must \
                     fail-closed with a typed error rather than substitute a \
                     hard-coded string that names no scaffold and no failure. \
                     Got {diagnostic:?}."
                );
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
