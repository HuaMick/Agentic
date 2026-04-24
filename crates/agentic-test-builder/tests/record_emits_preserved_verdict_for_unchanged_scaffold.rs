//! Story 23 acceptance test: `agentic test-build record` emits a
//! `verdict: "preserved"` entry for a scaffold whose classification
//! under the three-gate rule is PRESERVE, does NOT probe it, and
//! writes the documented preserved-shape row (no `red_path`, no
//! `diagnostic`).
//!
//! Justification (from stories/23.yml acceptance.tests[0]): without
//! this, the CLI would continue to refuse on any scaffold that parses
//! green, and the three-verdict shape named in ADR-0005's 2026-04-24
//! sub-amendment would remain reachable only by test-builder
//! self-authoring the JSONL — the exact fallback the six Phase 0
//! incidents documented.
//!
//! Red today is runtime-red: `TestBuilder::record` always emits
//! `verdict: "red"` and refuses with `ScaffoldNotRed` on any scaffold
//! whose probe comes back green. Under this scaffold's fixture the
//! scaffold probes green and the gates' third signal fails (no story
//! YAML commits after the last evidence row — the story's justification
//! has not moved). Record must therefore classify PRESERVED and emit
//! the preserved-shape row. The current implementation returns
//! `Err(TestBuilderError::ScaffoldNotRed)` before any JSONL is written,
//! so the `.expect("record must succeed ...")` call below panics.
//! Build-rust adds the classifier + the preserved-verdict emission to
//! drive this green.

use std::fs;
use std::path::Path;

use agentic_test_builder::TestBuilder;
use serde_json::json;
use tempfile::TempDir;

const STORY_ID: u32 = 99_023_001;

/// Fixture story: status `under_construction`, one scaffold. We use
/// under_construction so Gate 1 passes; Gate 2 is the one that fails
/// (the story YAML has no commits newer than the last evidence row —
/// we seed both at the same commit).
const FIXTURE_STORY_YAML: &str = r#"id: 99023001
title: "Fixture for story 23 preserved-verdict-for-unchanged-scaffold"

outcome: |
  Fixture used to prove record emits verdict "preserved" when a
  scaffold's classification under the three-gate rule is PRESERVE.

status: under_construction

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-preserve-crate/tests/preserved_scaffold.rs
      justification: |
        Proves the preserved-verdict path: the scaffold on disk is
        unchanged since the last evidence row, the story YAML has no
        commits newer than that row, so the classifier returns
        PRESERVE and record emits verdict "preserved" without probing.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

const FIXTURE_WORKSPACE_CARGO: &str = r#"[workspace]
resolver = "2"
members = ["crates/fixture-preserve-crate"]
"#;

const FIXTURE_CRATE_CARGO: &str = r#"[package]
name = "fixture-preserve-crate"
version = "0.0.1"
edition = "2021"

[lib]
path = "src/lib.rs"
"#;

const FIXTURE_CRATE_LIB: &str = r#"pub fn satisfied() -> u32 {
    1
}
"#;

/// A scaffold whose probe would come back green — the observable its
/// justification named is already satisfied by the crate's current
/// public API. Under the three-gate rule this file must classify as
/// PRESERVE (no YAML commits since last evidence row), so record must
/// NOT probe it at all.
const GREEN_SCAFFOLD_BODY: &str = r#"use fixture_preserve_crate::satisfied;

#[test]
fn preserved_scaffold() {
    assert_eq!(satisfied(), 1);
}
"#;

#[test]
fn record_emits_preserved_verdict_without_probing_when_classification_is_preserve() {
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();

    // Lay down a minimal cargo workspace, the fixture story, and the
    // scaffold file on disk.
    fs::write(repo_root.join("Cargo.toml"), FIXTURE_WORKSPACE_CARGO).expect("ws cargo");
    let crate_root = repo_root.join("crates/fixture-preserve-crate");
    fs::create_dir_all(crate_root.join("src")).expect("crate src");
    fs::create_dir_all(crate_root.join("tests")).expect("crate tests");
    fs::write(crate_root.join("Cargo.toml"), FIXTURE_CRATE_CARGO).expect("crate cargo");
    fs::write(crate_root.join("src/lib.rs"), FIXTURE_CRATE_LIB).expect("crate lib");

    let scaffold_path = crate_root.join("tests/preserved_scaffold.rs");
    fs::write(&scaffold_path, GREEN_SCAFFOLD_BODY).expect("write green scaffold");

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    // Seed evidence from a prior run so the classifier has something to
    // compare the story YAML's git log against. Gate 2 (story YAML
    // newer than this evidence row) must FAIL for this fixture — i.e.
    // no commits touch `stories/<id>.yml` after this row's commit. We
    // achieve that by seeding both the evidence and the story in the
    // same commit below.
    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    fs::create_dir_all(&evidence_dir).expect("evidence dir");
    // `commit` will be backfilled after the seed commit.
    let prior_evidence_path = evidence_dir.join("2026-04-01T00-00-00Z-red.jsonl");
    fs::write(&prior_evidence_path, "{}\n").expect("placeholder prior evidence");

    let head_commit = init_repo_and_commit_seed(repo_root);

    // Overwrite the prior evidence row with the real seed commit so
    // the classifier's git-log-vs-evidence-commit signal can read it.
    let prior_row = json!({
        "run_id": "00000000-0000-4000-8000-000000000001",
        "story_id": STORY_ID,
        "commit": head_commit,
        "timestamp": "2026-04-01T00:00:00Z",
        "verdicts": [
            {
                "file": "crates/fixture-preserve-crate/tests/preserved_scaffold.rs",
                "verdict": "red",
                "red_path": "compile",
                "diagnostic": "error[E0432]: unresolved import (seeded)"
            }
        ]
    });
    fs::write(&prior_evidence_path, format!("{prior_row}\n")).expect("rewrite prior evidence");
    // Stage + commit the rewritten prior evidence so the tree is clean
    // and the YAML's latest commit is no newer than the evidence row
    // (both are HEAD, neither is strictly newer — Gate 2 must resolve
    // to PRESERVE).
    commit_all(repo_root, "backfill prior evidence commit field");

    // Act: record. Classifier sees Gate 1 pass (under_construction)
    // but Gate 2 fail (no story YAML commit newer than the evidence
    // row's commit — both are at HEAD). The scaffold must classify
    // PRESERVE, record must NOT probe it, and the new evidence JSONL
    // row must carry verdict "preserved".
    let builder = TestBuilder::new(repo_root);
    let _outcome = builder.record(STORY_ID).expect(
        "record must succeed (not return ScaffoldNotRed) when a green scaffold's \
                 classification under the three-gate rule is PRESERVE",
    );

    // Assert: a new evidence JSONL was written under evidence/runs/<id>/.
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
    assert_eq!(
        verdicts.len(),
        1,
        "verdicts must carry one entry per acceptance.tests[] entry"
    );
    let verdict = verdicts[0]
        .as_object()
        .expect("verdict entry must be an object");
    assert_eq!(
        verdict.get("verdict").and_then(|v| v.as_str()),
        Some("preserved"),
        "per-verdict value must be \"preserved\" when classification is PRESERVE; \
         got {:?}",
        verdict.get("verdict")
    );
    // Shape: preserved rows carry only `file` + `verdict`. No
    // `red_path`, no `diagnostic` — the sub-amendment spells this out
    // as "omitted keys, not null".
    let mut keys: Vec<&str> = verdict.keys().map(|s| s.as_str()).collect();
    keys.sort();
    assert_eq!(
        keys,
        vec!["file", "verdict"],
        "preserved verdict must carry exactly {{file, verdict}} — no red_path, no diagnostic; \
         got keys {keys:?}"
    );
}

fn init_repo_and_commit_seed(root: &Path) -> String {
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
