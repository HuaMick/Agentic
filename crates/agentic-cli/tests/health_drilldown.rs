//! Story 3 acceptance test: the positional `<id>` selects drilldown
//! mode and unknown ids surface non-zero.
//!
//! Justification (from stories/3.yml): proves the positional `<id>`
//! argument selects drilldown mode — `agentic stories health <id>`
//! against a fixture story emits a single-story view (not the table
//! header) and exits 0; running against an id that does not exist on
//! disk exits non-zero with a message naming the missing id. Without
//! this, the only way to access `Dashboard::drilldown` is via Rust,
//! defeating the purpose of the subcommand for an operator.
//!
//! The scaffold writes one minimal fixture story, runs drilldown
//! against its id (asserting exit 0 and absence of the table header),
//! then runs drilldown against an id that doesn't exist and asserts
//! non-zero exit with the missing id named in stderr.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use tempfile::TempDir;

const KNOWN_ID: u32 = 88801;
const MISSING_ID: u32 = 99999;

fn fixture_yaml(id: u32) -> String {
    format!(
        r#"id: {id}
title: "Fixture story for story 3 CLI drilldown"

outcome: |
  Fixture authored inline so the CLI drilldown subcommand has one
  story to name.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/health_drilldown.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the agentic binary against this file.
  uat: |
    Run `agentic stories health <id>`; assert drilldown view, not
    table.

guidance: |
  Fixture authored inline for the story-3 drilldown scaffold. Not a
  real story.

depends_on: []
"#
    )
}

#[test]
fn stories_health_with_positional_id_selects_drilldown_and_unknown_id_exits_nonzero() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{KNOWN_ID}.yml")),
        fixture_yaml(KNOWN_ID),
    )
    .expect("write fixture");

    init_repo_and_commit_seed(repo_root);

    let store_tmp = TempDir::new().expect("store tempdir");

    // --- Known id: drilldown view, exit 0, no table header. ---
    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("stories")
        .arg("health")
        .arg(KNOWN_ID.to_string())
        .arg("--store")
        .arg(store_tmp.path())
        .assert();

    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert!(
        output.status.success(),
        "drilldown against a known id must exit 0; got status={:?}\n\
         stdout:\n{stdout}\nstderr:\n{stderr}",
        output.status
    );

    // Drilldown must not render the five-column table header.
    let has_table_header = stdout.lines().any(|l| {
        l.contains("ID")
            && l.contains("Title")
            && l.contains("Health")
            && l.contains("Failing tests")
            && l.contains("Healthy at")
    });
    assert!(
        !has_table_header,
        "drilldown mode must NOT emit the table header line; got stdout:\n{stdout}"
    );
    // Sanity: the drilldown view should at minimum reference the
    // story id so the operator knows what they're looking at.
    assert!(
        stdout.contains(&KNOWN_ID.to_string()),
        "drilldown output must name the story id {KNOWN_ID} somewhere; \
         got stdout:\n{stdout}"
    );

    // --- Unknown id: non-zero exit, stderr names the missing id. ---
    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("stories")
        .arg("health")
        .arg(MISSING_ID.to_string())
        .arg("--store")
        .arg(store_tmp.path())
        .assert();

    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert!(
        !output.status.success(),
        "drilldown against an unknown id must exit non-zero; \
         got status={:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status
    );
    assert!(
        stderr.contains(&MISSING_ID.to_string()),
        "stderr must name the missing id {MISSING_ID} so the operator can \
         see what was asked for; got stderr:\n{stderr}\nstdout:\n{stdout}"
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
    let commit_oid = repo
        .commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
    commit_oid.to_string()
}
