//! Story 25 acceptance test: category 5 reaches the operator through
//! the binary, AND the binary's exit-code contract shifts from
//! "always 0" to "exit 2 on any drift" per the 2026-04-29 amendment.
//!
//! Justification (from stories/25.yml): proves category 5 reaches the
//! operator through the binary — `agentic stories audit --json`
//! against a fixture corpus containing one story with `status: healthy`
//! on disk AND zero `uat_signings`/`manual_signings` rows for that
//! story id emits stdout whose `yaml_healthy_without_signing_row`
//! JSON key is an array containing exactly that story's id, the same
//! data appears under a `yaml-healthy-without-signing-row` heading
//! in the default human-readable report, and the exit code is the
//! gate-mode non-zero exit (exit 2) the audit now returns when ANY
//! drift is detected — see "Exit-code contract" in guidance for the
//! binary-level shift from "exit 0 regardless of drift" to "exit 2
//! on any non-empty report." A fixture clean of all five categories
//! returns exit 0 with empty arrays under all five keys. Without
//! this binary-level pin, the fifth category could be wired in the
//! dashboard library without the JSON output adopting it (the same
//! library/binary-wire gap story 1 and story 3's CLI tests exist to
//! close), and the pre-commit hook (story 29) wrapping `agentic
//! stories audit` would not see a non-zero exit on the forged-
//! promotion shape — turning the fifth category into yet another
//! silently-detected-but-not-enforced observation.
//!
//! Red today is runtime-red on multiple axes:
//!   - the binary unconditionally `std::process::exit(0)` after
//!     emitting its report (see crates/agentic-cli/src/main.rs's
//!     `StoriesSubcommand::Audit` arm), so the exit-2-on-drift
//!     assertion fires;
//!   - the `--json` writer does not yet emit a
//!     `yaml_healthy_without_signing_row` top-level key (the
//!     library does not yet carry the field), so the JSON-shape
//!     assertion fires too.
//! Either failure mode alone is valid red evidence per ADR-0005;
//! the assertions below pin both for completeness so a partial
//! implementation does not coast.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use serde_json::{json, Value};
use tempfile::TempDir;

const ID_FORGED_PROMOTION: u32 = 250801;
const ID_UAT_SIGNED: u32 = 250802;
const ID_MANUAL_SIGNED: u32 = 250803;

fn fixture_yaml(id: u32, status: &str, test_file_path: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for story-25 cat-5 binary scaffold"

outcome: |
  Fixture story for the yaml-healthy-without-signing-row binary scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: {test_file_path}
      justification: |
        Present so the fixture is schema-valid; the audit's category-5
        signal at the binary boundary reads only the union of
        uat_signings + manual_signings and the gate-mode exit code.
  uat: |
    Drive `agentic stories audit --json`; assert the
    yaml_healthy_without_signing_row top-level array contains the
    forged-promotion story id and exit code is 2.

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

/// Author a fixture corpus with one forged-promotion story (cat-5
/// drift), plus two negative-control healthy stories whose signings
/// land in `uat_signings` and `manual_signings` respectively. Returns
/// (repo_tempdir, store_tempdir, head_sha).
fn setup_fixture_with_cat5_drift_and_two_negative_controls() -> (TempDir, TempDir, String) {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // Forged-promotion fixture: YAML=healthy, no signings in either
    // table. Tests pass on disk so categories 4 + signing-required
    // cleanly partition.
    let forged_test = repo_root.join("fixture_tests").join("cat5_forged.rs");
    write_passing_test_source(&forged_test);
    fs::write(
        stories_dir.join(format!("{ID_FORGED_PROMOTION}.yml")),
        fixture_yaml(
            ID_FORGED_PROMOTION,
            "healthy",
            forged_test.to_str().expect("forged path utf8"),
        ),
    )
    .expect("write forged fixture");

    // uat-signed control: YAML=healthy + uat_signings Pass row at HEAD.
    let uat_test = repo_root.join("fixture_tests").join("cat5_uat.rs");
    write_passing_test_source(&uat_test);
    fs::write(
        stories_dir.join(format!("{ID_UAT_SIGNED}.yml")),
        fixture_yaml(
            ID_UAT_SIGNED,
            "healthy",
            uat_test.to_str().expect("uat path utf8"),
        ),
    )
    .expect("write uat-signed fixture");

    // manual-signed control: YAML=healthy + manual_signings Pass row.
    // No uat_signings row — the audit's category-5 query MUST treat
    // this as signed via the union with manual_signings.
    let manual_test = repo_root.join("fixture_tests").join("cat5_manual.rs");
    write_passing_test_source(&manual_test);
    fs::write(
        stories_dir.join(format!("{ID_MANUAL_SIGNED}.yml")),
        fixture_yaml(
            ID_MANUAL_SIGNED,
            "healthy",
            manual_test.to_str().expect("manual path utf8"),
        ),
    )
    .expect("write manual-signed fixture");

    init_repo_and_seed(repo_root);

    // The git seed commit's SHA is the HEAD the binary will discover
    // and stamp into its store reads. We hand it back so the seeded
    // signings ride at HEAD (otherwise the dashboard's healthy
    // classifier would mark the uat-signed control unhealthy via the
    // "UAT commit != HEAD" rule).
    let repo = git2::Repository::open(repo_root).expect("reopen repo");
    let head_sha = repo
        .head()
        .expect("HEAD")
        .peel_to_commit()
        .expect("HEAD commit")
        .id()
        .to_string();

    let store_tmp = TempDir::new().expect("store tempdir");
    seed_store_for_fixture(store_tmp.path(), &head_sha);
    (repo_tmp, store_tmp, head_sha)
}

fn seed_store_for_fixture(store_path: &Path, head_sha: &str) {
    use agentic_store::{Store, SurrealStore};

    let store = SurrealStore::open(store_path).expect("open SurrealStore for fixture seed");

    // uat-signed control: real UAT pass at HEAD + Pass test_runs.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000025802",
                "story_id": ID_UAT_SIGNED,
                "verdict": "pass",
                "commit": head_sha,
                "signed_at": "2026-04-26T00:00:00Z",
            }),
        )
        .expect("seed uat_signings Pass for uat-signed control");
    store
        .upsert(
            "test_runs",
            &ID_UAT_SIGNED.to_string(),
            json!({
                "story_id": ID_UAT_SIGNED,
                "verdict": "pass",
                "commit": head_sha,
                "ran_at": "2026-04-27T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed Pass test_runs for uat-signed control");

    // manual-signed control: backfilled manual ritual row (story 28's
    // shape). The audit's category-5 query MUST union this with
    // uat_signings; without that, every backfilled story would stay
    // flagged forever and the gate would block the same commits it
    // claims to permit.
    store
        .append(
            "manual_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000025803",
                "story_id": ID_MANUAL_SIGNED,
                "verdict": "pass",
                "commit": head_sha,
                "signed_at": "2026-04-26T01:00:00Z",
                "ritual_evidence": "manual ritual: pre-backfill stories 11/17/23 shape",
            }),
        )
        .expect("seed manual_signings Pass for manual-signed control");
    store
        .upsert(
            "test_runs",
            &ID_MANUAL_SIGNED.to_string(),
            json!({
                "story_id": ID_MANUAL_SIGNED,
                "verdict": "pass",
                "commit": head_sha,
                "ran_at": "2026-04-27T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed Pass test_runs for manual-signed control");
}

fn ids_in_array(arr: &Value) -> Vec<u32> {
    arr.as_array()
        .unwrap_or_else(|| panic!("expected JSON array; got {arr}"))
        .iter()
        .filter_map(|entry| entry.get("id").and_then(|v| v.as_u64()).map(|n| n as u32))
        .collect()
}

#[test]
fn agentic_stories_audit_emits_cat5_array_and_exits_two_on_forged_promotion_drift() {
    let (repo_tmp, store_tmp, _head_sha) =
        setup_fixture_with_cat5_drift_and_two_negative_controls();

    // ----- Drift run: --json mode -----
    let json_assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_tmp.path())
        .arg("stories")
        .arg("audit")
        .arg("--json")
        .arg("--store")
        .arg(store_tmp.path())
        .assert();

    let json_output = json_assert.get_output().clone();
    let json_stdout = String::from_utf8_lossy(&json_output.stdout).to_string();
    let json_stderr = String::from_utf8_lossy(&json_output.stderr).to_string();
    let json_status = json_output.status;

    // Per the 2026-04-29 exit-code amendment: drift now means exit 2
    // (could-not-attest, mirroring story 1's dirty-tree mapping and
    // story 11's ancestor refusal). The original "exit 0 regardless"
    // contract is dead; the binary participates in structural
    // enforcement now and the pre-commit hook (story 29) treats
    // exit 2 as commit-block.
    assert_eq!(
        json_status.code(),
        Some(2),
        "`agentic stories audit --json` against a fixture with category-5 \
         drift MUST exit 2 (gate-mode could-not-attest) per story 25's \
         2026-04-29 exit-code amendment; got status={json_status:?}\n\
         stdout:\n{json_stdout}\nstderr:\n{json_stderr}"
    );

    // The JSON payload is still emitted on stdout — exit code is the
    // gate-half, not a substitute for the report.
    let parsed: Value = serde_json::from_str(json_stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "stdout from `--json` (even on exit 2) must be parseable JSON; \
             parse error: {e}\nraw stdout:\n{json_stdout}\nstderr:\n{json_stderr}"
        )
    });
    let obj = parsed
        .as_object()
        .unwrap_or_else(|| panic!("top-level JSON must be an object; got: {parsed}"));

    // The fifth top-level key MUST be present per the 2026-04-29
    // category-5 amendment, named in snake_case to match the existing
    // four keys.
    let cat5 = obj.get("yaml_healthy_without_signing_row").unwrap_or_else(|| {
        panic!(
            "JSON must have a `yaml_healthy_without_signing_row` key per \
             story 25's category-5 amendment; got keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        )
    });
    assert!(
        cat5.is_array(),
        "`yaml_healthy_without_signing_row` must be an array; got: {cat5}"
    );

    let cat5_ids = ids_in_array(cat5);
    assert!(
        cat5_ids.contains(&ID_FORGED_PROMOTION),
        "yaml_healthy_without_signing_row must contain the forged-promotion \
         story {ID_FORGED_PROMOTION}; got {cat5_ids:?}\nfull JSON:\n{parsed}"
    );
    assert!(
        !cat5_ids.contains(&ID_UAT_SIGNED),
        "yaml_healthy_without_signing_row must NOT contain the uat-signed \
         control {ID_UAT_SIGNED}; got {cat5_ids:?}"
    );
    // The composition assertion: a `manual_signings` row alone (no
    // `uat_signings` row) MUST satisfy the gate. Without union, this
    // backfilled-manual-ritual story stays flagged forever.
    assert!(
        !cat5_ids.contains(&ID_MANUAL_SIGNED),
        "yaml_healthy_without_signing_row must NOT contain the manual-signed \
         control {ID_MANUAL_SIGNED} — the audit's category-5 query MUST union \
         uat_signings + manual_signings (story 28's backfill shape); \
         got {cat5_ids:?}"
    );

    // ----- Drift run: human-readable mode -----
    let human_assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_tmp.path())
        .arg("stories")
        .arg("audit")
        .arg("--store")
        .arg(store_tmp.path())
        .assert();

    let human_output = human_assert.get_output().clone();
    let human_stdout = String::from_utf8_lossy(&human_output.stdout).to_string();
    let human_stderr = String::from_utf8_lossy(&human_output.stderr).to_string();
    let human_status = human_output.status;

    assert_eq!(
        human_status.code(),
        Some(2),
        "`agentic stories audit` (human-readable) against a fixture with \
         category-5 drift MUST also exit 2 — exit code is mode-independent; \
         got status={human_status:?}\nstdout:\n{human_stdout}\nstderr:\n{human_stderr}"
    );

    // The human report MUST name the offending story id and the
    // category. The exact heading wording is loose ("yaml-healthy-
    // without-signing-row" or "Yaml-healthy-without-signing-row" or
    // similar), so we accept either the snake_case or kebab-case form.
    let combined = format!("{human_stdout}\n{human_stderr}");
    let combined_lower = combined.to_lowercase();
    assert!(
        combined_lower.contains("yaml") && combined_lower.contains("signing"),
        "human-readable audit output must name the category 5 heading \
         (something containing `yaml` AND `signing`); got:\n{combined}"
    );
    assert!(
        combined.contains(&ID_FORGED_PROMOTION.to_string()),
        "human-readable audit output must name the forged-promotion story id \
         {ID_FORGED_PROMOTION}; got:\n{combined}"
    );

    // ----- Clean run: drop the forged-promotion story, keep both
    //                  signed controls, expect exit 0. -----
    fs::remove_file(
        repo_tmp
            .path()
            .join("stories")
            .join(format!("{ID_FORGED_PROMOTION}.yml")),
    )
    .expect("remove forged-promotion fixture for clean-corpus subrun");

    let clean_assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_tmp.path())
        .arg("stories")
        .arg("audit")
        .arg("--json")
        .arg("--store")
        .arg(store_tmp.path())
        .assert();

    let clean_output = clean_assert.get_output().clone();
    let clean_stdout = String::from_utf8_lossy(&clean_output.stdout).to_string();
    let clean_stderr = String::from_utf8_lossy(&clean_output.stderr).to_string();
    let clean_status = clean_output.status;

    // Per the 2026-04-29 amendment: a fixture clean of all five
    // categories returns exit 0 with empty arrays under all five
    // keys. The same exit-0 happy path the original four-category
    // audit already delivered — extended to the fifth key.
    assert!(
        clean_status.success(),
        "`agentic stories audit --json` against a clean fixture (no drift in \
         any of five categories) MUST exit 0; got status={clean_status:?}\n\
         stdout:\n{clean_stdout}\nstderr:\n{clean_stderr}"
    );

    let clean_parsed: Value = serde_json::from_str(clean_stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "clean-corpus stdout from `--json` must be parseable JSON; \
             parse error: {e}\nraw stdout:\n{clean_stdout}"
        )
    });
    let clean_obj = clean_parsed
        .as_object()
        .unwrap_or_else(|| panic!("top-level JSON must be an object; got: {clean_parsed}"));

    let clean_cat5 = clean_obj
        .get("yaml_healthy_without_signing_row")
        .unwrap_or_else(|| {
            panic!(
                "clean-corpus JSON must have a `yaml_healthy_without_signing_row` key; \
                 got keys: {:?}",
                clean_obj.keys().collect::<Vec<_>>()
            )
        });
    let clean_cat5_arr = clean_cat5
        .as_array()
        .unwrap_or_else(|| panic!("`yaml_healthy_without_signing_row` must be an array"));
    assert!(
        clean_cat5_arr.is_empty(),
        "clean corpus must produce zero entries under \
         yaml_healthy_without_signing_row; got {clean_cat5_arr:?}"
    );
}
