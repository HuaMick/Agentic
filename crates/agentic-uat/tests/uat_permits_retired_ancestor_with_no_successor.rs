//! Story 21 acceptance test: a retired ancestor with no
//! `superseded_by` — the terminal-retirement case — satisfies the
//! dependency edge by the fact of retirement alone.
//!
//! Justification (from stories/21.yml):
//! Proves the terminal-retirement case: given a descendant `<id>`
//! whose `depends_on` names `<A>`, where `<A>` is `retired` with
//! NO `superseded_by` field, `Uat::run` with a Pass verdict on
//! `<id>` proceeds through its usual path and signs. A retired
//! ancestor with no successor is transparent — the dependency
//! edge is satisfied by the fact of retirement alone. Without
//! this, retired stories with no successor (legitimate for
//! experiments that were tried and abandoned with no replacement)
//! would permanently block every descendant, and the only escape
//! would be hand-editing `depends_on` across the subtree.
//!
//! Red today is compile-red: the `Status::Retired` variant the
//! fixture YAML relies on does not yet exist on the
//! `agentic_story::Status` enum.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_story::Status;
use agentic_uat::{StubExecutor, Uat, UatError, Verdict};
use tempfile::TempDir;

const LEAF_ID: u32 = 21301; // depends on TERMINAL_RETIRED_ID
const TERMINAL_RETIRED_ID: u32 = 21302; // retired, no superseded_by

const LEAF_YAML: &str = r#"id: 21301
title: "Leaf depending on a terminally-retired ancestor"

outcome: |
  Leaf fixture used to exercise the terminal-retirement case: the
  ancestor is retired with no successor, and the gate must treat
  the dependency edge as satisfied.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_permits_retired_ancestor_with_no_successor.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    Run the stub executor; expect Pass (terminal retirement is transparent).

guidance: |
  Fixture authored inline for the story-21 terminal-retirement scaffold.
  Not a real story.

depends_on:
  - 21302
"#;

const TERMINAL_RETIRED_YAML: &str = r#"id: 21302
title: "Terminally-retired ancestor (no successor)"

outcome: |
  Retired fixture with no superseded_by field — an experiment
  abandoned with no replacement. The chain-walk must accept this
  as a satisfied dependency.

status: retired
retired_reason: |
  Experiment abandoned with no replacement; dependency edges
  pointing at this story are satisfied by the fact of retirement
  alone.

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_permits_retired_ancestor_with_no_successor.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the terminally-retired
  ancestor whose presence alone satisfies descendant edges.

depends_on: []
"#;

#[test]
fn uat_run_pass_permits_when_retired_ancestor_has_no_successor() {
    // Cross-reference: Status::Retired must exist on the enum.
    let _retired_variant_exists: Status = Status::Retired;

    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let leaf_path = stories_dir.join(format!("{LEAF_ID}.yml"));
    let terminal_path = stories_dir.join(format!("{TERMINAL_RETIRED_ID}.yml"));
    fs::write(&leaf_path, LEAF_YAML).expect("write leaf fixture");
    fs::write(&terminal_path, TERMINAL_RETIRED_YAML).expect("write terminal retired fixture");

    let _head_sha = init_repo_and_commit_seed(repo_root);

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    let verdict = match uat.run(LEAF_ID) {
        Ok(v) => v,
        Err(UatError::AncestorNotHealthy { ancestor_id, .. }) => panic!(
            "terminal-retirement must transparently satisfy the dependency edge, \
             but Pass was refused with AncestorNotHealthy naming \
             ancestor_id={ancestor_id}; the retired ancestor \
             {TERMINAL_RETIRED_ID} has no successor to promote — refusing here \
             would permanently block every descendant"
        ),
        Err(other) => {
            panic!("terminal-retirement must permit Pass; got unexpected error {other:?}")
        }
    };
    assert!(
        matches!(verdict, Verdict::Pass),
        "stub-always-pass over a terminally-retired ancestor must yield Pass; got {verdict:?}"
    );

    // Exactly one signing row written for the leaf.
    let leaf_rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(LEAF_ID as u64)
        })
        .expect("store query should succeed");
    assert_eq!(
        leaf_rows.len(),
        1,
        "Pass must write exactly one uat_signings row for the leaf; got {} rows: {leaf_rows:?}",
        leaf_rows.len()
    );
    assert_eq!(
        leaf_rows[0].get("verdict").and_then(|v| v.as_str()),
        Some("pass"),
        "leaf signing row must carry verdict=\"pass\"; got {}",
        leaf_rows[0]
    );

    // Leaf YAML rewritten to healthy.
    let rewritten = fs::read_to_string(&leaf_path).expect("re-read leaf");
    assert!(
        rewritten.contains("status: healthy"),
        "Pass promotion must rewrite leaf status to healthy; got body:\n{rewritten}"
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
    let commit_oid = repo
        .commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
    commit_oid.to_string()
}
