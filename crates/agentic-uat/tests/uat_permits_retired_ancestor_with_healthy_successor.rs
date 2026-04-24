//! Story 11 acceptance test (retirement-chain amendment): the UAT
//! gate walks past a retired ancestor via its `superseded_by` edge
//! to a healthy successor, and permits the Pass.
//!
//! Justification (from stories/11.yml):
//! Proves the gate walks the supersession chain: given a descendant
//! `<leaf>` whose `depends_on` names `<mid>`, where `<mid>` is
//! `retired` with `superseded_by: <root>` and `<root>` is `healthy`
//! with a valid `uat_signings.verdict=pass` row, `Uat::run` with a
//! Pass verdict on `<leaf>` proceeds through its usual path, writes
//! exactly one `uat_signings.verdict=pass` row, and rewrites
//! `<leaf>`'s YAML to `status: healthy`. The retired ancestor
//! `<mid>` is transparent to the gate — the chain-walk lands on
//! `<root>` and finds it satisfied. This is the inverse of story 11's
//! original `uat_refuses_unhealthy_ancestor`: the gate must neither
//! refuse retirement as a failure nor treat it as a free pass; it
//! must follow `superseded_by` and evaluate the successor.
//!
//! Red today is compile-red: the `Status::Retired` variant the
//! fixture YAML relies on does not yet exist on the
//! `agentic_story::Status` enum (story 6's amendment adds it), and
//! `Uat::run`'s chain-walk code path is not yet implemented either.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_story::Status;
use agentic_uat::{StubExecutor, Uat, UatError, Verdict};
use serde_json::json;
use tempfile::TempDir;

// Story 11 test ids are in the 11xxx range; this group uses 11601..11603 so
// it does not collide with story 21's fixtures (21xxx) or story 11's other
// scaffolds (11001, 11101, 11201, 11301, 11401, 11501).
const LEAF_ID: u32 = 11601;
const RETIRED_MID_ID: u32 = 11602; // retired, superseded_by ROOT_ID
const ROOT_ID: u32 = 11603; // healthy with valid signing row

const LEAF_YAML: &str = r#"id: 11601
title: "Leaf depending on a retired ancestor whose successor is healthy"

outcome: |
  Fixture leaf used to exercise the chain-walk: depends on a retired
  ancestor that points at a healthy successor.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_permits_retired_ancestor_with_healthy_successor.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Run the stub executor; expect Pass (gate walks past retired).

guidance: |
  Fixture authored inline for the story-11 retirement-chain-healthy
  scaffold. Not a real story.

depends_on:
  - 11602
"#;

const RETIRED_MID_YAML: &str = r#"id: 11602
title: "Retired intermediary superseded by a healthy root"

outcome: |
  Retired fixture: the original contract was folded into successor
  11603. The chain-walk must treat this as transparent redirection.

status: retired
superseded_by: 11603
retired_reason: |
  Folded into successor 11603 under the story-11 chain-walk fixture
  corpus.

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_permits_retired_ancestor_with_healthy_successor.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the retired intermediary whose
  health responsibility has transferred to its successor.

depends_on: []
"#;

const ROOT_YAML: &str = r#"id: 11603
title: "Healthy root inheriting from a retired predecessor"

outcome: |
  Healthy successor fixture: on-disk status=healthy AND a valid
  signing row seeded in the store.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_permits_retired_ancestor_with_healthy_successor.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the healthy era head after the
  retirement.

depends_on: []
"#;

#[test]
fn uat_run_pass_permits_when_retired_ancestor_points_at_healthy_successor() {
    // Cross-reference: Status::Retired must exist on the enum. This is the
    // compile-red anchor today — the variant is added by story 6's amendment.
    let _retired_variant_exists: Status = Status::Retired;

    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let leaf_path = stories_dir.join(format!("{LEAF_ID}.yml"));
    let retired_path = stories_dir.join(format!("{RETIRED_MID_ID}.yml"));
    let root_path = stories_dir.join(format!("{ROOT_ID}.yml"));
    fs::write(&leaf_path, LEAF_YAML).expect("write leaf fixture");
    fs::write(&retired_path, RETIRED_MID_YAML).expect("write retired intermediary");
    fs::write(&root_path, ROOT_YAML).expect("write root fixture");

    let head_sha = init_repo_and_commit_seed(repo_root);

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed a valid signing row for the ROOT so the chain walk finds
    // evidence (not just the YAML claim) when it lands on the era head.
    store
        .append(
            "uat_signings",
            json!({
                "id": "seeded-root-signing-row",
                "story_id": ROOT_ID,
                "verdict": "pass",
                "commit": head_sha,
                "signed_at": "2026-04-24T00:00:00Z",
            }),
        )
        .expect("seed root signing row");

    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    let verdict = match uat.run(LEAF_ID) {
        Ok(v) => v,
        // The retired intermediary must NEVER appear in a refusal — its
        // health responsibility has transferred to the successor.
        Err(UatError::AncestorNotHealthy { ancestor_id, .. }) => panic!(
            "chain-walk must walk past retired ancestor {RETIRED_MID_ID} to its \
             healthy successor {ROOT_ID}, but Pass was refused with \
             AncestorNotHealthy naming ancestor_id={ancestor_id}"
        ),
        Err(other) => panic!(
            "chain-walk must permit Pass when the retired ancestor points at a \
             healthy successor; got unexpected error {other:?}"
        ),
    };
    assert!(
        matches!(verdict, Verdict::Pass),
        "stub-always-pass with retired-but-redirected ancestry must yield Pass; got {verdict:?}"
    );

    // Exactly one NEW signing row for the leaf (the seeded root row is
    // a separate story_id).
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

    // Leaf YAML rewritten to status: healthy.
    let rewritten = fs::read_to_string(&leaf_path).expect("re-read leaf");
    assert!(
        rewritten.contains("status: healthy"),
        "Pass promotion must rewrite leaf status to healthy; got body:\n{rewritten}"
    );

    // The retired intermediary's YAML is untouched.
    let retired_after = fs::read_to_string(&retired_path).expect("re-read retired");
    assert!(
        retired_after.contains("status: retired"),
        "retired intermediary YAML must be left untouched (still status: retired); got:\n{retired_after}"
    );
}

/// Initialise a git repo rooted at `root`, stage everything, commit, and
/// return the HEAD SHA. Duplicated from sibling scaffolds rather than
/// hoisted so each test is independently readable.
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
