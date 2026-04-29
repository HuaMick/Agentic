//! Story 28 acceptance test: guard 3 — history guard refuses backfill
//! when no commit in HEAD's history flipped the YAML to `healthy`.
//!
//! Justification (from stories/28.yml acceptance.tests[3]):
//!   Proves guard 3 (history guard) at the library boundary: given
//!   a clean working tree where `stories/<id>.yml`'s on-disk
//!   `status` is `healthy` AND a green-jsonl evidence file exists,
//!   but HEAD's history contains NO commit that flipped that
//!   story's YAML from anything other than `healthy` to `healthy`
//!   (e.g. the YAML was `healthy` from its first commit with no
//!   transition recorded),
//!   `Store::backfill_manual_signing(story_id)` returns
//!   `BackfillError::NoFlipInHistory { story_id }` and writes ZERO
//!   rows. The history guard is what makes the backfill correspond
//!   to a *committed* ritual, not a *staged* one.
//!
//! Red today is compile-red: `BackfillError::NoFlipInHistory` and
//! `Store::backfill_manual_signing` do not yet exist on the
//! `agentic-store` public surface.
//!
//! Fixture shape: the YAML is committed with `status: healthy` from
//! the very first commit. There is no parent commit whose tree
//! showed a non-healthy status, so the history-walk guard
//! (`git2::Revwalk` + `Tree::diff_to_tree`) finds no transition and
//! must refuse.

use std::fs;
use std::path::Path;

use agentic_store::{BackfillError, MemStore, Store};
use tempfile::TempDir;

const STORY_ID: u32 = 28_041;
const SIGNER_EMAIL: &str = "backfill-history-guard@agentic.local";

const STORY_YAML_HEALTHY_FROM_BIRTH: &str = r#"id: 28041
title: "Fixture for story-28 history-guard scaffold"

outcome: |
  Fixture used for the history-guard scaffold; YAML on disk is healthy
  but no commit in HEAD's history ever showed it as anything else, so
  the guard must refuse.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-store/tests/backfill_refuses_when_no_yaml_flip_in_head_history.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the library entry point against this file.
  uat: |
    Run the backfill; assert NoFlipInHistory and zero rows.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const GREEN_JSONL: &str = "{\"run_id\":\"aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee\",\"story_id\":28041,\"commit\":\"deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\",\"timestamp\":\"2026-04-29T00:00:00Z\",\"verdicts\":[{\"file\":\"crates/agentic-store/tests/backfill_refuses_when_no_yaml_flip_in_head_history.rs\",\"verdict\":\"green\"}]}\n";

#[test]
fn backfill_refuses_when_head_history_contains_no_flip_to_healthy() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, STORY_YAML_HEALTHY_FROM_BIRTH).expect("write healthy-from-birth yaml");

    // Green evidence file present so guard 2 passes.
    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    fs::create_dir_all(&evidence_dir).expect("evidence dir");
    fs::write(
        evidence_dir.join("2026-04-29T00-00-00Z-green.jsonl"),
        GREEN_JSONL,
    )
    .expect("write green evidence");

    // The KEY shape of this fixture: the YAML is committed AS healthy
    // in the first commit. No parent commit showed it as anything else;
    // the history walk finds no transition.
    init_repo_and_commit_healthy_from_birth(repo_root, SIGNER_EMAIL);

    // Sanity: HEAD shows healthy.
    let on_disk = fs::read_to_string(&story_path).expect("re-read story");
    assert!(
        on_disk.contains("status: healthy"),
        "fixture precondition: YAML at HEAD must say healthy; got:\n{on_disk}"
    );

    let store = MemStore::new();

    let err = store
        .backfill_manual_signing(STORY_ID, repo_root)
        .expect_err("history guard must refuse when no flip exists in HEAD's history");
    match &err {
        BackfillError::NoFlipInHistory { story_id } => {
            assert_eq!(*story_id, STORY_ID);
        }
        other => panic!(
            "history guard must surface NoFlipInHistory {{ story_id }}; got {other:?}"
        ),
    }

    let rows = store
        .query("manual_signings", &|_| true)
        .expect("manual_signings query must succeed");
    assert!(
        rows.is_empty(),
        "history-guard refusal must write zero manual_signings rows; got {rows:?}"
    );
}

fn init_repo_and_commit_healthy_from_birth(root: &Path, email: &str) -> String {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder")
            .expect("set user.name");
        cfg.set_str("user.email", email).expect("set user.email");
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
        .commit(
            Some("HEAD"),
            &sig,
            &sig,
            "seed: yaml committed healthy from birth — no flip transition",
            &tree,
            &[],
        )
        .expect("commit seed");
    commit_oid.to_string()
}
