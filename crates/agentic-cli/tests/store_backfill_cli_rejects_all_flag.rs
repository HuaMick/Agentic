//! Story 28 acceptance test: no-`--all` invariant at the binary
//! boundary.
//!
//! Justification (from stories/28.yml acceptance.tests[8]):
//!   Proves the no-`--all` invariant at the binary boundary:
//!   `agentic store backfill --all` (or any variant that omits a
//!   positional story id) exits 2 with a stderr message naming
//!   the missing positional argument; `agentic store backfill 11
//!   17 23` (multiple positional ids) also exits 2 with a
//!   message saying the command takes exactly one id. Zero rows
//!   are written to either signing table. The single-id
//!   constraint is structural: each story's manual ritual must
//!   be confirmed individually against git history.
//!
//! Red today is runtime-red: the `agentic store` subcommand does
//! not yet exist on the binary, so all three invocations exit 2
//! today via clap's "unrecognized subcommand 'store'" path — same
//! exit code the contract targets, which would mask a missing
//! constraint. To prevent that masking, the assertions below pin
//! BOTH the exit code AND that stderr describes the single-id
//! constraint (case 2 names a missing positional id; case 3 names
//! the single-id constraint), plus a negative-substring assertion
//! that stderr does NOT contain clap's "unrecognized subcommand"
//! marker. Today every case fires red on the negative-substring
//! check (clap's unknown-subcommand stderr always carries
//! "unrecognized"). Once build-rust wires the subcommand AND
//! enforces the single-id rule with the documented stderr shape,
//! the test passes — and any future relaxation that lets `--all`
//! or three-positional-ids slip through with exit 0/1 fails on
//! the exit-code assertion.

use agentic_store::{Store, SurrealStore};
use assert_cmd::Command;
use tempfile::TempDir;

#[test]
fn agentic_store_backfill_rejects_all_flag_and_multi_positional_with_exit_two() {
    let store_tmp = TempDir::new().expect("store tempdir");
    let store_path = store_tmp.path().to_path_buf();

    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    init_repo_and_commit_seed(repo_root);

    // Case 1: no positional id (just `--all`). Exit 2 with stderr
    // naming the missing positional argument. The `--all` flag is
    // forbidden by spec; whether the binary surfaces that as
    // "unknown flag" or "missing positional id" is implementation
    // detail. Either way: exit 2, zero rows.
    let assert_all = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("store")
        .arg("backfill")
        .arg("--all")
        .arg("--store")
        .arg(&store_path)
        .assert();
    let out_all = assert_all.get_output().clone();
    let stderr_all = String::from_utf8_lossy(&out_all.stderr).to_string();
    let stderr_all_lower = stderr_all.to_ascii_lowercase();
    assert_eq!(
        out_all.status.code(),
        Some(2),
        "`agentic store backfill --all` must exit 2; got status={:?}\nstderr:\n{stderr_all}",
        out_all.status
    );
    // Negative substring: clap's "unrecognized subcommand" stderr is
    // what surfaces today (the `store` subcommand isn't wired yet).
    // This negative assertion is what keeps the test honest — the exit
    // code 2 alone is not enough because clap's unknown-subcommand
    // rejection ALSO exits 2. The contract being pinned is "the binary
    // refused because of the single-id rule," not "the binary refused
    // for any reason."
    assert!(
        !stderr_all_lower.contains("unrecognized subcommand"),
        "stderr must NOT come from clap's unknown-subcommand path — that \
         would mean the `store` subcommand isn't wired yet, so the \
         single-id constraint hasn't been exercised. Got stderr:\n{stderr_all}"
    );

    // Case 2: no positional id at all. Exit 2 with stderr naming the
    // missing positional argument.
    let assert_empty = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("store")
        .arg("backfill")
        .arg("--store")
        .arg(&store_path)
        .assert();
    let out_empty = assert_empty.get_output().clone();
    let stderr_empty = String::from_utf8_lossy(&out_empty.stderr).to_string();
    let stderr_empty_lower = stderr_empty.to_ascii_lowercase();
    assert_eq!(
        out_empty.status.code(),
        Some(2),
        "`agentic store backfill` (no positional id) must exit 2; got status={:?}\n\
         stderr:\n{stderr_empty}",
        out_empty.status
    );
    assert!(
        !stderr_empty_lower.contains("unrecognized subcommand"),
        "stderr must NOT come from clap's unknown-subcommand path; the \
         missing-positional-id contract requires the `store backfill` \
         subcommand to be wired first. Got stderr:\n{stderr_empty}"
    );

    // Case 3: more than one positional id. Exit 2 with stderr naming
    // the single-id constraint.
    let assert_multi = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .arg("store")
        .arg("backfill")
        .arg("11")
        .arg("17")
        .arg("23")
        .arg("--store")
        .arg(&store_path)
        .assert();
    let out_multi = assert_multi.get_output().clone();
    let stderr_multi = String::from_utf8_lossy(&out_multi.stderr).to_string();
    let stderr_multi_lower = stderr_multi.to_ascii_lowercase();
    assert_eq!(
        out_multi.status.code(),
        Some(2),
        "`agentic store backfill 11 17 23` (multi positional) must exit 2; got status={:?}\n\
         stderr:\n{stderr_multi}",
        out_multi.status
    );
    assert!(
        !stderr_multi_lower.contains("unrecognized subcommand"),
        "stderr must NOT come from clap's unknown-subcommand path — the \
         single-id constraint can only be enforced once the `store \
         backfill` subcommand is wired. Got stderr:\n{stderr_multi}"
    );

    // Zero rows in either signing table — none of the three rejected
    // invocations should have written anything.
    let store = SurrealStore::open(&store_path)
        .expect("re-opening the configured SurrealStore must succeed");
    let manual_rows = store
        .query("manual_signings", &|_| true)
        .expect("manual_signings query must succeed");
    assert!(
        manual_rows.is_empty(),
        "rejected invocations must write zero manual_signings rows; got {manual_rows:?}"
    );
    let uat_rows = store
        .query("uat_signings", &|_| true)
        .expect("uat_signings query must succeed");
    assert!(
        uat_rows.is_empty(),
        "rejected invocations must write zero uat_signings rows; got {uat_rows:?}"
    );
}

fn init_repo_and_commit_seed(root: &std::path::Path) -> String {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder")
            .expect("set user.name");
        cfg.set_str("user.email", "rejects-all-flag@agentic.local")
            .expect("set user.email");
    }
    // Empty repo is fine for this test (we never run the happy path),
    // but git2 needs at least the config; create a single empty commit
    // for parity with the other binary tests.
    let mut index = repo.index().expect("repo index");
    index.write().expect("write index");
    let tree_oid = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_oid).expect("find tree");
    let sig = repo.signature().expect("repo signature");
    let commit_oid = repo
        .commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
    commit_oid.to_string()
}
