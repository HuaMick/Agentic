//! Story 14 acceptance test: `agentic test-build` invokes claude and writes a real scaffold.
//!
//! Justification (from stories/14.yml): Proves the contract reaches
//! the operator through the binary: running `agentic test-build
//! <fixture-id>` against a fixture `stories/` directory and a
//! stubbed `claude` binary on `PATH` (via a tempdir that prepends an
//! executable shim) invokes the shim exactly once per
//! acceptance.tests[] entry, captures stdout as the scaffold body,
//! writes parseable Rust scaffolds to the declared paths, and
//! records the red-state JSONL row with a non-empty diagnostic per
//! scaffold. The binary's exit code is 0 on success. Without this,
//! the library-level claims are library-level claims only — the
//! argv-to-subprocess wire could drop the prompt or mangle the
//! captured output and the operator would never notice.
//!
//! The scaffold drives the compiled `agentic` binary via
//! `assert_cmd` with `PATH` prepended by a tempdir containing a
//! counting `claude` shim. The shim writes a deterministic,
//! syn::parse_file-valid Rust body on stdout and increments an
//! on-disk counter so the test can verify one spawn per
//! acceptance.tests[] entry. The binary's exit code, the scaffold
//! files on disk, and the evidence JSONL are all inspected. Red
//! today is compile-red: the binary's TestBuild handler currently
//! has no wiring into the stubbed-claude subprocess path — it
//! writes panic-stubs instead, so the scaffold-body assertions will
//! fail to find the shim's stdout on disk.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

const STORY_ID: u32 = 14008;

const FIXTURE_YAML: &str = r#"id: 14008
title: "CLI fixture: agentic test-build invokes stubbed claude once per entry"

outcome: |
  A fixture whose two acceptance.tests[] entries drive the `agentic
  test-build` binary to spawn the stubbed claude twice, write two
  parseable scaffolds, and record a red-state JSONL row.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/cli-fixture/tests/first_entry.rs
      justification: |
        A substantive justification for the first scaffold; the
        stubbed claude's stdout must land verbatim on disk and the
        evidence row must carry a non-empty diagnostic.
    - file: crates/cli-fixture/tests/second_entry.rs
      justification: |
        A substantive justification for the second scaffold; the
        binary must spawn claude once for this entry too (count==2),
        the scaffold must parse, and the evidence row must carry a
        non-empty diagnostic.
  uat: |
    Run `agentic test-build` against the fixture with a stubbed
    claude on PATH; observe exit 0, two scaffolds, one evidence row.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const STUBBED_CLAUDE_STDOUT: &str = r#"//! Stubbed-claude scaffold body for story 14008.
use cli_fixture::noop;

#[test]
fn stubbed_body_asserts_observable() {
    assert_eq!(noop(), 0, "noop() must return 0 for the fixture's observable");
}
"#;

#[test]
fn test_build_invokes_claude_and_writes_real_scaffold_once_per_acceptance_test_entry() {
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(stories_dir.join(format!("{STORY_ID}.yml")), FIXTURE_YAML)
        .expect("write fixture story");

    materialise_fixture_crate(repo_root);

    // Stubbed-claude shim + invocation counter.
    let counter_path = repo_root.join(".bin/counter");
    let path_override = install_counting_shim(repo_root, STUBBED_CLAUDE_STDOUT, &counter_path);

    init_repo_and_commit_seed(repo_root);

    let first_path = repo_root.join("crates/cli-fixture/tests/first_entry.rs");
    let second_path = repo_root.join("crates/cli-fixture/tests/second_entry.rs");
    let evidence_dir = repo_root.join("evidence/runs").join(STORY_ID.to_string());

    // Drive the compiled `agentic` binary via assert_cmd.
    let assert = Command::cargo_bin("agentic")
        .expect("cargo_bin agentic must resolve")
        .current_dir(repo_root)
        .env("PATH", &path_override)
        .env("AGENTIC_CACHE", repo_root.join(".agentic-cache"))
        .arg("test-build")
        .arg(STORY_ID.to_string())
        .assert();

    let output = assert.get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert_eq!(
        output.status.code(),
        Some(0),
        "test-build must exit 0 on success; got status={:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status
    );

    // Counter: the shim was invoked exactly once per
    // acceptance.tests[] entry (two entries => two spawns).
    let count = fs::read_to_string(&counter_path)
        .map(|s| s.trim().parse::<u32>().unwrap_or(0))
        .unwrap_or(0);
    assert_eq!(
        count, 2,
        "stubbed claude must be spawned exactly once per acceptance.tests[] entry; got {count}"
    );

    // Both scaffolds landed and their bytes parse as Rust.
    for path in [&first_path, &second_path] {
        assert!(path.exists(), "scaffold at {} must exist", path.display());
        let body = fs::read_to_string(path).expect("read scaffold");
        syn::parse_file(&body)
            .unwrap_or_else(|e| panic!("scaffold at {} must parse; {e}", path.display()));
        // The scaffold's bytes must be the stubbed claude's stdout —
        // not a panic-stub. That is the whole point of story 14.
        assert!(
            body.contains("stubbed_body_asserts_observable"),
            "scaffold must be the stubbed claude's stdout, not a panic-stub; got:\n{body}"
        );
    }

    // Evidence: one JSONL row with two verdicts, each with a
    // non-empty diagnostic.
    let rows = collect_jsonl_rows(&evidence_dir);
    assert_eq!(rows.len(), 1, "one evidence file expected");
    let row = &rows[0];
    assert_eq!(row["story_id"].as_u64(), Some(u64::from(STORY_ID)));
    let verdicts: &Vec<Value> = row["verdicts"].as_array().expect("verdicts is array");
    assert_eq!(verdicts.len(), 2);
    for v in verdicts {
        assert_eq!(v["verdict"].as_str(), Some("red"));
        let diag = v["diagnostic"].as_str().expect("diagnostic present");
        assert!(
            !diag.is_empty(),
            "each verdict must carry a non-empty diagnostic; got {v:?}"
        );
    }
}

fn materialise_fixture_crate(repo_root: &Path) {
    let crate_root = repo_root.join("crates/cli-fixture");
    fs::create_dir_all(crate_root.join("src")).expect("fixture src dir");
    fs::create_dir_all(crate_root.join("tests")).expect("fixture tests dir");
    fs::write(
        crate_root.join("Cargo.toml"),
        r#"[package]
name = "cli-fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write fixture Cargo.toml");
    fs::write(
        crate_root.join("src/lib.rs"),
        "pub fn noop() -> u32 { 0 }\n",
    )
    .expect("write fixture lib.rs");
}

fn install_counting_shim(repo_root: &Path, stdout_body: &str, counter_path: &Path) -> String {
    let shim_dir = repo_root.join(".bin");
    fs::create_dir_all(&shim_dir).expect("shim dir");
    fs::write(counter_path, "0").expect("init counter");
    let shim_path = shim_dir.join("claude");
    let script = format!(
        "#!/bin/sh\nCOUNTER_PATH='{counter}'\nN=$(cat \"$COUNTER_PATH\")\nN_NEXT=$((N + 1))\necho \"$N_NEXT\" > \"$COUNTER_PATH\"\ncat <<'__AGENTIC_EOF__'\n{body}__AGENTIC_EOF__\n",
        counter = counter_path.display(),
        body = stdout_body
    );
    fs::write(&shim_path, script).expect("write shim");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&shim_path).expect("shim metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&shim_path, perms).expect("chmod shim");
    }
    let old_path = std::env::var("PATH").unwrap_or_default();
    format!("{}:{}", shim_dir.display(), old_path)
}

fn collect_jsonl_rows(dir: &Path) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    if !dir.exists() {
        panic!("evidence directory missing: {}", dir.display());
    }
    for entry in fs::read_dir(dir).expect("read evidence dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let Some(ext) = path.extension() else { continue };
        if ext != "jsonl" {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read jsonl");
        for line in content.lines().filter(|l| !l.trim().is_empty()) {
            let v: serde_json::Value = serde_json::from_str(line).expect("valid JSON");
            out.push(v);
        }
    }
    out
}

fn init_repo_and_commit_seed(root: &Path) -> String {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder").expect("set user.name");
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
