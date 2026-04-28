//! Story 25 acceptance test: the `--json` flag is plumbed end-to-end
//! through the `agentic stories audit` subcommand.
//!
//! Justification (from stories/25.yml): proves the `--json` contract
//! reaches the operator through the binary — `agentic stories audit
//! --json` against a fixture corpus containing at least one drifted
//! story per category emits stdout that `serde_json::from_str` parses
//! into a value carrying a top-level object with one key per category
//! (`implementation_without_flip`, `promotion_ready`,
//! `test_builder_not_started`, `healthy_with_failing_test`), each
//! mapping to an array of objects naming the offending story id and
//! any per-category context (e.g. the passing test files for category
//! 1, the failing test files for category 4). The same data appears
//! in human-readable form when `--json` is absent, and exit code is
//! 0 in both modes regardless of how many drifts the report contains
//! (the audit reports drift; it does not itself fail on drift).
//! Without this, machine consumers (CI jobs, future dashboards) have
//! to scrape TTY output to learn what drifted, and the symmetry with
//! `agentic stories health --json` (story 3) breaks at the boundary
//! they are most likely to lean on.
//!
//! Red today is compile-red AND/or runtime-red: the `agentic stories
//! audit` subcommand does not yet exist in the CLI's clap tree. The
//! binary either rejects the argv ("unrecognized subcommand") or, if
//! the subcommand exists but the library backing isn't wired,
//! produces output that doesn't parse as the four-key JSON object.
//! Either failure mode is valid red evidence per ADR-0005.

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::{json, Value};
use tempfile::TempDir;

const ID_CAT1: u32 = 250601;
const ID_CAT2: u32 = 250602;
const ID_CAT3: u32 = 250603;
const ID_CAT4: u32 = 250604;

fn fixture_yaml(id: u32, status: &str, test_file_path: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for story-25 binary --json scaffold"

outcome: |
  Fixture story for the audit --json binary scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: {test_file_path}
      justification: |
        Present so the fixture is schema-valid and the audit has a
        path to inspect for drift across all four categories.
  uat: |
    Drive `agentic stories audit --json`; assert the four top-level
    arrays exist and contain the expected ids.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#
    )
}

fn write_passing_test_source(path: &Path) {
    fs::create_dir_all(path.parent().expect("test path has parent")).expect("create parent dir");
    fs::write(
        path,
        r#"#[test]
fn passes() {
    assert!(true);
}
"#,
    )
    .expect("write passing fixture test source");
}

fn init_repo_and_seed(root: &Path) {
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

/// Set up a fixture corpus with one drifted story per category, plus
/// a tempdir store seeded with the necessary `test_runs` and
/// `uat_signings` rows so the audit's category 1, 2, and 4 signals
/// fire. Returns (repo_tempdir, store_tempdir).
fn setup_fixture_with_one_drift_per_category() -> (TempDir, TempDir) {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // Category 1: status=proposed AND tests pass.
    let cat1_test = repo_root.join("fixture_tests").join("cat1.rs");
    write_passing_test_source(&cat1_test);
    fs::write(
        stories_dir.join(format!("{ID_CAT1}.yml")),
        fixture_yaml(
            ID_CAT1,
            "proposed",
            cat1_test.to_str().expect("cat1 path utf8"),
        ),
    )
    .expect("write cat1 fixture");

    // Category 2: status=under_construction AND tests pass.
    let cat2_test = repo_root.join("fixture_tests").join("cat2.rs");
    write_passing_test_source(&cat2_test);
    fs::write(
        stories_dir.join(format!("{ID_CAT2}.yml")),
        fixture_yaml(
            ID_CAT2,
            "under_construction",
            cat2_test.to_str().expect("cat2 path utf8"),
        ),
    )
    .expect("write cat2 fixture");

    // Category 3: status=under_construction AND test absent.
    let cat3_test = repo_root.join("fixture_tests").join("cat3_absent.rs");
    fs::write(
        stories_dir.join(format!("{ID_CAT3}.yml")),
        fixture_yaml(
            ID_CAT3,
            "under_construction",
            cat3_test.to_str().expect("cat3 path utf8"),
        ),
    )
    .expect("write cat3 fixture");

    // Category 4: status=healthy AND latest test_runs is Fail.
    let cat4_test = repo_root.join("fixture_tests").join("cat4.rs");
    write_passing_test_source(&cat4_test);
    fs::write(
        stories_dir.join(format!("{ID_CAT4}.yml")),
        fixture_yaml(
            ID_CAT4,
            "healthy",
            cat4_test.to_str().expect("cat4 path utf8"),
        ),
    )
    .expect("write cat4 fixture");

    init_repo_and_seed(repo_root);

    let store_tmp = TempDir::new().expect("store tempdir");
    seed_store_for_fixture(store_tmp.path());
    (repo_tmp, store_tmp)
}

/// Seed the store on disk with the test_runs/uat_signings rows the
/// audit's per-category signals need. Reopens the same SurrealStore
/// the binary will open via its `--store` flag.
fn seed_store_for_fixture(store_path: &Path) {
    use agentic_store::{Store, SurrealStore};

    let store = SurrealStore::open(store_path).expect("open SurrealStore for fixture seed");

    // Cat 1 (status=proposed, tests pass): need a Pass test_runs row.
    store
        .upsert(
            "test_runs",
            &ID_CAT1.to_string(),
            json!({
                "story_id": ID_CAT1,
                "verdict": "pass",
                "commit": "f".repeat(40),
                "ran_at": "2026-04-27T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed cat1 Pass test_runs");

    // Cat 2 (status=under_construction, tests pass): need a Pass
    // test_runs row.
    store
        .upsert(
            "test_runs",
            &ID_CAT2.to_string(),
            json!({
                "story_id": ID_CAT2,
                "verdict": "pass",
                "commit": "f".repeat(40),
                "ran_at": "2026-04-27T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed cat2 Pass test_runs");

    // Cat 4 (status=healthy, test_runs=Fail): need historical UAT
    // pass + Fail test_runs row.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000025604",
                "story_id": ID_CAT4,
                "verdict": "pass",
                "commit": "f".repeat(40),
                "signed_at": "2026-04-26T00:00:00Z",
            }),
        )
        .expect("seed cat4 historical UAT pass");
    store
        .upsert(
            "test_runs",
            &ID_CAT4.to_string(),
            json!({
                "story_id": ID_CAT4,
                "verdict": "fail",
                "commit": "f".repeat(40),
                "ran_at": "2026-04-27T00:00:00Z",
                "failing_tests": ["cat4.rs"],
            }),
        )
        .expect("seed cat4 Fail test_runs");
}

/// Extract the set of `id` integers from a JSON array of audit
/// entries.
fn ids_in_array(arr: &Value) -> Vec<u32> {
    arr.as_array()
        .unwrap_or_else(|| panic!("expected JSON array; got {arr}"))
        .iter()
        .filter_map(|entry| entry.get("id").and_then(|v| v.as_u64()).map(|n| n as u32))
        .collect()
}

#[test]
fn stories_audit_json_emits_four_top_level_arrays_with_drifted_story_ids() {
    let (repo_tmp, store_tmp) = setup_fixture_with_one_drift_per_category();

    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_tmp.path())
        .arg("stories")
        .arg("audit")
        .arg("--json")
        .arg("--store")
        .arg(store_tmp.path())
        .assert();

    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let status = output.status;

    // Audit reports drift but does not fail on drift — exit 0 in
    // both modes regardless of how many drifts the report contains.
    assert!(
        status.success(),
        "`agentic stories audit --json` must exit 0 even when drift is \
         present (audit reports drift; does not fail on drift); got \
         status={status:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // stdout must parse as a single JSON object.
    let parsed: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "stdout from `--json` must be parseable via `serde_json::from_str`; \
             parse error: {e}\nraw stdout:\n{stdout}\nstderr:\n{stderr}"
        )
    });
    let obj = parsed
        .as_object()
        .unwrap_or_else(|| panic!("top-level JSON must be an object; got: {parsed}"));

    // All four snake_case keys (per story 25's "Output shape —
    // `--json`" guidance) must be present, each mapping to an array.
    for key in [
        "implementation_without_flip",
        "promotion_ready",
        "test_builder_not_started",
        "healthy_with_failing_test",
    ] {
        let val = obj
            .get(key)
            .unwrap_or_else(|| panic!("JSON must have a `{key}` key; got: {parsed}"));
        assert!(val.is_array(), "`{key}` must be an array; got: {val}");
    }

    // Each category's array must contain its corresponding drifted
    // story id.
    let cat1_ids = ids_in_array(&obj["implementation_without_flip"]);
    let cat2_ids = ids_in_array(&obj["promotion_ready"]);
    let cat3_ids = ids_in_array(&obj["test_builder_not_started"]);
    let cat4_ids = ids_in_array(&obj["healthy_with_failing_test"]);

    assert!(
        cat1_ids.contains(&ID_CAT1),
        "implementation_without_flip must contain {ID_CAT1}; got {cat1_ids:?}\n\
         full JSON:\n{parsed}"
    );
    assert!(
        cat2_ids.contains(&ID_CAT2),
        "promotion_ready must contain {ID_CAT2}; got {cat2_ids:?}\n\
         full JSON:\n{parsed}"
    );
    assert!(
        cat3_ids.contains(&ID_CAT3),
        "test_builder_not_started must contain {ID_CAT3}; got {cat3_ids:?}\n\
         full JSON:\n{parsed}"
    );
    assert!(
        cat4_ids.contains(&ID_CAT4),
        "healthy_with_failing_test must contain {ID_CAT4}; got {cat4_ids:?}\n\
         full JSON:\n{parsed}"
    );

    // Each fixture story appears in EXACTLY one category — drift is
    // partitioned, not duplicated.
    let mut seen: Vec<u32> = Vec::new();
    seen.extend(cat1_ids.iter());
    seen.extend(cat2_ids.iter());
    seen.extend(cat3_ids.iter());
    seen.extend(cat4_ids.iter());
    for id in [ID_CAT1, ID_CAT2, ID_CAT3, ID_CAT4] {
        let occurrences = seen.iter().filter(|&&x| x == id).count();
        assert_eq!(
            occurrences, 1,
            "drifted story {id} must appear in EXACTLY one of the four \
             top-level arrays; got {occurrences} occurrences across \
             cat1={cat1_ids:?} cat2={cat2_ids:?} cat3={cat3_ids:?} \
             cat4={cat4_ids:?}"
        );
    }
}

// Suppress the unused-import warning for `PathBuf` in some toolchain
// configurations — `Path` alone covers the runtime needs above.
#[allow(dead_code)]
fn _path_buf_marker(_p: PathBuf) {}
