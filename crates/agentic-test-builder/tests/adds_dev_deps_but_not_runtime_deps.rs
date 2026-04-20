//! Story 7 acceptance test: narrow Cargo.toml dev-deps authority.
//!
//! Justification (from stories/7.yml): Proves the narrow Cargo.toml
//! authority granted by `agents/test/test-builder/contract.yml`:
//! test-builder MAY append entries under `[dev-dependencies]` in the
//! TARGET crate's `Cargo.toml` when a scaffold it is about to author
//! would not compile without them (happy path — the entry appears, the
//! scaffold compiles and fails red, and the run summary names each
//! added dev-dep), but MUST NOT touch `[dependencies]`,
//! `[build-dependencies]`, `[features]`, `[package]`, or any other
//! section of that Cargo.toml, nor ANY other Cargo.toml in the
//! workspace (root or sibling crates), nor `Cargo.lock`.
//!
//! The scaffold exercises two sub-scenarios against the same fixture
//! crate layout:
//!
//!   - Allowed: a scaffold whose `use` demands a dev-dep that is NOT
//!     yet in `[dev-dependencies]`. After the run, the target crate's
//!     `Cargo.toml` has the entry appended under `[dev-dependencies]`
//!     and nowhere else; the run summary names the added dev-dep;
//!     sibling Cargo.tomls and the workspace-root Cargo.toml are
//!     byte-identical to before.
//!
//!   - Forbidden: a fixture that requests (via a marker the
//!     test-builder must reject) a runtime-deps edit. `TestBuilder::run`
//!     returns `TestBuilderError::OutOfScopeEdit`, writes zero
//!     scaffolds, writes zero evidence, and every Cargo.toml in the
//!     workspace — including the target crate's — is bytes-identical
//!     to pre-run.
//!
//! Red today is compile-red via the missing `agentic_test_builder`
//! public surface (`TestBuilder`, `TestBuilderError`,
//! `TestBuilderOutcome`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use agentic_test_builder::{TestBuilder, TestBuilderError, TestBuilderOutcome};
use tempfile::TempDir;

const ALLOWED_STORY_ID: u32 = 7009;
const FORBIDDEN_STORY_ID: u32 = 70091;

// Deterministic Rust body the stubbed `claude` emits for the ALLOWED
// scenario. The scaffold `use`s `proptest` — a dev-dep the target
// crate has NOT declared — so cargo check fails with an
// `unresolved import proptest` error, which the library's
// auto-dev-dep path detects and appends to
// crates/devdep-fixture/Cargo.toml's [dev-dependencies]. The
// FORBIDDEN scenario never reaches the subprocess (the library
// short-circuits on the `runtime-dep`/`must reject` justification
// text), so no stub body is needed for it.
const STUBBED_CLAUDE_STDOUT: &str = r#"//! Story 7009 scaffold authored by stubbed `claude` shim.
use proptest::prelude::*;

proptest! {
    #[test]
    fn scaffold_uses_proptest(_x in 0u32..1) {
        assert!(true);
    }
}
"#;

const ALLOWED_STORY_YAML: &str = r#"id: 7009
title: "Dev-deps fixture: scaffold needs a dev-dep not yet declared"

outcome: |
  A fixture story whose sole scaffold requires a `proptest` dev-dep the
  target crate has not yet declared — test-builder appends it under
  [dev-dependencies] and nowhere else.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/devdep-fixture/tests/needs_dev_dep.rs
      justification: |
        A scaffold that imports a symbol from `proptest`; without the
        dev-dep entry appended to [dev-dependencies] the scaffold cannot
        compile, so test-builder MAY add it. The addition is dev-scope
        only; runtime [dependencies] must remain untouched.
  uat: |
    Drive `TestBuilder::run` against this fixture; observe the dev-dep
    appended to the target crate's [dev-dependencies] and reported in
    the run summary.

guidance: |
  Fixture authored inline for the dev-deps authority scaffold. Not a
  real story.

depends_on: []
"#;

const FORBIDDEN_STORY_YAML: &str = r#"id: 70091
title: "Runtime-deps fixture: request must be rejected as out-of-scope"

outcome: |
  A fixture story whose scaffold body is marked with the
  `//! test-builder-requests-runtime-dep: serde` directive — a runtime-
  deps edit test-builder must refuse as out-of-scope.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/devdep-fixture/tests/requests_runtime_dep.rs
      justification: |
        A substantive justification naming a runtime-dep edit
        test-builder must reject. The narrow [dev-dependencies]
        carve-out does not extend to [dependencies]; the request must
        surface as TestBuilderError::OutOfScopeEdit.
  uat: |
    Drive `TestBuilder::run` against this fixture; observe the typed
    out-of-scope error and zero Cargo.toml mutations anywhere.

guidance: |
  Fixture authored inline for the runtime-deps refusal. Not a real
  story.

depends_on: []
"#;

const TARGET_CARGO_TOML_BEFORE: &str = r#"[package]
name = "devdep-fixture"
version = "0.1.0"
edition = "2021"

[dependencies]

[dev-dependencies]
"#;

const SIBLING_CARGO_TOML: &str = r#"[package]
name = "sibling-fixture"
version = "0.1.0"
edition = "2021"
"#;

const WORKSPACE_CARGO_TOML: &str = r#"[workspace]
resolver = "2"
members = ["crates/devdep-fixture", "crates/sibling-fixture"]
"#;

#[test]
fn adds_dev_deps_but_not_runtime_deps_appends_to_dev_dependencies_only_and_rejects_runtime_dep_requests() {
    let tmp = TempDir::new().expect("tempdir");
    let repo_root = tmp.path();

    // Workspace scaffold: one target crate, one sibling, plus the
    // workspace-root Cargo.toml. All are seeded with known bytes so we
    // can assert what did and did not change.
    fs::write(repo_root.join("Cargo.toml"), WORKSPACE_CARGO_TOML).expect("write workspace toml");

    let target_root = repo_root.join("crates/devdep-fixture");
    let sibling_root = repo_root.join("crates/sibling-fixture");
    for root in [&target_root, &sibling_root] {
        fs::create_dir_all(root.join("src")).expect("mkdir src");
        fs::create_dir_all(root.join("tests")).expect("mkdir tests");
        fs::write(root.join("src/lib.rs"), b"").expect("empty lib");
    }
    fs::write(target_root.join("Cargo.toml"), TARGET_CARGO_TOML_BEFORE)
        .expect("target toml");
    fs::write(sibling_root.join("Cargo.toml"), SIBLING_CARGO_TOML)
        .expect("sibling toml");

    // Seed both fixture stories.
    fs::create_dir_all(repo_root.join("stories")).expect("stories dir");
    fs::write(
        repo_root.join(format!("stories/{ALLOWED_STORY_ID}.yml")),
        ALLOWED_STORY_YAML,
    )
    .expect("write allowed fixture");
    fs::write(
        repo_root.join(format!("stories/{FORBIDDEN_STORY_ID}.yml")),
        FORBIDDEN_STORY_YAML,
    )
    .expect("write forbidden fixture");

    // Stub `claude` onto a tempdir-rooted PATH so the library's
    // subprocess wire is exercised without needing real claude auth.
    // Only the ALLOWED scenario reaches the subprocess; the FORBIDDEN
    // scenario short-circuits on the out-of-scope justification text.
    let path_override = install_claude_shim(repo_root, STUBBED_CLAUDE_STDOUT);
    std::env::set_var("PATH", &path_override);
    std::env::set_var("AGENTIC_CACHE", repo_root.join(".agentic-cache"));

    init_repo_and_commit_seed(repo_root);

    // --- Allowed scenario: dev-dep IS appended; nothing else mutates.
    let bytes_before = snapshot_cargo_tomls(repo_root);

    let builder = TestBuilder::new(repo_root);
    let outcome: TestBuilderOutcome = builder
        .run(ALLOWED_STORY_ID)
        .expect("allowed dev-dep run must succeed");

    // Target crate's Cargo.toml grew a [dev-dependencies] entry; no
    // other section changed.
    let target_toml_after =
        fs::read_to_string(target_root.join("Cargo.toml")).expect("read target toml");
    let (target_before_prefix, _) = target_toml_after
        .split_once("[dev-dependencies]")
        .expect("[dev-dependencies] section preserved");
    assert!(
        target_before_prefix.contains("[package]"),
        "target [package] section must be unchanged"
    );
    assert!(
        target_before_prefix.contains("[dependencies]"),
        "target [dependencies] section must be unchanged and empty"
    );
    // The appended entry must be in the dev-deps section.
    let (_, dev_deps_section) = target_toml_after
        .split_once("[dev-dependencies]")
        .expect("split on dev-deps");
    assert!(
        dev_deps_section.contains("proptest"),
        "dev-dep `proptest` must appear under [dev-dependencies]; got section: {dev_deps_section:?}"
    );

    // The run summary names the added dev-dep.
    let added = outcome.added_dev_deps();
    assert!(
        added.iter().any(|(crate_name, dep)| crate_name == "devdep-fixture" && dep == "proptest"),
        "run summary must name added dev-dep; got {added:?}"
    );

    // Sibling and workspace-root tomls are bytes-identical.
    assert_eq!(
        fs::read(sibling_root.join("Cargo.toml")).expect("sibling toml after"),
        *bytes_before.get(&sibling_root.join("Cargo.toml")).unwrap(),
        "sibling Cargo.toml must be bytes-identical after an allowed dev-dep edit"
    );
    assert_eq!(
        fs::read(repo_root.join("Cargo.toml")).expect("workspace toml after"),
        *bytes_before.get(&repo_root.join("Cargo.toml")).unwrap(),
        "workspace-root Cargo.toml must be bytes-identical after an allowed dev-dep edit"
    );

    // --- Forbidden scenario: request a runtime-deps edit. Every
    // Cargo.toml in the workspace must be bytes-identical to its state
    // at the START of this scenario; no evidence; no scaffold.
    let bytes_before_forbidden = snapshot_cargo_tomls(repo_root);

    let forbidden_scaffold = target_root.join("tests/requests_runtime_dep.rs");
    let evidence_dir = repo_root
        .join("evidence/runs")
        .join(FORBIDDEN_STORY_ID.to_string());

    let err = builder
        .run(FORBIDDEN_STORY_ID)
        .expect_err("runtime-deps request must surface as Err");
    assert!(
        matches!(err, TestBuilderError::OutOfScopeEdit),
        "runtime-deps request must surface as TestBuilderError::OutOfScopeEdit; got {err:?}"
    );

    assert!(
        !forbidden_scaffold.exists(),
        "forbidden scenario must write zero scaffolds"
    );
    if evidence_dir.exists() {
        let any_jsonl = fs::read_dir(&evidence_dir)
            .expect("read evidence dir")
            .filter_map(|e| e.ok())
            .any(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "jsonl")
            });
        assert!(
            !any_jsonl,
            "forbidden scenario must write zero evidence"
        );
    }

    // Every Cargo.toml bytes-identical to pre-forbidden.
    let bytes_after_forbidden = snapshot_cargo_tomls(repo_root);
    assert_eq!(
        bytes_after_forbidden, bytes_before_forbidden,
        "forbidden scenario must leave every Cargo.toml bytes-identical"
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

fn snapshot_cargo_tomls(repo_root: &Path) -> HashMap<PathBuf, Vec<u8>> {
    let mut out = HashMap::new();
    for path in [
        repo_root.join("Cargo.toml"),
        repo_root.join("crates/devdep-fixture/Cargo.toml"),
        repo_root.join("crates/sibling-fixture/Cargo.toml"),
    ] {
        let bytes = fs::read(&path).expect("read toml");
        out.insert(path, bytes);
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
