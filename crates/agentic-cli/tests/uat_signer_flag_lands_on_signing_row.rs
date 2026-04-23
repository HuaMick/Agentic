//! Story 18 acceptance test: `agentic uat <id> --verdict pass --signer
//! "<value>"` wires the flag through argv to the `uat_signings` row.
//!
//! Justification (from stories/18.yml acceptance.tests[11]):
//!   Proves the contract reaches the operator through the
//!   binary: `agentic uat <id> --verdict pass --signer
//!   "operator-17"` on a clean fixture repo with a
//!   resolvable git config (which the flag must override)
//!   exits 0 and writes a `uat_signings` row whose
//!   `signer` is exactly `"operator-17"`. Running the
//!   same binary with `--signer ""` exits 2
//!   (`SignerInvalid`, whitespace-rejection reaches the
//!   argv path) and writes zero rows. Running with no
//!   flag, `AGENTIC_SIGNER=env-person@example.com`
//!   exported, and a git config set, exits 0 and writes
//!   a row whose `signer` is `env-person@example.com`.
//!   Without this the library-level claims above are
//!   library-level claims only; the argv-to-resolver
//!   wire could drop `--signer`, mis-order the chain, or
//!   silently convert an empty flag value to "unset"
//!   (which would bypass the rejection) and the operator
//!   would never notice.
//!
//! Red today: runtime-red via the missing `--signer` flag on the
//! compiled `agentic uat` subcommand (clap rejects the unknown arg);
//! the test asserts the flag WINS over a resolvable git email, which
//! requires `--signer` to exist and be wired through to the resolver.

use std::fs;
use std::path::Path;

use agentic_store::{Store, SurrealStore};
use assert_cmd::Command;
use tempfile::TempDir;

const STORY_ID: u32 = 90901;

const FIXTURE_YAML: &str = r#"id: 90901
title: "Fixture story for story 18 --signer flag wire"

outcome: |
  Fixture that the CLI uat subcommand promotes to healthy with a
  --signer flag whose value must land on the signing row.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-cli/tests/uat_signer_flag_lands_on_signing_row.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the agentic binary against this file.
  uat: |
    Run `agentic uat <id> --verdict pass --signer "<value>"`.

guidance: |
  Fixture authored inline for story-18 flag-lands-on-row scaffold.
  Not a real story.

depends_on: []
"#;

#[test]
fn uat_signer_flag_wins_over_git_and_empty_flag_is_rejected_and_env_is_tier_2() {
    // --- Subtest A: --signer flag value lands on the row, winning
    // over a configured git email. ---
    let repo_tmp = TempDir::new().expect("repo tempdir");
    let repo_root = repo_tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{STORY_ID}.yml")),
        FIXTURE_YAML,
    )
    .expect("write fixture");

    init_repo_with_email(repo_root, "git-person@example.com");

    let store_tmp = TempDir::new().expect("store tempdir");
    let store_path = store_tmp.path().to_path_buf();

    std::env::remove_var("AGENTIC_SIGNER");

    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic")
        .current_dir(repo_root)
        .env_remove("AGENTIC_SIGNER")
        .arg("uat")
        .arg(STORY_ID.to_string())
        .arg("--verdict")
        .arg("pass")
        .arg("--signer")
        .arg("operator-17")
        .arg("--store")
        .arg(&store_path)
        .assert();
    let output = assert.get_output().clone();
    let status = output.status;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert_eq!(
        status.code(),
        Some(0),
        "--signer flag on a valid path must exit 0; got status={status:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let store = SurrealStore::open(&store_path).expect("reopen store");
    let rows = store
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64)
        })
        .expect("query");
    assert_eq!(rows.len(), 1, "exactly one signing row; got {rows:?}");
    assert_eq!(
        rows[0].get("signer").and_then(|v| v.as_str()),
        Some("operator-17"),
        "flag value must beat git config; got row={row}",
        row = rows[0]
    );
    drop(store);

    // --- Subtest B: --signer "" (whitespace-only) exits 2 and writes
    // zero rows — the whitespace-rejection reaches argv. ---
    const STORY_ID_B: u32 = 90902;
    let repo_b_tmp = TempDir::new().expect("repo b tempdir");
    let repo_b = repo_b_tmp.path();
    let stories_b = repo_b.join("stories");
    fs::create_dir_all(&stories_b).expect("stories b");
    let fixture_b = FIXTURE_YAML
        .replace("90901", &STORY_ID_B.to_string())
        .replace(
            "uat_signer_flag_lands_on_signing_row.rs",
            "uat_signer_flag_lands_on_signing_row_B.rs",
        );
    fs::write(stories_b.join(format!("{STORY_ID_B}.yml")), &fixture_b)
        .expect("write fixture b");
    init_repo_with_email(repo_b, "git-person@example.com");
    let store_b_tmp = TempDir::new().expect("store b tempdir");
    let assert_b = Command::cargo_bin("agentic")
        .expect("cargo_bin")
        .current_dir(repo_b)
        .env_remove("AGENTIC_SIGNER")
        .arg("uat")
        .arg(STORY_ID_B.to_string())
        .arg("--verdict")
        .arg("pass")
        .arg("--signer")
        .arg("   ")
        .arg("--store")
        .arg(store_b_tmp.path())
        .assert();
    let output_b = assert_b.get_output().clone();
    assert_eq!(
        output_b.status.code(),
        Some(2),
        "--signer \"   \" must exit 2 (whitespace-rejection at argv); got {:?}\nstderr:\n{}",
        output_b.status,
        String::from_utf8_lossy(&output_b.stderr)
    );
    let store_b = SurrealStore::open(store_b_tmp.path()).expect("reopen store b");
    let rows_b = store_b
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID_B as u64)
        })
        .expect("query b");
    assert!(
        rows_b.is_empty(),
        "whitespace-only --signer must write zero rows; got {rows_b:?}"
    );
    drop(store_b);

    // --- Subtest C: no flag, env set, git set — env wins (tier 2). ---
    const STORY_ID_C: u32 = 90903;
    let repo_c_tmp = TempDir::new().expect("repo c");
    let repo_c = repo_c_tmp.path();
    let stories_c = repo_c.join("stories");
    fs::create_dir_all(&stories_c).expect("stories c");
    let fixture_c = FIXTURE_YAML.replace("90901", &STORY_ID_C.to_string());
    fs::write(stories_c.join(format!("{STORY_ID_C}.yml")), &fixture_c)
        .expect("write c");
    init_repo_with_email(repo_c, "git-person@example.com");
    let store_c_tmp = TempDir::new().expect("store c");
    let assert_c = Command::cargo_bin("agentic")
        .expect("cargo_bin")
        .current_dir(repo_c)
        .env("AGENTIC_SIGNER", "env-person@example.com")
        .arg("uat")
        .arg(STORY_ID_C.to_string())
        .arg("--verdict")
        .arg("pass")
        .arg("--store")
        .arg(store_c_tmp.path())
        .assert();
    let output_c = assert_c.get_output().clone();
    assert_eq!(
        output_c.status.code(),
        Some(0),
        "tier-2 env resolution must exit 0; got {:?}\nstderr:\n{}",
        output_c.status,
        String::from_utf8_lossy(&output_c.stderr)
    );
    let store_c = SurrealStore::open(store_c_tmp.path()).expect("reopen c");
    let rows_c = store_c
        .query("uat_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID_C as u64)
        })
        .expect("query c");
    assert_eq!(rows_c.len(), 1);
    assert_eq!(
        rows_c[0].get("signer").and_then(|v| v.as_str()),
        Some("env-person@example.com"),
        "tier-2 env must win when no flag; got row={row}",
        row = rows_c[0]
    );
}

fn init_repo_with_email(root: &Path, email: &str) {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder").expect("set user.name");
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
    let _ = repo
        .commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
}
