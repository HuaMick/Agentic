//! Story 15 acceptance test: per-scaffold `diagnostic` in evidence is
//! a snapshot of THAT scaffold's own probe, not the last-probed
//! scaffold's output written into every row.
//!
//! Given a fixture story with three scaffolds, each failing with a
//! DIFFERENT observable:
//!   - scaffold A: unresolved import `alpha_missing` (compile-red)
//!   - scaffold B: unresolved import `beta_missing` (compile-red)
//!   - scaffold C: runtime `assert_eq!(1, 2)` panic (runtime-red)
//! `agentic test-build record <id>` must write an evidence JSONL
//! whose three verdict rows each carry a `diagnostic` that contains
//! the string signature of THAT scaffold's own failure, and the
//! three diagnostics must be pairwise distinct.
//!
//! Justification (from stories/15.yml acceptance.tests[9]): without
//! this, the evidence JSONL lies to downstream readers about what
//! actually failed and which scaffold owns which failure — a forgery
//! axis one layer down from the evidence-atomicity contract.
//!
//! Red today is natural: current impl probes each scaffold via
//! `cargo check`/`cargo test` at the crate granularity (no
//! `--test <name>` filter), so every iteration surfaces whichever
//! failure cargo hits first across the whole crate. The per-verdict
//! `diagnostic` ends up aliased to the same string for every row.

use std::fs;
use std::path::Path;

use agentic_test_builder::TestBuilder;
use tempfile::TempDir;

const STORY_ID: u32 = 99_015_010;

const FIXTURE_STORY_YAML: &str = r#"id: 99015010
title: "Fixture for story 15 per-scaffold-diagnostic-not-aliased"

outcome: |
  Fixture used to prove each scaffold's evidence diagnostic is a
  per-iteration snapshot of its own probe output, not the shared
  trailing buffer of the last-probed scaffold.

status: proposed

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-alias-crate/tests/scaffold_alpha.rs
      justification: |
        Proves scaffold A's diagnostic names its own unresolved
        import `alpha_missing`, not any other scaffold's failure.
    - file: crates/fixture-alias-crate/tests/scaffold_beta.rs
      justification: |
        Proves scaffold B's diagnostic names its own unresolved
        import `beta_missing`, not any other scaffold's failure.
    - file: crates/fixture-alias-crate/tests/scaffold_gamma.rs
      justification: |
        Proves scaffold C's diagnostic names its own runtime
        assertion failure with the literal values 1 and 2,
        distinct from the other two scaffolds' compile errors.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

const FIXTURE_WORKSPACE_CARGO: &str = r#"[workspace]
resolver = "2"
members = ["crates/fixture-alias-crate"]
"#;

const FIXTURE_CRATE_CARGO: &str = r#"[package]
name = "fixture-alias-crate"
version = "0.0.1"
edition = "2021"

[lib]
path = "src/lib.rs"
"#;

const FIXTURE_CRATE_LIB: &str = r#"// Deliberately empty — scaffolds A and B reference unresolved
// symbols; scaffold C's failure is a runtime assertion.
"#;

/// Scaffold A — compile-red on `alpha_missing`.
const SCAFFOLD_ALPHA: &str = r#"use fixture_alias_crate::alpha_missing;

#[test]
fn scaffold_alpha_probes_compile_red() {
    let _ = alpha_missing();
}
"#;

/// Scaffold B — compile-red on `beta_missing`.
const SCAFFOLD_BETA: &str = r#"use fixture_alias_crate::beta_missing;

#[test]
fn scaffold_beta_probes_compile_red() {
    let _ = beta_missing();
}
"#;

/// Scaffold C — runtime-red via `assert_eq!(1, 2)`.
const SCAFFOLD_GAMMA: &str = r#"#[test]
fn scaffold_gamma_probes_runtime_red() {
    assert_eq!(1, 2);
}
"#;

#[test]
fn record_evidence_diagnostic_is_per_scaffold_not_aliased() {
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();

    fs::write(repo_root.join("Cargo.toml"), FIXTURE_WORKSPACE_CARGO).expect("ws cargo");
    let crate_root = repo_root.join("crates/fixture-alias-crate");
    fs::create_dir_all(crate_root.join("src")).expect("crate src");
    fs::create_dir_all(crate_root.join("tests")).expect("crate tests");
    fs::write(crate_root.join("Cargo.toml"), FIXTURE_CRATE_CARGO).expect("crate cargo");
    fs::write(crate_root.join("src/lib.rs"), FIXTURE_CRATE_LIB).expect("crate lib");

    fs::write(crate_root.join("tests/scaffold_alpha.rs"), SCAFFOLD_ALPHA).expect("alpha");
    fs::write(crate_root.join("tests/scaffold_beta.rs"), SCAFFOLD_BETA).expect("beta");
    fs::write(crate_root.join("tests/scaffold_gamma.rs"), SCAFFOLD_GAMMA).expect("gamma");

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    init_repo_and_commit_seed(repo_root);

    // Act: record all three scaffolds. Each probe must capture its
    // own scaffold's failure — not alias to a shared buffer holding
    // the last-probed scaffold's output.
    let builder = TestBuilder::new(repo_root);
    let _outcome = builder
        .record(STORY_ID)
        .expect("record must succeed writing evidence for three red scaffolds");

    // Assert: read the single evidence JSONL.
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
    assert_eq!(
        verdicts.len(),
        3,
        "three scaffolds -> three verdict rows"
    );

    // Locate each verdict by its scaffold filename.
    let mut alpha_diag: Option<String> = None;
    let mut beta_diag: Option<String> = None;
    let mut gamma_diag: Option<String> = None;
    for v in verdicts {
        let obj = v.as_object().expect("verdict is object");
        let file = obj
            .get("file")
            .and_then(|s| s.as_str())
            .expect("verdict.file");
        let diagnostic = obj
            .get("diagnostic")
            .and_then(|s| s.as_str())
            .expect("verdict.diagnostic")
            .to_string();
        if file.ends_with("scaffold_alpha.rs") {
            alpha_diag = Some(diagnostic);
        } else if file.ends_with("scaffold_beta.rs") {
            beta_diag = Some(diagnostic);
        } else if file.ends_with("scaffold_gamma.rs") {
            gamma_diag = Some(diagnostic);
        } else {
            panic!("unexpected verdict file: {file}");
        }
    }

    let alpha_diag = alpha_diag.expect("verdict for scaffold_alpha.rs");
    let beta_diag = beta_diag.expect("verdict for scaffold_beta.rs");
    let gamma_diag = gamma_diag.expect("verdict for scaffold_gamma.rs");

    // Each diagnostic must contain the string signature of ITS OWN
    // scaffold's failure.
    assert!(
        alpha_diag.contains("alpha_missing"),
        "scaffold A's diagnostic must name its own unresolved import `alpha_missing`; \
         got: {alpha_diag:?}. If this string is missing, the per-verdict diagnostic \
         has been aliased to another scaffold's output."
    );
    assert!(
        beta_diag.contains("beta_missing"),
        "scaffold B's diagnostic must name its own unresolved import `beta_missing`; \
         got: {beta_diag:?}. If this string is missing, the per-verdict diagnostic \
         has been aliased to another scaffold's output."
    );
    assert!(
        gamma_diag.to_lowercase().contains("assertion")
            && gamma_diag.contains('1')
            && gamma_diag.contains('2'),
        "scaffold C's diagnostic must name its own runtime assertion failure and \
         carry the literal values 1 and 2 from `assert_eq!(1, 2)`; got: {gamma_diag:?}."
    );

    // And they must be pairwise distinct — no two rows share the
    // same diagnostic string.
    assert_ne!(
        alpha_diag, beta_diag,
        "scaffold A's and B's diagnostics must differ; got both = {alpha_diag:?}. \
         Identical diagnostics across rows is the aliasing bug this test pins."
    );
    assert_ne!(
        alpha_diag, gamma_diag,
        "scaffold A's and C's diagnostics must differ; got both = {alpha_diag:?}."
    );
    assert_ne!(
        beta_diag, gamma_diag,
        "scaffold B's and C's diagnostics must differ; got both = {beta_diag:?}."
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
