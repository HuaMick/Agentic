//! Story 21 acceptance test: the UAT gate walks past a retired
//! ancestor via its `superseded_by` edge to the successor, and
//! permits the Pass when the successor is healthy with a valid
//! signing row.
//!
//! Justification (from stories/21.yml):
//! Proves the gate walks the supersession chain past a retired
//! ancestor: given a descendant `<id>` whose `depends_on` names
//! `<A>`, where `<A>` is `retired (superseded_by: <B>)` and `<B>`
//! is `healthy` with a valid `uat_signings.verdict=pass` row,
//! `Uat::run` with a Pass verdict on `<id>` proceeds through its
//! usual path, writes exactly one `uat_signings.verdict=pass` row,
//! and rewrites `<id>`'s YAML to `status: healthy`. The retired
//! ancestor `<A>` does NOT appear in any refusal — its health
//! responsibility has transferred to `<B>`.
//!
//! Red today is compile-red: `Status::Retired` does not yet exist
//! on the enum, so the fixture YAML's `status: retired` fails at
//! load time until story 6's amendment lands the fifth enum value.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_story::Status;
use agentic_uat::{StubExecutor, Uat, UatError, Verdict};
use serde_json::json;
use tempfile::TempDir;

const LEAF_ID: u32 = 21101; // depends on RETIRED_ANCESTOR_ID
const RETIRED_ANCESTOR_ID: u32 = 21102; // retired, superseded_by SUCCESSOR_ID
const SUCCESSOR_ID: u32 = 21103; // healthy with valid signing row

const LEAF_YAML: &str = r#"id: 21101
title: "Leaf depending on a retired ancestor whose successor is healthy"

outcome: |
  A fixture leaf used to exercise the chain-walk through retirement:
  depends on a retired ancestor that points at a healthy successor.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_permits_retired_ancestor_when_successor_healthy.rs
      justification: |
        Present so this fixture is itself schema-valid; the live
        test drives Uat::run against this file.
  uat: |
    Run the stub executor; expect Pass (gate walks past retired).

guidance: |
  Fixture authored inline for the story-21 chain-walk-healthy scaffold.
  Not a real story.

depends_on:
  - 21102
"#;

const RETIRED_ANCESTOR_YAML: &str = r#"id: 21102
title: "Retired ancestor superseded by a healthy successor"

outcome: |
  Retired fixture: the original contract was folded into successor
  21103. The chain-walk must treat this as transparent redirection.

status: retired
superseded_by: 21103
retired_reason: |
  Folded into successor 21103 under the story-21 chain-walk
  fixture corpus.

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_permits_retired_ancestor_when_successor_healthy.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the retired intermediary whose
  health responsibility transferred to its successor.

depends_on: []
"#;

const SUCCESSOR_YAML: &str = r#"id: 21103
title: "Healthy successor inheriting from a retired predecessor"

outcome: |
  Healthy successor fixture: on-disk status=healthy AND a valid
  signing row seeded in the store.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_permits_retired_ancestor_when_successor_healthy.rs
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
    // Cross-reference: Status::Retired must exist on the enum.
    let _retired_variant_exists: Status = Status::Retired;

    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let leaf_path = stories_dir.join(format!("{LEAF_ID}.yml"));
    let retired_path = stories_dir.join(format!("{RETIRED_ANCESTOR_ID}.yml"));
    let successor_path = stories_dir.join(format!("{SUCCESSOR_ID}.yml"));
    fs::write(&leaf_path, LEAF_YAML).expect("write leaf fixture");
    fs::write(&retired_path, RETIRED_ANCESTOR_YAML).expect("write retired ancestor");
    fs::write(&successor_path, SUCCESSOR_YAML).expect("write successor");

    let head_sha = init_repo_and_commit_seed(repo_root);

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed a valid signing row for the SUCCESSOR so the gate's
    // chain walk finds evidence (not just the YAML claim).
    store
        .append(
            "uat_signings",
            json!({
                "id": "seeded-successor-signing-row",
                "story_id": SUCCESSOR_ID,
                "verdict": "pass",
                "commit": head_sha,
                "signed_at": "2026-04-23T00:00:00Z",
            }),
        )
        .expect("seed successor signing row");

    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    let verdict = match uat.run(LEAF_ID) {
        Ok(v) => v,
        // The retired intermediary must NEVER appear in a refusal —
        // its health responsibility has transferred to the successor.
        Err(UatError::AncestorNotHealthy { ancestor_id, .. }) => panic!(
            "chain-walk must walk past retired ancestor {RETIRED_ANCESTOR_ID} to its \
             healthy successor {SUCCESSOR_ID}, but Pass was refused with \
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

    // Exactly one NEW signing row for the leaf (the seeded
    // successor row is separate).
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

    // The retired ancestor's YAML is untouched.
    let retired_after = fs::read_to_string(&retired_path).expect("re-read retired");
    assert!(
        retired_after.contains("status: retired"),
        "retired ancestor YAML must be left untouched (still status: retired); got:\n{retired_after}"
    );
}

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
