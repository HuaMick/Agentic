//! Story 7 acceptance test: runtime-red happy path.
//!
//! Justification (from stories/7.yml): Proves the runtime-red happy path:
//! given a story whose acceptance.tests[] lists two file paths that do
//! not exist on disk and whose justifications are substantive prose whose
//! first line implies behaviour the fixture's existing implementation can
//! already be CALLED into (imports resolve and the crate compiles) but
//! that asserts a condition the implementation does not yet satisfy,
//! `TestBuilder::run` writes both files as compilable Rust integration
//! tests, runs each one, observes each fail with a `panic!` whose message
//! is the justification's first line, and appends one red-state JSONL row
//! to `evidence/runs/<id>/` with one `verdict: red` entry per scaffold,
//! `red_path: "runtime"` on each entry, `diagnostic` equal to that first
//! line of the panic message, and the current HEAD commit hash.
//!
//! The scaffold constructs a `TempDir` containing a fresh git repo with
//! a seed commit and a fixture story whose two acceptance.tests[] files
//! do not yet exist. A minimal "fixture crate" is materialised inside
//! the same tempdir so the scaffolds will compile (imports resolve) but
//! assert against an unimplemented observable, driving each scaffold to
//! runtime-red. `TestBuilder::run` is invoked; its return value and the
//! on-disk evidence artefact are inspected for the shape named in the
//! justification. Red today is compile-red via the missing
//! `agentic_test_builder` public surface (`TestBuilder`,
//! `TestBuilderOutcome`).

use std::fs;
use std::path::Path;

use agentic_test_builder::{TestBuilder, TestBuilderOutcome};
use tempfile::TempDir;

const STORY_ID: u32 = 7001;

// Deterministic Rust body the stubbed `claude` emits on stdout. Both
// acceptance.tests[] entries receive this same body; each scaffold
// compiles (the fixture crate declares `one()`) but the assertion
// rejects the observable (`one()` returns 0, the scaffold asserts 1),
// so cargo test fails with an `assertion failed` panic — runtime-red.
const STUBBED_CLAUDE_STDOUT: &str = r#"//! Story 7001 scaffold authored by stubbed `claude` shim.
use fixture_crate::one;

#[test]
fn scaffold_asserts_one_returns_one() {
    let actual = one();
    assert_eq!(actual, 1, "one() must return 1");
}
"#;

const FIXTURE_STORY_YAML: &str = r#"id: 7001
title: "Runtime-red fixture story with two missing scaffold paths"

outcome: |
  A fixture story whose two acceptance.tests[] files do not yet exist —
  the test-builder writes them, runs them, and records runtime-red
  evidence rows.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/fixture-crate/tests/fixture_asserts_one.rs
      justification: |
        The fixture implementation returns 0 but the scaffold asserts it
        returns 1, so the compiled test panics on assertion failure.
    - file: crates/fixture-crate/tests/fixture_asserts_two.rs
      justification: |
        The fixture implementation returns false but the scaffold asserts
        it returns true, so the compiled test panics on assertion failure.
  uat: |
    Drive `TestBuilder::run` against this fixture; observe two runtime-red
    rows in evidence/runs/7001/<ts>-red.jsonl.

guidance: |
  Fixture authored inline for the runtime-red happy-path scaffold. Not a
  real story.

depends_on: []
"#;

#[test]
fn scaffolds_missing_files_writes_both_as_runtime_red_and_records_one_evidence_row_per_scaffold() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    // A minimal fixture crate the scaffolds can compile against — its
    // implementation is already callable (so the scaffolds are NOT
    // compile-red) but returns values that the scaffold's assertions
    // reject (so the scaffolds ARE runtime-red).
    materialise_fixture_crate(repo_root);

    // Stub `claude` onto a tempdir-rooted PATH so the library's
    // subprocess wire is exercised without needing real claude auth.
    let path_override = install_claude_shim(repo_root, STUBBED_CLAUDE_STDOUT);
    std::env::set_var("PATH", &path_override);
    std::env::set_var("AGENTIC_CACHE", repo_root.join(".agentic-cache"));

    let commit = init_repo_and_commit_seed(repo_root);

    let builder = TestBuilder::new(repo_root);
    let outcome: TestBuilderOutcome =
        builder.run(STORY_ID).expect("happy-path run must succeed");

    // Both scaffold files were written.
    for name in ["fixture_asserts_one.rs", "fixture_asserts_two.rs"] {
        let path = repo_root
            .join("crates/fixture-crate/tests")
            .join(name);
        assert!(
            path.exists(),
            "scaffold at {} must be created",
            path.display()
        );
        let bytes = fs::read(&path).expect("read scaffold");
        assert!(
            !bytes.is_empty(),
            "scaffold at {} must be non-empty",
            path.display()
        );
    }

    // Exactly one evidence file in evidence/runs/<id>/ with runtime-red
    // rows for both scaffolds.
    let evidence_dir = repo_root.join("evidence/runs").join(STORY_ID.to_string());
    let rows = collect_jsonl_rows(&evidence_dir);
    assert_eq!(
        rows.len(),
        1,
        "exactly one evidence file expected under {}; got {}",
        evidence_dir.display(),
        rows.len()
    );
    let row = &rows[0];
    assert_eq!(row["story_id"].as_u64(), Some(u64::from(STORY_ID)));
    assert_eq!(row["commit"].as_str(), Some(commit.as_str()));

    let verdicts = row["verdicts"].as_array().expect("verdicts is array");
    assert_eq!(
        verdicts.len(),
        2,
        "two acceptance.tests entries => two verdict rows"
    );
    for v in verdicts {
        assert_eq!(v["verdict"].as_str(), Some("red"));
        assert_eq!(
            v["red_path"].as_str(),
            Some("runtime"),
            "both scaffolds are runtime-red per the justification"
        );
        let diag = v["diagnostic"].as_str().expect("diagnostic present");
        assert!(
            !diag.is_empty(),
            "diagnostic must carry the first line of the panic message"
        );
    }

    // The outcome summary names both created scaffolds.
    let created = outcome.created_paths();
    assert_eq!(created.len(), 2, "two scaffolds created");
}

fn materialise_fixture_crate(repo_root: &Path) {
    let crate_root = repo_root.join("crates/fixture-crate");
    fs::create_dir_all(crate_root.join("src")).expect("fixture src dir");
    fs::create_dir_all(crate_root.join("tests")).expect("fixture tests dir");
    fs::write(
        crate_root.join("Cargo.toml"),
        r#"[package]
name = "fixture-crate"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write fixture Cargo.toml");
    fs::write(
        crate_root.join("src/lib.rs"),
        r#"pub fn one() -> u32 { 0 }
pub fn flag() -> bool { false }
"#,
    )
    .expect("write fixture lib.rs");
}

/// Install a `claude` shim onto a tempdir and return a PATH string
/// that prepends that tempdir — so spawning `claude` from a child
/// process finds the shim, which writes `stdout_body` verbatim on
/// stdout regardless of argv/stdin.
fn install_claude_shim(repo_root: &Path, stdout_body: &str) -> String {
    let shim_dir = repo_root.join(".bin");
    fs::create_dir_all(&shim_dir).expect("shim dir");
    let shim_path = shim_dir.join("claude");
    let script = format!(
        "#!/bin/sh\ncat <<'__AGENTIC_EOF__'\n{body}__AGENTIC_EOF__\n",
        body = stdout_body
    );
    fs::write(&shim_path, script).expect("write shim");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&shim_path).expect("shim metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&shim_path, perms).expect("chmod shim");
    }
    let old_path = std::env::var("PATH").unwrap_or_default();
    format!("{}:{}", shim_dir.display(), old_path)
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
