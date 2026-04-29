//! Story 28 acceptance test: guard 2 — evidence guard refuses backfill
//! when no `*-green.jsonl` evidence file exists for the story.
//!
//! Justification (from stories/28.yml acceptance.tests[2]):
//!   Proves guard 2 (evidence guard) at the library boundary: given
//!   a clean working tree where `stories/<id>.yml`'s on-disk `status`
//!   is `healthy` AND the YAML-flip commit exists in HEAD's history
//!   AND `evidence/runs/<id>/` either does not exist on disk or
//!   contains zero files matching `*-green.jsonl`,
//!   `Store::backfill_manual_signing(story_id)` returns
//!   `BackfillError::NoGreenEvidence { story_id, evidence_dir }` and
//!   writes ZERO rows.
//!
//! Red today is compile-red: `BackfillError::NoGreenEvidence` and
//! `Store::backfill_manual_signing` do not yet exist on the
//! `agentic-store` public surface.

use std::fs;
use std::path::{Path, PathBuf};

use agentic_store::{BackfillError, MemStore, Store};
use tempfile::TempDir;

const STORY_ID_NO_DIR: u32 = 28_031;
const STORY_ID_EMPTY_DIR: u32 = 28_032;
const SIGNER_EMAIL: &str = "backfill-evidence-guard@agentic.local";

fn fixture_yaml_under_construction(id: u32) -> String {
    format!(
        r#"id: {id}
title: "Fixture for story-28 evidence-guard scaffold"

outcome: |
  Fixture used for the evidence-guard scaffold; YAML at HEAD says
  healthy with a flip commit in history, but no green-jsonl evidence
  file exists.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-store/tests/backfill_refuses_when_no_green_evidence_file.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the library entry point against this file.
  uat: |
    Run the backfill; assert NoGreenEvidence and zero rows.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#
    )
}

fn fixture_yaml_healthy(id: u32) -> String {
    format!(
        r#"id: {id}
title: "Fixture for story-28 evidence-guard scaffold"

outcome: |
  Fixture used for the evidence-guard scaffold; YAML at HEAD says
  healthy with a flip commit in history, but no green-jsonl evidence
  file exists.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-store/tests/backfill_refuses_when_no_green_evidence_file.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the library entry point against this file.
  uat: |
    Run the backfill; assert NoGreenEvidence and zero rows.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#
    )
}

#[test]
fn backfill_refuses_when_evidence_dir_is_missing_or_empty_of_green_jsonl() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // Fixture 1: evidence/runs/<id>/ does not exist at all.
    let no_dir_path = stories_dir.join(format!("{STORY_ID_NO_DIR}.yml"));
    fs::write(&no_dir_path, fixture_yaml_under_construction(STORY_ID_NO_DIR))
        .expect("write fixture 1 uc");

    // Fixture 2: evidence/runs/<id>/ exists but is empty (no
    // `*-green.jsonl` file). We seed a non-green evidence file (a
    // `-red.jsonl`) to prove the guard's pattern-match is specifically
    // for `*-green.jsonl`, not for any file in the dir.
    let empty_path = stories_dir.join(format!("{STORY_ID_EMPTY_DIR}.yml"));
    fs::write(&empty_path, fixture_yaml_under_construction(STORY_ID_EMPTY_DIR))
        .expect("write fixture 2 uc");
    let evidence_dir_2: PathBuf = repo_root.join(format!("evidence/runs/{STORY_ID_EMPTY_DIR}"));
    fs::create_dir_all(&evidence_dir_2).expect("evidence dir 2");
    fs::write(
        evidence_dir_2.join("2026-04-29T00-00-00Z-red.jsonl"),
        b"{\"verdicts\":[]}\n",
    )
    .expect("write red jsonl (not green)");

    // Seed commit (parent of flip commits).
    init_repo_seed(repo_root, SIGNER_EMAIL);

    // Flip commits for both fixtures (still no green-jsonl files).
    fs::write(&no_dir_path, fixture_yaml_healthy(STORY_ID_NO_DIR)).expect("flip 1");
    fs::write(&empty_path, fixture_yaml_healthy(STORY_ID_EMPTY_DIR)).expect("flip 2");
    commit_all(repo_root, SIGNER_EMAIL, "story: flip both fixtures to healthy");

    let store = MemStore::new();

    // Fixture 1: evidence/runs/<id>/ does not exist.
    let err_no_dir = store
        .backfill_manual_signing(STORY_ID_NO_DIR, repo_root)
        .expect_err("evidence guard must refuse missing evidence dir");
    match &err_no_dir {
        BackfillError::NoGreenEvidence {
            story_id,
            evidence_dir: _,
        } => {
            assert_eq!(*story_id, STORY_ID_NO_DIR);
        }
        other => panic!(
            "evidence guard must surface NoGreenEvidence {{ story_id, evidence_dir }}; \
             got {other:?}"
        ),
    }

    // Fixture 2: evidence/runs/<id>/ exists but no `*-green.jsonl`.
    let err_empty = store
        .backfill_manual_signing(STORY_ID_EMPTY_DIR, repo_root)
        .expect_err("evidence guard must refuse evidence dir with no *-green.jsonl");
    match &err_empty {
        BackfillError::NoGreenEvidence {
            story_id,
            evidence_dir: _,
        } => {
            assert_eq!(*story_id, STORY_ID_EMPTY_DIR);
        }
        other => panic!(
            "evidence guard must surface NoGreenEvidence {{ story_id, evidence_dir }}; \
             got {other:?}"
        ),
    }

    // ZERO rows in manual_signings for either story.
    let rows = store
        .query("manual_signings", &|_| true)
        .expect("manual_signings query must succeed");
    assert!(
        rows.is_empty(),
        "evidence-guard refusal must write zero manual_signings rows; got {rows:?}"
    );
}

fn init_repo_seed(root: &Path, email: &str) -> String {
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
        .commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
    commit_oid.to_string()
}

fn commit_all(root: &Path, email: &str, message: &str) -> String {
    let repo = git2::Repository::open(root).expect("git open");
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
    let parent_ref = repo.head().expect("get HEAD");
    let parent_oid = parent_ref.target().expect("get HEAD oid");
    let parent = repo.find_commit(parent_oid).expect("find parent commit");
    let commit_oid = repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])
        .expect("commit");
    commit_oid.to_string()
}
