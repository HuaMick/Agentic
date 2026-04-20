//! Story 14 acceptance test: scaffold body imports the target crate's symbols.
//!
//! Justification (from stories/14.yml): Proves the scaffold actually
//! exercises the story's contract at the type level: given a fixture
//! story whose justification names a symbol in a target crate
//! (`TargetStruct::do_work`) that the crate does not yet declare,
//! `TestBuilder::run` writes a scaffold that `use`s the target crate
//! and calls the named symbol (or a close analogue). `cargo check
//! --workspace --tests` fails with a rustc error attributable to the
//! unresolved import, and the red-state JSONL row carries
//! `red_path: "compile"` with `diagnostic` equal to the first line of
//! the rustc error. Without this, the scaffold body could be a
//! panic-stub that never names the target crate's symbols —
//! ungreenable by any implementation, which is the exact gap this
//! story closes.
//!
//! The scaffold stubs `claude` out by placing a deterministic shell
//! script on `PATH` via a `TempDir`-rooted override. The shim emits a
//! Rust source that `use`s the target crate's declared name and calls
//! `TargetStruct::do_work()` — a symbol the fixture crate's `src/`
//! does NOT declare. The resulting scaffold must therefore reference
//! the missing symbol on disk (bytes check), and the evidence row
//! must classify it as `red_path: "compile"`. Red today is compile-
//! red via the missing `TestBuilder::run` behaviour that wires the
//! `claude`-authored body to the target scaffold path (the current
//! panic-stub shape ignores the stubbed `claude` entirely).

use std::fs;
use std::path::Path;

use agentic_test_builder::{TestBuilder, TestBuilderOutcome};
use tempfile::TempDir;

const STORY_ID: u32 = 14001;

const FIXTURE_STORY_YAML: &str = r#"id: 14001
title: "Claude-authored scaffold references a symbol the target crate has not declared"

outcome: |
  A fixture story whose scaffold must `use` the target crate and call
  a symbol the crate's src/ does not declare.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/imports-symbols-fixture/tests/references_missing_symbol.rs
      justification: |
        The scaffold must call the target crate's
        `TargetStruct::do_work` — a symbol `imports-symbols-fixture`'s
        `src/` does NOT yet declare, so cargo check fails with an
        unresolved-import rustc error naming that symbol. The scaffold
        body is greenable only by adding the symbol to the target
        crate's src/.
  uat: |
    Drive TestBuilder::run against this fixture; read the scaffold
    off disk and assert it `use`s the target crate and references
    TargetStruct::do_work.

guidance: |
  Fixture authored inline for story 14's scaffold-imports-symbols
  test. Not a real story.

depends_on: []
"#;

// Deterministic Rust body the stubbed `claude` emits on stdout. The
// `use` path matches the fixture crate's declared name (snake-case),
// and `TargetStruct::do_work()` is the undeclared symbol the
// justification names.
const STUBBED_CLAUDE_STDOUT: &str = r#"//! Story 14001 scaffold authored by stubbed `claude` shim.
use imports_symbols_fixture::TargetStruct;

#[test]
fn scaffold_calls_target_struct_do_work() {
    let t = TargetStruct::new();
    assert_eq!(t.do_work(), 1, "TargetStruct::do_work must return 1");
}
"#;

#[test]
fn scaffold_body_imports_target_crate_symbols_names_undeclared_symbol_and_records_compile_red() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();

    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(
        stories_dir.join(format!("{STORY_ID}.yml")),
        FIXTURE_STORY_YAML,
    )
    .expect("write fixture story");

    materialise_fixture_crate(repo_root);

    // Stub `claude` onto a tempdir-rooted PATH. The shim writes
    // STUBBED_CLAUDE_STDOUT on stdout regardless of arguments so the
    // library's subprocess wire is exercised without needing real
    // claude auth.
    let path_override = install_claude_shim(repo_root, STUBBED_CLAUDE_STDOUT);
    std::env::set_var("PATH", &path_override);

    // Isolate the cache so the run actually spawns `claude` (no
    // unrelated cache hits).
    let cache_root = repo_root.join(".agentic-cache");
    std::env::set_var("AGENTIC_CACHE", &cache_root);

    let commit = init_repo_and_commit_seed(repo_root);

    let builder = TestBuilder::new(repo_root);
    let outcome: TestBuilderOutcome = builder
        .run(STORY_ID)
        .expect("claude-backed happy-path run must succeed");

    // Scaffold was written at the declared path.
    let scaffold_path = repo_root
        .join("crates/imports-symbols-fixture/tests/references_missing_symbol.rs");
    assert!(
        scaffold_path.exists(),
        "scaffold at {} must be created",
        scaffold_path.display()
    );

    // The scaffold's bytes must `use` the target crate AND reference
    // `TargetStruct::do_work` — the symbol the justification names and
    // the fixture crate does NOT declare. A panic-stub shape would
    // fail both checks.
    let body = fs::read_to_string(&scaffold_path).expect("read scaffold");
    assert!(
        body.contains("use imports_symbols_fixture"),
        "scaffold must `use` the target crate via its declared name; got:\n{body}"
    );
    assert!(
        body.contains("TargetStruct") && body.contains("do_work"),
        "scaffold must reference the undeclared symbol TargetStruct::do_work; got:\n{body}"
    );

    // Evidence row: one verdict, red_path = compile, diagnostic is
    // the first line of the rustc error attributable to the missing
    // symbol (starts with `error`).
    let evidence_dir = repo_root.join("evidence/runs").join(STORY_ID.to_string());
    let rows = collect_jsonl_rows(&evidence_dir);
    assert_eq!(rows.len(), 1, "one evidence file expected");
    let row = &rows[0];
    assert_eq!(row["story_id"].as_u64(), Some(u64::from(STORY_ID)));
    assert_eq!(row["commit"].as_str(), Some(commit.as_str()));

    let verdicts = row["verdicts"].as_array().expect("verdicts is array");
    assert_eq!(verdicts.len(), 1);
    let v = &verdicts[0];
    assert_eq!(v["verdict"].as_str(), Some("red"));
    assert_eq!(
        v["red_path"].as_str(),
        Some("compile"),
        "a scaffold that references an undeclared symbol is compile-red"
    );
    let diag = v["diagnostic"].as_str().expect("diagnostic present");
    assert!(
        diag.starts_with("error"),
        "compile-red diagnostic must be the first line of the rustc error; got {diag:?}"
    );

    // Outcome names the single created scaffold.
    assert_eq!(outcome.created_paths().len(), 1);
}

fn materialise_fixture_crate(repo_root: &Path) {
    let crate_root = repo_root.join("crates/imports-symbols-fixture");
    fs::create_dir_all(crate_root.join("src")).expect("fixture src dir");
    fs::create_dir_all(crate_root.join("tests")).expect("fixture tests dir");
    fs::write(
        crate_root.join("Cargo.toml"),
        r#"[package]
name = "imports-symbols-fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write fixture Cargo.toml");
    // Deliberately empty: `TargetStruct::do_work` is UNDECLARED so
    // the scaffold's `use` fails to resolve — that is the compile-red
    // path.
    fs::write(
        crate_root.join("src/lib.rs"),
        "// intentionally empty — TargetStruct::do_work is the undeclared symbol\n",
    )
    .expect("write fixture lib.rs");
}

/// Install a `claude` shim onto a tempdir and return a PATH string
/// that prepends that tempdir — so spawning `claude` from a child
/// process finds the shim, which writes `stdout_body` verbatim on
/// stdout regardless of argv/stdin.
fn install_claude_shim(repo_root: &Path, stdout_body: &str) -> String {
    let shim_dir = repo_root.join(".bin");
    fs::create_dir_all(&shim_dir).expect("shim dir");
    let shim_path = shim_dir.join("claude");
    // POSIX shell shim: emit stdout_body verbatim. On non-POSIX hosts
    // the scaffold would need a .bat shim; the test suite targets the
    // WSL CI shape per the project's environment docs.
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
