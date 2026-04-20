//! Story 14 acceptance test: a scaffold survives an implementation edit that turns it green.
//!
//! Justification (from stories/14.yml): Proves the greenability
//! contract: given the fixture from
//! `scaffold_body_asserts_on_justification_observable`, after the
//! scaffold is written and the red-state evidence recorded, editing
//! ONLY the target crate's `src/` (changing the stub to satisfy the
//! observable the scaffold asserts) causes `cargo test` against that
//! scaffold to exit 0. No edit to the scaffold file is required. The
//! scaffold file's bytes remain unchanged between red-state and
//! green runs (preserved by the existing bytes-immutable rule).
//! Without this, the scaffold is ungreenable by implementation and
//! the red-green contract ADR-0005 names is theatre — an implementer
//! could write any code and the scaffold would still panic.
//!
//! The scaffold drives `TestBuilder::run` against a fixture where
//! the target crate's stub returns 0 and the claude shim emits a
//! body asserting `valid_input() == 1`. It records the scaffold's
//! bytes (sha), then edits ONLY `src/lib.rs` to return 1, re-runs
//! `cargo test --manifest-path <fixture>/Cargo.toml --test <name>`
//! via `std::process::Command` and asserts exit 0. Finally, the
//! scaffold bytes must match the pre-edit sha — the implementation
//! edit must not have touched the scaffold file. Red today is
//! compile-red via the missing wiring from stubbed `claude` stdout
//! to the scaffold body.

use std::fs;
use std::path::Path;
use std::process::Command;

use agentic_test_builder::TestBuilder;
use tempfile::TempDir;

const STORY_ID: u32 = 14003;

const FIXTURE_STORY_YAML: &str = r#"id: 14003
title: "Claude-authored scaffold is greenable by an implementation edit alone"

outcome: |
  A fixture story whose scaffold turns green when the target crate's
  src/ is edited to satisfy the asserted observable — without any
  test-file edit.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/greenability-fixture/tests/greenable_by_src_edit.rs
      justification: |
        Scaffold asserts `valid_input() == 1`; the target crate's stub
        returns 0 (red); an edit to src/lib.rs that flips the return
        value to 1 turns the scaffold green without any edit to the
        test file itself — that is the greenability contract
        ADR-0005 depends on.
  uat: |
    Drive TestBuilder::run, record bytes, edit src/, cargo test; the
    scaffold bytes must be unchanged at green time.

guidance: |
  Fixture authored inline for story 14's greenability test. Not a
  real story.

depends_on: []
"#;

const STUBBED_CLAUDE_STDOUT: &str = r#"//! Story 14003 scaffold authored by stubbed `claude` shim.
use greenability_fixture::valid_input;

#[test]
fn scaffold_greenable_by_src_edit() {
    assert_eq!(valid_input(), 1, "valid_input must return 1");
}
"#;

#[test]
fn scaffold_body_survives_implementation_turning_it_green_bytes_unchanged_after_src_edit() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{STORY_ID}.yml")),
        FIXTURE_STORY_YAML,
    )
    .expect("write fixture");

    materialise_fixture_crate(repo_root, "0");

    let path_override = install_claude_shim(repo_root, STUBBED_CLAUDE_STDOUT);
    std::env::set_var("PATH", &path_override);
    std::env::set_var("AGENTIC_CACHE", repo_root.join(".agentic-cache"));

    init_repo_and_commit_seed(repo_root);

    let builder = TestBuilder::new(repo_root);
    builder.run(STORY_ID).expect("happy-path run must succeed");

    let scaffold_path = repo_root
        .join("crates/greenability-fixture/tests/greenable_by_src_edit.rs");
    assert!(scaffold_path.exists(), "scaffold must be created");

    // Capture the scaffold's bytes BEFORE the implementation edit.
    let bytes_before = fs::read(&scaffold_path).expect("read scaffold before");

    // Edit ONLY the target crate's src/ so valid_input() now returns
    // 1 — the scaffold's assertion is now satisfiable.
    let src_path = repo_root.join("crates/greenability-fixture/src/lib.rs");
    fs::write(&src_path, "pub fn valid_input() -> u32 { 1 }\n").expect("rewrite src/lib.rs");

    // Capture bytes AFTER the src edit — the test file must be
    // untouched (bytes-immutable rule).
    let bytes_after = fs::read(&scaffold_path).expect("read scaffold after");
    assert_eq!(
        bytes_before, bytes_after,
        "scaffold bytes must be unchanged by a src/ edit — the \
         greenability contract forbids test-file edits"
    );

    // Running `cargo test` against the fixture crate's single test
    // must now exit 0 — the greenability payoff.
    let status = Command::new("cargo")
        .arg("test")
        .arg("--manifest-path")
        .arg(repo_root.join("crates/greenability-fixture/Cargo.toml"))
        .arg("--test")
        .arg("greenable_by_src_edit")
        .status()
        .expect("spawn cargo test");
    assert!(
        status.success(),
        "scaffold must exit 0 after the src/ edit — greenability contract broke"
    );
}

fn materialise_fixture_crate(repo_root: &Path, initial_return: &str) {
    let crate_root = repo_root.join("crates/greenability-fixture");
    fs::create_dir_all(crate_root.join("src")).expect("fixture src dir");
    fs::create_dir_all(crate_root.join("tests")).expect("fixture tests dir");
    fs::write(
        crate_root.join("Cargo.toml"),
        r#"[package]
name = "greenability-fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write fixture Cargo.toml");
    fs::write(
        crate_root.join("src/lib.rs"),
        format!("pub fn valid_input() -> u32 {{ {initial_return} }}\n"),
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
