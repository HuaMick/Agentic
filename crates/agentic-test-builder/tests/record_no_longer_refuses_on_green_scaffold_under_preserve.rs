//! Story 23 acceptance test: the sub-amendment's regression fence.
//! A scaffold that exists on disk and probes green (cargo check OK,
//! cargo test exits 0) BUT whose classification under the three-gate
//! rule is PRESERVE no longer causes record to return
//! `TestBuilderError::ScaffoldNotRed` — record emits `verdict:
//! "preserved"` for it and exits 0.
//!
//! Justification (from stories/23.yml acceptance.tests[3]): without
//! this, the sub-amendment's "the CLI no longer exits non-zero on the
//! first non-red scaffold when the scaffold legitimately classifies
//! as preserved or re-authored" observable is unpinned, and a future
//! refactor could resurrect the blanket refusal.
//!
//! Red today is runtime-red: current `record` probes every scaffold
//! regardless of classification and returns `ScaffoldNotRed` on any
//! whose probe comes back green. This test's `.expect("record must
//! succeed...")` therefore panics on the current impl.

use std::fs;
use std::path::Path;

use agentic_test_builder::TestBuilder;
use serde_json::json;
use tempfile::TempDir;

const STORY_ID: u32 = 99_023_004;

/// Status `under_construction` so Gate 1 alone doesn't pre-empt the
/// PRESERVE outcome. Gate 2 fails by construction: the evidence
/// row's commit equals the story's head commit so
/// `git log <evidence-commit>..HEAD -- stories/<id>.yml` is empty.
const FIXTURE_STORY_YAML: &str = r#"id: 99023004
title: "Fixture for story 23 record-no-longer-refuses-on-green-scaffold-under-preserve"

outcome: |
  Fixture used to prove record no longer returns ScaffoldNotRed for a
  scaffold that classifies as PRESERVE under the three-gate rule,
  even when that scaffold's probe would come back green.

status: under_construction

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-preserve-green-crate/tests/preserve_green_scaffold.rs
      justification: |
        Proves the sub-amendment's "no blanket refusal on green
        scaffolds classified as preserved" observable: this scaffold
        passes its probe but the three-gate rule classifies it as
        PRESERVE, so record must emit verdict "preserved" and exit 0.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

const FIXTURE_WORKSPACE_CARGO: &str = r#"[workspace]
resolver = "2"
members = ["crates/fixture-preserve-green-crate"]
"#;

const FIXTURE_CRATE_CARGO: &str = r#"[package]
name = "fixture-preserve-green-crate"
version = "0.0.1"
edition = "2021"

[lib]
path = "src/lib.rs"
"#;

const FIXTURE_CRATE_LIB: &str = r#"pub fn satisfied() -> u32 {
    1
}
"#;

/// Green scaffold: its `use` references a symbol the crate DOES
/// declare; `cargo check` succeeds and `cargo test` passes. If the
/// classifier were to probe this scaffold, record's pre-sub-amendment
/// behaviour would return `ScaffoldNotRed`. The three-gate rule must
/// classify PRESERVE first and skip the probe entirely.
const GREEN_SCAFFOLD_BODY: &str = r#"use fixture_preserve_green_crate::satisfied;

#[test]
fn preserve_green_scaffold() {
    assert_eq!(satisfied(), 1);
}
"#;

#[test]
fn record_emits_preserved_not_scaffold_not_red_on_green_classified_preserve() {
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();

    fs::write(repo_root.join("Cargo.toml"), FIXTURE_WORKSPACE_CARGO).expect("ws cargo");
    let crate_root = repo_root.join("crates/fixture-preserve-green-crate");
    fs::create_dir_all(crate_root.join("src")).expect("crate src");
    fs::create_dir_all(crate_root.join("tests")).expect("crate tests");
    fs::write(crate_root.join("Cargo.toml"), FIXTURE_CRATE_CARGO).expect("crate cargo");
    fs::write(crate_root.join("src/lib.rs"), FIXTURE_CRATE_LIB).expect("crate lib");

    let scaffold_path = crate_root.join("tests/preserve_green_scaffold.rs");
    fs::write(&scaffold_path, GREEN_SCAFFOLD_BODY).expect("write green scaffold");

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    // Seed a prior evidence row pointing at the head commit we're
    // about to create — Gate 2 will resolve to PRESERVE (no YAML
    // commits after the evidence row's commit).
    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    fs::create_dir_all(&evidence_dir).expect("evidence dir");
    let prior_evidence_path = evidence_dir.join("2026-04-01T00-00-00Z-red.jsonl");
    fs::write(&prior_evidence_path, "{}\n").expect("placeholder prior evidence");

    let head_commit = init_repo_and_commit_seed(repo_root);

    // Backfill the prior evidence row with the real commit and commit
    // the backfill. Both the story YAML's latest commit and the
    // evidence row's `commit` field now equal HEAD.
    let prior_row = json!({
        "run_id": "00000000-0000-4000-8000-000000000004",
        "story_id": STORY_ID,
        "commit": head_commit,
        "timestamp": "2026-04-01T00:00:00Z",
        "verdicts": [
            {
                "file": "crates/fixture-preserve-green-crate/tests/preserve_green_scaffold.rs",
                "verdict": "red",
                "red_path": "compile",
                "diagnostic": "seeded: was red at some earlier commit"
            }
        ]
    });
    fs::write(&prior_evidence_path, format!("{prior_row}\n")).expect("rewrite prior evidence");
    commit_all(repo_root, "backfill prior evidence commit field");

    // Act: record. The scaffold's probe (were it run) would succeed,
    // but the classifier must resolve PRESERVE first and skip the
    // probe. Record must exit Ok and write a preserved-verdict row.
    let builder = TestBuilder::new(repo_root);
    let outcome = builder.record(STORY_ID);

    // The contract: record must NOT fail with ScaffoldNotRed on this
    // fixture. It must return Ok and write the evidence row.
    assert!(
        outcome.is_ok(),
        "record must exit Ok (not ScaffoldNotRed) when a green scaffold's \
         classification is PRESERVE; got {outcome:?}"
    );

    // The emitted row's single verdict must be "preserved".
    let files: Vec<_> = fs::read_dir(&evidence_dir)
        .expect("read evidence dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.ends_with("-red.jsonl") && !name.starts_with("2026-04-01")
        })
        .collect();
    assert_eq!(
        files.len(),
        1,
        "record must write exactly one new *-red.jsonl file; got {files:?}"
    );
    let body = fs::read_to_string(&files[0]).expect("read evidence");
    let row: serde_json::Value =
        serde_json::from_str(body.trim()).expect("evidence row must be valid JSON");
    let verdicts = row
        .get("verdicts")
        .and_then(|v| v.as_array())
        .expect("verdicts must be an array");
    assert_eq!(verdicts.len(), 1, "exactly one verdict entry");
    let verdict_kind = verdicts[0]
        .as_object()
        .and_then(|o| o.get("verdict"))
        .and_then(|v| v.as_str());
    assert_eq!(
        verdict_kind,
        Some("preserved"),
        "verdict must be \"preserved\" for a green scaffold classified as PRESERVE; \
         got {verdict_kind:?}"
    );
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
    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
    oid.to_string()
}

fn commit_all(root: &Path, msg: &str) -> String {
    let repo = git2::Repository::open(root).expect("open repo");
    let mut index = repo.index().expect("repo index");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("stage all");
    index.write().expect("write index");
    let tree_oid = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_oid).expect("find tree");
    let sig = repo.signature().expect("repo signature");
    let parent = repo
        .head()
        .ok()
        .and_then(|h| h.target())
        .and_then(|oid| repo.find_commit(oid).ok());
    let parents: Vec<&git2::Commit> = parent.as_ref().map(|c| vec![c]).unwrap_or_default();
    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
        .expect("commit");
    oid.to_string()
}
