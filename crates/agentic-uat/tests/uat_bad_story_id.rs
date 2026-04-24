//! Story 1 acceptance test: clean failure on an unknown story id.
//!
//! Justification (from stories/1.yml): proves clean failure on bad
//! input at the library boundary — invoking `Uat::run` with a
//! non-existent story id returns `UatError::UnknownStory`, does not
//! panic, writes no row to `uat_signings`, and does not create or
//! touch any YAML file. Without this a typo or stale id could produce
//! a panic trace that looks like a system fault when it is actually
//! user error — eroding trust in the verdict precisely when we need
//! it.
//!
//! The scaffold builds a fresh clean-tree git repo with an EMPTY
//! `stories/` directory (no fixture written), invokes the
//! `Uat::run(BAD_ID)` with an id that does not exist, and asserts: a
//! typed `UatError::UnknownStory { id }` carrying the bad id, zero
//! signing rows, and no file created under `stories/`. The bad-id
//! observable is unchanged by story 1's contract — the lookup still
//! misses before any signing or signer resolution work begins.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agentic_store::{MemStore, Store};
use agentic_uat::{StubExecutor, Uat, UatError};
use tempfile::TempDir;

const BAD_ID: u32 = 99_999;

#[test]
fn uat_run_returns_unknown_story_on_unknown_id_without_panic_or_side_effects() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // Seed the repo so the tree is clean; no story fixtures inside
    // `stories/` on purpose — the lookup must miss.
    init_repo_and_commit_seed(repo_root);

    // Snapshot the empty stories dir so we can assert no file appeared.
    let before: Vec<_> = fs::read_dir(&stories_dir)
        .expect("list stories dir")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect stories dir entries");
    assert!(
        before.is_empty(),
        "test precondition: stories dir must start empty; got {before:?}"
    );

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let executor = StubExecutor::always_pass();
    let uat = Uat::new(store.clone(), executor, stories_dir.clone());

    // The call must NOT panic and must NOT return Ok. It returns a
    // typed UnknownStory error carrying the offending id.
    let err = uat
        .run(BAD_ID)
        .expect_err("unknown story id must surface as Err, not Ok");
    match err {
        UatError::UnknownStory { id } => {
            assert_eq!(
                id, BAD_ID,
                "UnknownStory must carry the offending id; got {id}, expected {BAD_ID}"
            );
        }
        other => panic!("expected UatError::UnknownStory; got {other:?}"),
    }

    // No row written.
    let rows = store
        .query("uat_signings", &|_doc| true)
        .expect("store query should succeed");
    assert!(
        rows.is_empty(),
        "unknown-story refusal must write zero uat_signings rows; got {} rows: {rows:?}",
        rows.len()
    );

    // No file created — the lookup must not have left a placeholder.
    let after: Vec<_> = fs::read_dir(&stories_dir)
        .expect("list stories dir after run")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect stories dir entries after run");
    assert!(
        after.is_empty(),
        "unknown-story refusal must not create any file under stories/; got {after:?}"
    );
}

/// See uat_pass.rs for rationale.
fn init_repo_and_commit_seed(root: &Path) -> String {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder")
            .expect("set user.name");
        cfg.set_str("user.email", "test@agentic.local")
            .expect("set user.email");
    }
    // Seed with an empty .gitkeep so `git init` produces a non-empty
    // initial tree; otherwise `write_tree` yields the empty tree and a
    // subsequent clean-tree check has nothing to diff against.
    fs::write(root.join(".gitkeep"), b"").expect("write .gitkeep");
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
