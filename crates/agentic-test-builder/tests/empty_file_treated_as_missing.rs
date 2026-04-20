//! Story 7 acceptance test: empty or whitespace-only file is SCAFFOLDED.
//!
//! Justification (from stories/7.yml): Proves the empty-file
//! classification: given a story whose acceptance.tests[] entry points
//! at a file that exists on disk but whose content is empty (zero bytes)
//! or whitespace-only, `TestBuilder::run` treats it as SCAFFOLD (writes
//! a real failing test into it and records a `red` verdict), not
//! PRESERVED. The process.yml contract is "preserved means non-empty
//! content"; without a test pinning the empty-file case, a zero-byte
//! file left behind by a prior failed run would be forever preserved as
//! a no-op — re-rennying would never happen, and the story would never
//! get a real scaffold.
//!
//! The scaffold creates a fixture story with TWO acceptance.tests[]
//! entries: one pointing at a zero-byte file and one pointing at a
//! whitespace-only file (spaces + newlines only). Both must be
//! reclassified as SCAFFOLD: their content after the run must be
//! non-empty (the scaffold was written), their evidence rows must
//! carry `verdict: red` (not `preserved`). Red today is compile-red
//! via the missing `agentic_test_builder` public surface
//! (`TestBuilder`).

use std::fs;
use std::path::Path;

use agentic_test_builder::TestBuilder;
use tempfile::TempDir;

const STORY_ID: u32 = 7007;

// Deterministic Rust body the stubbed `claude` emits on stdout. The
// scaffold compiles (no external symbols) but its assertion fails at
// runtime — runtime-red. The test only asserts non-empty content +
// verdict=red + red_path is a string, so any runtime-red scaffold
// body satisfies the observable.
const STUBBED_CLAUDE_STDOUT: &str = r#"//! Story 7007 scaffold authored by stubbed `claude` shim.
#[test]
fn scaffold_asserts_unimplemented_observable() {
    assert_eq!(1u32, 2u32, "observable is not yet satisfied");
}
"#;

const FIXTURE_STORY_YAML: &str = r#"id: 7007
title: "Empty-file fixture: zero-byte and whitespace-only entries must be scaffolded"

outcome: |
  A fixture story whose two acceptance.tests[] entries exist on disk but
  carry no real content — test-builder must treat both as SCAFFOLD, not
  PRESERVED.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/empty-file-fixture/tests/zero_bytes.rs
      justification: |
        A substantive justification for the zero-byte case — the file
        exists on disk but is empty; test-builder must scaffold a real
        failing test into it, not treat it as preserved.
    - file: crates/empty-file-fixture/tests/whitespace_only.rs
      justification: |
        A substantive justification for the whitespace-only case — the
        file exists on disk but contains only spaces and newlines;
        test-builder must scaffold a real failing test into it.
  uat: |
    Drive `TestBuilder::run` against this fixture; observe both files
    rewritten with real scaffolds and red verdicts in the evidence row.

guidance: |
  Fixture authored inline for the empty-file scaffold. Not a real story.

depends_on: []
"#;

#[test]
fn empty_file_treated_as_missing_is_scaffolded_not_preserved_for_both_zero_bytes_and_whitespace_only() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    let fixture_root = repo_root.join("crates/empty-file-fixture");
    fs::create_dir_all(fixture_root.join("src")).expect("fixture src");
    fs::create_dir_all(fixture_root.join("tests")).expect("fixture tests");
    fs::write(
        fixture_root.join("Cargo.toml"),
        r#"[package]
name = "empty-file-fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write fixture Cargo.toml");
    fs::write(fixture_root.join("src/lib.rs"), b"").expect("write fixture lib.rs");

    let zero_path = fixture_root.join("tests/zero_bytes.rs");
    let whitespace_path = fixture_root.join("tests/whitespace_only.rs");
    fs::write(&zero_path, b"").expect("seed zero-byte file");
    fs::write(&whitespace_path, b"   \n\t\n   \n").expect("seed whitespace-only file");

    // Stub `claude` onto a tempdir-rooted PATH so the library's
    // subprocess wire is exercised without needing real claude auth.
    let path_override = install_claude_shim(repo_root, STUBBED_CLAUDE_STDOUT);
    std::env::set_var("PATH", &path_override);
    std::env::set_var("AGENTIC_CACHE", repo_root.join(".agentic-cache"));

    init_repo_and_commit_seed(repo_root);

    let builder = TestBuilder::new(repo_root);
    builder.run(STORY_ID).expect("happy-path run must succeed");

    // Both files now have non-empty content (the scaffolds were written).
    for path in [&zero_path, &whitespace_path] {
        let bytes = fs::read(path).expect("read post-run");
        assert!(
            !bytes.is_empty(),
            "empty-on-entry file {} must be scaffolded, not left empty",
            path.display()
        );
        let content = String::from_utf8(bytes).expect("utf-8");
        assert!(
            !content.trim().is_empty(),
            "whitespace-on-entry file {} must be scaffolded with real content",
            path.display()
        );
    }

    // Evidence rows carry `red`, not `preserved`, for both entries.
    let evidence_dir = repo_root.join("evidence/runs").join(STORY_ID.to_string());
    let rows = collect_jsonl_rows(&evidence_dir);
    assert_eq!(rows.len(), 1);
    let verdicts = rows[0]["verdicts"].as_array().expect("verdicts array");
    assert_eq!(verdicts.len(), 2);
    for v in verdicts {
        assert_eq!(
            v["verdict"].as_str(),
            Some("red"),
            "empty-file entries classify as SCAFFOLD => verdict=red, not preserved; got {v:?}"
        );
        assert!(
            v["red_path"].is_string(),
            "red verdicts carry red_path"
        );
    }
}

/// Install a `claude` shim onto a tempdir and return a PATH string
/// that prepends that tempdir — so spawning `claude` from a child
/// process finds the shim, which writes `stdout_body` verbatim on
/// stdout regardless of argv/stdin.
fn install_claude_shim(repo_root: &Path, stdout_body: &str) -> String {
    let shim_dir = repo_root.join(".bin");
    fs::create_dir_all(&shim_dir).expect("shim dir");
    let shim_path = shim_dir.join("claude");
    let script = format!(
        "#!/bin/sh\ncat <<'__AGENTIC_EOF__'\n{body}__AGENTIC_EOF__\n",
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
