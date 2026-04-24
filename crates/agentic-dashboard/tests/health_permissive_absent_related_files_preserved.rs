//! Story 13 acceptance test: the ancestor rule does NOT silently
//! change story 9's permissive-absent `related_files` rule.
//!
//! Justification (from stories/13.yml): proves the ancestor rule does
//! NOT silently change the permissive-absent rule story 9 pinned: a
//! story whose `related_files` is absent AND whose ancestors all
//! classify `healthy` AND whose own UAT pass is older than HEAD still
//! classifies `healthy` — the file-staleness check remains permissive
//! when the field is absent, independent of the new ancestor rule.
//!
//! The scaffold builds a tempdir git repo with two commits (so "older
//! than HEAD" is a real condition with a real head SHA the classifier
//! can diff against), seeds a fixture with a healthy direct parent A
//! and a leaf L whose:
//!   - YAML says `status: healthy`,
//!   - `related_files` is ABSENT (not declared in the YAML),
//!   - UAT pass is at C0, and
//!   - HEAD is C1 (the UAT commit is older than HEAD).
//! Assertion: L classifies `healthy` — the absence of `related_files`
//! keeps file-staleness permissive even though the UAT commit is no
//! longer HEAD, AND the healthy ancestor A does not trigger the new
//! ancestor rule. Red today: runtime-red, because while the current
//! classifier already honours story 9's permissive-absent rule, the
//! test ALSO pins that this story's new rule did not tighten it. To
//! generate a natural red today we additionally assert — in the same
//! test — that a SIBLING leaf with the same permissive-absent shape
//! but an under_construction parent classifies unhealthy with an
//! `"ancestor:<A>"` reason, which is the observable the new rule
//! introduces. That second assertion is what fires today; the first
//! is the non-regression guard this story owns.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::{json, Value};
use tempfile::TempDir;

const ID_A: u32 = 913081; // healthy direct parent
const ID_BAD: u32 = 913082; // under_construction parent for the sibling probe
const ID_L_PERMISSIVE: u32 = 913083; // leaf: depends_on=[A], related_files absent
const ID_L_SIBLING: u32 = 913084; // leaf: depends_on=[BAD], related_files absent

fn fixture(id: u32, status: &str, depends_on: &[u32]) -> String {
    // Note: `related_files` is DELIBERATELY absent from the YAML emitted
    // by this helper — this scaffold pins the absent-field default.
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for permissive-absent + ancestor scaffold"

outcome: |
  Fixture row for the permissive-absent-related-files scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_permissive_absent_related_files_preserved.rs
      justification: |
        Present so the fixture is schema-valid. The live test asserts
        that absent related_files stays permissive independent of the
        new ancestor rule.
  uat: |
    Render the dashboard; assert L_PERMISSIVE classifies healthy and
    L_SIBLING classifies unhealthy with an ancestor reason.

guidance: |
  Fixture authored inline for the permissive-absent-related-files
  scaffold. Not a real story.

{deps_yaml}
"#
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
    let tree = repo.find_tree(tree_oid).expect("write tree");
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
fn absent_related_files_stays_permissive_while_new_ancestor_rule_fires_on_sibling_leaf() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let repo = init_repo(repo_root);

    fs::write(repo_root.join("README.md"), b"# seed\n").expect("write README at C0");

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{ID_A}.yml")),
        fixture(ID_A, "healthy", &[]),
    )
    .expect("write A healthy");
    fs::write(
        stories_dir.join(format!("{ID_BAD}.yml")),
        fixture(ID_BAD, "under_construction", &[]),
    )
    .expect("write BAD under_construction");
    fs::write(
        stories_dir.join(format!("{ID_L_PERMISSIVE}.yml")),
        fixture(ID_L_PERMISSIVE, "healthy", &[ID_A]),
    )
    .expect("write L_PERMISSIVE depends_on=[A]");
    fs::write(
        stories_dir.join(format!("{ID_L_SIBLING}.yml")),
        fixture(ID_L_SIBLING, "healthy", &[ID_BAD]),
    )
    .expect("write L_SIBLING depends_on=[BAD]");

    // C0: initial seed. UAT passes reference this commit.
    let c0 = commit_all(&repo, "C0 seed", &[]);
    // C1: an unrelated edit to README.md so HEAD != C0.
    fs::write(repo_root.join("README.md"), b"# seed\n# edited at C1\n")
        .expect("rewrite README at C1");
    let c0_commit = repo
        .find_commit(git2::Oid::from_str(&c0).expect("parse C0"))
        .expect("find C0");
    let _c1 = commit_all(&repo, "C1 edit README", &[&c0_commit]);
    let head = head_sha(&repo);
    assert_ne!(head, c0, "HEAD must differ from C0 for this scaffold");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // A: UAT pass @ HEAD, tests pass — healthy.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000913081",
                "story_id": ID_A,
                "verdict": "pass",
                "commit": head,
                "signed_at": "2026-04-19T00:00:00Z",
            }),
        )
        .expect("seed A uat@HEAD");
    store
        .upsert(
            "test_runs",
            &ID_A.to_string(),
            json!({
                "story_id": ID_A,
                "verdict": "pass",
                "commit": head,
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed A tests pass");

    // L_PERMISSIVE: UAT pass @ C0 (older than HEAD), tests pass — own
    // signals would go stale under a strict rule, but related_files is
    // absent so story 9's permissive-absent rule keeps it healthy.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000913083",
                "story_id": ID_L_PERMISSIVE,
                "verdict": "pass",
                "commit": c0,
                "signed_at": "2026-04-18T00:00:00Z",
            }),
        )
        .expect("seed L_PERMISSIVE uat@C0");
    store
        .upsert(
            "test_runs",
            &ID_L_PERMISSIVE.to_string(),
            json!({
                "story_id": ID_L_PERMISSIVE,
                "verdict": "pass",
                "commit": head,
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed L_PERMISSIVE tests pass");

    // L_SIBLING: same permissive-absent shape, but parent is BAD. Its
    // UAT is at C0 and its tests pass. The new ancestor rule must fire.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000913084",
                "story_id": ID_L_SIBLING,
                "verdict": "pass",
                "commit": c0,
                "signed_at": "2026-04-18T00:00:00Z",
            }),
        )
        .expect("seed L_SIBLING uat@C0");
    store
        .upsert(
            "test_runs",
            &ID_L_SIBLING.to_string(),
            json!({
                "story_id": ID_L_SIBLING,
                "verdict": "pass",
                "commit": head,
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed L_SIBLING tests pass");

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

    let row_of = |id: u32| -> &Value {
        stories
            .iter()
            .find(|s| s.get("id").and_then(|v| v.as_u64()) == Some(id as u64))
            .unwrap_or_else(|| panic!("stories[] must include id {id}; got: {parsed}"))
    };

    // Non-regression guard (story 9's permissive-absent rule): L_PERMISSIVE
    // classifies `healthy` despite UAT@C0 being older than HEAD — because
    // related_files is absent AND its only ancestor A is healthy.
    let perm_health = row_of(ID_L_PERMISSIVE)
        .get("health")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("L_PERMISSIVE row must carry health"));
    assert_eq!(
        perm_health,
        "healthy",
        "L_PERMISSIVE has `related_files` absent (story 9 permissive-absent \
         rule), its UAT commit is older than HEAD, and its only ancestor \
         A classifies healthy — the permissive-absent rule must be \
         preserved and L_PERMISSIVE must classify `healthy`. Got \
         {perm_health} on row {}",
        row_of(ID_L_PERMISSIVE)
    );

    // New-rule observable (story 13): L_SIBLING has the same permissive-
    // absent shape but its direct parent is under_construction. It must
    // classify `unhealthy` with `not_healthy_reason` containing
    // `"ancestor:<ID_BAD>"`. Today's classifier does not emit
    // `not_healthy_reason` — this assertion fires.
    let sib_row = row_of(ID_L_SIBLING);
    let sib_health = sib_row
        .get("health")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("L_SIBLING row must carry health; got: {sib_row}"));
    assert_eq!(
        sib_health, "unhealthy",
        "L_SIBLING has the same permissive-absent `related_files` shape as \
         L_PERMISSIVE but depends_on=[BAD under_construction] — the new \
         ancestor rule must fire and classify it `unhealthy`. Got \
         {sib_health} on row {sib_row}"
    );
    let sib_reason = sib_row.get("not_healthy_reason").unwrap_or_else(|| {
        panic!(
            "L_SIBLING's unhealthy row must carry `not_healthy_reason`; \
             got: {sib_row}"
        )
    });
    let sib_tokens: Vec<String> = sib_reason
        .as_array()
        .unwrap_or_else(|| panic!("`not_healthy_reason` must be an array; got {sib_reason:?}"))
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    let want = format!("ancestor:{ID_BAD}");
    assert_eq!(
        sib_tokens,
        vec![want.clone()],
        "L_SIBLING's `not_healthy_reason` must be EXACTLY [{want:?}] — \
         own signals are clean (permissive-absent related_files) and the \
         sole direct offender is BAD. Got {sib_tokens:?} on row {sib_row}"
    );
}
