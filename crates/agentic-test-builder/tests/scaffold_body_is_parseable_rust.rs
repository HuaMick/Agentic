//! Story 14 acceptance test: scaffold bodies must parse as Rust source.
//!
//! Justification (from stories/14.yml): Proves syntactic validation:
//! given any fixture story whose justifications are substantive,
//! every scaffold the binary writes parses as a valid Rust source
//! file (via `syn::parse_file` or equivalent). If `claude`'s output
//! is not parseable Rust (malformed token stream, truncated block,
//! mixed prose-and-code), the binary returns
//! `TestBuilderError::ScaffoldParseError` naming the offending file
//! path and the parser's error message, writes zero scaffolds (even
//! for the valid sibling entries in the same story), writes zero
//! evidence, and leaves the tree in its pre-run state.
//!
//! The scaffold exercises both sub-scenarios:
//!
//!   Happy path: the stubbed `claude` emits a parseable Rust body;
//!   the scaffold lands on disk, its bytes round-trip through
//!   `syn::parse_file`, and the evidence row is written.
//!
//!   Refusal: the stubbed `claude` emits a malformed body (a
//!   truncated block that `syn` cannot parse); the whole run must
//!   refuse with `TestBuilderError::ScaffoldParseError { path,
//!   stderr }`, write zero scaffolds (even the valid first sibling),
//!   and write zero evidence. Red today is compile-red via the
//!   missing `TestBuilderError::ScaffoldParseError` variant.

use std::fs;
use std::path::Path;

use agentic_test_builder::{TestBuilder, TestBuilderError};
use tempfile::TempDir;

const HAPPY_STORY_ID: u32 = 14004;
const MALFORMED_STORY_ID: u32 = 140041;

const HAPPY_STORY_YAML: &str = r#"id: 14004
title: "Parseable-Rust fixture: claude stdout is valid Rust"

outcome: |
  A fixture whose claude shim emits parseable Rust; scaffolds land,
  evidence is written.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/parseable-fixture/tests/valid_rust.rs
      justification: |
        A substantive justification so the scaffold lands; the stubbed
        claude emits a syntactically valid Rust source that
        `syn::parse_file` accepts.
  uat: |
    Run, read scaffold off disk, parse with syn::parse_file, expect
    Ok.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const MALFORMED_STORY_YAML: &str = r#"id: 140041
title: "Malformed-claude fixture: stdout is truncated Rust"

outcome: |
  A fixture whose claude shim emits a malformed Rust token stream;
  the whole run must refuse with ScaffoldParseError.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/parseable-fixture/tests/first_sibling_valid.rs
      justification: |
        A substantive justification for the first (valid) sibling —
        the ScaffoldParseError on the second entry must still roll
        back this file so NO scaffold is written for the story.
    - file: crates/parseable-fixture/tests/second_sibling_malformed.rs
      justification: |
        A substantive justification for the entry whose stubbed
        claude emits malformed Rust; TestBuilder::run must return
        ScaffoldParseError naming this file.
  uat: |
    Run, observe ScaffoldParseError with `path` equal to the
    offending scaffold's path and `stderr` naming the parser's first
    error line.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const VALID_RUST_STDOUT: &str = r#"//! Valid Rust scaffold.
use parseable_fixture::noop;

#[test]
fn valid_rust_parses() {
    noop();
}
"#;

// Deliberately malformed: an opening brace with no closing brace.
// `syn::parse_file` rejects this with an "expected `}`" error.
const MALFORMED_RUST_STDOUT: &str = "fn broken( {\n    let x = ;\n// EOF mid-block\n";

#[test]
fn scaffold_body_is_parseable_rust_parses_valid_and_refuses_malformed_with_scaffold_parse_error() {
    // ---- Happy path: syn::parse_file accepts the stubbed body.
    {
        let tmp = TempDir::new().expect("tempdir");
        let repo_root = tmp.path();

        seed_story(repo_root, HAPPY_STORY_ID, HAPPY_STORY_YAML);
        materialise_fixture_crate(repo_root);

        let path_override = install_claude_shim(repo_root, VALID_RUST_STDOUT);
        std::env::set_var("PATH", &path_override);
        std::env::set_var("AGENTIC_CACHE", repo_root.join(".agentic-cache"));

        init_repo_and_commit_seed(repo_root);

        let builder = TestBuilder::new(repo_root);
        builder
            .run(HAPPY_STORY_ID)
            .expect("happy path must succeed");

        let scaffold_path = repo_root
            .join("crates/parseable-fixture/tests/valid_rust.rs");
        let body = fs::read_to_string(&scaffold_path).expect("read scaffold");
        syn::parse_file(&body).expect("scaffold must parse as Rust source");
    }

    // ---- Refusal path: malformed claude output surfaces as
    // ScaffoldParseError and rolls back ALL scaffolds (even the
    // first sibling that was valid).
    {
        let tmp = TempDir::new().expect("tempdir");
        let repo_root = tmp.path();

        seed_story(repo_root, MALFORMED_STORY_ID, MALFORMED_STORY_YAML);
        materialise_fixture_crate(repo_root);

        // A shim that dispatches per-invocation: the first spawn
        // emits valid Rust, the second emits malformed. We use a
        // counter file on disk to track invocation count.
        let path_override = install_counting_claude_shim(
            repo_root,
            VALID_RUST_STDOUT,
            MALFORMED_RUST_STDOUT,
        );
        std::env::set_var("PATH", &path_override);
        std::env::set_var("AGENTIC_CACHE", repo_root.join(".agentic-cache"));

        init_repo_and_commit_seed(repo_root);

        let first_sibling = repo_root
            .join("crates/parseable-fixture/tests/first_sibling_valid.rs");
        let second_sibling = repo_root
            .join("crates/parseable-fixture/tests/second_sibling_malformed.rs");
        let evidence_dir = repo_root
            .join("evidence/runs")
            .join(MALFORMED_STORY_ID.to_string());

        let builder = TestBuilder::new(repo_root);
        let err = builder
            .run(MALFORMED_STORY_ID)
            .expect_err("malformed claude output must surface as Err");

        match &err {
            TestBuilderError::ScaffoldParseError { path, stderr } => {
                let p = path.to_string_lossy();
                assert!(
                    p.ends_with("second_sibling_malformed.rs"),
                    "ScaffoldParseError.path must name the offending scaffold; got {p}"
                );
                assert!(
                    !stderr.is_empty(),
                    "ScaffoldParseError.stderr must carry the parser's error message"
                );
            }
            other => panic!(
                "malformed output must surface as TestBuilderError::ScaffoldParseError; got {other:?}"
            ),
        }

        // Roll-back: BOTH scaffolds absent on disk.
        assert!(
            !first_sibling.exists(),
            "ScaffoldParseError must roll back the first sibling too — all-or-nothing"
        );
        assert!(
            !second_sibling.exists(),
            "offending scaffold must not be left on disk"
        );

        // Zero evidence.
        if evidence_dir.exists() {
            let any_jsonl = fs::read_dir(&evidence_dir)
                .expect("read evidence dir")
                .filter_map(|e| e.ok())
                .any(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"));
            assert!(
                !any_jsonl,
                "ScaffoldParseError must write zero evidence"
            );
        }
    }
}

fn seed_story(repo_root: &Path, id: u32, yaml: &str) {
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(stories_dir.join(format!("{id}.yml")), yaml).expect("write fixture");
}

fn materialise_fixture_crate(repo_root: &Path) {
    let crate_root = repo_root.join("crates/parseable-fixture");
    fs::create_dir_all(crate_root.join("src")).expect("fixture src dir");
    fs::create_dir_all(crate_root.join("tests")).expect("fixture tests dir");
    fs::write(
        crate_root.join("Cargo.toml"),
        r#"[package]
name = "parseable-fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write fixture Cargo.toml");
    fs::write(crate_root.join("src/lib.rs"), "pub fn noop() {}\n")
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

/// Install a shim whose first invocation emits `first_stdout` and
/// whose second invocation emits `second_stdout`. Used to drive the
/// "first scaffold fine, second malformed" scenario.
fn install_counting_claude_shim(
    repo_root: &Path,
    first_stdout: &str,
    second_stdout: &str,
) -> String {
    let shim_dir = repo_root.join(".bin");
    fs::create_dir_all(&shim_dir).expect("shim dir");
    let counter_path = shim_dir.join("counter");
    fs::write(&counter_path, "0").expect("init counter");
    let shim_path = shim_dir.join("claude");
    let script = format!(
        "#!/bin/sh\nCOUNTER_PATH='{counter}'\nN=$(cat \"$COUNTER_PATH\")\nN_NEXT=$((N + 1))\necho \"$N_NEXT\" > \"$COUNTER_PATH\"\nif [ \"$N\" = \"0\" ]; then\ncat <<'__AGENTIC_EOF_A__'\n{first}__AGENTIC_EOF_A__\nelse\ncat <<'__AGENTIC_EOF_B__'\n{second}__AGENTIC_EOF_B__\nfi\n",
        counter = counter_path.display(),
        first = first_stdout,
        second = second_stdout
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
