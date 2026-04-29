//! Story 28 acceptance test: bootstrap mode (`--bootstrap` flag) relaxes
//! ONLY the no-green-evidence guard; every other guard (status, history,
//! dirty-tree, both no-double-attestation guards) remains in force, and
//! a successful bootstrap row is distinguished from the manual-ritual
//! row by `source: "bootstrap-cross-machine"` instead of
//! `source: "manual-backfill"`.
//!
//! Justification (from stories/28.yml acceptance.tests, the new
//! `backfill_bootstrap_skips_green_evidence_guard.rs` entry):
//!   Proves the bootstrap mode at the library boundary: given a clean
//!   working tree where `stories/<id>.yml`'s on-disk `status` is
//!   `healthy` AND HEAD's history contains a commit that flipped the
//!   YAML to `healthy` AND `evidence/runs/<id>/` either does not exist
//!   or contains ZERO files matching `*-green.jsonl` (i.e. the
//!   original signing rows and the on-disk green-jsonl trail were
//!   lost across a machine boundary), invoking the library entry
//!   point in bootstrap mode (e.g.
//!   `Store::backfill_manual_signing(story_id, BackfillMode::Bootstrap)`
//!   or the equivalent `BackfillOptions { bootstrap: true }` shape)
//!   writes EXACTLY one row to `manual_signings` whose fields match
//!   the manual-ritual happy-path row in every respect EXCEPT that
//!   `source` is the literal string `"bootstrap-cross-machine"`
//!   rather than `"manual-backfill"`. The same test ALSO proves the
//!   bootstrap mode remains gated on every other guard.
//!
//! Sub-cases pinned by this file (one `#[test]` per case for isolation):
//!   1. happy_path: YAML healthy, flip in history, NO green-jsonl,
//!      both signing tables empty, clean tree → exactly one
//!      manual_signings row with `source="bootstrap-cross-machine"`.
//!   2. dirty_tree: every other precondition met, tree dirtied →
//!      `BackfillError::DirtyTree`, zero rows.
//!   3. status_not_healthy: YAML on disk is `under_construction`
//!      under bootstrap mode → `BackfillError::StatusNotHealthy`,
//!      zero rows.
//!   4. no_flip_in_history: YAML committed healthy from birth (no
//!      transition commit) → `BackfillError::NoFlipInHistory`, zero
//!      rows.
//!   5. uat_signings_already_present: a `uat_signings.verdict=pass`
//!      row exists for the story at HEAD → `AlreadyAttested
//!      { table: "uat_signings" }`, zero new rows.
//!   6. manual_signings_already_present: a `manual_signings` row
//!      already exists for the story → `AlreadyAttested
//!      { table: "manual_signings" }`, zero new rows.
//!
//! Red today: COMPILE-RED for every sub-case. The bootstrap-mode
//! parameter does not yet exist on the `Store` trait. The scaffold
//! references `agentic_store::BackfillMode` and the three-argument
//! method `Store::backfill_manual_signing_with_mode(story_id,
//! repo_root, BackfillMode::Bootstrap)`. Either rustc symbol does not
//! resolve until build-rust lands the bootstrap surface; whichever
//! shape build-rust chooses (enum mode parameter, options struct,
//! second method), the row-shape and refusal-shape assertions
//! below are the load-bearing contract.
//!
//! Method signature this scaffold pins:
//!     pub enum BackfillMode { Manual, Bootstrap }
//!     fn backfill_manual_signing_with_mode(
//!         &self,
//!         story_id: u32,
//!         repo_root: &Path,
//!         mode: BackfillMode,
//!     ) -> Result<(), BackfillError>;
//! Build-rust may equivalently re-shape this as
//! `BackfillOptions { bootstrap: bool }` or as a `bootstrap: bool`
//! flag on the existing two-argument method; the refusal-variant
//! and `source`-string assertions below remain the contract.

use std::fs;
use std::path::{Path, PathBuf};

use agentic_store::{BackfillError, BackfillMode, MemStore, Store};
use serde_json::json;
use tempfile::TempDir;

const STORY_ID_HAPPY: u32 = 28_101;
const STORY_ID_DIRTY: u32 = 28_102;
const STORY_ID_NOT_HEALTHY: u32 = 28_103;
const STORY_ID_NO_FLIP: u32 = 28_104;
const STORY_ID_UAT_PRESENT: u32 = 28_105;
const STORY_ID_MANUAL_PRESENT: u32 = 28_106;

const SIGNER_EMAIL: &str = "backfill-bootstrap@agentic.local";

fn yaml_healthy(id: u32) -> String {
    format!(
        r#"id: {id}
title: "Fixture for story-28 bootstrap-mode scaffold"

outcome: |
  Fixture used for the bootstrap-mode scaffold; the YAML on disk says
  healthy and HEAD's history contains a flip commit, but no
  *-green.jsonl evidence file is present (cross-clone provenance loss).

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-store/tests/backfill_bootstrap_skips_green_evidence_guard.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the library entry point against this file in bootstrap
        mode.
  uat: |
    Run the backfill in bootstrap mode; assert the single row's
    `source` is `"bootstrap-cross-machine"`.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#
    )
}

fn yaml_under_construction(id: u32) -> String {
    format!(
        r#"id: {id}
title: "Fixture for story-28 bootstrap-mode scaffold (parent commit)"

outcome: |
  Parent-commit shape used to provide a flip transition in HEAD's
  history. The flip commit overwrites this with the healthy YAML.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-store/tests/backfill_bootstrap_skips_green_evidence_guard.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the library entry point against this file in bootstrap
        mode.
  uat: |
    Run the backfill in bootstrap mode; assert the single row's
    `source` is `"bootstrap-cross-machine"`.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#
    )
}

// ---------------------------------------------------------------------
// Sub-case 1: happy path
// ---------------------------------------------------------------------

#[test]
fn bootstrap_mode_writes_one_manual_signings_row_with_bootstrap_cross_machine_source_when_green_jsonl_absent()
{
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID_HAPPY}.yml"));
    fs::write(&story_path, yaml_under_construction(STORY_ID_HAPPY))
        .expect("write under_construction yaml");

    // The evidence directory is deliberately NOT created. Bootstrap mode
    // exists precisely because the green-jsonl trail did not survive the
    // machine-boundary crossing; refusing to backfill on its absence
    // would leave the cross-clone recovery shape impossible.
    let evidence_dir: PathBuf = repo_root.join(format!("evidence/runs/{STORY_ID_HAPPY}"));
    assert!(
        !evidence_dir.exists(),
        "fixture precondition: evidence/runs/{STORY_ID_HAPPY}/ must not exist on disk \
         for the bootstrap happy path; got something at {evidence_dir:?}"
    );

    let head_sha = init_repo_seed_then_flip_no_evidence(
        repo_root,
        SIGNER_EMAIL,
        &story_path,
        &yaml_healthy(STORY_ID_HAPPY),
        STORY_ID_HAPPY,
    );

    // Sanity: HEAD's tree shows status: healthy AND no green-jsonl exists.
    assert!(
        fs::read_to_string(&story_path)
            .expect("re-read story")
            .contains("status: healthy"),
        "fixture precondition: YAML at HEAD must say healthy"
    );
    assert!(
        !evidence_dir.exists()
            || fs::read_dir(&evidence_dir)
                .ok()
                .map(|mut d| d.next().is_none())
                .unwrap_or(true),
        "fixture precondition: evidence dir must be missing or empty under bootstrap"
    );

    let store = MemStore::new();

    // The library entry point this scaffold pins. Compile-red until
    // build-rust lands `BackfillMode` and the mode-aware method.
    store
        .backfill_manual_signing_with_mode(STORY_ID_HAPPY, repo_root, BackfillMode::Bootstrap)
        .expect(
            "bootstrap mode must succeed when green-jsonl is absent but every other guard passes",
        );

    let manual_rows = store
        .query("manual_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID_HAPPY as u64)
        })
        .expect("manual_signings query must succeed");
    assert_eq!(
        manual_rows.len(),
        1,
        "bootstrap happy path must write exactly one manual_signings row; got {} rows: \
         {manual_rows:?}",
        manual_rows.len()
    );

    let row = &manual_rows[0];

    // The single distinguishing-from-manual-ritual contract: the row's
    // `source` field tells an auditor which recovery shape produced it.
    assert_eq!(
        row.get("source").and_then(|v| v.as_str()),
        Some("bootstrap-cross-machine"),
        "bootstrap-mode row.source must equal \"bootstrap-cross-machine\" so an auditor \
         can tell the row came from cross-clone provenance recovery (NOT from the manual \
         ritual); got row={row}"
    );

    // The other row fields match the manual-ritual contract verbatim.
    assert_eq!(
        row.get("story_id").and_then(|v| v.as_u64()),
        Some(STORY_ID_HAPPY as u64),
        "row.story_id must equal {STORY_ID_HAPPY}; got row={row}"
    );
    assert_eq!(
        row.get("verdict").and_then(|v| v.as_str()),
        Some("pass"),
        "row.verdict must equal \"pass\" (bootstrap path never writes a fail row); \
         got row={row}"
    );
    let commit = row
        .get("commit")
        .and_then(|v| v.as_str())
        .expect("row.commit must be a string");
    assert_eq!(
        commit, head_sha,
        "row.commit must equal HEAD SHA {head_sha:?}; got {commit:?}"
    );
    assert_eq!(
        commit.len(),
        40,
        "row.commit must be a full 40-char SHA; got {commit:?}"
    );
    assert!(
        commit.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "row.commit must be lowercase hex; got {commit:?}"
    );
    let signer = row
        .get("signer")
        .and_then(|v| v.as_str())
        .expect("row.signer must be a string");
    assert!(
        !signer.trim().is_empty(),
        "row.signer must be non-empty (story 18 four-tier chain); got {signer:?}"
    );
    let signed_at = row
        .get("signed_at")
        .and_then(|v| v.as_str())
        .expect("row.signed_at must be a string");
    assert!(
        !signed_at.trim().is_empty(),
        "row.signed_at must be a non-empty RFC3339 UTC timestamp; got {signed_at:?}"
    );

    // No row landed in uat_signings; the two paths share no rows.
    let uat_rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID_HAPPY as u64)
        })
        .expect("uat_signings query must succeed");
    assert!(
        uat_rows.is_empty(),
        "bootstrap mode must write zero uat_signings rows; got {uat_rows:?}"
    );
}

// ---------------------------------------------------------------------
// Sub-case 2: dirty tree under bootstrap
// ---------------------------------------------------------------------

#[test]
fn bootstrap_mode_still_refuses_with_dirty_tree_error_when_working_tree_is_dirty() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID_DIRTY}.yml"));
    fs::write(&story_path, yaml_under_construction(STORY_ID_DIRTY))
        .expect("write under_construction yaml");

    init_repo_seed_then_flip_no_evidence(
        repo_root,
        SIGNER_EMAIL,
        &story_path,
        &yaml_healthy(STORY_ID_DIRTY),
        STORY_ID_DIRTY,
    );

    // Dirty the tree AFTER the flip commit. Bootstrap is a recovery
    // tool, not an escape hatch — the row carries a commit SHA and a
    // dirty-tree row is forgeable in exactly the same way the
    // manual-ritual contract refuses on.
    fs::write(repo_root.join("untracked-bootstrap.txt"), b"uncommitted\n")
        .expect("write untracked file");

    let store = MemStore::new();

    let err = store
        .backfill_manual_signing_with_mode(STORY_ID_DIRTY, repo_root, BackfillMode::Bootstrap)
        .expect_err(
            "bootstrap mode must NOT relax the dirty-tree guard; the row carries a \
             commit SHA and a forgeable row is worse than no row",
        );
    match &err {
        BackfillError::DirtyTree => {}
        other => panic!(
            "bootstrap mode must surface BackfillError::DirtyTree on a dirty tree; \
             got {other:?}"
        ),
    }

    let rows = store
        .query("manual_signings", &|_| true)
        .expect("manual_signings query must succeed");
    assert!(
        rows.is_empty(),
        "bootstrap dirty-tree refusal must write zero manual_signings rows; got {rows:?}"
    );
}

// ---------------------------------------------------------------------
// Sub-case 3: status not healthy under bootstrap
// ---------------------------------------------------------------------

#[test]
fn bootstrap_mode_still_refuses_with_status_not_healthy_when_yaml_status_is_not_healthy() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID_NOT_HEALTHY}.yml"));
    // YAML on disk says under_construction. Even under bootstrap, the
    // status guard refuses — bootstrap relaxes ONLY the green-jsonl
    // guard, not the YAML-claim guard.
    fs::write(&story_path, yaml_under_construction(STORY_ID_NOT_HEALTHY))
        .expect("write under_construction yaml");

    // Commit the under_construction YAML so the tree is clean.
    init_repo_single_commit(repo_root, SIGNER_EMAIL, "seed: under_construction");

    let store = MemStore::new();

    let err = store
        .backfill_manual_signing_with_mode(
            STORY_ID_NOT_HEALTHY,
            repo_root,
            BackfillMode::Bootstrap,
        )
        .expect_err(
            "bootstrap mode must NOT relax the status guard; a story whose YAML never \
             claimed healthy has nothing to recover",
        );
    match &err {
        BackfillError::StatusNotHealthy {
            story_id,
            observed_status,
        } => {
            assert_eq!(*story_id, STORY_ID_NOT_HEALTHY);
            assert_eq!(
                observed_status, "under_construction",
                "StatusNotHealthy.observed_status must echo the YAML's on-disk value; \
                 got {observed_status:?}"
            );
        }
        other => panic!(
            "bootstrap mode must surface StatusNotHealthy when the YAML status is not \
             healthy; got {other:?}"
        ),
    }

    let rows = store
        .query("manual_signings", &|_| true)
        .expect("manual_signings query must succeed");
    assert!(
        rows.is_empty(),
        "bootstrap status-guard refusal must write zero manual_signings rows; got {rows:?}"
    );
}

// ---------------------------------------------------------------------
// Sub-case 4: no flip in history under bootstrap
// ---------------------------------------------------------------------

#[test]
fn bootstrap_mode_still_refuses_with_no_flip_in_history_when_yaml_was_committed_healthy_from_birth()
{
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID_NO_FLIP}.yml"));
    // YAML is committed AS healthy in the very first commit — there is
    // no parent commit whose tree showed a non-healthy status, so the
    // history walk finds no transition.
    fs::write(&story_path, yaml_healthy(STORY_ID_NO_FLIP)).expect("write healthy-from-birth yaml");

    init_repo_single_commit(
        repo_root,
        SIGNER_EMAIL,
        "seed: yaml committed healthy from birth — no flip transition",
    );

    let store = MemStore::new();

    let err = store
        .backfill_manual_signing_with_mode(STORY_ID_NO_FLIP, repo_root, BackfillMode::Bootstrap)
        .expect_err(
            "bootstrap mode must NOT relax the history guard; the YAML claim must be in \
             committed history, not staged or always-healthy",
        );
    match &err {
        BackfillError::NoFlipInHistory { story_id } => {
            assert_eq!(*story_id, STORY_ID_NO_FLIP);
        }
        other => panic!(
            "bootstrap mode must surface NoFlipInHistory when no flip commit exists; \
             got {other:?}"
        ),
    }

    let rows = store
        .query("manual_signings", &|_| true)
        .expect("manual_signings query must succeed");
    assert!(
        rows.is_empty(),
        "bootstrap history-guard refusal must write zero manual_signings rows; got {rows:?}"
    );
}

// ---------------------------------------------------------------------
// Sub-case 5: uat_signings already present under bootstrap
// ---------------------------------------------------------------------

#[test]
fn bootstrap_mode_still_refuses_already_attested_when_uat_signings_carries_pass_row_for_story() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID_UAT_PRESENT}.yml"));
    fs::write(&story_path, yaml_under_construction(STORY_ID_UAT_PRESENT))
        .expect("write under_construction yaml");

    let head_sha = init_repo_seed_then_flip_no_evidence(
        repo_root,
        SIGNER_EMAIL,
        &story_path,
        &yaml_healthy(STORY_ID_UAT_PRESENT),
        STORY_ID_UAT_PRESENT,
    );

    let store = MemStore::new();

    // Seed a uat_signings.verdict=pass row at HEAD for this story —
    // the precondition the no-double-attestation guard refuses on.
    // Bootstrap mode must NOT relax this guard: a story already signed
    // via the real UAT path has nothing to recover.
    store
        .append(
            "uat_signings",
            json!({
                "id": "seeded-uat-row-bootstrap-case",
                "story_id": STORY_ID_UAT_PRESENT,
                "verdict": "pass",
                "commit": head_sha,
                "signed_at": "2026-04-29T00:00:00Z",
                "signer": "real-uat-pass@agentic.local",
            }),
        )
        .expect("seed uat_signings row");

    let err = store
        .backfill_manual_signing_with_mode(
            STORY_ID_UAT_PRESENT,
            repo_root,
            BackfillMode::Bootstrap,
        )
        .expect_err(
            "bootstrap mode must NOT relax the no-double-attestation guard against \
             uat_signings; the gate composition is already satisfied",
        );
    match &err {
        BackfillError::AlreadyAttested { story_id, table } => {
            assert_eq!(*story_id, STORY_ID_UAT_PRESENT);
            assert_eq!(
                table, "uat_signings",
                "AlreadyAttested.table must name the table holding the existing row \
                 (uat_signings); got {table:?}"
            );
        }
        other => panic!(
            "bootstrap mode must surface AlreadyAttested {{ table: \"uat_signings\" }}; \
             got {other:?}"
        ),
    }

    // Zero rows in manual_signings — the refusal must not even attempt
    // to write a duplicate.
    let manual_rows = store
        .query("manual_signings", &|_| true)
        .expect("manual_signings query must succeed");
    assert!(
        manual_rows.is_empty(),
        "bootstrap no-double-attestation refusal (uat_signings) must write zero \
         manual_signings rows; got {manual_rows:?}"
    );
}

// ---------------------------------------------------------------------
// Sub-case 6: manual_signings already present under bootstrap
// ---------------------------------------------------------------------

#[test]
fn bootstrap_mode_still_refuses_already_attested_when_manual_signings_already_has_a_row_for_story() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID_MANUAL_PRESENT}.yml"));
    fs::write(&story_path, yaml_under_construction(STORY_ID_MANUAL_PRESENT))
        .expect("write under_construction yaml");

    let head_sha = init_repo_seed_then_flip_no_evidence(
        repo_root,
        SIGNER_EMAIL,
        &story_path,
        &yaml_healthy(STORY_ID_MANUAL_PRESENT),
        STORY_ID_MANUAL_PRESENT,
    );

    let store = MemStore::new();

    // Seed an existing manual_signings row at HEAD. Whether the seed's
    // `source` was a prior `manual-backfill` or `bootstrap-cross-machine`
    // is irrelevant — the idempotency guard refuses on existence, not
    // provenance. We use `manual-backfill` here to mirror the existing
    // sibling test's seed shape.
    store
        .append(
            "manual_signings",
            json!({
                "id": "seeded-manual-row-bootstrap-case",
                "story_id": STORY_ID_MANUAL_PRESENT,
                "verdict": "pass",
                "commit": head_sha,
                "signed_at": "2026-04-29T00:00:00Z",
                "signer": "first-backfill@agentic.local",
                "source": "manual-backfill",
            }),
        )
        .expect("seed manual_signings row");

    let err = store
        .backfill_manual_signing_with_mode(
            STORY_ID_MANUAL_PRESENT,
            repo_root,
            BackfillMode::Bootstrap,
        )
        .expect_err(
            "bootstrap mode must NOT relax the idempotency guard; a second invocation \
             on a story already in manual_signings is a forging shape",
        );
    match &err {
        BackfillError::AlreadyAttested { story_id, table } => {
            assert_eq!(*story_id, STORY_ID_MANUAL_PRESENT);
            assert_eq!(
                table, "manual_signings",
                "AlreadyAttested.table must name manual_signings when that table holds \
                 the existing row; got {table:?}"
            );
        }
        other => panic!(
            "bootstrap mode must surface AlreadyAttested {{ table: \"manual_signings\" }}; \
             got {other:?}"
        ),
    }

    // Exactly one row in manual_signings — the seeded one. The refusal
    // must NOT add a duplicate.
    let manual_rows = store
        .query("manual_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID_MANUAL_PRESENT as u64)
        })
        .expect("manual_signings query must succeed");
    assert_eq!(
        manual_rows.len(),
        1,
        "bootstrap idempotency refusal must NOT add a duplicate manual_signings row; \
         got {} rows: {manual_rows:?}",
        manual_rows.len()
    );
}

// ---------------------------------------------------------------------
// Fixture helpers — hand-rolled per the established convention in the
// sibling backfill_*.rs scaffolds. The agentic-test-support kit ships
// only setup/fixture material at the corpus level (FixtureCorpus,
// FixtureRepo, RecordingExecutor); none of its primitives cover the
// specific shape the backfill scaffolds need (a git repo whose HEAD
// history contains a YAML-flip commit AND optionally lacks a
// green-jsonl evidence file). The sibling tests use the same
// hand-rolled init_repo_seed_then_flip helper; this scaffold mirrors
// it with the green-evidence-write step deliberately omitted.
// ---------------------------------------------------------------------

/// Initialise a git repo, create one seed commit with the
/// `under_construction` YAML already on disk, then create a SECOND
/// commit that flips the YAML to `healthy`. Unlike the manual-ritual
/// fixture helpers, this variant DOES NOT write a `*-green.jsonl`
/// evidence file — the absence of that file IS the bootstrap-mode
/// contract under test. Returns HEAD's full 40-char SHA.
fn init_repo_seed_then_flip_no_evidence(
    root: &Path,
    email: &str,
    story_path: &Path,
    healthy_yaml: &str,
    _story_id: u32,
) -> String {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder")
            .expect("set user.name");
        cfg.set_str("user.email", email).expect("set user.email");
    }

    // Seed commit: YAML on disk says `under_construction`.
    let mut index = repo.index().expect("repo index");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("stage all");
    index.write().expect("write index");
    let seed_tree_oid = index.write_tree().expect("write seed tree");
    let seed_tree = repo.find_tree(seed_tree_oid).expect("find seed tree");
    let sig = repo.signature().expect("repo signature");
    let seed_commit_oid = repo
        .commit(
            Some("HEAD"),
            &sig,
            &sig,
            "seed: under_construction",
            &seed_tree,
            &[],
        )
        .expect("commit seed");
    let seed_commit = repo.find_commit(seed_commit_oid).expect("find seed commit");

    // Flip the YAML to healthy in a SECOND commit. NO green-jsonl
    // evidence file is written; that is the bootstrap-mode contract.
    fs::write(story_path, healthy_yaml).expect("flip yaml to healthy");

    let mut index = repo.index().expect("repo index 2");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("stage all 2");
    index.write().expect("write index 2");
    let flip_tree_oid = index.write_tree().expect("write flip tree");
    let flip_tree = repo.find_tree(flip_tree_oid).expect("find flip tree");
    let flip_commit_oid = repo
        .commit(
            Some("HEAD"),
            &sig,
            &sig,
            "story(bootstrap): UAT promotion to healthy (no evidence trail)",
            &flip_tree,
            &[&seed_commit],
        )
        .expect("commit flip");

    format!("{}", flip_commit_oid)
}

/// Initialise a git repo and create a single commit with whatever
/// happens to be on disk. Used by sub-cases that don't need a flip
/// transition (the status-not-healthy and no-flip-in-history cases).
fn init_repo_single_commit(root: &Path, email: &str, message: &str) -> String {
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
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &[])
        .expect("commit seed");
    commit_oid.to_string()
}
