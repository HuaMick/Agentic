//! Story 23 acceptance test: the new-story loop (every scaffold
//! `red`, one row per invocation) is preserved additively. For a
//! `proposed` story with no prior `evidence/runs/<id>/*.jsonl` and
//! scaffolds that all probe red, record writes one row whose every
//! verdict carries `verdict: "red"` (NOT `"re-authored"`;
//! re-authored requires an earlier evidence row to re-author from).
//!
//! Justification (from stories/23.yml acceptance.tests[4]): without
//! this, the classification rule could accidentally route
//! first-authoring scaffolds through the re-authored path (because
//! Gate 2's "story YAML newer than last evidence row" trivially
//! holds when there is no last evidence row), and the audit trail
//! would lose the "this is the story's first red record" signal
//! from day one of a new story's life.
//!
//! The test asserts the classifier's first-authoring outcome
//! through the typed `TestBuilder::classify_scaffold(&Story, &Path,
//! &git2::Repository) -> ScaffoldClassification` library surface the
//! story promises (canonical 3-arg signature pinned by stories/23.yml
//! `guidance` and the parallel
//! `record_classification_matches_three_gate_rule.rs`), and that
//! `record` on a fresh-checkout-shape fixture stamps every verdict
//! as `red`, never `re-authored`.

use std::fs;
use std::path::Path;

use agentic_story::Story;
use agentic_test_builder::{ScaffoldClassification, TestBuilder};
use tempfile::TempDir;

const STORY_ID: u32 = 99_023_005;

const SCAFFOLD_A: &str = "crates/fixture-new-story-crate/tests/scaffold_a.rs";
const SCAFFOLD_B: &str = "crates/fixture-new-story-crate/tests/scaffold_b.rs";

const FIXTURE_STORY_YAML: &str = r#"id: 99023005
title: "Fixture for story 23 new-story-loop-emits-all-red-verdicts"

outcome: |
  Fixture used to prove first-authoring scaffolds classify as `red`
  (not `re-authored`) when no prior evidence row exists.

status: proposed

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-new-story-crate/tests/scaffold_a.rs
      justification: |
        First-authoring: no prior evidence row exists for this story,
        so the classifier must return FirstAuthoring and record must
        stamp verdict "red", not "re-authored".
    - file: crates/fixture-new-story-crate/tests/scaffold_b.rs
      justification: |
        Second first-authoring scaffold in the same invocation; same
        classification as scaffold_a; same "red" verdict.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

const FIXTURE_WORKSPACE_CARGO: &str = r#"[workspace]
resolver = "2"
members = ["crates/fixture-new-story-crate"]
"#;

const FIXTURE_CRATE_CARGO: &str = r#"[package]
name = "fixture-new-story-crate"
version = "0.0.1"
edition = "2021"

[lib]
path = "src/lib.rs"
"#;

const FIXTURE_CRATE_LIB: &str = r#"// Empty: both scaffolds reference symbols this crate does not
// declare, so each is compile-red.
"#;

const SCAFFOLD_A_BODY: &str = r#"use fixture_new_story_crate::missing_alpha;

#[test]
fn scaffold_a() {
    assert_eq!(missing_alpha(), 0);
}
"#;

const SCAFFOLD_B_BODY: &str = r#"use fixture_new_story_crate::missing_beta;

#[test]
fn scaffold_b() {
    assert_eq!(missing_beta(), 0);
}
"#;

#[test]
fn new_story_first_authoring_classifies_as_red_not_re_authored() {
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();

    fs::write(repo_root.join("Cargo.toml"), FIXTURE_WORKSPACE_CARGO).expect("ws cargo");
    let crate_root = repo_root.join("crates/fixture-new-story-crate");
    fs::create_dir_all(crate_root.join("src")).expect("crate src");
    fs::create_dir_all(crate_root.join("tests")).expect("crate tests");
    fs::write(crate_root.join("Cargo.toml"), FIXTURE_CRATE_CARGO).expect("crate cargo");
    fs::write(crate_root.join("src/lib.rs"), FIXTURE_CRATE_LIB).expect("crate lib");

    fs::write(crate_root.join("tests/scaffold_a.rs"), SCAFFOLD_A_BODY).expect("scaffold a");
    fs::write(crate_root.join("tests/scaffold_b.rs"), SCAFFOLD_B_BODY).expect("scaffold b");

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    // IMPORTANT: no `evidence/runs/<STORY_ID>/` directory is created.
    // The classifier must treat both scaffolds as FirstAuthoring.

    init_repo_and_commit_seed(repo_root);

    // Load the story and open the repo for classify_scaffold's
    // canonical (&Story, &Path, &git2::Repository) signature.
    let story = Story::load(&story_path).expect("load fixture story");
    let repo = git2::Repository::open(repo_root).expect("open repo");

    // Act + assert 1: classifier surface. Both scaffolds must be
    // classified FirstAuthoring — not ReAuthor — because no prior
    // `evidence/runs/<STORY_ID>/` directory exists for this fixture.
    let builder = TestBuilder::new(repo_root);
    let c_a = builder.classify_scaffold(&story, Path::new(SCAFFOLD_A), &repo);
    let c_b = builder.classify_scaffold(&story, Path::new(SCAFFOLD_B), &repo);
    assert!(
        matches!(c_a, ScaffoldClassification::FirstAuthoring),
        "scaffold_a must classify FirstAuthoring when no prior evidence row exists; \
         got {c_a:?}"
    );
    assert!(
        matches!(c_b, ScaffoldClassification::FirstAuthoring),
        "scaffold_b must classify FirstAuthoring when no prior evidence row exists; \
         got {c_b:?}"
    );

    // Act + assert 2: record emits verdict "red" for both, never
    // "re-authored".
    let _outcome = builder.record(STORY_ID).expect("record must succeed");

    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    let files: Vec<_> = fs::read_dir(&evidence_dir)
        .expect("evidence dir must exist after record")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with("-red.jsonl"))
        .collect();
    assert_eq!(
        files.len(),
        1,
        "record must write exactly one *-red.jsonl file; got {files:?}"
    );
    let body = fs::read_to_string(&files[0]).expect("read evidence");
    let row: serde_json::Value =
        serde_json::from_str(body.trim()).expect("evidence row must be valid JSON");
    let verdicts = row
        .get("verdicts")
        .and_then(|v| v.as_array())
        .expect("verdicts array");
    assert_eq!(verdicts.len(), 2, "two verdict entries");
    for (i, v) in verdicts.iter().enumerate() {
        let verdict = v
            .get("verdict")
            .and_then(|x| x.as_str())
            .unwrap_or_default();
        assert_eq!(
            verdict, "red",
            "first-authoring entry {i} must carry verdict \"red\", not \"re-authored\"; \
             got {verdict:?}"
        );
        assert!(
            v.get("red_path")
                .and_then(|x| x.as_str())
                .map(|p| matches!(p, "compile" | "runtime"))
                .unwrap_or(false),
            "entry {i} must carry a valid red_path"
        );
        assert!(
            !v.get("diagnostic")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim()
                .is_empty(),
            "entry {i} must carry a non-empty diagnostic"
        );
    }
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
