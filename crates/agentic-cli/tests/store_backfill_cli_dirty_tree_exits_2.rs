//! Story 28 acceptance test: fail-closed-on-dirty-tree at the CLI
//! boundary — `agentic store backfill <id>` exits 2 with no row writes.
//!
//! Justification (from stories/28.yml acceptance.tests[9]):
//!   Proves the fail-closed contract reaches the operator via the
//!   binary's exit code: with an uncommitted change in the fixture
//!   repo and an otherwise valid backfill target, `agentic store
//!   backfill <id>` exits 2 (not 0, not 1), writes zero rows to
//!   `manual_signings`, and emits stderr naming the dirty tree as
//!   the cause. Without this test, a wrapper that mistranslated
//!   `BackfillError::DirtyTree` to exit 1 would turn "retry after
//!   committing" into "the backfill itself failed," exactly the
//!   CI-level distinction the fail-closed-on-dirty-tree pattern's
//!   exit-code mapping exists to preserve. Reuses the same
//!   exit-code semantics story 1's `uat_dirty_tree_exits_2.rs`
//!   already pins for the UAT gate.
//!
//! Red today is runtime-red: the `agentic store` subcommand does not
//! yet exist on the binary, so `assert_cmd::Command::cargo_bin`
//! resolves the binary but the argv `["store", "backfill", "<id>",
//! ...]` is rejected by clap with an "unrecognized subcommand" exit
//! that today defaults to 2 (clap's argparse failure code) — but
//! that exit comes BEFORE the dirty-tree check fires, so the
//! contract this test pins ("dirty-tree refusal surfaces as exit 2")
//! is still unproven. Once build-rust wires the subcommand through
//! to the library's dirty-tree guard, this test becomes the
//! operator-facing contract: a dirty tree on an otherwise-valid
//! backfill target must surface as exit 2 with a stderr message
//! naming the dirty tree, NOT as exit 0 (catastrophic silent
//! attestation), NOT as exit 1 (operationally indistinguishable
//! from a real failure of the backfill operation).

use std::fs;
use std::path::{Path, PathBuf};

use agentic_store::{Store, SurrealStore};
use assert_cmd::Command;
use tempfile::TempDir;

const STORY_ID: u32 = 28_301;
const SIGNER_EMAIL: &str = "store-backfill-cli-dirty@agentic.local";

const STORY_YAML_HEALTHY: &str = r#"id: 28301
title: "Fixture for story-28 CLI dirty-tree scaffold"

outcome: |
  Fixture used for the CLI dirty-tree scaffold; YAML on disk says
  healthy with a flip commit in history and green-jsonl evidence
  present, but the working tree is dirty so the binary must refuse
  with exit 2.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/store_backfill_cli_dirty_tree_exits_2.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the agentic binary against this file.
  uat: |
    Dirty the tree; run `agentic store backfill <id>`; assert exit 2.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const STORY_YAML_UNDER_CONSTRUCTION: &str = r#"id: 28301
title: "Fixture for story-28 CLI dirty-tree scaffold"

outcome: |
  Fixture used for the CLI dirty-tree scaffold; YAML on disk says
  healthy with a flip commit in history and green-jsonl evidence
  present, but the working tree is dirty so the binary must refuse
  with exit 2.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/store_backfill_cli_dirty_tree_exits_2.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the agentic binary against this file.
  uat: |
    Dirty the tree; run `agentic store backfill <id>`; assert exit 2.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const GREEN_JSONL: &str = "{\"run_id\":\"aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee\",\"story_id\":28301,\"commit\":\"deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\",\"timestamp\":\"2026-04-29T00:00:00Z\",\"verdicts\":[{\"file\":\"crates/agentic-cli/tests/store_backfill_cli_dirty_tree_exits_2.rs\",\"verdict\":\"green\"}]}\n";

#[test]
fn agentic_store_backfill_on_dirty_tree_exits_two_writes_no_rows_and_names_dirty_tree_in_stderr() {
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, STORY_YAML_UNDER_CONSTRUCTION).expect("write uc yaml");

    let evidence_dir: PathBuf = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    fs::create_dir_all(&evidence_dir).expect("evidence dir");

    init_repo_seed_then_flip(
        repo_root,
        SIGNER_EMAIL,
        &story_path,
        STORY_YAML_HEALTHY,
        &evidence_dir,
    );

    // Dirty the working tree AFTER the flip commit by adding an
    // untracked file. `git2::Repository::statuses()` reports untracked
    // content as dirty (matches the behaviour pinned in story 1's
    // dirty-tree scaffold and in the library-level
    // `backfill_refuses_with_dirty_tree.rs`).
    fs::write(repo_root.join("dirty.txt"), b"uncommitted\n").expect("write dirty file");

    let store_tmp = TempDir::new().expect("store tempdir");
    let store_path = store_tmp.path().to_path_buf();

    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("store")
        .arg("backfill")
        .arg(STORY_ID.to_string())
        .arg("--store")
        .arg(&store_path)
        .assert();

    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let status = output.status;

    // Exit code 2 EXACTLY: the could-not-write-row contract from story
    // 28's exit-code section. 0 would be silent attestation against a
    // forgeable HEAD (catastrophic); 1 would be operationally confused
    // with a real failure of the operation. Story 28's guidance maps
    // every guard refusal — including DirtyTree — to exit 2.
    assert_eq!(
        status.code(),
        Some(2),
        "dirty-tree refusal must surface as exit 2 (could-not-write-row), \
         NOT 0 (silent attestation) or 1 (real-failure confusion); \
         got status={status:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // stderr must name the dirty tree as the cause. The exact wording
    // is implementation-detail, but the operator must be able to read
    // the message and know the next step is "commit your changes." A
    // case-insensitive substring match keeps this test stable across
    // small wording revisions while still catching the missing-message
    // shape.
    let stderr_lower = stderr.to_ascii_lowercase();
    assert!(
        stderr_lower.contains("dirty"),
        "stderr must name the dirty tree as the cause so the operator \
         knows the retry path is `git commit`, not `agentic ... again`; \
         got stderr:\n{stderr}"
    );

    // ZERO rows in manual_signings — the dirty-tree guard must refuse
    // BEFORE any write. Re-open the configured SurrealStore from the
    // same path the binary was pointed at; this is how the scaffold
    // distinguishes "wired to the right store" from "wired to a
    // default store that swallowed the write." Same idiom story 1's
    // `uat_dirty_tree_exits_2.rs` uses.
    let store = SurrealStore::open(&store_path)
        .expect("re-opening the configured SurrealStore must succeed");
    let manual_rows = store
        .query("manual_signings", &|_| true)
        .expect("manual_signings query must succeed");
    assert!(
        manual_rows.is_empty(),
        "dirty-tree refusal must write zero manual_signings rows; \
         got {} rows: {manual_rows:?}",
        manual_rows.len()
    );

    // ZERO rows in uat_signings either — backfill never touches that
    // table (only the no-double-attestation guard reads it).
    let uat_rows = store
        .query("uat_signings", &|_| true)
        .expect("uat_signings query must succeed");
    assert!(
        uat_rows.is_empty(),
        "dirty-tree refusal must write zero uat_signings rows; \
         got {} rows: {uat_rows:?}",
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

    fs::write(story_path, healthy_yaml).expect("flip yaml to healthy");
    fs::write(
        evidence_dir.join("2026-04-29T00-00-00Z-green.jsonl"),
        GREEN_JSONL,
    )
    .expect("write green evidence");

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
            "story(28301): UAT promotion to healthy",
            &flip_tree,
            &[&seed_commit],
        )
        .expect("commit flip");

    format!("{}", flip_commit_oid)
}
