//! Story 13 acceptance test: composition of the ancestor-inheritance
//! rule with story 9's file-staleness signal and story 3's failing-
//! tests signal, with the exact combined ordering pinned.
//!
//! Justification (from stories/13.yml): proves composition with story 9
//! AND pins the exact combined ordering inside `not_healthy_reason`
//! when own-signals and ancestor offenders fire on the same row. Given
//! a story whose OWN latest `test_runs.verdict` is `fail`, whose own
//! `related_files` have changed since its UAT pass commit, AND whose
//! two direct `depends_on` parents `<A>` and `<C>` both classify
//! non-healthy (with a healthy `<B>` in between and `<A>.id < <C>.id`),
//! the dashboard classifies it as `unhealthy` and emits
//! `not_healthy_reason` equal to EXACTLY
//! `["own_tests", "own_files", "ancestor:<A>", "ancestor:<C>"]` —
//! own-signals first in the locked order `"own_tests"` then
//! `"own_files"`, then every offending direct parent ordered ascending
//! by id; `<B>` is absent.
//!
//! The scaffold constructs a tempdir git repo with two commits so the
//! file-staleness branch can actually fire (the classifier uses a real
//! git diff), materialises four stories (A under_construction, B
//! healthy, C under_construction, L leaf depends_on [A, B, C]), seeds
//! L's own signals to fire both `own_tests` (test_runs=fail) and
//! `own_files` (uat@C0, related_files glob matches file changed in
//! C1), then asserts L's JSON row's `not_healthy_reason` is EXACTLY
//! the four-element sequence in the locked order. Red today is
//! runtime-red because the classifier does not yet emit
//! `not_healthy_reason` at all — the field is absent, so the assertion
//! on its shape fails.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::{json, Value};
use tempfile::TempDir;

// IDs chosen so A.id < B.id < C.id < L.id, and specifically A.id < C.id
// so the ancestor-offender ordering `"ancestor:<A>", "ancestor:<C>"`
// is the ascending-by-id contract.
const ID_A: u32 = 913031; // direct parent — under_construction (offender)
const ID_B: u32 = 913032; // direct parent — healthy (NOT an offender)
const ID_C: u32 = 913033; // direct parent — under_construction (offender)
const ID_L: u32 = 913034; // leaf — own_tests fail + own_files stale + two offending parents

fn fixture(id: u32, status: &str, depends_on: &[u32], related_files: &[&str]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    let rf_yaml = if related_files.is_empty() {
        String::new()
    } else {
        let lines: Vec<String> = related_files
            .iter()
            .map(|p| format!("  - \"{p}\""))
            .collect();
        format!("\nrelated_files:\n{}\n", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for composes-with-file-staleness scaffold"

outcome: |
  Fixture row for the composes-with-file-staleness scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_composes_with_file_staleness.rs
      justification: |
        Present so the fixture is schema-valid. The live test drives
        Dashboard::with_repo against this YAML to exercise combined
        own-signal and ancestor-offender reason channels.
  uat: |
    Render the dashboard; assert L's not_healthy_reason is exactly
    ["own_tests", "own_files", "ancestor:<A>", "ancestor:<C>"].

guidance: |
  Fixture authored inline for the composes-with-file-staleness
  scaffold. Not a real story.

{deps_yaml}{rf_yaml}"#
    )
}

fn init_repo(root: &Path) -> git2::Repository {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder")
            .expect("set user.name");
        cfg.set_str("user.email", "test@agentic.local")
            .expect("set user.email");
    }
    repo
}

fn commit_all(repo: &git2::Repository, message: &str, parents: &[&git2::Commit]) -> String {
    let mut index = repo.index().expect("repo index");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("stage all");
    index.write().expect("write index");
    let tree_oid = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_oid).expect("find tree");
    let sig = repo.signature().expect("repo signature");
    let parent_refs: Vec<&git2::Commit> = parents.to_vec();
    let commit_oid = repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
        .expect("commit");
    commit_oid.to_string()
}

fn head_sha(repo: &git2::Repository) -> String {
    repo.head()
        .expect("repo head")
        .peel_to_commit()
        .expect("head commit")
        .id()
        .to_string()
}

#[test]
fn leaf_with_own_tests_fail_own_files_stale_and_two_ancestor_offenders_emits_locked_four_token_reason(
) {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let repo = init_repo(repo_root);

    // Seed a file inside the leaf's related_files glob.
    let watched_dir = repo_root.join("crates/agentic-example/src");
    fs::create_dir_all(&watched_dir).expect("create watched dir");
    let watched_file = watched_dir.join("lib.rs");
    fs::write(&watched_file, b"// seed\n").expect("write watched at C0");
    fs::write(repo_root.join("README.md"), b"# seed\n").expect("write README at C0");

    // Materialise the four fixture stories.
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    // A: under_construction, no UAT — offender.
    fs::write(
        stories_dir.join(format!("{ID_A}.yml")),
        fixture(ID_A, "under_construction", &[], &[]),
    )
    .expect("write A");
    // B: healthy, own signals all clean — NOT an offender.
    fs::write(
        stories_dir.join(format!("{ID_B}.yml")),
        fixture(ID_B, "healthy", &[], &[]),
    )
    .expect("write B");
    // C: under_construction, no UAT — offender.
    fs::write(
        stories_dir.join(format!("{ID_C}.yml")),
        fixture(ID_C, "under_construction", &[], &[]),
    )
    .expect("write C");
    // L: healthy YAML (on-disk), depends_on [A, B, C], related_files
    // matches the watched file.
    fs::write(
        stories_dir.join(format!("{ID_L}.yml")),
        fixture(
            ID_L,
            "healthy",
            &[ID_A, ID_B, ID_C],
            &["crates/agentic-example/src/**"],
        ),
    )
    .expect("write L");

    // C0: seed. L's UAT pass will reference this commit.
    let c0 = commit_all(&repo, "C0 seed", &[]);

    // C1: edit the watched file — triggers file-staleness for L.
    fs::write(&watched_file, b"// seed\n// edited at C1\n").expect("rewrite watched file at C1");
    let c0_commit = repo
        .find_commit(git2::Oid::from_str(&c0).expect("parse C0 oid"))
        .expect("find C0 commit");
    let _c1 = commit_all(&repo, "C1 edit watched file", &[&c0_commit]);

    let head = head_sha(&repo);
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // B: healthy — seed clean signals @ HEAD.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000913032",
                "story_id": ID_B,
                "verdict": "pass",
                "commit": head,
                "signed_at": "2026-04-19T00:00:00Z",
            }),
        )
        .expect("seed B uat_signings pass@HEAD");
    store
        .upsert(
            "test_runs",
            &ID_B.to_string(),
            json!({
                "story_id": ID_B,
                "verdict": "pass",
                "commit": head,
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed B test_runs pass");

    // L: UAT pass @ C0 (so the related_files diff against HEAD fires),
    // and test_runs.verdict = fail (so own_tests fires).
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000913034",
                "story_id": ID_L,
                "verdict": "pass",
                "commit": c0,
                "signed_at": "2026-04-18T00:00:00Z",
            }),
        )
        .expect("seed L uat_signings pass@C0");
    store
        .upsert(
            "test_runs",
            &ID_L.to_string(),
            json!({
                "story_id": ID_L,
                "verdict": "fail",
                "commit": head,
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": ["some_test_name"],
            }),
        )
        .expect("seed L test_runs fail");

    // A and C carry no UAT signing — classify `under_construction`.

    let dashboard =
        Dashboard::with_repo(store.clone(), stories_dir.clone(), PathBuf::from(repo_root));
    let rendered = dashboard
        .render_json()
        .expect("render_json should succeed on the four-story fixture");

    let parsed: Value = serde_json::from_str(&rendered)
        .unwrap_or_else(|e| panic!("render_json output must parse as JSON: {e}; raw:\n{rendered}"));

    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));

    let l_row = stories
        .iter()
        .find(|s| s.get("id").and_then(|v| v.as_u64()) == Some(ID_L as u64))
        .unwrap_or_else(|| panic!("stories[] must include L (id {ID_L}); got: {parsed}"));

    // L must classify as unhealthy.
    let l_health = l_row
        .get("health")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("L row must carry health; got: {l_row}"));
    assert_eq!(
        l_health, "unhealthy",
        "L has own_tests fail, own_files stale, and two offending direct \
         parents — it must classify `unhealthy`. Got L.health={l_health}; \
         row: {l_row}"
    );

    // The load-bearing assertion: not_healthy_reason is EXACTLY the
    // four-element sequence in the locked order.
    let reason = l_row.get("not_healthy_reason").unwrap_or_else(|| {
        panic!(
            "L's unhealthy row must carry a `not_healthy_reason` field \
             listing each active reason as a token; got: {l_row}"
        )
    });
    let reason_arr = reason
        .as_array()
        .unwrap_or_else(|| panic!("`not_healthy_reason` must be a JSON array; got {reason:?}"));
    let reason_tokens: Vec<String> = reason_arr
        .iter()
        .map(|v| {
            v.as_str()
                .unwrap_or_else(|| panic!("reason tokens must be strings; got {v:?}"))
                .to_string()
        })
        .collect();

    let expected: Vec<String> = vec![
        "own_tests".to_string(),
        "own_files".to_string(),
        format!("ancestor:{ID_A}"),
        format!("ancestor:{ID_C}"),
    ];
    assert_eq!(
        reason_tokens, expected,
        "`not_healthy_reason` must be EXACTLY {expected:?} in that order \
         (own-signals first in locked `own_tests`/`own_files` order, then \
         offending direct parents ascending by id; healthy parent B must \
         be absent); got {reason_tokens:?}"
    );
}
