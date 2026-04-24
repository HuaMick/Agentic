//! Story 11 acceptance test (retirement-chain amendment): the
//! chain-walk is recursive, not single-hop — a chain of two
//! retirements before a healthy era head is traversed end-to-end
//! and the descendant is permitted to Pass.
//!
//! Justification (from stories/11.yml):
//! Proves the chain-walk is recursive, not single-hop: given a
//! descendant `<leaf>` whose `depends_on` names `<mid1>`, where
//! `<mid1>` is `retired (superseded_by: <mid2>)`, `<mid2>` is
//! `retired (superseded_by: <root>)`, and `<root>` is `healthy`
//! with a valid `uat_signings.verdict=pass` row, `Uat::run` with a
//! Pass verdict on `<leaf>` proceeds through its usual path and
//! signs. The gate walks both retirement hops before landing on
//! `<root>`. Without this, a second-era retirement (a story whose
//! successor was itself later retired in favour of a newer
//! replacement) would block every descendant, and the tree
//! metaphor's "eras succeeding eras" promise would decay to "one
//! retirement hop maximum." The visited-set guard on the
//! chain-walk bounds the traversal to corpus size regardless of
//! chain depth.
//!
//! Red today is compile-red: the `Status::Retired` variant the
//! fixture YAML relies on does not yet exist on the
//! `agentic_story::Status` enum, and the multi-hop chain-walk
//! code path in `Uat::run` is not yet implemented either.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_story::Status;
use agentic_uat::{StubExecutor, Uat, UatError, Verdict};
use serde_json::json;
use tempfile::TempDir;

const LEAF_ID: u32 = 11801;
const MID1_ID: u32 = 11802; // retired, superseded_by MID2_ID
const MID2_ID: u32 = 11803; // retired, superseded_by ROOT_ID
const ROOT_ID: u32 = 11804; // healthy with valid signing row

const LEAF_YAML: &str = r#"id: 11801
title: "Leaf depending on a two-hop supersession chain ending in a healthy root"

outcome: |
  Leaf fixture used to exercise the multi-hop chain-walk: depends on
  a retired ancestor whose successor is also retired, whose successor
  is finally healthy. The chain-walk must traverse BOTH retirement
  hops before landing on the healthy era head.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_walks_multi_hop_supersession_chain.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives Uat::run against this file.
  uat: |
    Run the stub executor; expect Pass (gate walks two retirement hops).

guidance: |
  Fixture authored inline for the story-11 multi-hop chain-walk
  scaffold. Not a real story.

depends_on:
  - 11802
"#;

const MID1_YAML: &str = r#"id: 11802
title: "First retired intermediary (era 1 of 3)"

outcome: |
  First retired fixture: superseded by 11803, which was itself later
  superseded by 11804. The chain-walk visits this id first, sees
  retired, and follows the superseded_by edge to MID2.

status: retired
superseded_by: 11803
retired_reason: |
  Folded into successor 11803 under the story-11 multi-hop fixture
  corpus; later superseded again by 11804 when 11803 itself retired.

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_walks_multi_hop_supersession_chain.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; first hop of the multi-hop chain. Must
  never surface in the refusal or the signing row.

depends_on: []
"#;

const MID2_YAML: &str = r#"id: 11803
title: "Second retired intermediary (era 2 of 3)"

outcome: |
  Second retired fixture: superseded by 11804. The chain-walk must
  continue past this hop rather than stopping — the algorithm is
  recursive, not single-step.

status: retired
superseded_by: 11804
retired_reason: |
  Folded into successor 11804 after a further contract shift.

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_walks_multi_hop_supersession_chain.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; second hop of the multi-hop chain. Must
  never surface in the refusal or the signing row either.

depends_on: []
"#;

const ROOT_YAML: &str = r#"id: 11804
title: "Healthy era head at the end of a two-hop supersession chain"

outcome: |
  Healthy successor at the end of the multi-hop chain: on-disk
  status=healthy AND a valid signing row seeded in the store. The
  chain-walk terminates here.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_walks_multi_hop_supersession_chain.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the healthy era head that two
  prior retirements now redirect to.

depends_on: []
"#;

#[test]
fn uat_run_pass_permits_when_multi_hop_supersession_chain_terminates_in_healthy_root() {
    // Cross-reference: Status::Retired must exist on the enum.
    let _retired_variant_exists: Status = Status::Retired;

    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let leaf_path = stories_dir.join(format!("{LEAF_ID}.yml"));
    let mid1_path = stories_dir.join(format!("{MID1_ID}.yml"));
    let mid2_path = stories_dir.join(format!("{MID2_ID}.yml"));
    let root_path = stories_dir.join(format!("{ROOT_ID}.yml"));
    fs::write(&leaf_path, LEAF_YAML).expect("write leaf fixture");
    fs::write(&mid1_path, MID1_YAML).expect("write mid1 fixture");
    fs::write(&mid2_path, MID2_YAML).expect("write mid2 fixture");
    fs::write(&root_path, ROOT_YAML).expect("write root fixture");

    let head_sha = init_repo_and_commit_seed(repo_root);

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed a valid signing row for the ROOT only. The two retired
    // intermediaries must NOT need signing rows — retirement is
    // transparent to the gate.
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
        // Neither retired intermediary may surface in a refusal.
        Err(UatError::AncestorNotHealthy { ancestor_id, .. }) => panic!(
            "multi-hop chain-walk must traverse retired intermediaries {MID1_ID} \
             and {MID2_ID} to the healthy root {ROOT_ID}, but Pass was refused with \
             AncestorNotHealthy naming ancestor_id={ancestor_id}; the chain-walk \
             is single-hop or the visited-set guard bailed early"
        ),
        Err(UatError::Cycle { edge }) => panic!(
            "a two-hop acyclic supersession chain must not be seen as a cycle; got \
             UatError::Cycle {{ edge: {edge:?} }} — the visited-set guard is over-eager"
        ),
        Err(other) => panic!(
            "multi-hop chain-walk must permit Pass when the chain ends in a healthy root; \
             got unexpected error {other:?}"
        ),
    };
    assert!(
        matches!(verdict, Verdict::Pass),
        "stub-always-pass over a two-hop retirement chain ending in healthy must yield \
         Pass; got {verdict:?}"
    );

    // Exactly one new signing row for the leaf; none for the retired
    // intermediaries.
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

    let mid1_rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(MID1_ID as u64)
        })
        .expect("store query should succeed");
    let mid2_rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(MID2_ID as u64)
        })
        .expect("store query should succeed");
    assert!(
        mid1_rows.is_empty(),
        "retired intermediary {MID1_ID} must not acquire a signing row during the \
         chain-walk; got {mid1_rows:?}"
    );
    assert!(
        mid2_rows.is_empty(),
        "retired intermediary {MID2_ID} must not acquire a signing row during the \
         chain-walk; got {mid2_rows:?}"
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
