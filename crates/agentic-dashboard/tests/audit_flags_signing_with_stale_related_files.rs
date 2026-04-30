//! Story 25 acceptance test: audit flags category 6 (signing-with-
//! stale-related-files) drift via the dashboard's existing `own_files`
//! predicate (story 9's classifier).
//!
//! Justification (from stories/25.yml): proves category 6 — given a
//! fixture corpus containing one story whose YAML has `status: healthy`
//! AND `agentic-store` carries a Pass row in `uat_signings` (or
//! `manual_signings`) at commit C0 AND the story's `related_files: [...]`
//! glob set intersects the file-set changed between C0 and HEAD, the
//! audit report names that story id under a category-6 heading
//! `signing_with_stale_related_files`, the report's `is_empty()` returns
//! false, and the audit's exit-code mapping treats the non-empty
//! category as drift (exit 2 via the gate-mode contract). The
//! intersection signal MUST source from the same `git diff` walk and
//! the same glob-matching predicate the dashboard's classifier already
//! uses for story-9's `not_healthy_reason: ["own_files"]` rule (see
//! `classify_health` in `crates/agentic-dashboard/src/lib.rs`); a
//! parallel reimplementation in the audit is exactly the second-source-
//! of-truth drift category 4 already exists to prevent. A story whose
//! YAML says `status: healthy` AND has a Pass signing AND whose
//! `related_files` is empty (or absent) does NOT route to category 6
//! (the absent-related_files-is-permissive rule from story 9 is
//! inherited verbatim). A story whose `related_files` set does not
//! intersect the C0..HEAD diff also does NOT route to category 6. A
//! story without ANY signing row routes to category 5, not category 6
//! — the two categories are mutually exclusive on the attestation axis
//! (no signing vs stale signing).
//!
//! Red today is compile-red: `AuditReport` carries the original five
//! category fields (cat 1-5) but does not yet carry a sixth
//! `signing_with_stale_related_files` field, and `run_audit` does not
//! yet consult the dashboard's `own_files` predicate. The scaffold
//! references both, so `cargo check` fails on the missing field.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agentic_dashboard::audit::{run_audit, AuditReport};
use agentic_store::{MemStore, Store};
use agentic_test_support::FixtureCorpus;

const ID_STALE_SIGNING: u32 = 250601;
const ID_FRESH_SIGNING: u32 = 250602;
const ID_NO_RELATED_FILES: u32 = 250603;
const RELATED_GLOB: &str = "crates/agentic-store/src/**";
const TRACKED_FILE_REL: &str = "crates/agentic-store/src/lib.rs";
const UNRELATED_FILE_REL: &str = "crates/agentic-uat/src/lib.rs";

fn fixture_yaml(
    id: u32,
    status: &str,
    test_file_path: &str,
    related_files: &[&str],
) -> String {
    let related_block = if related_files.is_empty() {
        "related_files: []\n".to_string()
    } else {
        let items = related_files
            .iter()
            .map(|p| format!("  - {p}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("related_files:\n{items}\n")
    };

    format!(
        r#"id: {id}
title: "Fixture {id} for story-25 cat-6 drift scaffold"

outcome: |
  Fixture story for the signing-with-stale-related-files drift scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: {test_file_path}
      justification: |
        Present so the fixture is schema-valid; the audit's category-6
        signal reads the union of uat_signings + manual_signings against
        the C0..HEAD diff intersected with related_files.
  uat: |
    Drive the audit against this YAML; assert membership in
    signing_with_stale_related_files when the signing commit lags HEAD
    on a file matching related_files.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []

{related_block}
"#
    )
}

fn write_test_source(path: &PathBuf) {
    fs::create_dir_all(path.parent().expect("test path has parent")).expect("create parent dir");
    fs::write(
        path,
        r#"#[test]
fn placeholder() {
    assert!(true);
}
"#,
    )
    .expect("write fixture test source");
}

/// Initialise a real git repo at `root`, land an initial commit C0 with
/// `tracked_file_rel` containing initial content, then a second commit
/// C1 modifying that same file. Returns `(c0_sha, c1_sha)` as 40-char
/// lowercase hex.
///
/// The audit's category-6 check delegates to the dashboard's
/// `compute_git_diff` (git2-driven C0..HEAD diff) and the dashboard's
/// `check_related_files_intersection` (globset over related_files). A
/// real git tempdir is required because synthetic SHAs cannot be looked
/// up via `git2::Repository::find_commit`. This is the first audit
/// scaffold that needs real git; the other category tests use synthetic
/// in-memory rows only.
fn init_repo_with_two_commits(
    root: &Path,
    tracked_file_rel: &str,
) -> (String, String) {
    let repo = git2::Repository::init(root).expect("git init tempdir");

    {
        let mut config = repo.config().expect("get repo config");
        config
            .set_str("user.email", "test@example.com")
            .expect("set user.email");
        config
            .set_str("user.name", "Test User")
            .expect("set user.name");
    }

    let signature =
        git2::Signature::now("Test User", "test@example.com").expect("create signature");

    // C0: initial commit with the tracked file.
    let tracked_path = root.join(tracked_file_rel);
    fs::create_dir_all(tracked_path.parent().expect("tracked file has parent"))
        .expect("create parent dirs for tracked file");
    fs::write(&tracked_path, "// initial content\n").expect("write tracked file C0");

    let mut index = repo.index().expect("get index");
    index
        .add_path(Path::new(tracked_file_rel))
        .expect("add tracked file to index");
    index.write().expect("write index");
    let tree_id = index.write_tree().expect("write tree C0");
    let tree = repo.find_tree(tree_id).expect("find tree C0");

    let c0_oid = repo
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            "C0: initial commit",
            &tree,
            &[],
        )
        .expect("commit C0");

    // C1: modify the same tracked file so the C0..C1 diff intersects
    // the fixture story's related_files glob.
    fs::write(&tracked_path, "// modified content\n").expect("modify tracked file at C1");
    let mut index = repo.index().expect("re-get index");
    index
        .add_path(Path::new(tracked_file_rel))
        .expect("re-add tracked file to index");
    index.write().expect("re-write index");
    let tree_id = index.write_tree().expect("write tree C1");
    let tree = repo.find_tree(tree_id).expect("find tree C1");

    let parent = repo.find_commit(c0_oid).expect("find C0 commit");
    let c1_oid = repo
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            "C1: modify tracked file",
            &tree,
            &[&parent],
        )
        .expect("commit C1");

    (format!("{c0_oid}"), format!("{c1_oid}"))
}

#[test]
fn audit_flags_healthy_story_whose_signing_commit_lags_head_on_a_related_file_under_signing_with_stale_related_files(
) {
    let corpus = FixtureCorpus::new();
    let stories_dir = corpus.stories_dir();
    let root = corpus.path();

    // Real git tempdir with two commits. C1 modifies a file matching
    // the fixture story's related_files glob; the dashboard's
    // `compute_git_diff(C0, C1)` will return that path, and
    // `check_related_files_intersection` will report a non-empty
    // overlap.
    let (c0_sha, c1_sha) = init_repo_with_two_commits(root, TRACKED_FILE_REL);

    // Drifted fixture: YAML=healthy, signing pinned to C0, related_files
    // glob intersects the C0..C1 diff. This is the cohort the dashboard
    // already flags as `not_healthy_reason: ["own_files"]` (stories 6,
    // 9, 12, 13, 16, 17, 18, 25, 27 at HEAD 8ba186e per story 25's
    // 2026-04-30 amendment).
    let stale_test_path = root.join("fixture_tests").join("cat6_stale.rs");
    write_test_source(&stale_test_path);
    fs::write(
        stories_dir.join(format!("{ID_STALE_SIGNING}.yml")),
        fixture_yaml(
            ID_STALE_SIGNING,
            "healthy",
            stale_test_path.to_str().expect("stale path utf8"),
            &[RELATED_GLOB],
        ),
    )
    .expect("write stale-signing fixture");

    // Negative control 1: YAML=healthy, signing pinned to C1 (HEAD).
    // The C1..C1 diff is empty, so the intersection is empty and the
    // story MUST NOT route to category 6 — re-attesting at HEAD is
    // exactly the remediation path the audit's category-6 surface
    // points operators at.
    let fresh_test_path = root.join("fixture_tests").join("cat6_fresh.rs");
    write_test_source(&fresh_test_path);
    fs::write(
        stories_dir.join(format!("{ID_FRESH_SIGNING}.yml")),
        fixture_yaml(
            ID_FRESH_SIGNING,
            "healthy",
            fresh_test_path.to_str().expect("fresh path utf8"),
            &[RELATED_GLOB],
        ),
    )
    .expect("write fresh-signing fixture");

    // Negative control 2: YAML=healthy, signing pinned to C0, but
    // related_files is EMPTY. Per story 9's permissive default
    // (inherited verbatim by category 6), an absent related_files set
    // is NOT drift — the story has not declared any files as
    // load-bearing, so a stale signing has nothing to be stale against.
    let no_rel_test_path = root.join("fixture_tests").join("cat6_no_related.rs");
    write_test_source(&no_rel_test_path);
    fs::write(
        stories_dir.join(format!("{ID_NO_RELATED_FILES}.yml")),
        fixture_yaml(
            ID_NO_RELATED_FILES,
            "healthy",
            no_rel_test_path.to_str().expect("no-rel path utf8"),
            &[],
        ),
    )
    .expect("write no-related-files fixture");

    // Seed the store. All three stories get a Pass test_runs row at
    // HEAD (C1) so the dashboard's category-4 classifier treats them
    // as test-green; the only axis under test here is signing
    // staleness.
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Stale-signing fixture: uat_signings.commit = C0 (lags HEAD).
    store
        .append(
            "uat_signings",
            serde_json::json!({
                "id": "01900000-0000-7000-8000-000000025601",
                "story_id": ID_STALE_SIGNING,
                "verdict": "pass",
                "commit": c0_sha,
                "signed_at": "2026-04-26T00:00:00Z",
            }),
        )
        .expect("seed stale uat_signings row at C0");
    store
        .upsert(
            "test_runs",
            &ID_STALE_SIGNING.to_string(),
            serde_json::json!({
                "story_id": ID_STALE_SIGNING,
                "verdict": "pass",
                "commit": c1_sha,
                "ran_at": "2026-04-27T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed Pass test_runs for stale-signing fixture");

    // Fresh-signing control: uat_signings.commit = C1 (HEAD).
    store
        .append(
            "uat_signings",
            serde_json::json!({
                "id": "01900000-0000-7000-8000-000000025602",
                "story_id": ID_FRESH_SIGNING,
                "verdict": "pass",
                "commit": c1_sha,
                "signed_at": "2026-04-26T01:00:00Z",
            }),
        )
        .expect("seed fresh uat_signings row at C1");
    store
        .upsert(
            "test_runs",
            &ID_FRESH_SIGNING.to_string(),
            serde_json::json!({
                "story_id": ID_FRESH_SIGNING,
                "verdict": "pass",
                "commit": c1_sha,
                "ran_at": "2026-04-27T01:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed Pass test_runs for fresh-signing control");

    // No-related-files control: uat_signings.commit = C0 (would be
    // stale if related_files were declared) but related_files is empty.
    store
        .append(
            "uat_signings",
            serde_json::json!({
                "id": "01900000-0000-7000-8000-000000025603",
                "story_id": ID_NO_RELATED_FILES,
                "verdict": "pass",
                "commit": c0_sha,
                "signed_at": "2026-04-26T02:00:00Z",
            }),
        )
        .expect("seed uat_signings row at C0 for no-related-files control");
    store
        .upsert(
            "test_runs",
            &ID_NO_RELATED_FILES.to_string(),
            serde_json::json!({
                "story_id": ID_NO_RELATED_FILES,
                "verdict": "pass",
                "commit": c1_sha,
                "ran_at": "2026-04-27T02:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed Pass test_runs for no-related-files control");

    // Sanity: the unrelated path is NOT touched by the C0..C1 diff;
    // referenced here so a future scaffold extension that flips a
    // negative control onto an unrelated glob does not have to
    // re-derive the constant.
    let _unused = UNRELATED_FILE_REL;

    let report: AuditReport = run_audit(&stories_dir, root, store.clone(), c1_sha.clone())
        .expect("audit must succeed against a real-git tempdir corpus");

    // Category-6 entries: the field that does not exist on AuditReport
    // yet. Compile-red until build-rust adds it. The accessor name is
    // pinned by story 25's "Output shape — `--json`" guidance:
    // `signing_with_stale_related_files` (snake_case JSON key, struct
    // field name matches).
    let cat6_ids: Vec<u32> = report
        .signing_with_stale_related_files
        .iter()
        .map(|entry| entry.id)
        .collect();

    // Drifted fixture MUST appear under category 6.
    assert!(
        cat6_ids.contains(&ID_STALE_SIGNING),
        "audit must flag story {ID_STALE_SIGNING} (YAML=healthy, \
         uat_signings.commit=C0, related_files glob intersects the \
         C0..C1 diff) under signing_with_stale_related_files; got \
         ids={cat6_ids:?}"
    );

    // Fresh-signing control MUST NOT appear under category 6 — the
    // signing commit equals HEAD so the diff is empty by construction.
    // This is the remediation contract: re-running UAT at HEAD makes
    // the story leave category 6.
    assert!(
        !cat6_ids.contains(&ID_FRESH_SIGNING),
        "audit must NOT flag fresh-signing control {ID_FRESH_SIGNING} \
         (uat_signings.commit=C1=HEAD, empty diff) under \
         signing_with_stale_related_files; got ids={cat6_ids:?}"
    );

    // No-related-files control MUST NOT appear under category 6 — story
    // 9's permissive default is inherited verbatim. An empty
    // related_files list is not drift even if the signing commit lags.
    assert!(
        !cat6_ids.contains(&ID_NO_RELATED_FILES),
        "audit must NOT flag no-related-files control \
         {ID_NO_RELATED_FILES} (related_files=[], even though \
         uat_signings.commit=C0 lags HEAD) under \
         signing_with_stale_related_files — story 9's permissive \
         default is inherited verbatim; got ids={cat6_ids:?}"
    );

    // Report-level invariant: a non-empty category 6 makes
    // `is_empty()` return false. This is the exit-code-2 hook the
    // pre-commit gate (story 29) consumes.
    assert!(
        !report.is_empty(),
        "AuditReport::is_empty() must return false when \
         signing_with_stale_related_files is non-empty; otherwise the \
         exit-code contract (exit 2 on any drift) silently regresses \
         for category 6"
    );

    // Mutual-exclusion contract: a story flagged under category 6 must
    // NOT also appear under category 5 (the two are mutually exclusive
    // on the attestation axis — no signing vs stale signing). Without
    // this guard, the same story double-counts and the operator's
    // remediation guidance ("re-UAT at HEAD" for cat 6 vs "drive UAT
    // for the first time" for cat 5) becomes ambiguous.
    let cat5_ids: Vec<u32> = report
        .yaml_healthy_without_signing_row
        .iter()
        .map(|entry| entry.id)
        .collect();
    assert!(
        !cat5_ids.contains(&ID_STALE_SIGNING),
        "story {ID_STALE_SIGNING} has a Pass uat_signings row at C0; \
         it MUST NOT appear under yaml_healthy_without_signing_row \
         (category 5) — categories 5 and 6 are mutually exclusive on \
         the attestation axis. got cat5={cat5_ids:?}"
    );

    // Cross-category isolation: the drifted story (YAML=healthy) must
    // not appear under categories 1, 2, or 3 (those are reserved for
    // proposed / under_construction). It MAY overlap with category 4
    // only if a Fail test_runs row exists; we seeded Pass, so it must
    // NOT appear there either.
    for (label, ids) in [
        (
            "implementation_without_flip",
            &report.implementation_without_flip,
        ),
        ("promotion_ready", &report.promotion_ready),
        ("test_builder_not_started", &report.test_builder_not_started),
        (
            "healthy_with_failing_test",
            &report.healthy_with_failing_test,
        ),
    ] {
        let ids_in_cat: Vec<u32> = ids.iter().map(|e| e.id).collect();
        assert!(
            !ids_in_cat.contains(&ID_STALE_SIGNING),
            "story {ID_STALE_SIGNING} (YAML=healthy, stale signing) \
             must appear under ONLY signing_with_stale_related_files; \
             also found under {label}: {ids_in_cat:?}"
        );
    }
}
