//! Story 23 acceptance test: the three-verdict contract reaches the
//! operator through the compiled `agentic` binary.
//!
//! Justification (from stories/23.yml acceptance.tests[6]): running
//! `agentic test-build record <fixture-id>` against a fixture story
//! whose scaffolds classify as a mix of `red`, `preserved`, and
//! `re-authored` exits 0, writes the mixed-verdict evidence JSONL,
//! and names each recorded file and its verdict in stdout. Without
//! this, the library-level claims are library-level claims only —
//! the argv-to-subcommand wire could drop the classification output,
//! silently map `preserved` to `red` on write, or misroute the
//! evidence path, and the operator running the CLI against a real
//! amended story would not notice until the next UAT pass tripped
//! over the bad row.
//!
//! Red today is runtime-red: the compiled binary's `record` path
//! always stamps `verdict: "red"` and refuses with `ScaffoldNotRed`
//! (exit 2) on any scaffold whose probe comes back green. The
//! preserved-classification fixture entry has a green probe, so the
//! binary exits 2 on the current impl and never writes a row. The
//! `.assert()` for exit code 0 below therefore panics.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use serde_json::json;
use tempfile::TempDir;

const STORY_ID: u32 = 99_023_007;

const RED_FILE: &str = "crates/fixture-cli-mixed-crate/tests/red_scaffold.rs";
const PRESERVED_FILE: &str = "crates/fixture-cli-mixed-crate/tests/preserved_scaffold.rs";
const REAUTHORED_FILE: &str = "crates/fixture-cli-mixed-crate/tests/reauthored_scaffold.rs";

const FIXTURE_STORY_YAML_V1: &str = r#"id: 99023007
title: "Fixture for story 23 CLI mixed-verdicts roundtrip"

outcome: |
  Fixture used to prove the CLI binary emits the three-verdict shape
  on a mixed-amendment pipeline.

status: under_construction

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-cli-mixed-crate/tests/red_scaffold.rs
      justification: |
        First-authoring at v1: probes red and must record red.
    - file: crates/fixture-cli-mixed-crate/tests/preserved_scaffold.rs
      justification: |
        Present since the last evidence row; classifies PRESERVE at
        v1 because the story YAML has not moved past v1.
    - file: crates/fixture-cli-mixed-crate/tests/reauthored_scaffold.rs
      justification: |
        V1 justification: scaffold red at v1.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

const FIXTURE_STORY_YAML_V2: &str = r#"id: 99023007
title: "Fixture for story 23 CLI mixed-verdicts roundtrip"

outcome: |
  Fixture used to prove the CLI binary emits the three-verdict shape
  on a mixed-amendment pipeline.

status: under_construction

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-cli-mixed-crate/tests/red_scaffold.rs
      justification: |
        First-authoring at v2: probes red and must record red.
    - file: crates/fixture-cli-mixed-crate/tests/preserved_scaffold.rs
      justification: |
        Present since the last evidence row; classifies PRESERVE
        because the preserved entry's justification did not change.
    - file: crates/fixture-cli-mixed-crate/tests/reauthored_scaffold.rs
      justification: |
        V2 justification (AMENDED): scaffold re-probes red against
        the tightened observable.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

const FIXTURE_WORKSPACE_CARGO: &str = r#"[workspace]
resolver = "2"
members = ["crates/fixture-cli-mixed-crate"]
"#;

const FIXTURE_CRATE_CARGO: &str = r#"[package]
name = "fixture-cli-mixed-crate"
version = "0.0.1"
edition = "2021"

[lib]
path = "src/lib.rs"
"#;

const FIXTURE_CRATE_LIB: &str = r#"pub fn known_symbol() -> u32 {
    1
}
"#;

const RED_SCAFFOLD_BODY: &str = r#"use fixture_cli_mixed_crate::red_missing_symbol;

#[test]
fn red_scaffold() {
    assert_eq!(red_missing_symbol(), 0);
}
"#;

const PRESERVED_SCAFFOLD_BODY: &str = r#"use fixture_cli_mixed_crate::known_symbol;

#[test]
fn preserved_scaffold() {
    assert_eq!(known_symbol(), 1);
}
"#;

const REAUTHORED_SCAFFOLD_BODY: &str = r#"use fixture_cli_mixed_crate::reauthor_missing_symbol;

#[test]
fn reauthored_scaffold() {
    assert_eq!(reauthor_missing_symbol(), 0);
}
"#;

#[test]
fn agentic_test_build_record_emits_mixed_verdicts_through_binary() {
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();

    fs::write(repo_root.join("Cargo.toml"), FIXTURE_WORKSPACE_CARGO).expect("ws cargo");
    let crate_root = repo_root.join("crates/fixture-cli-mixed-crate");
    fs::create_dir_all(crate_root.join("src")).expect("crate src");
    fs::create_dir_all(crate_root.join("tests")).expect("crate tests");
    fs::write(crate_root.join("Cargo.toml"), FIXTURE_CRATE_CARGO).expect("crate cargo");
    fs::write(crate_root.join("src/lib.rs"), FIXTURE_CRATE_LIB).expect("crate lib");

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

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML_V1).expect("write story v1");

    // Commit 1: seed.
    let v1_commit = init_repo_and_commit_seed(repo_root);

    // Commit 2: seed prior evidence. All three verdicts red at v1.
    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    fs::create_dir_all(&evidence_dir).expect("evidence dir");
    let prior = evidence_dir.join("2026-04-01T00-00-00Z-red.jsonl");
    let prior_row = json!({
        "run_id": "00000000-0000-4000-8000-000000000007",
        "story_id": STORY_ID,
        "commit": v1_commit,
        "timestamp": "2026-04-01T00:00:00Z",
        "verdicts": [
            { "file": RED_FILE, "verdict": "red",
              "red_path": "compile", "diagnostic": "seeded red at v1" },
            { "file": PRESERVED_FILE, "verdict": "red",
              "red_path": "compile", "diagnostic": "seeded red at v1" },
            { "file": REAUTHORED_FILE, "verdict": "red",
              "red_path": "compile", "diagnostic": "seeded red at v1" }
        ]
    });
    fs::write(&prior, format!("{prior_row}\n")).expect("write prior evidence");
    commit_all(repo_root, "seed prior evidence at v1");

    // Commit 3: amend the story YAML to v2.
    fs::write(&story_path, FIXTURE_STORY_YAML_V2).expect("write story v2");
    commit_all(repo_root, "amend story 23 fixture CLI to v2");

    // Act: invoke the compiled `agentic` binary.
    let record_assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("test-build")
        .arg("record")
        .arg(STORY_ID.to_string())
        .assert();
    let record_output = record_assert.get_output().clone();
    let record_stdout = String::from_utf8_lossy(&record_output.stdout).to_string();
    let record_stderr = String::from_utf8_lossy(&record_output.stderr).to_string();

    assert_eq!(
        record_output.status.code(),
        Some(0),
        "`agentic test-build record <id>` must exit 0 when scaffolds classify \
         as a mix of red/preserved/re-authored; got status={:?}\n\
         stdout:\n{record_stdout}\nstderr:\n{record_stderr}",
        record_output.status
    );

    // stdout must name each recorded scaffold AND its verdict so the
    // operator can see the classification.
    for (file, verdict) in [
        (RED_FILE, "red"),
        (PRESERVED_FILE, "preserved"),
        (REAUTHORED_FILE, "re-authored"),
    ] {
        assert!(
            record_stdout.contains(file),
            "record stdout must name each recorded scaffold; expected {file:?} in:\n{record_stdout}"
        );
        assert!(
            record_stdout.contains(verdict),
            "record stdout must name each scaffold's verdict; expected {verdict:?} in:\n{record_stdout}"
        );
    }

    // Evidence file shape: three verdict entries in declaration order.
    let files: Vec<_> = fs::read_dir(&evidence_dir)
        .expect("evidence dir must exist after record")
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
    assert_eq!(verdicts.len(), 3, "three verdicts in one row");

    let v_pres = verdicts[1]
        .as_object()
        .and_then(|o| o.get("verdict"))
        .and_then(|v| v.as_str());
    assert_eq!(
        v_pres,
        Some("preserved"),
        "entry 1 must be verdict \"preserved\"; got {v_pres:?}"
    );
    let v_reauth = verdicts[2]
        .as_object()
        .and_then(|o| o.get("verdict"))
        .and_then(|v| v.as_str());
    assert_eq!(
        v_reauth,
        Some("re-authored"),
        "entry 2 must be verdict \"re-authored\"; got {v_reauth:?}"
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
