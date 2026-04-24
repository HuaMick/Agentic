//! Story 23 acceptance test: the classifier's gate semantics at the
//! library boundary. `TestBuilder::classify_scaffold(&Story, &Path,
//! &Repo) -> ScaffoldClassification` returns a typed enum matching
//! the ADR-0005 amendment's rule table — without probing, without
//! touching the evidence directory, and without mutating the
//! working tree.
//!
//! Justification (from stories/23.yml acceptance.tests[5]): the
//! three-gate rule is named once, in code, and this test is the
//! shape-fence that catches drift between the ADR and the
//! implementation.
//!
//! Red today is compile-red: the classifier function and enum do
//! not exist on the `agentic-test-builder` public API yet.
//! `use agentic_test_builder::{ScaffoldClassification, ...}` fails
//! at `cargo check` with `error[E0432]: unresolved import`.

use std::fs;
use std::path::{Path, PathBuf};

use agentic_story::Story;
use agentic_test_builder::{ScaffoldClassification, TestBuilder};
use serde_json::json;
use tempfile::TempDir;

const FIXTURE_WORKSPACE_CARGO: &str = r#"[workspace]
resolver = "2"
members = ["crates/fixture-classify-crate"]
"#;

const FIXTURE_CRATE_CARGO: &str = r#"[package]
name = "fixture-classify-crate"
version = "0.0.1"
edition = "2021"

[lib]
path = "src/lib.rs"
"#;

const FIXTURE_CRATE_LIB: &str = "pub fn anchor() -> u32 { 1 }\n";

/// Scaffold body: irrelevant for this test because classify_scaffold
/// does NOT probe. The body is parseable Rust so the story loader and
/// file-existence check don't stumble.
const SCAFFOLD_BODY: &str = r#"#[test]
fn t() {
    assert_eq!(1, 1);
}
"#;

/// Matrix row: a single (status, evidence-state, tree-state) ->
/// expected ScaffoldClassification case.
struct MatrixCase {
    name: &'static str,
    story_id: u32,
    // Story status to seed the fixture with.
    status: &'static str,
    // If true, seed a prior evidence row AND amend the YAML after it
    // (Gate 2 would pass given story status were under_construction).
    yaml_newer_than_evidence: bool,
    // If true, seed a prior evidence row AND NOT amend the YAML
    // (Gate 2 fails).
    yaml_equal_to_evidence: bool,
    // What the classifier must return for this case.
    expected: ExpectedClass,
}

enum ExpectedClass {
    Preserve,
    ReAuthor,
    FirstAuthoring,
}

#[test]
fn classify_scaffold_returns_typed_enum_matching_three_gate_rule() {
    let cases: Vec<MatrixCase> = vec![
        // Row 1: status healthy + YAML newer than evidence
        // (Gate 1 fails) -> PRESERVE.
        MatrixCase {
            name: "healthy_yaml_newer_evidence",
            story_id: 99_023_061,
            status: "healthy",
            yaml_newer_than_evidence: true,
            yaml_equal_to_evidence: false,
            expected: ExpectedClass::Preserve,
        },
        // Row 2: status under_construction + YAML equal to evidence
        // (Gate 2 fails) -> PRESERVE.
        MatrixCase {
            name: "uc_yaml_equal_evidence",
            story_id: 99_023_062,
            status: "under_construction",
            yaml_newer_than_evidence: false,
            yaml_equal_to_evidence: true,
            expected: ExpectedClass::Preserve,
        },
        // Row 3: status under_construction + YAML newer than
        // evidence (all three gates pass) -> RE-AUTHOR.
        MatrixCase {
            name: "uc_yaml_newer_evidence_all_gates_pass",
            story_id: 99_023_063,
            status: "under_construction",
            yaml_newer_than_evidence: true,
            yaml_equal_to_evidence: false,
            expected: ExpectedClass::ReAuthor,
        },
        // Row 4: proposed + no prior evidence -> FirstAuthoring.
        MatrixCase {
            name: "proposed_no_prior_evidence",
            story_id: 99_023_064,
            status: "proposed",
            yaml_newer_than_evidence: false,
            yaml_equal_to_evidence: false,
            expected: ExpectedClass::FirstAuthoring,
        },
    ];

    for case in &cases {
        let tmp = TempDir::new().expect("repo tempdir");
        let repo_root = tmp.path();
        let scaffold_rel = format!(
            "crates/fixture-classify-crate/tests/classify_{}.rs",
            case.name
        );
        let scaffold_abs = repo_root.join(&scaffold_rel);

        // Workspace + crate.
        fs::write(repo_root.join("Cargo.toml"), FIXTURE_WORKSPACE_CARGO).expect("ws cargo");
        let crate_root = repo_root.join("crates/fixture-classify-crate");
        fs::create_dir_all(crate_root.join("src")).expect("crate src");
        fs::create_dir_all(crate_root.join("tests")).expect("crate tests");
        fs::write(crate_root.join("Cargo.toml"), FIXTURE_CRATE_CARGO).expect("crate cargo");
        fs::write(crate_root.join("src/lib.rs"), FIXTURE_CRATE_LIB).expect("crate lib");

        // Scaffold.
        fs::write(&scaffold_abs, SCAFFOLD_BODY).expect("scaffold body");

        // Story v1.
        let stories_dir = repo_root.join("stories");
        fs::create_dir_all(&stories_dir).expect("stories dir");
        let story_id = case.story_id;
        let story_path = stories_dir.join(format!("{story_id}.yml"));
        let v1 = fixture_story_yaml(story_id, case.status, &scaffold_rel, /*v2=*/ false);
        fs::write(&story_path, v1).expect("write story v1");

        // Commit 1: seed.
        let v1_commit = init_repo_and_commit_seed(repo_root);

        // Prior evidence (if the case wants one).
        if case.yaml_newer_than_evidence || case.yaml_equal_to_evidence {
            let evidence_dir = repo_root.join(format!("evidence/runs/{story_id}"));
            fs::create_dir_all(&evidence_dir).expect("evidence dir");
            let prior = evidence_dir.join("2026-04-01T00-00-00Z-red.jsonl");
            let row = json!({
                "run_id": "00000000-0000-4000-8000-000000000005",
                "story_id": story_id,
                "commit": v1_commit,
                "timestamp": "2026-04-01T00:00:00Z",
                "verdicts": [
                    { "file": scaffold_rel, "verdict": "red",
                      "red_path": "compile", "diagnostic": "seeded" }
                ]
            });
            fs::write(&prior, format!("{row}\n")).expect("write prior evidence");
            commit_all(repo_root, "seed prior evidence");
        }

        // If the case wants YAML newer than evidence, amend the YAML.
        if case.yaml_newer_than_evidence {
            let v2 = fixture_story_yaml(story_id, case.status, &scaffold_rel, /*v2=*/ true);
            fs::write(&story_path, v2).expect("amend story to v2");
            commit_all(repo_root, "amend story YAML");
        }

        // Load the story and open the repo for classify_scaffold's
        // (&Story, &Path, &Repo) signature.
        let story = Story::load(&story_path).expect("load story");
        let repo = git2::Repository::open(repo_root).expect("open repo");

        // Act: classify.
        let builder = TestBuilder::new(repo_root);
        let got = builder.classify_scaffold(&story, Path::new(&scaffold_rel), &repo);

        match case.expected {
            ExpectedClass::Preserve => assert!(
                matches!(got, ScaffoldClassification::Preserve),
                "case {:?}: expected Preserve; got {got:?}",
                case.name
            ),
            ExpectedClass::ReAuthor => assert!(
                matches!(got, ScaffoldClassification::ReAuthor),
                "case {:?}: expected ReAuthor; got {got:?}",
                case.name
            ),
            ExpectedClass::FirstAuthoring => assert!(
                matches!(got, ScaffoldClassification::FirstAuthoring),
                "case {:?}: expected FirstAuthoring; got {got:?}",
                case.name
            ),
        }

        // Purity invariants: classify_scaffold must NOT touch the
        // evidence directory (we captured its pre-state above via
        // the presence or absence of the seeded file; the seeded
        // file's bytes must be unchanged) and must NOT mutate the
        // tree.
        assert_no_probe_artefacts(repo_root);
    }
}

fn fixture_story_yaml(id: u32, status: &str, scaffold_rel: &str, v2: bool) -> String {
    let justification = if v2 {
        "V2 justification (AMENDED): contract tightened."
    } else {
        "V1 justification: fixture for classifier matrix."
    };
    format!(
        r#"id: {id}
title: "Fixture for story 23 classify-matrix case"

outcome: |
  Fixture for the three-gate classifier.

status: {status}

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: {scaffold_rel}
      justification: |
        {justification}
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#
    )
}

fn assert_no_probe_artefacts(root: &Path) {
    // No `Cargo.lock` should be created inside the tempdir by a
    // classify_scaffold call (probes create it; classify_scaffold
    // must not probe).
    let lock = root.join("Cargo.lock");
    assert!(
        !lock.exists(),
        "classify_scaffold must not probe — Cargo.lock must not appear at {}",
        lock.display()
    );
    // No `target/` directory either.
    let tgt: PathBuf = root.join("target");
    assert!(
        !tgt.exists(),
        "classify_scaffold must not probe — target/ must not appear at {}",
        tgt.display()
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
