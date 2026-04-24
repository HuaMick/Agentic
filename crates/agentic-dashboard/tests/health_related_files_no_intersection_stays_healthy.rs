//! Story 9 acceptance test: the core relaxation — no file intersection
//! means healthy stays healthy.
//!
//! Justification (from stories/9.yml): proves the core relaxation — a
//! story whose latest `uat_signings.verdict=pass` commit is older than
//! HEAD, but where the set of files changed between that commit and
//! HEAD has no intersection with the story's `related_files` globs,
//! renders as `healthy` (not `unhealthy`). Without this the legacy
//! strict-equality rule is still in force and the whole point of the
//! story is unshipped.
//!
//! The scaffold constructs a tempdir git repo with two commits. C0
//! seeds a file under `crates/agentic-uat/src/lib.rs` and a
//! `stories/<id>.yml` that declares
//! `related_files: ["crates/agentic-uat/src/**"]`. C1 edits
//! `README.md` only (a file NOT under the glob). It seeds
//! `uat_signings` with a Pass at C0 and `test_runs` with a Pass at
//! C1 (HEAD). The dashboard is constructed via the new repo-aware
//! constructor that the story's guidance pins (diff between
//! `uat_commit` and HEAD is an internal call). The assertion: the
//! story's row classifies as `healthy`. Red today is compile-red via
//! the missing repo-aware `Dashboard` constructor and the missing
//! `related_files` field on `agentic_story::Story`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::json;
use tempfile::TempDir;

const STORY_ID: u32 = 9901;

fn fixture_yaml() -> String {
    format!(
        r#"id: {STORY_ID}
title: "A story whose related files did not change between C0 and HEAD"

outcome: |
  A fixture whose YAML declares related_files and whose latest UAT pass
  is at an older commit; the file intersection is empty so it must stay
  healthy.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_related_files_no_intersection_stays_healthy.rs
      justification: |
        Present so the fixture is schema-valid. The live test drives
        the repo-aware Dashboard constructor against this YAML.
  uat: |
    Render the dashboard; assert this row stays `healthy` despite the
    UAT commit being older than HEAD.

guidance: |
  Fixture authored inline for the no-intersection scaffold. Not a real
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

/// Stage every file under `root` and commit with `message`. Returns the
/// full 40-char SHA.
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

#[test]
fn story_whose_related_files_did_not_change_stays_healthy_despite_newer_head() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let repo = init_repo(repo_root);

    // Lay out the repo so `crates/agentic-uat/src/lib.rs` exists at C0 —
    // this is the file our glob watches and must NOT change between C0
    // and HEAD for the no-intersection branch.
    let uat_src_dir = repo_root.join("crates/agentic-uat/src");
    fs::create_dir_all(&uat_src_dir).expect("create uat src dir");
    fs::write(uat_src_dir.join("lib.rs"), b"// seed\n").expect("write lib.rs");
    fs::write(repo_root.join("README.md"), b"# seed\n").expect("write README");

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(stories_dir.join(format!("{STORY_ID}.yml")), fixture_yaml()).expect("write fixture");

    // C0: the seed commit. This is what the UAT signing will reference.
    let c0 = commit_all(&repo, "C0 seed", &[]);

    // C1: edit only README.md — NOT under the glob. The file
    // intersection between C0..HEAD and ["crates/agentic-uat/src/**"]
    // must therefore be empty.
    fs::write(repo_root.join("README.md"), b"# seed\nedited at C1\n")
        .expect("rewrite README at C1");
    let c0_commit = repo
        .find_commit(git2::Oid::from_str(&c0).expect("parse C0 oid"))
        .expect("find C0 commit");
    let _c1 = commit_all(&repo, "C1 edit README only", &[&c0_commit]);

    // Seed evidence: UAT pass at C0, test_runs pass at HEAD. The
    // classifier must still compute healthy because no file under
    // `crates/agentic-uat/src/**` changed in C0..HEAD.
    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000009901",
                "story_id": STORY_ID,
                "verdict": "pass",
                "commit": c0,
                "signed_at": "2026-04-18T00:00:00Z",
            }),
        )
        .expect("seed uat pass at C0");
    store
        .upsert(
            "test_runs",
            &STORY_ID.to_string(),
            json!({
                "story_id": STORY_ID,
                "verdict": "pass",
                "commit": head_sha(&repo),
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed test_runs pass at HEAD");

    // Construct the repo-aware dashboard. The repo_root carries enough
    // information for the dashboard to discover HEAD AND compute the
    // C0..HEAD file diff internally.
    let dashboard =
        Dashboard::with_repo(store.clone(), stories_dir.clone(), PathBuf::from(repo_root));

    let rendered = dashboard
        .render_table()
        .expect("render_table should succeed on a repo with a clean two-commit history");

    let row = rendered
        .lines()
        .find(|line| line.contains(&STORY_ID.to_string()))
        .unwrap_or_else(|| {
            panic!("rendered table must contain a row for story {STORY_ID}; got:\n{rendered}")
        });

    // The core assertion of story 9: legacy strict-equality is gone,
    // so a story whose related_files did not intersect the C0..HEAD
    // diff remains healthy even though uat_commit != HEAD.
    assert!(
        row.contains("healthy"),
        "story {STORY_ID} must classify as `healthy` when no file under \
         `crates/agentic-uat/src/**` changed between UAT commit and HEAD; \
         got row: {row:?}\nfull table:\n{rendered}"
    );
    assert!(
        !row.contains("unhealthy"),
        "row must NOT classify as `unhealthy`; the whole point of story 9 \
         is the strict-equality rule no longer fires when the file \
         intersection is empty. Got row: {row:?}\nfull table:\n{rendered}"
    );
}

fn head_sha(repo: &git2::Repository) -> String {
    repo.head()
        .expect("repo head")
        .peel_to_commit()
        .expect("head commit")
        .id()
        .to_string()
}
