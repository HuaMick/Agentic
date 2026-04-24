//! Story 9 acceptance test: the default-policy choice when
//! `related_files` is absent or empty.
//!
//! Justification (from stories/9.yml): proves the default-policy choice
//! — a story whose YAML omits `related_files` (or declares it as an
//! empty array), with a latest UAT pass commit older than HEAD and no
//! failing tests, renders as `healthy`. Without this pinning, the
//! absent-field behaviour is an implementation detail and future drift
//! could silently turn "I haven't declared dependencies yet" into
//! "every unrelated edit knocks me red," which is exactly the
//! ergonomic failure this story exists to fix.
//!
//! The scaffold runs the assertion under BOTH permissive shapes — a
//! story that omits `related_files` entirely, and a story that
//! declares it as an empty array — in one test, back-to-back. For
//! each, it builds a tempdir git repo with two commits where C1 edits
//! a file that previously was covered by the removed glob (matching
//! the UAT walkthrough's step 12) to be sure the absent-field behaviour
//! is NOT a silent strict-equality fallback. Both permissive shapes
//! must classify as `healthy`. Red today is compile-red via the
//! missing repo-aware `Dashboard` constructor and the missing
//! `related_files` field.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::json;
use tempfile::TempDir;

fn fixture_omit_yaml(id: u32) -> String {
    format!(
        r#"id: {id}
title: "Fixture that OMITS related_files"

outcome: |
  A fixture whose YAML omits related_files entirely; the dashboard
  must treat this as permissive (healthy despite UAT commit older
  than HEAD).

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_related_files_absent_is_permissive.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Render the dashboard; assert this row classifies healthy despite an
    older UAT commit.

guidance: |
  Fixture authored inline for the absent-is-permissive scaffold. Not a
  real story.

depends_on: []
"#
    )
}

fn fixture_empty_array_yaml(id: u32) -> String {
    format!(
        r#"id: {id}
title: "Fixture that declares related_files as empty array"

outcome: |
  A fixture whose YAML declares related_files as the empty array; the
  dashboard must treat this as permissive (same behaviour as omission).

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_related_files_absent_is_permissive.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Render the dashboard; assert this row classifies healthy despite an
    older UAT commit.

guidance: |
  Fixture authored inline for the absent-is-permissive scaffold. Not a
  real story.

related_files: []

depends_on: []
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

/// Set up the per-shape scenario: a repo where C1 edits a file that
/// (under the legacy strict-equality rule, or if the loader silently
/// fell back to a global-watch default) would knock the row unhealthy.
/// With permissive-default in force, both shapes must classify healthy.
fn assert_permissive_shape_stays_healthy(fixture_yaml: &str, story_id: u32, tag: &str) {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let repo = init_repo(repo_root);

    let uat_src_dir = repo_root.join("crates/agentic-uat/src");
    fs::create_dir_all(&uat_src_dir).expect("create uat src dir");
    let target_file = uat_src_dir.join("lib.rs");
    fs::write(&target_file, b"// seed\n").expect("write lib.rs at C0");

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(stories_dir.join(format!("{story_id}.yml")), fixture_yaml).expect("write fixture");

    let c0 = commit_all(&repo, "C0 seed", &[]);

    // C1: edit a file that would have been covered by a
    // `crates/agentic-uat/src/**` glob — so the test is meaningful
    // (the absent-field behaviour is NOT a strict-equality fallback).
    fs::write(&target_file, b"// seed\n// edited at C1\n").expect("rewrite target lib.rs at C1");
    let c0_commit = repo
        .find_commit(git2::Oid::from_str(&c0).expect("parse C0 oid"))
        .expect("find C0 commit");
    let _c1 = commit_all(&repo, "C1 edit file under legacy glob", &[&c0_commit]);

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    store
        .append(
            "uat_signings",
            json!({
                "id": format!("01900000-0000-7000-8000-{:012x}", story_id),
                "story_id": story_id,
                "verdict": "pass",
                "commit": c0,
                "signed_at": "2026-04-18T00:00:00Z",
            }),
        )
        .expect("seed uat pass at C0");
    store
        .upsert(
            "test_runs",
            &story_id.to_string(),
            json!({
                "story_id": story_id,
                "verdict": "pass",
                "commit": head_sha(&repo),
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed test_runs pass at HEAD");

    let dashboard =
        Dashboard::with_repo(store.clone(), stories_dir.clone(), PathBuf::from(repo_root));

    let rendered = dashboard
        .render_table()
        .expect("render_table should succeed");
    let row = rendered
        .lines()
        .find(|line| line.contains(&story_id.to_string()))
        .unwrap_or_else(|| {
            panic!(
                "[{tag}] rendered table must contain a row for story {story_id}; \
                 got:\n{rendered}"
            )
        });
    assert!(
        row.contains("healthy"),
        "[{tag}] story {story_id} must classify as `healthy` when related_files is \
         permissive ({tag}); got row: {row:?}\nfull table:\n{rendered}"
    );
    assert!(
        !row.contains("unhealthy"),
        "[{tag}] row must NOT classify as `unhealthy` — the absent-field default is \
         permissive; got row: {row:?}\nfull table:\n{rendered}"
    );
}

#[test]
fn story_whose_related_files_is_absent_or_empty_is_permissive_not_strict_equality() {
    // Shape 1: related_files OMITTED entirely.
    assert_permissive_shape_stays_healthy(&fixture_omit_yaml(9903), 9903, "omit");

    // Shape 2: related_files: [] explicitly.
    assert_permissive_shape_stays_healthy(&fixture_empty_array_yaml(9904), 9904, "empty-array");
}
