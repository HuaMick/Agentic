//! Story 23 acceptance test: `agentic test-build record` emits a
//! `verdict: "re-authored"` entry for a scaffold when all three
//! ADR-0005 amendment gates pass (story status `under_construction`,
//! story YAML has commits newer than the last evidence row, tree is
//! clean), and the scaffold probes red against the amended
//! justification.
//!
//! Justification (from stories/23.yml acceptance.tests[1]): without
//! this, the RE-AUTHORED distinction collapses into plain `"red"` at
//! the CLI boundary, and the audit-trail value the sub-amendment
//! grants ("which verdicts in this row were re-authoring events vs
//! first-authoring?") is lost.
//!
//! Red today is runtime-red: `TestBuilder::record` always stamps
//! `verdict: "red"` regardless of classification. The assertion
//! below — verdict must be `"re-authored"` — fails on the current
//! implementation.

use std::fs;
use std::path::Path;

use agentic_test_builder::TestBuilder;
use serde_json::json;
use tempfile::TempDir;

const STORY_ID: u32 = 99_023_002;

/// Story YAML v1: the version the prior evidence row was recorded
/// against. The scaffold on disk was red against THIS version.
const FIXTURE_STORY_YAML_V1: &str = r#"id: 99023002
title: "Fixture for story 23 re-authored-verdict-when-three-gates-pass"

outcome: |
  Fixture used to prove record emits verdict "re-authored" when the
  three-gate rule's signals all fire and the scaffold probes red.

status: under_construction

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-reauthor-crate/tests/reauthored_scaffold.rs
      justification: |
        V1 justification: proves the scaffold was red at v1.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

/// Story YAML v2: the amended justification the scaffold must now
/// re-prove red against. A story-writer edit moved the contract.
const FIXTURE_STORY_YAML_V2: &str = r#"id: 99023002
title: "Fixture for story 23 re-authored-verdict-when-three-gates-pass"

outcome: |
  Fixture used to prove record emits verdict "re-authored" when the
  three-gate rule's signals all fire and the scaffold probes red.

status: under_construction

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-reauthor-crate/tests/reauthored_scaffold.rs
      justification: |
        V2 justification (AMENDED): the observable has tightened since
        v1; the scaffold must re-prove red against the new assertion.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

const FIXTURE_WORKSPACE_CARGO: &str = r#"[workspace]
resolver = "2"
members = ["crates/fixture-reauthor-crate"]
"#;

const FIXTURE_CRATE_CARGO: &str = r#"[package]
name = "fixture-reauthor-crate"
version = "0.0.1"
edition = "2021"

[lib]
path = "src/lib.rs"
"#;

const FIXTURE_CRATE_LIB: &str = r#"// Empty: the scaffold references a symbol this crate does not
// declare, so `cargo check` fails compile-red. After the amendment
// the symbol is still missing — the scaffold continues to probe red
// against the amended justification.
"#;

/// Scaffold body: compile-red both at v1 and v2. The amendment does
/// not need to change the body for this fixture; the classifier's job
/// is to notice the story YAML moved between v1 and v2.
const RED_SCAFFOLD_BODY: &str = r#"use fixture_reauthor_crate::does_not_exist;

#[test]
fn reauthored_scaffold() {
    assert_eq!(does_not_exist(), 0);
}
"#;

#[test]
fn record_emits_re_authored_verdict_when_three_gates_all_fire() {
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();

    // Lay down workspace + crate + scaffold.
    fs::write(repo_root.join("Cargo.toml"), FIXTURE_WORKSPACE_CARGO).expect("ws cargo");
    let crate_root = repo_root.join("crates/fixture-reauthor-crate");
    fs::create_dir_all(crate_root.join("src")).expect("crate src");
    fs::create_dir_all(crate_root.join("tests")).expect("crate tests");
    fs::write(crate_root.join("Cargo.toml"), FIXTURE_CRATE_CARGO).expect("crate cargo");
    fs::write(crate_root.join("src/lib.rs"), FIXTURE_CRATE_LIB).expect("crate lib");

    let scaffold_path = crate_root.join("tests/reauthored_scaffold.rs");
    fs::write(&scaffold_path, RED_SCAFFOLD_BODY).expect("write red scaffold");

    // Story v1.
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML_V1).expect("write story v1");

    // Commit 1: workspace + crate + scaffold + story v1.
    let v1_commit = init_repo_and_commit_seed(repo_root);

    // Commit 2: seed prior evidence against v1 (Gate 2's "most recent
    // evidence row" — its `commit` field points at v1).
    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    fs::create_dir_all(&evidence_dir).expect("evidence dir");
    let prior_evidence_path = evidence_dir.join("2026-04-01T00-00-00Z-red.jsonl");
    let prior_row = json!({
        "run_id": "00000000-0000-4000-8000-000000000002",
        "story_id": STORY_ID,
        "commit": v1_commit,
        "timestamp": "2026-04-01T00:00:00Z",
        "verdicts": [
            {
                "file": "crates/fixture-reauthor-crate/tests/reauthored_scaffold.rs",
                "verdict": "red",
                "red_path": "compile",
                "diagnostic": "error[E0432]: unresolved import fixture_reauthor_crate::does_not_exist"
            }
        ]
    });
    fs::write(&prior_evidence_path, format!("{prior_row}\n")).expect("write prior evidence");
    commit_all(repo_root, "seed prior evidence at v1");

    // Commit 3: story-writer amends the story YAML to v2. This is the
    // commit Gate 2 keys off — `git log ... <v1_commit>..HEAD --
    // stories/<id>.yml` must return exactly this commit.
    fs::write(&story_path, FIXTURE_STORY_YAML_V2).expect("write story v2");
    commit_all(repo_root, "amend story 23 fixture to v2");

    // Act: record. Gate 1 passes (under_construction), Gate 2 passes
    // (story YAML commit newer than evidence row's commit), Gate 3
    // passes (tree is clean after the v2 commit). The scaffold probes
    // red. Verdict must be "re-authored".
    let builder = TestBuilder::new(repo_root);
    let _outcome = builder
        .record(STORY_ID)
        .expect("record must succeed on a clean tree when all three gates pass");

    // Find the new evidence file (the one we seeded starts with
    // 2026-04-01; the fresh one carries a later timestamp).
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
    assert_eq!(verdicts.len(), 1, "verdicts must have one entry");
    let verdict = verdicts[0]
        .as_object()
        .expect("verdict entry must be an object");
    assert_eq!(
        verdict.get("verdict").and_then(|v| v.as_str()),
        Some("re-authored"),
        "per-verdict value must be \"re-authored\" when all three gates fire; \
         got {:?}",
        verdict.get("verdict")
    );

    // Shape: re-authored rows carry {file, verdict, red_path, diagnostic}.
    let red_path = verdict
        .get("red_path")
        .and_then(|v| v.as_str())
        .expect("re-authored verdict must carry `red_path`");
    assert!(
        matches!(red_path, "compile" | "runtime"),
        "red_path must be 'compile' or 'runtime'; got {red_path:?}"
    );
    let diagnostic = verdict
        .get("diagnostic")
        .and_then(|v| v.as_str())
        .expect("re-authored verdict must carry `diagnostic`");
    assert!(
        !diagnostic.trim().is_empty(),
        "diagnostic must be a non-empty probe-captured string"
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
