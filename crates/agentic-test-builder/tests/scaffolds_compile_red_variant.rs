//! Story 7 acceptance test: compile-red happy path.
//!
//! Justification (from stories/7.yml): Proves the compile-red happy path
//! — the second branch of the evidence row's `red_path` discriminant,
//! currently unpinned if `scaffolds_missing_files.rs` covers only the
//! runtime-red branch. Given a story whose acceptance.tests[] lists a
//! file path that does not exist on disk and whose substantive
//! justification implies a symbol (e.g. a public function, struct, or
//! method) the fixture's implementation has not yet declared,
//! `TestBuilder::run` writes a scaffold that references that symbol,
//! `cargo check --package <crate> --test <name>` fails with a rustc
//! error (not a runtime panic), and the red-state JSONL row for that
//! scaffold carries `verdict: red`, `red_path: "compile"`, and
//! `diagnostic` equal to the first line of the captured rustc error
//! output.
//!
//! The scaffold roots a fixture crate whose `src/lib.rs` does NOT
//! declare the symbol named in the justification's first sentence, so
//! when `TestBuilder::run` writes a scaffold that `use`s that symbol
//! the test-builder's own `cargo check` probe fails attributably. The
//! resulting evidence row must set `red_path` to `"compile"` and its
//! `diagnostic` to the first line of that rustc error. Red today is
//! compile-red via the missing `agentic_test_builder` public surface
//! (`TestBuilder`, `TestBuilderOutcome`).

use std::fs;
use std::path::Path;

use agentic_test_builder::{TestBuilder, TestBuilderOutcome};
use tempfile::TempDir;

const STORY_ID: u32 = 7002;

// Deterministic Rust body the stubbed `claude` emits on stdout. The
// scaffold `use`s the fixture crate's declared name (snake-case) and
// references `not_yet_declared` — the undeclared symbol the
// justification names. cargo check must fail with an unresolved-import
// rustc error attributable to that symbol, yielding compile-red.
const STUBBED_CLAUDE_STDOUT: &str = r#"//! Story 7002 scaffold authored by stubbed `claude` shim.
use compile_red_fixture::not_yet_declared;

#[test]
fn scaffold_calls_not_yet_declared() {
    let v = not_yet_declared();
    assert_eq!(v, 1, "not_yet_declared must return 1");
}
"#;

const FIXTURE_STORY_YAML: &str = r#"id: 7002
title: "Compile-red fixture story: scaffold imports a symbol that does not exist"

outcome: |
  A fixture story whose sole acceptance test references a public function
  the fixture crate has not declared — the test-builder writes the
  scaffold, observes cargo check fail with an unresolved-import rustc
  error, and records compile-red evidence.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/compile-red-fixture/tests/uses_missing_symbol.rs
      justification: |
        The fixture's public function `not_yet_declared` is referenced by
        the scaffold so cargo check fails with an unresolved-import error
        naming that symbol; the scaffold cannot even compile until the
        implementation declares the function.
  uat: |
    Drive `TestBuilder::run` against this fixture; observe one compile-red
    row in evidence/runs/7002/<ts>-red.jsonl.

guidance: |
  Fixture authored inline for the compile-red happy-path scaffold. Not a
  real story.

depends_on: []
"#;

#[test]
fn scaffolds_compile_red_variant_writes_scaffold_that_fails_cargo_check_with_unresolved_import() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    // Fixture crate exists but does NOT declare `not_yet_declared`. Any
    // scaffold that `use`s that symbol will fail cargo check with an
    // unresolved-import error — that is the compile-red path.
    materialise_compile_red_fixture(repo_root);

    // Stub `claude` onto a tempdir-rooted PATH so the library's
    // subprocess wire is exercised without needing real claude auth.
    let path_override = install_claude_shim(repo_root, STUBBED_CLAUDE_STDOUT);
    std::env::set_var("PATH", &path_override);
    std::env::set_var("AGENTIC_CACHE", repo_root.join(".agentic-cache"));

    let commit = init_repo_and_commit_seed(repo_root);

    let builder = TestBuilder::new(repo_root);
    let outcome: TestBuilderOutcome =
        builder.run(STORY_ID).expect("compile-red run must succeed");

    // Scaffold was written.
    let scaffold_path = repo_root
        .join("crates/compile-red-fixture/tests/uses_missing_symbol.rs");
    assert!(
        scaffold_path.exists(),
        "scaffold at {} must be created",
        scaffold_path.display()
    );

    // The one evidence row's red_path is "compile".
    let evidence_dir = repo_root.join("evidence/runs").join(STORY_ID.to_string());
    let rows = collect_jsonl_rows(&evidence_dir);
    assert_eq!(rows.len(), 1, "exactly one evidence file expected");
    let row = &rows[0];
    assert_eq!(row["story_id"].as_u64(), Some(u64::from(STORY_ID)));
    assert_eq!(row["commit"].as_str(), Some(commit.as_str()));

    let verdicts = row["verdicts"].as_array().expect("verdicts is array");
    assert_eq!(verdicts.len(), 1, "one scaffold => one verdict");
    let v = &verdicts[0];
    assert_eq!(v["verdict"].as_str(), Some("red"));
    assert_eq!(
        v["red_path"].as_str(),
        Some("compile"),
        "missing-symbol scaffold is compile-red, not runtime-red"
    );
    let diag = v["diagnostic"].as_str().expect("diagnostic present");
    assert!(
        diag.starts_with("error"),
        "compile-red diagnostic is the first line of the rustc error; got {diag:?}"
    );

    // The outcome summary names the created scaffold.
    assert_eq!(outcome.created_paths().len(), 1);
}

fn materialise_compile_red_fixture(repo_root: &Path) {
    let crate_root = repo_root.join("crates/compile-red-fixture");
    fs::create_dir_all(crate_root.join("src")).expect("fixture src dir");
    fs::create_dir_all(crate_root.join("tests")).expect("fixture tests dir");
    fs::write(
        crate_root.join("Cargo.toml"),
        r#"[package]
name = "compile-red-fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write fixture Cargo.toml");
    // Deliberately does NOT declare `not_yet_declared`.
    fs::write(
        crate_root.join("src/lib.rs"),
        "// intentionally empty — the symbol the scaffold references is undeclared\n",
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
