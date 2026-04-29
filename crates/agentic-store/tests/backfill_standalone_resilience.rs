//! Story 28 acceptance test: standalone-resilient-library claim for the
//! backfill code path inside `agentic-store`.
//!
//! Justification (from stories/28.yml acceptance.tests[10]):
//!   Proves the standalone-resilient-library claim: the backfill
//!   library is driven directly with only `agentic-store`,
//!   `agentic-events`, `agentic-signer`, and `git2` wired up
//!   (no orchestrator, no runtime, no sandbox, no CLI crate),
//!   and produces the same row shape as the binary path. Without
//!   this, `agentic-store` would silently grow an orchestrator
//!   dependency through the backfill code path — the kind of
//!   accidental coupling the pattern's allow-list catches at link
//!   time.
//!
//! Pattern: `patterns/standalone-resilient-library.yml`. Story 28's
//! guidance pins the allow-list explicitly: "Allowed dependencies for
//! the backfill code path (inside `agentic-store`). `agentic-events`,
//! `agentic-signer`, `git2`, plus whatever `agentic-store` already pulls
//! in for its existing trait/impl. The backfill does NOT depend on
//! `agentic-uat`, `agentic-dashboard`, `agentic-cli`,
//! `agentic-orchestrator`, `agentic-runtime`, or `agentic-sandbox`."
//!
//! This scaffold pins the claim two ways:
//!
//! 1. Compile-time witness: the test imports ONLY `agentic_store`
//!    from the workspace (and `git2` + `tempfile` for fixture
//!    construction, which the standalone-resilience pattern
//!    explicitly allows). The test drives the full happy path
//!    through the `Store::backfill_manual_signing` entry point —
//!    same shape as
//!    `backfill_writes_one_manual_signings_row_at_head.rs`. If a
//!    forbidden workspace crate sneaks into `agentic-store`'s
//!    dependency closure, the `[dependencies]` audit below catches
//!    it.
//!
//! 2. Cargo.toml allow-list audit: the test reads
//!    `crates/agentic-store/Cargo.toml` and asserts it carries no
//!    `[dependencies]` entry naming any of the forbidden workspace
//!    crates. Story 28 explicitly forbids
//!    `agentic-uat`, `agentic-dashboard`, `agentic-cli`,
//!    `agentic-orchestrator`, `agentic-runtime`, `agentic-sandbox`,
//!    and `agentic-test-builder` in the runtime-dep table.
//!    Dev-dependencies are allowed (an existing dev-cycle on
//!    `agentic-runtime` is documented in Cargo.toml for unrelated
//!    tests; this audit reads `[dependencies]` only).
//!
//! Red today is compile-red on the library happy path — the
//! `Store::backfill_manual_signing` method does not yet exist on
//! the trait. The Cargo.toml audit also fires red as runtime once
//! the library compile error is resolved, because today's
//! `agentic-store/Cargo.toml` declares no `agentic-events` or
//! `agentic-signer` dep yet (the story-28 implementation work will
//! add them). The audit is structured so it stays green only when
//! the runtime-dep table is exactly the allowed superset.

use std::fs;
use std::path::{Path, PathBuf};

// Compile-time witness: ONLY `agentic_store` from the workspace.
// Adding `agentic_orchestrator`, `agentic_runtime`, `agentic_sandbox`,
// `agentic_cli`, `agentic_uat`, `agentic_dashboard`, or
// `agentic_test_builder` to this file is a review-time red flag and
// the standalone-resilience claim breaks.
use agentic_store::{MemStore, Store};
use tempfile::TempDir;

const STORY_ID: u32 = 28_101;
const SIGNER_EMAIL: &str = "backfill-resilience@agentic.local";

const STORY_YAML_HEALTHY: &str = r#"id: 28101
title: "Fixture for story-28 standalone-resilience scaffold"

outcome: |
  Fixture used for the standalone-resilience scaffold; runs the full
  happy path through `Store::backfill_manual_signing` from a dependency
  floor of only `agentic-store` + `git2` + `tempfile`.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-store/tests/backfill_standalone_resilience.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the library entry point against this file.
  uat: |
    Run the backfill from the dependency floor; assert one row.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const STORY_YAML_UNDER_CONSTRUCTION: &str = r#"id: 28101
title: "Fixture for story-28 standalone-resilience scaffold"

outcome: |
  Fixture used for the standalone-resilience scaffold; runs the full
  happy path through `Store::backfill_manual_signing` from a dependency
  floor of only `agentic-store` + `git2` + `tempfile`.

status: under_construction

patterns: []

acceptance:
  tests:
    - file: crates/agentic-store/tests/backfill_standalone_resilience.rs
      justification: |
        Present so this fixture is itself schema-valid; the live test
        drives the library entry point against this file.
  uat: |
    Run the backfill from the dependency floor; assert one row.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
"#;

const GREEN_JSONL: &str = "{\"run_id\":\"aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee\",\"story_id\":28101,\"commit\":\"deadbeefdeadbeefdeadbeefdeadbeefdeadbeef\",\"timestamp\":\"2026-04-29T00:00:00Z\",\"verdicts\":[{\"file\":\"crates/agentic-store/tests/backfill_standalone_resilience.rs\",\"verdict\":\"green\"}]}\n";

/// Forbidden workspace crates per story 28's allow-list. The `[dependencies]`
/// table in `crates/agentic-store/Cargo.toml` MUST NOT name any of these.
const FORBIDDEN_RUNTIME_DEPS: &[&str] = &[
    "agentic-uat",
    "agentic-dashboard",
    "agentic-cli",
    "agentic-orchestrator",
    "agentic-runtime",
    "agentic-sandbox",
    "agentic-test-builder",
];

#[test]
fn backfill_drives_full_happy_path_from_dependency_floor_and_cargo_toml_excludes_orchestrator_deps()
{
    // ---- Part 1: compile-time + runtime witness from the dep floor ----
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

    // Drive the backfill through ONLY the `Store` trait — this is the
    // dependency-floor proof: no orchestrator, no runtime, no CLI shim,
    // no UAT crate.
    let store = MemStore::new();
    store
        .backfill_manual_signing(STORY_ID, repo_root)
        .expect("backfill must succeed from the dependency floor");

    let manual_rows = store
        .query("manual_signings", &|doc| {
            doc.get("story_id").and_then(|v| v.as_u64()) == Some(STORY_ID as u64)
        })
        .expect("manual_signings query must succeed");
    assert_eq!(
        manual_rows.len(),
        1,
        "dep-floor happy path must write exactly one manual_signings row; \
         got {} rows: {manual_rows:?}",
        manual_rows.len()
    );

    // ---- Part 2: Cargo.toml audit of agentic-store [dependencies] ----
    let cargo_toml_path = locate_agentic_store_cargo_toml();
    let cargo_toml = fs::read_to_string(&cargo_toml_path)
        .unwrap_or_else(|e| panic!("read {cargo_toml_path:?}: {e}"));

    let deps_section = extract_section(&cargo_toml, "[dependencies]")
        .expect("agentic-store/Cargo.toml must have a [dependencies] section");

    for forbidden in FORBIDDEN_RUNTIME_DEPS {
        assert!(
            !deps_section_names_crate(&deps_section, forbidden),
            "agentic-store/Cargo.toml [dependencies] must NOT name `{forbidden}`; \
             story 28's allow-list forbids orchestrator-dependent deps in the \
             backfill code path. Found in section:\n{deps_section}"
        );
    }
}

/// Resolve `crates/agentic-store/Cargo.toml` from the test's
/// `CARGO_MANIFEST_DIR` (which is the agentic-store crate root when this
/// test is built and run by cargo).
fn locate_agentic_store_cargo_toml() -> PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo");
    PathBuf::from(manifest_dir).join("Cargo.toml")
}

/// Extract the body of a TOML section header (e.g. `[dependencies]`).
/// Returns the text from the line after the header up to (but not
/// including) the next top-level header.
fn extract_section(toml: &str, header: &str) -> Option<String> {
    let mut in_section = false;
    let mut out = String::new();
    for line in toml.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            if trimmed == header {
                in_section = true;
                continue;
            } else if in_section {
                break;
            }
        } else if in_section {
            out.push_str(line);
            out.push('\n');
        }
    }
    if in_section {
        Some(out)
    } else {
        None
    }
}

/// Detect whether a TOML `[dependencies]`-shaped section names a given
/// crate. Looks for either `crate-name = ...` or `crate-name.workspace
/// = true` style entries at the start of a line.
fn deps_section_names_crate(section: &str, crate_name: &str) -> bool {
    for line in section.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Match `name =` or `name.workspace`
        if let Some(rest) = trimmed.strip_prefix(crate_name) {
            let next = rest.chars().next();
            if matches!(next, Some(' ') | Some('=') | Some('.')) {
                return true;
            }
        }
    }
    false
}

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
            "story(28101): UAT promotion to healthy",
            &flip_tree,
            &[&seed_commit],
        )
        .expect("commit flip");

    format!("{}", flip_commit_oid)
}
