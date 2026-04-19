//! Story 9 acceptance test: the staleness trigger — an intersecting
//! change since the UAT commit flips the row unhealthy.
//!
//! Justification (from stories/9.yml): proves the staleness trigger — a
//! story whose latest UAT pass commit is older than HEAD, where at
//! least one file changed between that commit and HEAD matches at least
//! one of the story's `related_files` entries (including glob expansion
//! for entries like `crates/foo/src/**`), renders as `unhealthy` with a
//! reason distinguishable from "tests went red." Without this the
//! dashboard can no longer surface "code that mattered to this story
//! changed since it was proven" — the signal the strict rule used to
//! give us unconditionally.
//!
//! The scaffold constructs a tempdir git repo with two commits:
//!   - C0: seeds `crates/agentic-uat/src/lib.rs` and a story YAML
//!         declaring `related_files: ["crates/agentic-uat/src/**"]`.
//!   - C1: edits `crates/agentic-uat/src/lib.rs` — a file INSIDE the
//!         glob.
//! `uat_signings` carries a Pass at C0; `test_runs` carries a Pass at
//! HEAD (so `test_run_fail` is NOT what drives the unhealth — the
//! file-intersection rule is). The assertion: the row is `unhealthy`
//! AND the unhealth reason is NOT a failing-tests signal (the JSON
//! mode's `stale_related_files` field, pinned by the story's
//! "Unhealthy reason channel" guidance, carries the changed path).
//! Red today is compile-red via the missing repo-aware `Dashboard`
//! constructor and the missing `related_files` field.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::{json, Value};
use tempfile::TempDir;

const STORY_ID: u32 = 9902;

fn fixture_yaml() -> String {
    format!(
        r#"id: {STORY_ID}
title: "A story whose related files changed between UAT commit and HEAD"

outcome: |
  A fixture whose related_files glob matches at least one file that
  changed between the UAT signing's commit and HEAD, so it must
  classify as unhealthy for a reason distinct from failing tests.

status: unhealthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_related_files_intersection_is_unhealthy.rs
      justification: |
        Present so the fixture is schema-valid. The live test drives
        the repo-aware Dashboard against this YAML.
  uat: |
    Render the dashboard; assert this row classifies unhealthy with a
    reason distinct from "tests went red."

guidance: |
  Fixture authored inline for the intersection-is-unhealthy scaffold.
  Not a real story.

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
        cfg.set_str("user.name", "test-builder").expect("set user.name");
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
fn story_whose_related_files_changed_since_uat_commit_classifies_unhealthy_for_file_reason() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let repo = init_repo(repo_root);

    let uat_src_dir = repo_root.join("crates/agentic-uat/src");
    fs::create_dir_all(&uat_src_dir).expect("create uat src dir");
    let watched_file = uat_src_dir.join("lib.rs");
    fs::write(&watched_file, b"// seed\n").expect("write lib.rs at C0");
    fs::write(repo_root.join("README.md"), b"# seed\n").expect("write README");

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{STORY_ID}.yml")),
        fixture_yaml(),
    )
    .expect("write fixture");

    // C0: seed. The UAT signing will reference this commit.
    let c0 = commit_all(&repo, "C0 seed", &[]);

    // C1: edit the watched file — directly inside the glob. This is
    // the intersecting change the staleness rule must fire on.
    fs::write(&watched_file, b"// seed\n// edited at C1\n")
        .expect("rewrite watched lib.rs at C1");
    let c0_commit = repo
        .find_commit(git2::Oid::from_str(&c0).expect("parse C0 oid"))
        .expect("find C0 commit");
    let _c1 = commit_all(&repo, "C1 edit watched file", &[&c0_commit]);

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    // UAT pass at C0.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000009902",
                "story_id": STORY_ID,
                "verdict": "pass",
                "commit": c0,
                "signed_at": "2026-04-18T00:00:00Z",
            }),
        )
        .expect("seed uat pass at C0");
    // test_runs verdict=pass at HEAD. Failing-tests is NOT the driver
    // here — only the file intersection is.
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

    let dashboard = Dashboard::with_repo(
        store.clone(),
        stories_dir.clone(),
        PathBuf::from(repo_root),
    );

    // Table-mode classification must be `unhealthy`.
    let rendered = dashboard
        .render_table()
        .expect("render_table should succeed on a two-commit repo");
    let row = rendered
        .lines()
        .find(|line| line.contains(&STORY_ID.to_string()))
        .unwrap_or_else(|| {
            panic!(
                "rendered table must contain a row for story {STORY_ID}; got:\n{rendered}"
            )
        });
    assert!(
        row.contains("unhealthy"),
        "story {STORY_ID} must classify as `unhealthy` when a related file \
         changed between UAT commit and HEAD; got row: {row:?}\n\
         full table:\n{rendered}"
    );

    // Reason channel — JSON mode must grow a `stale_related_files`
    // array listing the matched paths, per the story's "Unhealthy
    // reason channel" guidance. This is how consumers distinguish
    // "unhealthy because tests" from "unhealthy because files" without
    // scraping strings.
    let json_output = dashboard
        .render_json()
        .expect("render_json should succeed on the same fixture");
    let parsed: Value = serde_json::from_str(&json_output)
        .unwrap_or_else(|e| panic!("render_json output must parse as JSON: {e}; raw:\n{json_output}"));
    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level must have `stories` as an array; got: {parsed}"));
    let this_story = stories
        .iter()
        .find(|s| s.get("id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64))
        .unwrap_or_else(|| panic!("stories[] must include an entry for id {STORY_ID}; got: {parsed}"));

    let stale = this_story
        .get("stale_related_files")
        .unwrap_or_else(|| {
            panic!(
                "unhealthy-for-file-reason row must carry a `stale_related_files` \
                 field so the reason channel is distinguishable from failing tests; \
                 got row: {this_story}"
            )
        });
    let stale_arr = stale.as_array().unwrap_or_else(|| {
        panic!(
            "`stale_related_files` must be a JSON array; got {stale:?} on row {this_story}"
        )
    });
    assert!(
        !stale_arr.is_empty(),
        "`stale_related_files` must be non-empty on a row that is unhealthy \
         because of a file intersection; got empty array on row {this_story}"
    );
    // The specific changed path must appear (full path, not basename —
    // per the story's guidance "full paths, not basenames").
    let stale_has_watched_path = stale_arr.iter().any(|v| {
        v.as_str()
            .map(|s| s.contains("crates/agentic-uat/src/lib.rs"))
            .unwrap_or(false)
    });
    assert!(
        stale_has_watched_path,
        "`stale_related_files` must list the changed file `crates/agentic-uat/src/lib.rs` \
         as the full path; got {stale_arr:?} on row {this_story}"
    );

    // And the reason must be distinguishable from "tests went red":
    // failing_tests must NOT be populated for this row — the row is
    // unhealthy despite test_runs=pass.
    let failing_tests = this_story
        .get("failing_tests")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("failing_tests must be emitted as an array; got row {this_story}"));
    assert!(
        failing_tests.is_empty(),
        "failing_tests must be empty on a row that is unhealthy ONLY because \
         of a file intersection (test_runs=pass); got failing_tests={failing_tests:?}"
    );
}
