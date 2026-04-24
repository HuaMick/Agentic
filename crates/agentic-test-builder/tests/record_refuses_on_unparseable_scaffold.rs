//! Story 15 acceptance test: `record` refuses when a scaffold on disk
//! does not parse as a Rust source file (truncated, prose-mixed, a
//! partially-written file an aborted user edit left behind). The
//! refusal is typed `ScaffoldParseError` naming the path and the
//! parser's error message.
//!
//! Justification (from stories/15.yml acceptance.tests[4]): without
//! this, an unparseable scaffold would surface downstream as a
//! workspace-wide `cargo check` failure attributed to the wrong
//! cause, and the evidence row would either not write or write with
//! a diagnostic from the wrong source.
//!
//! Red today is compile-red: `TestBuilder::record` and the
//! `ScaffoldParseError { file, parse_error }` variant are story-15
//! additions that do not exist yet.

use std::fs;
use std::path::Path;

use agentic_test_builder::{TestBuilder, TestBuilderError};
use tempfile::TempDir;

const STORY_ID: u32 = 99_015_005;

const FIXTURE_YAML: &str = r#"id: 99015005
title: "Fixture for story 15 record-refuses-on-unparseable-scaffold"

outcome: |
  Fixture used to prove record refuses with ScaffoldParseError when a
  scaffold on disk does not parse as a Rust source file.

status: proposed

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-crate/tests/unparseable.rs
      justification: |
        Proves record's syntactic gate: a scaffold whose bytes are
        not a parseable Rust source file surfaces as
        ScaffoldParseError naming the offending path and the
        parser's error, not as a confused cargo check failure.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

/// Byte sequence that `syn::parse_file` rejects: an unclosed brace and
/// prose mixed in where code is expected. This mirrors the
/// justification's "partially-written file left behind by an aborted
/// user edit" shape.
const UNPARSEABLE_SCAFFOLD_BODY: &str = r#"//! User scratch notes — TODO finish this test.

fn missing_close_brace() {
    // this is not valid Rust:
    let x = ;
    assert!(x == 1
    // no closing brace, no semicolon terminator
"#;

#[test]
fn record_refuses_with_scaffold_parse_error_naming_path_and_parser_error() {
    // Pre-condition on the scaffold body: it MUST actually fail to
    // parse via `syn::parse_file`. If this assertion ever starts
    // failing, the fixture needs regenerating — and we refuse to
    // proceed silently.
    assert!(
        syn::parse_file(UNPARSEABLE_SCAFFOLD_BODY).is_err(),
        "fixture invariant: the unparseable scaffold body must actually \
         fail syn::parse_file so the test hits the code path it claims to"
    );

    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    let scaffold_path = repo_root.join("crates/fixture-crate/tests/unparseable.rs");
    fs::create_dir_all(scaffold_path.parent().unwrap()).expect("tests dir");
    fs::write(&scaffold_path, UNPARSEABLE_SCAFFOLD_BODY).expect("write unparseable scaffold");

    init_repo_and_commit_seed(repo_root);

    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    let before_listing = listing(repo_root);

    let builder = TestBuilder::new(repo_root);
    let result = builder.record(STORY_ID);

    match result {
        Err(TestBuilderError::ScaffoldParseError { file, parse_error }) => {
            assert_eq!(
                file,
                scaffold_path,
                "ScaffoldParseError must name the unparseable scaffold path; got {}",
                file.display()
            );
            assert!(
                !parse_error.trim().is_empty(),
                "ScaffoldParseError.parse_error must carry the parser's error message, not be empty"
            );
        }
        other => panic!(
            "record must return ScaffoldParseError naming {}; got {:?}",
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
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
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
