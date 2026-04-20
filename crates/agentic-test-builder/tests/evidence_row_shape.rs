//! Story 7 acceptance test: JSONL evidence row shape.
//!
//! Justification (from stories/7.yml): Proves the evidence shape that
//! downstream agents (build-rust, `agentic uat`, `agentic-verify`) rely
//! on: after a happy-path run the JSONL row contains exactly the keys
//! `run_id` (UUID v4), `story_id`, `commit` (40-hex HEAD SHA),
//! `timestamp` (ISO-8601 UTC), and `verdicts`. `verdicts` is an array
//! with one entry per acceptance.tests[].file keyed by `file`, with
//! `verdict` in {`red`, `preserved`}. Red rows carry two additional
//! fields: `red_path` in {`compile`, `runtime`} and `diagnostic` — the
//! first line of the rustc error for compile-red, the first line of the
//! panic message for runtime-red. Preserved rows carry neither
//! `red_path` nor `diagnostic`.
//!
//! The scaffold runs a happy-path fixture (one preserved file, one
//! scaffolded file) and asserts the exact key set of the top-level row
//! and the per-verdict shape for each kind. The assertions pin every
//! key downstream parsers rely on — a future change that adds, removes,
//! or renames one of them must update this scaffold deliberately. Red
//! today is compile-red via the missing `agentic_test_builder` public
//! surface (`TestBuilder`).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use agentic_test_builder::TestBuilder;
use tempfile::TempDir;

const STORY_ID: u32 = 7006;

const EXISTING_BYTES: &[u8] = b"//! Pre-existing test.\n#[test]\nfn already() { panic!(\"already\"); }\n";

// Deterministic Rust body the stubbed `claude` emits on stdout for the
// missing (scaffolded_entry) acceptance entry. The fixture crate's
// src/lib.rs is empty so the body uses no imports and is runtime-red
// via a false assert_eq — red_path will be "runtime" (the assertion
// accepts either compile or runtime).
const STUBBED_CLAUDE_STDOUT: &str = r#"//! Story 7006 scaffold authored by stubbed `claude` shim.
#[test]
fn scaffold_runtime_red() {
    assert_eq!(1u32, 2u32, "scaffold observable not yet satisfied");
}
"#;

const FIXTURE_STORY_YAML: &str = r#"id: 7006
title: "Evidence-shape fixture: one preserved, one scaffolded"

outcome: |
  A fixture story whose evidence row must pin every key downstream
  agents read (run_id, story_id, commit, timestamp, verdicts) and the
  per-verdict sub-shape for `red` and `preserved` kinds.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/evidence-shape-fixture/tests/preserved_entry.rs
      justification: |
        This file already exists with non-empty content; its verdict row
        must be exactly {file, verdict: preserved} — no red_path, no
        diagnostic.
    - file: crates/evidence-shape-fixture/tests/scaffolded_entry.rs
      justification: |
        This file does not yet exist; its verdict row must include
        red_path and diagnostic alongside {file, verdict: red}.
  uat: |
    Drive `TestBuilder::run` against this fixture; parse the JSONL and
    assert the exact key set.

guidance: |
  Fixture authored inline for the evidence-shape scaffold. Not a real
  story.

depends_on: []
"#;

#[test]
fn evidence_row_shape_has_exact_top_level_keys_and_per_verdict_sub_shape_for_red_and_preserved() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    let fixture_root = repo_root.join("crates/evidence-shape-fixture");
    fs::create_dir_all(fixture_root.join("src")).expect("fixture src");
    fs::create_dir_all(fixture_root.join("tests")).expect("fixture tests");
    fs::write(
        fixture_root.join("Cargo.toml"),
        r#"[package]
name = "evidence-shape-fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write fixture Cargo.toml");
    fs::write(fixture_root.join("src/lib.rs"), b"").expect("write fixture lib.rs");
    fs::write(
        fixture_root.join("tests/preserved_entry.rs"),
        EXISTING_BYTES,
    )
    .expect("seed preserved file");

    // Stub `claude` onto a tempdir-rooted PATH so the library's
    // subprocess wire is exercised without needing real claude auth.
    let path_override = install_claude_shim(repo_root, STUBBED_CLAUDE_STDOUT);
    std::env::set_var("PATH", &path_override);
    std::env::set_var("AGENTIC_CACHE", repo_root.join(".agentic-cache"));

    let commit = init_repo_and_commit_seed(repo_root);

    let builder = TestBuilder::new(repo_root);
    builder.run(STORY_ID).expect("happy-path run must succeed");

    let evidence_dir = repo_root.join("evidence/runs").join(STORY_ID.to_string());
    let rows = collect_jsonl_rows(&evidence_dir);
    assert_eq!(rows.len(), 1, "exactly one evidence file");
    let row = &rows[0];

    // Top-level key set is exactly: run_id, story_id, commit, timestamp, verdicts.
    let top_keys: BTreeSet<&str> = row
        .as_object()
        .expect("row is object")
        .keys()
        .map(String::as_str)
        .collect();
    let expected_top: BTreeSet<&str> =
        ["run_id", "story_id", "commit", "timestamp", "verdicts"]
            .into_iter()
            .collect();
    assert_eq!(
        top_keys, expected_top,
        "top-level keys must be exactly {{run_id, story_id, commit, timestamp, verdicts}}"
    );

    // run_id: UUID v4 (version nibble = 4, variant bits = 10xx).
    let run_id = row["run_id"].as_str().expect("run_id is string");
    assert!(
        is_uuid_v4(run_id),
        "run_id must be a UUID v4 string; got {run_id:?}"
    );

    // story_id: integer matching fixture.
    assert_eq!(row["story_id"].as_u64(), Some(u64::from(STORY_ID)));

    // commit: 40-hex HEAD SHA.
    let commit_field = row["commit"].as_str().expect("commit is string");
    assert_eq!(commit_field.len(), 40, "commit must be 40-hex SHA");
    assert!(
        commit_field.chars().all(|c| c.is_ascii_hexdigit()),
        "commit must be hex; got {commit_field:?}"
    );
    assert_eq!(commit_field, commit);

    // timestamp: ISO-8601 UTC (ends with Z).
    let ts = row["timestamp"].as_str().expect("timestamp is string");
    assert!(
        ts.ends_with('Z') && ts.contains('T'),
        "timestamp must be ISO-8601 UTC ending in Z; got {ts:?}"
    );

    // Per-verdict sub-shape.
    let verdicts = row["verdicts"].as_array().expect("verdicts is array");
    assert_eq!(verdicts.len(), 2);

    let red = verdicts
        .iter()
        .find(|v| v["verdict"].as_str() == Some("red"))
        .expect("one red verdict");
    let red_keys: BTreeSet<&str> = red
        .as_object()
        .expect("red is object")
        .keys()
        .map(String::as_str)
        .collect();
    let expected_red: BTreeSet<&str> =
        ["file", "verdict", "red_path", "diagnostic"]
            .into_iter()
            .collect();
    assert_eq!(
        red_keys, expected_red,
        "red verdict keys must be exactly {{file, verdict, red_path, diagnostic}}"
    );
    let rp = red["red_path"].as_str().expect("red_path is string");
    assert!(
        rp == "compile" || rp == "runtime",
        "red_path must be one of {{compile, runtime}}; got {rp:?}"
    );

    let preserved = verdicts
        .iter()
        .find(|v| v["verdict"].as_str() == Some("preserved"))
        .expect("one preserved verdict");
    let preserved_keys: BTreeSet<&str> = preserved
        .as_object()
        .expect("preserved is object")
        .keys()
        .map(String::as_str)
        .collect();
    let expected_preserved: BTreeSet<&str> =
        ["file", "verdict"].into_iter().collect();
    assert_eq!(
        preserved_keys, expected_preserved,
        "preserved verdict keys must be exactly {{file, verdict}} — no red_path, no diagnostic"
    );
}

/// Install a `claude` shim onto a tempdir and return a PATH string
/// that prepends that tempdir — so spawning `claude` from a child
/// process finds the shim, which writes `stdout_body` verbatim on
/// stdout regardless of argv/stdin.
fn install_claude_shim(repo_root: &Path, stdout_body: &str) -> String {
    let shim_dir = repo_root.join(".bin");
    fs::create_dir_all(&shim_dir).expect("shim dir");
    let shim_path = shim_dir.join("claude");
    // Drain stdin first to avoid a Broken-pipe race — the library
    // writes the prompt to our stdin; if we exit before reading, the
    // parent's write racily fails with EPIPE.
    let script = format!(
        "#!/bin/sh\ncat >/dev/null\ncat <<'__AGENTIC_EOF__'\n{body}__AGENTIC_EOF__\n",
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

fn is_uuid_v4(s: &str) -> bool {
    // 8-4-4-4-12 hex with version '4' and variant in {8,9,a,b}.
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    if [parts[0].len(), parts[1].len(), parts[2].len(), parts[3].len(), parts[4].len()]
        != [8, 4, 4, 4, 12]
    {
        return false;
    }
    if !s.chars().all(|c| c == '-' || c.is_ascii_hexdigit()) {
        return false;
    }
    let version = parts[2].chars().next().unwrap_or('x');
    if version != '4' {
        return false;
    }
    let variant = parts[3].chars().next().unwrap_or('x');
    matches!(variant, '8' | '9' | 'a' | 'b' | 'A' | 'B')
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
