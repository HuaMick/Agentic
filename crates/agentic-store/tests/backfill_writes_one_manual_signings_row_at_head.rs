//! Story 28 acceptance test: happy path — `backfill_manual_signing` writes
//! exactly one `manual_signings` row at HEAD with the documented shape.
//!
//! Justification (from stories/28.yml acceptance.tests[0]):
//!   Proves the happy path at the library boundary: given a clean
//!   working tree at HEAD, a corpus where `stories/<id>.yml` parses,
//!   its on-disk `status` is `healthy`, an
//!   `evidence/runs/<id>/*-green.jsonl` file exists in the working tree
//!   at HEAD, and HEAD's history contains a commit that flipped
//!   `stories/<id>.yml` from `under_construction` to `healthy`, the
//!   library entry point `Store::backfill_manual_signing(story_id)`
//!   writes exactly one row to the `manual_signings` table whose fields
//!   are `story_id=<id>`, `verdict="pass"`, `commit=<HEAD-SHA>`,
//!   `signer=<resolved-via-story-18-chain>`, `signed_at=<now-rfc3339>`,
//!   AND a distinguishing `source="manual-backfill"` field so an
//!   auditor reading the row knows it came from the backfill path,
//!   not from `agentic uat`. Zero rows are written to `uat_signings`.
//!
//! Red today is compile-red: `Store::backfill_manual_signing` does not
//! yet exist on the trait surface, so `cargo check` fails with an
//! unresolved-method error. When build-rust lands the trait method,
//! the row-shape assertions below become the runtime contract for the
//! single row the call writes.
//!
//! Method signature this scaffold pins (matches story 28 guidance —
//! "Putting the backfill entry point on the existing `Store` trait —
//! `Store::backfill_manual_signing`"):
//!     fn backfill_manual_signing(
//!         &self,
//!         story_id: u32,
//!         repo_root: &Path,
//!     ) -> Result<(), BackfillError>;
//! Build-rust may rename `repo_root` or thread a `Resolver` parameter
//! for the signer resolution; the row-shape assertions are the
//! load-bearing contract.

use std::fs;
use std::path::{Path, PathBuf};

use agentic_store::{MemStore, Store};
use tempfile::TempDir;

const STORY_ID: u32 = 28_001;
const SIGNER_EMAIL: &str = "backfill-happy@agentic.local";

const STORY_YAML_HEALTHY: &str = r#"id: 28001
title: "Fixture for story-28 backfill happy-path"

outcome: |
  Fixture used for the backfill happy-path scaffold; its YAML on disk
  must say `status: healthy` and HEAD's history must contain a commit
  flipping the YAML from `under_construction` to `healthy`.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-store/tests/backfill_writes_one_manual_signings_row_at_head.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the library entry point against this file.
  uat: |
    Run the backfill; assert one manual_signings row with the documented
    shape.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const STORY_YAML_UNDER_CONSTRUCTION: &str = r#"id: 28001
title: "Fixture for story-28 backfill happy-path"

outcome: |
  Fixture used for the backfill happy-path scaffold; its YAML on disk
  must say `status: healthy` and HEAD's history must contain a commit
  flipping the YAML from `under_construction` to `healthy`.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-store/tests/backfill_writes_one_manual_signings_row_at_head.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the library entry point against this file.
  uat: |
    Run the backfill; assert one manual_signings row with the documented
    shape.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const GREEN_JSONL: &str = "{\"run_id\":\"aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee\",\"story_id\":28001,\"commit\":\"deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\",\"timestamp\":\"2026-04-29T00:00:00Z\",\"verdicts\":[{\"file\":\"crates/agentic-store/tests/backfill_writes_one_manual_signings_row_at_head.rs\",\"verdict\":\"green\"}]}\n";

#[test]
fn backfill_manual_signing_writes_exactly_one_row_with_documented_shape_at_head() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();

    // Seed a story file in `under_construction` and commit it. This is
    // the parent of the flip commit the history guard looks for.
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, STORY_YAML_UNDER_CONSTRUCTION).expect("write under_construction yaml");

    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    fs::create_dir_all(&evidence_dir).expect("evidence dir");

    let head_sha = init_repo_seed_then_flip(
        repo_root,
        SIGNER_EMAIL,
        &story_path,
        STORY_YAML_HEALTHY,
        &evidence_dir,
    );

    // Sanity: HEAD's tree shows status: healthy.
    let on_disk = fs::read_to_string(&story_path).expect("re-read story");
    assert!(
        on_disk.contains("status: healthy"),
        "fixture precondition: YAML at HEAD must say healthy; got:\n{on_disk}"
    );

    let store = MemStore::new();

    // The library entry point this story introduces. Compile-red until
    // build-rust lands `backfill_manual_signing` on the `Store` trait.
    store
        .backfill_manual_signing(STORY_ID, repo_root)
        .expect("backfill_manual_signing must succeed on a fully-valid fixture");

    // Exactly one row in `manual_signings` for this story_id at HEAD.
    let manual_rows = store
        .query("manual_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64)
        })
        .expect("manual_signings query must succeed");
    assert_eq!(
        manual_rows.len(),
        1,
        "backfill must write exactly one manual_signings row for story {STORY_ID}; \
         got {} rows: {manual_rows:?}",
        manual_rows.len()
    );

    let row = &manual_rows[0];

    // Required scalar fields per story-28 row contract.
    assert_eq!(
        row.get("story_id").and_then(|v| v.as_u64()),
        Some(STORY_ID as u64),
        "row.story_id must equal {STORY_ID}; got row={row}"
    );
    assert_eq!(
        row.get("verdict").and_then(|v| v.as_str()),
        Some("pass"),
        "row.verdict must equal \"pass\" (the backfill never writes a fail row); got row={row}"
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

    // The distinguishing provenance field — the row's whole reason for
    // living in a separate table rather than as a `source` column on
    // `uat_signings`. Required even though the table name encodes the
    // same fact, per story 28 guidance.
    assert_eq!(
        row.get("source").and_then(|v| v.as_str()),
        Some("manual-backfill"),
        "row.source must equal \"manual-backfill\" so an auditor can tell at a glance \
         the row came from the backfill path; got row={row}"
    );

    // Zero `uat_signings` rows — the backfill writes only to
    // manual_signings. The two paths share no rows.
    let uat_rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64)
        })
        .expect("uat_signings query must succeed");
    assert!(
        uat_rows.is_empty(),
        "backfill must write zero uat_signings rows; got {} rows: {uat_rows:?}",
        uat_rows.len()
    );
}

/// Initialise a git repo, create one seed commit with the
/// `under_construction` YAML already on disk, then create a SECOND
/// commit that flips the YAML to `healthy` and adds the `*-green.jsonl`
/// evidence file. Returns HEAD's full 40-char SHA.
fn init_repo_seed_then_flip(
    root: &Path,
    email: &str,
    story_path: &Path,
    healthy_yaml: &str,
    evidence_dir: &Path,
) -> String {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder")
            .expect("set user.name");
        cfg.set_str("user.email", email).expect("set user.email");
    }

    // Seed commit: the YAML on disk already says `under_construction`.
    let mut index = repo.index().expect("repo index");
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .expect("stage all");
    index.write().expect("write index");
    let seed_tree_oid = index.write_tree().expect("write seed tree");
    let seed_tree = repo.find_tree(seed_tree_oid).expect("find seed tree");
    let sig = repo.signature().expect("repo signature");
    let seed_commit_oid = repo
        .commit(Some("HEAD"), &sig, &sig, "seed: under_construction", &seed_tree, &[])
        .expect("commit seed");
    let seed_commit = repo.find_commit(seed_commit_oid).expect("find seed commit");

    // Flip the YAML to healthy and add a `*-green.jsonl` evidence file
    // in a SECOND commit. This is the commit the history guard MUST
    // discover.
    fs::write(story_path, healthy_yaml).expect("flip yaml to healthy");
    let green_path: PathBuf = evidence_dir.join("2026-04-29T00-00-00Z-green.jsonl");
    fs::write(&green_path, GREEN_JSONL).expect("write green evidence");

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
            "story(28001): UAT promotion to healthy",
            &flip_tree,
            &[&seed_commit],
        )
        .expect("commit flip");

    format!("{}", flip_commit_oid)
}
