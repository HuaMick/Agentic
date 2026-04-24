//! Story 21 acceptance test: the chain walk composes retirement
//! transparency with transitive `depends_on` traversal — a retired
//! ancestor sitting above a broken foundation must not let the
//! descendant promote.
//!
//! Justification (from stories/21.yml):
//! Proves the chain-walk is transitive in both axes at once —
//! retirement AND depends_on: given a descendant `<id>` whose
//! `depends_on` names `<A>`, where `<A>` is `retired (superseded_by:
//! <B>)`, `<B>` is `healthy` with a valid signing, but `<B>` itself
//! depends_on `<C>` which is `under_construction`, `Uat::run` with
//! a Pass verdict on `<id>` returns `UatError::AncestorNotHealthy`
//! naming `<C>` (the first non-healthy link in the combined walk).
//! The gate composes retirement-transparency with transitive
//! `depends_on` traversal — not two separate passes.
//!
//! Red today is compile-red: the `Status::Retired` variant the
//! fixture YAML relies on does not yet exist on the
//! `agentic_story::Status` enum.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_story::Status;
use agentic_uat::{StubExecutor, Uat, UatError};
use serde_json::json;
use tempfile::TempDir;

const LEAF_ID: u32 = 21401; // depends on A (retired)
const A_RETIRED_ID: u32 = 21402; // retired, superseded_by B
const B_HEALTHY_ID: u32 = 21403; // healthy + signed; depends_on C
const C_UNHEALTHY_ID: u32 = 21404; // under_construction — the first bad link

const LEAF_YAML: &str = r#"id: 21401
title: "Leaf depending on a retired ancestor whose successor depends on an unhealthy root"

outcome: |
  Leaf fixture used to exercise the retirement-transitive combined
  walk: the retired ancestor's successor is healthy on its own, but
  depends on a deeper unhealthy link.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_transitive_unhealthy_through_retirement_chain.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    Run the stub executor; expect refusal naming C (the first bad link).

guidance: |
  Fixture authored inline for the story-21 combined-walk scaffold.
  Not a real story.

depends_on:
  - 21402
"#;

const A_RETIRED_YAML: &str = r#"id: 21402
title: "Retired ancestor superseded by a healthy-but-transitively-broken successor"

outcome: |
  Retired fixture: the chain-walk redirects past this to the
  successor, which is itself transitively broken.

status: retired
superseded_by: 21403
retired_reason: |
  Folded into successor 21403 under the story-21 combined-walk
  fixture corpus.

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_transitive_unhealthy_through_retirement_chain.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the retired intermediary that
  redirects to a transitively-broken successor.

depends_on: []
"#;

const B_HEALTHY_YAML: &str = r#"id: 21403
title: "Healthy successor that transitively depends on an unhealthy root"

outcome: |
  Healthy on-disk AND a valid signing row exists; but depends_on
  21404 which is under_construction, so the combined walk must
  name 21404 as the offending link.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_transitive_unhealthy_through_retirement_chain.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the locally-healthy successor
  whose transitive ancestry is broken.

depends_on:
  - 21404
"#;

const C_UNHEALTHY_YAML: &str = r#"id: 21404
title: "Under-construction root beneath the retirement chain"

outcome: |
  Under-construction root fixture — the first non-healthy link in
  the combined retirement-plus-depends_on walk.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-uat/tests/uat_refuses_transitive_unhealthy_through_retirement_chain.rs
      justification: |
        Present so this fixture is itself schema-valid.
  uat: |
    N/A for this fixture.

guidance: |
  Fixture authored inline; serves as the broken foundation the
  combined walk must surface.

depends_on: []
"#;

#[test]
fn uat_run_pass_refuses_naming_first_bad_link_when_retirement_and_depends_on_compose() {
    // Cross-reference: Status::Retired must exist on the enum.
    let _retired_variant_exists: Status = Status::Retired;

    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let leaf_path = stories_dir.join(format!("{LEAF_ID}.yml"));
    let a_path = stories_dir.join(format!("{A_RETIRED_ID}.yml"));
    let b_path = stories_dir.join(format!("{B_HEALTHY_ID}.yml"));
    let c_path = stories_dir.join(format!("{C_UNHEALTHY_ID}.yml"));
    fs::write(&leaf_path, LEAF_YAML).expect("write leaf fixture");
    fs::write(&a_path, A_RETIRED_YAML).expect("write A (retired)");
    fs::write(&b_path, B_HEALTHY_YAML).expect("write B (healthy but transitively broken)");
    fs::write(&c_path, C_UNHEALTHY_YAML).expect("write C (under_construction root)");

    let head_sha = init_repo_and_commit_seed(repo_root);
    let leaf_bytes_before = fs::read(&leaf_path).expect("read leaf before run");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed a valid signing row for B so the chain-walk past
    // retirement finds B healthy with evidence — the refusal must
    // then surface the deeper C, not B.
    store
        .append(
            "uat_signings",
            json!({
                "id": "seeded-b-signing-row",
                "story_id": B_HEALTHY_ID,
                "verdict": "pass",
                "commit": head_sha,
                "signed_at": "2026-04-23T00:00:00Z",
            }),
        )
        .expect("seed B signing row");

    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    let err = uat.run(LEAF_ID).expect_err(
        "Pass through a retirement chain onto a transitively-broken foundation must be refused",
    );

    match err {
        UatError::AncestorNotHealthy { ancestor_id, .. } => {
            assert_eq!(
                ancestor_id, C_UNHEALTHY_ID,
                "combined retirement+depends_on walk must name the FIRST \
                 non-healthy link ({C_UNHEALTHY_ID}); got {ancestor_id}"
            );
            assert_ne!(
                ancestor_id, A_RETIRED_ID,
                "retired intermediary ({A_RETIRED_ID}) must never surface in a refusal"
            );
            assert_ne!(
                ancestor_id, B_HEALTHY_ID,
                "locally-healthy successor ({B_HEALTHY_ID}) must not be named — \
                 it is healthy with a valid signing; the refusal must reach past it \
                 to the deeper broken link"
            );
        }
        other => panic!(
            "combined walk through retirement onto transitively-unhealthy root \
             must return UatError::AncestorNotHealthy naming {C_UNHEALTHY_ID}; \
             got {other:?}"
        ),
    }

    // No leaf signing rows written.
    let leaf_rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(LEAF_ID as u64)
        })
        .expect("store query should succeed");
    assert!(
        leaf_rows.is_empty(),
        "combined-walk refusal must write zero leaf uat_signings rows; got {leaf_rows:?}"
    );

    // Leaf YAML unchanged on disk.
    let leaf_bytes_after = fs::read(&leaf_path).expect("read leaf after run");
    assert_eq!(
        leaf_bytes_after, leaf_bytes_before,
        "combined-walk refusal must not rewrite the target story YAML"
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
