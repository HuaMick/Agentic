//! Story 15 acceptance test: `plan` is strictly read-only — no scaffold
//! written, no evidence written, no `evidence/runs/<id>/` directory
//! created, and `git status --porcelain` is byte-identical pre- and
//! post-call. A plan invocation on a dirty tree exits 0 (not 2) because
//! it is not an attestation — per the fail-closed-on-dirty-tree
//! pattern's read-mode carve-out.
//!
//! Justification (from stories/15.yml acceptance.tests[1]): proves
//! planning has zero side effects on any fixture story (clean tree or
//! dirty). Without this, the planner would smuggle attestational
//! semantics into a read path and the two-phase plan/record split
//! would collapse into one gated command.
//!
//! Red today is compile-red: `TestBuilder::plan` is the new API
//! surface; `agentic-test-builder` does not declare it yet, so `cargo
//! check` fails on the unresolved item. Build-rust adds it and this
//! test runs green.

use std::fs;
use std::path::Path;

use agentic_story::Story;
use agentic_test_builder::TestBuilder;
use tempfile::TempDir;

const STORY_ID: u32 = 99_015_002;

const FIXTURE_YAML: &str = r#"id: 99015002
title: "Fixture for story 15 plan-is-read-only acceptance test"

outcome: |
  Fixture used to prove `TestBuilder::plan` writes nothing to disk,
  even when the working tree around it is dirty.

status: proposed

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-crate/tests/observes_something.rs
      justification: |
        Proves the fixture crate exposes a function whose observable
        is worth pinning in a scaffold; the justification text is
        substantive so the shared loader accepts the entry.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

#[test]
fn plan_writes_no_files_and_leaves_git_status_porcelain_byte_identical() {
    // Arrange: a real git repo so we can observe `git status --porcelain`
    // before and after the plan call. The fixture story lives inside.
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_YAML).expect("write fixture");

    init_repo_and_commit_seed(repo_root);

    // Deliberately dirty the tree outside the scaffold paths. The
    // plan-is-read-only claim requires that a dirty tree is NOT a
    // refusal condition for plan; the planner returns normally and
    // leaves the dirt where it was.
    fs::write(repo_root.join("dirty-over-here.txt"), b"unrelated\n")
        .expect("write dirty file");

    let porcelain_before = git_porcelain(repo_root);
    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    assert!(
        !evidence_dir.exists(),
        "pre-test invariant: the evidence dir must not exist yet"
    );

    // Act: load the story and plan it. No side effects permitted.
    let story = Story::load(&story_path).expect("load fixture story");
    let _plan = TestBuilder::plan(&story);

    // Assert 1: no scaffold file was authored under any path the
    // fixture names.
    let scaffold_path = repo_root.join("crates/fixture-crate/tests/observes_something.rs");
    assert!(
        !scaffold_path.exists(),
        "plan must not author any scaffold file; unexpected file at {}",
        scaffold_path.display()
    );

    // Assert 2: no `evidence/runs/<id>/` directory was created.
    assert!(
        !evidence_dir.exists(),
        "plan must not create evidence/runs/<id>/; found {}",
        evidence_dir.display()
    );

    // Assert 3: git status --porcelain is byte-identical.
    let porcelain_after = git_porcelain(repo_root);
    assert_eq!(
        porcelain_before, porcelain_after,
        "plan must not change `git status --porcelain`; before:\n{porcelain_before}\nafter:\n{porcelain_after}"
    );
}

/// Minimal `git status --porcelain`-equivalent, expressed via git2 so
/// the test has no shell dependency. Returns a normalised string whose
/// equality is the byte-identical claim the justification names.
fn git_porcelain(repo_root: &Path) -> String {
    let repo = git2::Repository::open(repo_root).expect("open repo");
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true)
        .include_ignored(false)
        .exclude_submodules(true);
    let statuses = repo.statuses(Some(&mut opts)).expect("statuses");
    let mut lines: Vec<String> = statuses
        .iter()
        .map(|entry| {
            format!(
                "{:?} {}",
                entry.status(),
                entry.path().unwrap_or("").to_string()
            )
        })
        .collect();
    lines.sort();
    lines.join("\n")
}

fn init_repo_and_commit_seed(root: &Path) {
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
    repo.commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
}
