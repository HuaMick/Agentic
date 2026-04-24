//! Story 9 acceptance test: glob semantics at the boundary downstream
//! code has to get right.
//!
//! Justification (from stories/9.yml): proves glob semantics at the
//! boundary downstream code has to get right —
//! `related_files: ["crates/agentic-uat/src/**"]` matches a change to
//! `crates/agentic-uat/src/nested/mod.rs` (double-star crosses path
//! separators) and does NOT match `crates/agentic-uat/Cargo.toml` (the
//! glob is rooted at `src/`). Without this, glob semantics become
//! implementation-defined across implementations — and the difference
//! between "watches the whole crate" and "watches only the top-level
//! module" is the difference between a useful field and a trap.
//!
//! The scaffold runs TWO sub-scenarios back-to-back, each with its own
//! tempdir repo and own story id:
//!   (1) `**` crosses path separators: C1 edits
//!       `crates/agentic-uat/src/nested/mod.rs` — a path the
//!       double-star must expand to. Assert unhealthy.
//!   (2) Glob is rooted: C1 edits `crates/agentic-uat/Cargo.toml` —
//!       a sibling of `src/`, NOT under it. The glob must NOT match.
//!       Assert healthy.
//! Red today is compile-red via the missing repo-aware `Dashboard`
//! constructor and the missing `related_files` field.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::json;
use tempfile::TempDir;

fn fixture_yaml(id: u32) -> String {
    format!(
        r#"id: {id}
title: "Fixture pinning glob semantics at the double-star boundary"

outcome: |
  A fixture whose related_files uses `crates/agentic-uat/src/**`; the
  dashboard's glob engine must match nested paths and must NOT match
  sibling files outside `src/`.

status: unhealthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_related_files_glob_matches_nested_path.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Render the dashboard under two scenarios; assert glob semantics.

guidance: |
  Fixture authored inline for the glob-semantics scaffold. Not a real
  story.

related_files:
  - "crates/agentic-uat/src/**"

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

/// Seed the repo with the two paths we'll potentially mutate at C1:
///   - `crates/agentic-uat/src/nested/mod.rs` (inside the glob)
///   - `crates/agentic-uat/Cargo.toml` (sibling of `src/`, outside)
fn seed_layout(repo_root: &Path) {
    let nested = repo_root.join("crates/agentic-uat/src/nested");
    fs::create_dir_all(&nested).expect("create nested dir");
    fs::write(nested.join("mod.rs"), b"// seed\n").expect("write nested mod.rs");
    fs::write(
        repo_root.join("crates/agentic-uat/Cargo.toml"),
        b"[package]\nname = \"agentic-uat\"\nversion = \"0.0.0\"\n",
    )
    .expect("write sibling Cargo.toml");
}

fn build_fixture_repo(story_id: u32) -> (TempDir, String, PathBuf) {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path().to_path_buf();
    let repo = init_repo(&repo_root);

    seed_layout(&repo_root);

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{story_id}.yml")),
        fixture_yaml(story_id),
    )
    .expect("write fixture");

    let c0 = commit_all(&repo, "C0 seed", &[]);
    (tmp, c0, stories_dir)
}

#[test]
fn related_files_glob_double_star_crosses_path_separators_and_is_rooted_at_its_prefix() {
    // ---- Sub-scenario (1): double-star crosses path separators. ----
    // C1 edits a deeply nested path inside `src/`. The
    // `crates/agentic-uat/src/**` glob MUST match. Expected: unhealthy.
    {
        let story_id: u32 = 9905;
        let (tmp, c0, stories_dir) = build_fixture_repo(story_id);
        let repo_root = tmp.path();
        let repo = git2::Repository::open(repo_root).expect("reopen repo");

        let nested_file = repo_root.join("crates/agentic-uat/src/nested/mod.rs");
        fs::write(&nested_file, b"// seed\n// edited at C1\n")
            .expect("rewrite nested/mod.rs at C1");
        let c0_commit = repo
            .find_commit(git2::Oid::from_str(&c0).expect("parse C0 oid"))
            .expect("find C0 commit");
        let _c1 = commit_all(&repo, "C1 edit deeply nested", &[&c0_commit]);

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
            .expect("seed test_runs pass");

        let dashboard =
            Dashboard::with_repo(store.clone(), stories_dir.clone(), PathBuf::from(repo_root));
        let rendered = dashboard
            .render_table()
            .expect("render_table should succeed on nested-change scenario");
        let row = rendered
            .lines()
            .find(|line| line.contains(&story_id.to_string()))
            .unwrap_or_else(|| {
                panic!(
                    "[nested] rendered table must contain a row for story {story_id}; \
                     got:\n{rendered}"
                )
            });
        assert!(
            row.contains("unhealthy"),
            "[nested] `crates/agentic-uat/src/**` must match the nested path \
             `crates/agentic-uat/src/nested/mod.rs` (double-star crosses path \
             separators); got row: {row:?}\nfull table:\n{rendered}"
        );
    }

    // ---- Sub-scenario (2): glob is rooted at `src/`. ----
    // C1 edits `crates/agentic-uat/Cargo.toml` — sibling of `src/`,
    // NOT under it. The glob must NOT match. Expected: healthy.
    {
        let story_id: u32 = 9906;
        let (tmp, c0, stories_dir) = build_fixture_repo(story_id);
        let repo_root = tmp.path();
        let repo = git2::Repository::open(repo_root).expect("reopen repo");

        let sibling_file = repo_root.join("crates/agentic-uat/Cargo.toml");
        fs::write(
            &sibling_file,
            b"[package]\nname = \"agentic-uat\"\nversion = \"0.0.1\"\n",
        )
        .expect("rewrite sibling Cargo.toml at C1");
        let c0_commit = repo
            .find_commit(git2::Oid::from_str(&c0).expect("parse C0 oid"))
            .expect("find C0 commit");
        let _c1 = commit_all(&repo, "C1 edit sibling of src", &[&c0_commit]);

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
            .expect("seed test_runs pass");

        let dashboard =
            Dashboard::with_repo(store.clone(), stories_dir.clone(), PathBuf::from(repo_root));
        let rendered = dashboard
            .render_table()
            .expect("render_table should succeed on sibling-change scenario");
        let row = rendered
            .lines()
            .find(|line| line.contains(&story_id.to_string()))
            .unwrap_or_else(|| {
                panic!(
                    "[sibling] rendered table must contain a row for story {story_id}; \
                     got:\n{rendered}"
                )
            });
        assert!(
            row.contains("healthy"),
            "[sibling] `crates/agentic-uat/src/**` must NOT match the sibling file \
             `crates/agentic-uat/Cargo.toml` (glob is rooted at `src/`); \
             got row: {row:?}\nfull table:\n{rendered}"
        );
        assert!(
            !row.contains("unhealthy"),
            "[sibling] row must NOT classify as `unhealthy` — the sibling file is \
             outside the glob root; got row: {row:?}"
        );
    }
}
