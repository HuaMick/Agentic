//! Story 23 acceptance test: record writes exactly one JSONL row
//! containing a mixed set of verdicts (`red`, `preserved`,
//! `re-authored`) in declaration order, and fails atomically — if any
//! per-scaffold classification or probe throws mid-run, no partial
//! JSONL row is written and no `evidence/runs/<id>/` directory is
//! created.
//!
//! Justification (from stories/23.yml acceptance.tests[2]): without
//! this, a partial-amendment pipeline could write a row with only the
//! `red` scaffolds and silently drop the `preserved` and
//! `re-authored` entries — the evidence chain would lie about which
//! scaffolds the CLI actually considered at that commit, and the
//! per-commit-atomicity invariant the amendment pinned would regress.
//!
//! Red today is runtime-red: the current record always stamps
//! `verdict: "red"` for every entry (never `"preserved"`, never
//! `"re-authored"`), and refuses with `ScaffoldNotRed` on any green
//! scaffold, so the call returns Err and no mixed row is ever
//! written.

use std::fs;
use std::path::Path;

use agentic_test_builder::TestBuilder;
use serde_json::json;
use tempfile::TempDir;

const STORY_ID: u32 = 99_023_003;

const RED_FILE: &str = "crates/fixture-mixed-crate/tests/red_scaffold.rs";
const PRESERVED_FILE: &str = "crates/fixture-mixed-crate/tests/preserved_scaffold.rs";
const REAUTHORED_FILE: &str = "crates/fixture-mixed-crate/tests/reauthored_scaffold.rs";

/// Story v1: three scaffolds, all red at seed time. Same declaration
/// order as the evidence row must produce.
const FIXTURE_STORY_YAML_V1: &str = r#"id: 99023003
title: "Fixture for story 23 mixed-verdicts-in-one-atomic-row"

outcome: |
  Fixture used to prove record writes one atomic row per invocation
  carrying one verdict per scaffold, and that declaration order is
  preserved across mixed verdicts.

status: under_construction

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-mixed-crate/tests/red_scaffold.rs
      justification: |
        First authoring: the scaffold has no prior evidence row and
        probes red at this commit.
    - file: crates/fixture-mixed-crate/tests/preserved_scaffold.rs
      justification: |
        Present since the last evidence row and unchanged since.
        Classifies as PRESERVE.
    - file: crates/fixture-mixed-crate/tests/reauthored_scaffold.rs
      justification: |
        V1 justification: scaffold red at v1.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

/// Story v2: only the re-authored scaffold's justification moves. The
/// preserved scaffold stays byte-identical in justification; the new
/// red scaffold entry is also added in v2 (first-authoring). The
/// amendment commit is what Gate 2 keys off for the re-authored
/// scaffold.
const FIXTURE_STORY_YAML_V2: &str = r#"id: 99023003
title: "Fixture for story 23 mixed-verdicts-in-one-atomic-row"

outcome: |
  Fixture used to prove record writes one atomic row per invocation
  carrying one verdict per scaffold, and that declaration order is
  preserved across mixed verdicts.

status: under_construction

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-mixed-crate/tests/red_scaffold.rs
      justification: |
        First authoring at v2: the scaffold has no prior evidence
        row and probes red at this commit (the first evidence write
        for this file).
    - file: crates/fixture-mixed-crate/tests/preserved_scaffold.rs
      justification: |
        Present since the last evidence row and unchanged since.
        Classifies as PRESERVE.
    - file: crates/fixture-mixed-crate/tests/reauthored_scaffold.rs
      justification: |
        V2 justification (AMENDED): the observable has tightened;
        the scaffold re-probes red against the amended contract.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

const FIXTURE_WORKSPACE_CARGO: &str = r#"[workspace]
resolver = "2"
members = ["crates/fixture-mixed-crate"]
"#;

const FIXTURE_CRATE_CARGO: &str = r#"[package]
name = "fixture-mixed-crate"
version = "0.0.1"
edition = "2021"

[lib]
path = "src/lib.rs"
"#;

const FIXTURE_CRATE_LIB: &str = r#"// The crate leaves the red scaffolds' referenced symbols
// undeclared so both red scaffolds remain compile-red at probe
// time. The preserved scaffold depends only on `known_symbol` which
// IS declared below so, had the classifier probed it, it would come
// back green — proving the preserved classification skipped the
// probe.
pub fn known_symbol() -> u32 {
    1
}
"#;

const RED_SCAFFOLD_BODY: &str = r#"use fixture_mixed_crate::red_missing_symbol;

#[test]
fn red_scaffold() {
    assert_eq!(red_missing_symbol(), 0);
}
"#;

/// Preserved scaffold body: uses a symbol the crate DOES declare, so
/// if any probe runs it would come back green. The classifier must
/// not probe this entry — PRESERVE skips the probe entirely.
const PRESERVED_SCAFFOLD_BODY: &str = r#"use fixture_mixed_crate::known_symbol;

#[test]
fn preserved_scaffold() {
    assert_eq!(known_symbol(), 1);
}
"#;

const REAUTHORED_SCAFFOLD_BODY: &str = r#"use fixture_mixed_crate::reauthor_missing_symbol;

#[test]
fn reauthored_scaffold() {
    assert_eq!(reauthor_missing_symbol(), 0);
}
"#;

#[test]
fn record_writes_one_row_with_mixed_verdicts_in_declaration_order() {
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();

    // Workspace + crate.
    fs::write(repo_root.join("Cargo.toml"), FIXTURE_WORKSPACE_CARGO).expect("ws cargo");
    let crate_root = repo_root.join("crates/fixture-mixed-crate");
    fs::create_dir_all(crate_root.join("src")).expect("crate src");
    fs::create_dir_all(crate_root.join("tests")).expect("crate tests");
    fs::write(crate_root.join("Cargo.toml"), FIXTURE_CRATE_CARGO).expect("crate cargo");
    fs::write(crate_root.join("src/lib.rs"), FIXTURE_CRATE_LIB).expect("crate lib");

    // Three scaffolds on disk. The preserved one is there from the
    // v1 era — its body is unchanged through v2.
    fs::write(crate_root.join("tests/red_scaffold.rs"), RED_SCAFFOLD_BODY).expect("red scaffold");
    fs::write(
        crate_root.join("tests/preserved_scaffold.rs"),
        PRESERVED_SCAFFOLD_BODY,
    )
    .expect("preserved scaffold");
    fs::write(
        crate_root.join("tests/reauthored_scaffold.rs"),
        REAUTHORED_SCAFFOLD_BODY,
    )
    .expect("reauthored scaffold");

    // Story v1.
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML_V1).expect("write story v1");

    // Commit 1: seed.
    let v1_commit = init_repo_and_commit_seed(repo_root);

    // Commit 2: seed prior evidence. All three scaffolds were red at
    // v1. The `commit` field points at v1_commit.
    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    fs::create_dir_all(&evidence_dir).expect("evidence dir");
    let prior_evidence_path = evidence_dir.join("2026-04-01T00-00-00Z-red.jsonl");
    let prior_row = json!({
        "run_id": "00000000-0000-4000-8000-000000000003",
        "story_id": STORY_ID,
        "commit": v1_commit,
        "timestamp": "2026-04-01T00:00:00Z",
        "verdicts": [
            {
                "file": RED_FILE,
                "verdict": "red",
                "red_path": "compile",
                "diagnostic": "seeded: red at v1"
            },
            {
                "file": PRESERVED_FILE,
                "verdict": "red",
                "red_path": "compile",
                "diagnostic": "seeded: red at v1"
            },
            {
                "file": REAUTHORED_FILE,
                "verdict": "red",
                "red_path": "compile",
                "diagnostic": "seeded: red at v1"
            }
        ]
    });
    fs::write(&prior_evidence_path, format!("{prior_row}\n")).expect("write prior evidence");
    commit_all(repo_root, "seed prior evidence at v1");

    // Commit 3: amend story to v2 (only the third justification
    // changed; the second did NOT). Gate 2 for the re-authored
    // scaffold fires against THIS commit.
    fs::write(&story_path, FIXTURE_STORY_YAML_V2).expect("write story v2");
    commit_all(repo_root, "amend story 23 fixture to v2");

    // Act: record on a clean tree. Classification:
    //   - red_scaffold:          first-authoring -> "red"
    //     (the file IS in the prior evidence row, but its
    //     justification text changed — per story 23 guidance, only
    //     the re-authored path carries "re-authored"; first-authoring
    //     here is interpreted strictly: a scaffold whose body the
    //     classifier treats as freshly written against amended
    //     justification may ALSO be called "red"). Implementations
    //     are free to choose "red" vs "re-authored" for the first
    //     entry per the sub-amendment's guidance. This scaffold asserts
    //     ONLY on the second and third entries, which are the ones
    //     whose classification is load-bearing.
    //   - preserved_scaffold:    PRESERVE       -> "preserved"
    //   - reauthored_scaffold:   RE-AUTHOR      -> "re-authored"
    let builder = TestBuilder::new(repo_root);
    let _outcome = builder
        .record(STORY_ID)
        .expect("record must succeed on a clean tree with mixed-classification scaffolds");

    // Locate the new evidence file.
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
        .expect("verdicts must be a JSON array");
    assert_eq!(
        verdicts.len(),
        3,
        "verdicts must carry exactly three entries in declaration order; got {}",
        verdicts.len()
    );

    // Declaration order is the story's `acceptance.tests[]` order.
    let file0 = verdicts[0]
        .get("file")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let file1 = verdicts[1]
        .get("file")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let file2 = verdicts[2]
        .get("file")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(
        file0, RED_FILE,
        "declaration order: entry 0 must be red scaffold"
    );
    assert_eq!(
        file1, PRESERVED_FILE,
        "declaration order: entry 1 must be preserved scaffold"
    );
    assert_eq!(
        file2, REAUTHORED_FILE,
        "declaration order: entry 2 must be re-authored scaffold"
    );

    // Entry 1 (preserved): verdict "preserved", shape {file, verdict} only.
    let v1 = verdicts[1].as_object().expect("preserved entry is object");
    assert_eq!(
        v1.get("verdict").and_then(|v| v.as_str()),
        Some("preserved"),
        "entry 1 verdict must be \"preserved\"; got {:?}",
        v1.get("verdict")
    );
    let mut keys1: Vec<&str> = v1.keys().map(|s| s.as_str()).collect();
    keys1.sort();
    assert_eq!(
        keys1,
        vec!["file", "verdict"],
        "preserved entry must carry exactly {{file, verdict}}; got {keys1:?}"
    );

    // Entry 2 (re-authored): verdict "re-authored", shape {file,
    // verdict, red_path, diagnostic}.
    let v2 = verdicts[2]
        .as_object()
        .expect("re-authored entry is object");
    assert_eq!(
        v2.get("verdict").and_then(|v| v.as_str()),
        Some("re-authored"),
        "entry 2 verdict must be \"re-authored\"; got {:?}",
        v2.get("verdict")
    );
    let red_path = v2
        .get("red_path")
        .and_then(|v| v.as_str())
        .expect("re-authored entry must carry `red_path`");
    assert!(
        matches!(red_path, "compile" | "runtime"),
        "red_path must be 'compile' or 'runtime'"
    );
    let diagnostic = v2
        .get("diagnostic")
        .and_then(|v| v.as_str())
        .expect("re-authored entry must carry `diagnostic`");
    assert!(
        !diagnostic.trim().is_empty(),
        "re-authored diagnostic must be non-empty"
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
