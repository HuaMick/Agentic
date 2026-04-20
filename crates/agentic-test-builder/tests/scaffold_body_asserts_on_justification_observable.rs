//! Story 14 acceptance test: scaffold asserts on the justification's observable.
//!
//! Justification (from stories/14.yml): Proves runtime-red via a real
//! assertion derived from the justification: given a fixture story
//! whose justification names an observable ("the function returns 1
//! for valid input") and whose target crate already declares a
//! callable stub that returns 0, `TestBuilder::run` writes a scaffold
//! that calls the stub and asserts the named observable. `cargo test`
//! runs it; the test fails on `assertion failed` (not on
//! `panic!(<text>)` with the justification as a message). The red-
//! state row carries `red_path: "runtime"` with `diagnostic` captured
//! from the assertion's panic output. Without this, the runtime-red
//! path degrades to the panic-stub shape, which cannot be turned
//! green by editing the implementation — only by editing the test,
//! which build-rust is forbidden from doing.
//!
//! The scaffold stubs `claude` to emit a deterministic Rust body that
//! (a) imports the target crate's already-declared function and
//! (b) asserts the observable the justification names (returns 1).
//! The fixture crate's `src/lib.rs` provides the stub `pub fn
//! valid_input() -> u32 { 0 }` so the scaffold COMPILES (imports
//! resolve) but its assertion must FAIL with an `assertion failed`
//! panic attributable to the value mismatch — that is the runtime-
//! red path. Red today is compile-red via the missing wiring from
//! stubbed `claude` stdout to the scaffold body (current
//! panic-stub shape ignores the shim).

use std::fs;
use std::path::Path;

use agentic_test_builder::{TestBuilder, TestBuilderOutcome};
use tempfile::TempDir;

const STORY_ID: u32 = 14002;

const FIXTURE_STORY_YAML: &str = r#"id: 14002
title: "Claude-authored scaffold asserts the observable named in the justification"

outcome: |
  A fixture story whose scaffold calls an already-declared stub and
  asserts the observable the justification names.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/observable-fixture/tests/asserts_returns_one.rs
      justification: |
        The target crate declares `pub fn valid_input() -> u32` as a
        stub that returns 0; the scaffold asserts it returns 1 for
        valid input, so the compiled test panics on `assertion failed`
        (NOT on panic!(<justification>)). An implementation edit in
        src/ that flips the stub to return 1 turns the scaffold green
        without any test edit.
  uat: |
    Drive TestBuilder::run, then cargo test the scaffold; observe
    assertion failure rather than a justification-text panic.

guidance: |
  Fixture authored inline for story 14's runtime-red scaffold test.
  Not a real story.

depends_on: []
"#;

// Deterministic body the stubbed `claude` emits. The assertion
// compares `valid_input()` to 1 — a real assertion whose failure is
// `assertion `left == right` failed`, NOT the justification text.
const STUBBED_CLAUDE_STDOUT: &str = r#"//! Story 14002 scaffold authored by stubbed `claude` shim.
use observable_fixture::valid_input;

#[test]
fn scaffold_asserts_valid_input_returns_one() {
    let actual = valid_input();
    assert_eq!(actual, 1, "valid_input must return 1 for valid input");
}
"#;

#[test]
fn scaffold_body_asserts_on_justification_observable_runs_real_assertion_not_panic_stub() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{STORY_ID}.yml")),
        FIXTURE_STORY_YAML,
    )
    .expect("write fixture");

    materialise_fixture_crate(repo_root);

    let path_override = install_claude_shim(repo_root, STUBBED_CLAUDE_STDOUT);
    std::env::set_var("PATH", &path_override);
    std::env::set_var("AGENTIC_CACHE", repo_root.join(".agentic-cache"));

    init_repo_and_commit_seed(repo_root);

    let builder = TestBuilder::new(repo_root);
    let outcome: TestBuilderOutcome = builder
        .run(STORY_ID)
        .expect("claude-backed happy-path run must succeed");

    // Scaffold exists at the declared path.
    let scaffold_path = repo_root
        .join("crates/observable-fixture/tests/asserts_returns_one.rs");
    assert!(scaffold_path.exists(), "scaffold must be created");

    // Scaffold contains a real assertion — NOT the panic-stub shape.
    let body = fs::read_to_string(&scaffold_path).expect("read scaffold");
    assert!(
        body.contains("assert"),
        "scaffold must contain a real assertion, not a panic-stub; got:\n{body}"
    );
    // Panic-stub degradation guard: the justification's first line
    // must NOT appear as a `panic!(<justification>)` argument. A
    // `panic!(\"...\")` that echoes the justification text is the old
    // scaffold shape this story closes.
    let panic_stub_signature =
        format!("panic!(\"{}\"", "The target crate declares `pub fn valid_input()");
    assert!(
        !body.contains(&panic_stub_signature),
        "scaffold body must not be a panic-stub echoing the justification; got:\n{body}"
    );

    // Evidence row says runtime-red, and the diagnostic comes from the
    // assertion's panic (commonly contains "assertion" or "left" or
    // "right"), NOT the justification text.
    let evidence_dir = repo_root.join("evidence/runs").join(STORY_ID.to_string());
    let rows = collect_jsonl_rows(&evidence_dir);
    assert_eq!(rows.len(), 1);
    let verdicts = rows[0]["verdicts"].as_array().expect("verdicts array");
    assert_eq!(verdicts.len(), 1);
    let v = &verdicts[0];
    assert_eq!(v["verdict"].as_str(), Some("red"));
    assert_eq!(
        v["red_path"].as_str(),
        Some("runtime"),
        "an assertion-failure scaffold is runtime-red, not compile-red"
    );
    let diag = v["diagnostic"].as_str().expect("diagnostic present");
    assert!(!diag.is_empty(), "diagnostic must carry the first panic line");
    // The diagnostic must be the panic's first line, not the
    // justification text.
    assert!(
        !diag.contains("The target crate declares"),
        "diagnostic must come from the assertion panic, not the justification text; got {diag:?}"
    );

    assert_eq!(outcome.created_paths().len(), 1);
}

fn materialise_fixture_crate(repo_root: &Path) {
    let crate_root = repo_root.join("crates/observable-fixture");
    fs::create_dir_all(crate_root.join("src")).expect("fixture src dir");
    fs::create_dir_all(crate_root.join("tests")).expect("fixture tests dir");
    fs::write(
        crate_root.join("Cargo.toml"),
        r#"[package]
name = "observable-fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write fixture Cargo.toml");
    // Stub that COMPILES (so the scaffold's `use` resolves) but
    // returns the WRONG value (so the scaffold's assert fails).
    fs::write(
        crate_root.join("src/lib.rs"),
        "pub fn valid_input() -> u32 { 0 }\n",
    )
    .expect("write fixture lib.rs");
}

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
