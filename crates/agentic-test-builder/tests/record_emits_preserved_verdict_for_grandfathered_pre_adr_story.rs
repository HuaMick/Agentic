//! Story 23 acceptance test: `agentic test-build record` materialises
//! ADR-0005's grandfather-bridge classification rule (sub-amendment,
//! "Pre-ADR-0005 bridge") at the CLI/library boundary without a YAML
//! flag, without a synthetic red backfill, and without any in-test
//! perturbation of the evidence directory.
//!
//! Justification (from stories/23.yml acceptance.tests[5]): given a
//! fixture story whose `status` is `healthy` AND whose
//! `evidence/runs/<id>/` directory does not exist on disk at all (the
//! canonical bridge shape — a story that reached `healthy` either
//! pre-ADR-0005 or via story 10's bootstrap exception), and whose
//! `acceptance.tests[]` names one or more scaffolds that exist on
//! disk and probe green, `agentic test-build record <id>` classifies
//! every named scaffold as `verdict: "preserved"`, writes exactly one
//! new JSONL row at `evidence/runs/<id>/<timestamp>-red.jsonl`
//! carrying that verdict per scaffold, exits 0, and runs no probe.
//! The detection signal is exactly two predicates ANDed: (a) the
//! story's directory under `evidence/runs/` is absent at
//! record-invocation time, and (b) the story's on-disk YAML `status`
//! is `healthy`.
//!
//! After the row lands, the directory exists; a subsequent invocation
//! against the same unchanged story routes through the normal
//! preserved path (three gates do not all pass on a `healthy` story —
//! Gate A demands `under_construction`), so the grandfather
//! classification fires exactly once per story across its lifetime,
//! retiring the bridge state on first record. Both invocations
//! therefore produce a preserved-verdict row; the lifecycle invariant
//! is observed by the chain growing append-only across the two calls
//! (two distinct *-red.jsonl files in the directory after call 2),
//! each carrying the documented preserved shape.
//!
//! Without this test, the ADR's "either works" punt on declaration
//! shape stays unresolved at the test boundary and a future refactor
//! could accidentally reintroduce the YAML-flag option, fork-merge
//! the two paths, or forge a synthetic `red` row to satisfy a stricter
//! probe — three drift surfaces this test fences against.

use std::fs;
use std::path::Path;

use agentic_test_builder::TestBuilder;
use tempfile::TempDir;

const STORY_ID: u32 = 99_023_006;

const SCAFFOLD_A: &str = "crates/fixture-grandfather-crate/tests/grandfather_scaffold_a.rs";
const SCAFFOLD_B: &str = "crates/fixture-grandfather-crate/tests/grandfather_scaffold_b.rs";

/// Fixture story: `status: healthy`, two scaffolds, NO
/// `evidence/runs/<id>/` directory ever seeded. This is the canonical
/// bridge shape — a story that reached `healthy` before any
/// evidence-chain row was ever written for it.
const FIXTURE_STORY_YAML: &str = r#"id: 99023006
title: "Fixture for story 23 grandfathered-pre-adr-story"

outcome: |
  Fixture used to prove record's grandfather-bridge classification
  fires on (a) status healthy + (b) no evidence/runs/<id>/ directory,
  emitting one preserved verdict per scaffold without probing.

status: healthy

patterns:
- standalone-resilient-library

acceptance:
  tests:
    - file: crates/fixture-grandfather-crate/tests/grandfather_scaffold_a.rs
      justification: |
        Grandfather case scaffold A: lives on disk green (the impl
        already satisfies its observable). The CLI must classify
        PRESERVED via the directory-absent + healthy predicate,
        not via a probe.
    - file: crates/fixture-grandfather-crate/tests/grandfather_scaffold_b.rs
      justification: |
        Grandfather case scaffold B: a second scaffold in the same
        invocation, also green, also classified PRESERVED via the
        same two-predicate detection.
  uat: |
    Not executed by this scaffold.

guidance: |
  Fixture-only.

depends_on: []
"#;

const FIXTURE_WORKSPACE_CARGO: &str = r#"[workspace]
resolver = "2"
members = ["crates/fixture-grandfather-crate"]
"#;

const FIXTURE_CRATE_CARGO: &str = r#"[package]
name = "fixture-grandfather-crate"
version = "0.0.1"
edition = "2021"

[lib]
path = "src/lib.rs"
"#;

/// The crate declares `known_symbol`; both grandfather scaffolds
/// exercise it. If the classifier accidentally probed either scaffold,
/// the probe would come back GREEN (compile passes; test runs and
/// passes) and `record` would return `Err(ScaffoldNotRed)`. The test's
/// `.expect("record must succeed ...")` call below is therefore the
/// indirect signal that no probe ran — a failed probe would surface
/// the green scaffold as `ScaffoldNotRed` long before the verdict-shape
/// assertions are reached.
const FIXTURE_CRATE_LIB: &str = r#"pub fn known_symbol() -> u32 {
    7
}
"#;

const SCAFFOLD_A_BODY: &str = r#"use fixture_grandfather_crate::known_symbol;

#[test]
fn grandfather_scaffold_a() {
    assert_eq!(known_symbol(), 7);
}
"#;

const SCAFFOLD_B_BODY: &str = r#"use fixture_grandfather_crate::known_symbol;

#[test]
fn grandfather_scaffold_b() {
    assert!(known_symbol() > 0);
}
"#;

#[test]
fn record_classifies_grandfathered_scaffolds_preserved_when_directory_absent_and_status_healthy() {
    let tmp = TempDir::new().expect("repo tempdir");
    let repo_root = tmp.path();

    // Lay down a minimal cargo workspace + the fixture crate + two
    // green scaffolds that reference a symbol the crate DOES declare,
    // so that any accidental probe would come back green and trip
    // `ScaffoldNotRed`.
    fs::write(repo_root.join("Cargo.toml"), FIXTURE_WORKSPACE_CARGO).expect("ws cargo");
    let crate_root = repo_root.join("crates/fixture-grandfather-crate");
    fs::create_dir_all(crate_root.join("src")).expect("crate src");
    fs::create_dir_all(crate_root.join("tests")).expect("crate tests");
    fs::write(crate_root.join("Cargo.toml"), FIXTURE_CRATE_CARGO).expect("crate cargo");
    fs::write(crate_root.join("src/lib.rs"), FIXTURE_CRATE_LIB).expect("crate lib");

    fs::write(crate_root.join("tests/grandfather_scaffold_a.rs"), SCAFFOLD_A_BODY)
        .expect("scaffold a");
    fs::write(crate_root.join("tests/grandfather_scaffold_b.rs"), SCAFFOLD_B_BODY)
        .expect("scaffold b");

    // Story YAML with status: healthy.
    let stories_dir = repo_root.join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    let story_path = stories_dir.join(format!("{STORY_ID}.yml"));
    fs::write(&story_path, FIXTURE_STORY_YAML).expect("write fixture story");

    // CRITICAL: do NOT create `evidence/runs/<STORY_ID>/`. The
    // grandfather detection is exactly two predicates ANDed: the
    // story's directory under `evidence/runs/` is absent at
    // record-invocation time AND the story's on-disk YAML `status`
    // is `healthy`. Seeding the directory (even empty) breaks the
    // first predicate.
    let evidence_dir = repo_root.join(format!("evidence/runs/{STORY_ID}"));
    assert!(
        !evidence_dir.exists(),
        "test pre-condition: evidence/runs/{STORY_ID}/ must NOT exist before record runs",
    );

    init_repo_and_commit_seed(repo_root);

    // Sanity: post-commit, the directory still does not exist.
    assert!(
        !evidence_dir.exists(),
        "evidence directory must remain absent until record creates it",
    );

    // ---- First invocation: the grandfather branch fires. ----
    let builder = TestBuilder::new(repo_root);
    let outcome_one = builder.record(STORY_ID).expect(
        "record must succeed on a healthy story whose evidence/runs/<id>/ \
                 is absent — the grandfather-bridge classification must fire and \
                 NO probe must run (a probe would return green and yield \
                 ScaffoldNotRed)",
    );

    // The recorded plan must list both scaffolds in declaration order.
    let recorded_paths: Vec<String> = outcome_one
        .recorded_paths()
        .iter()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    assert_eq!(
        recorded_paths,
        vec![SCAFFOLD_A.to_string(), SCAFFOLD_B.to_string()],
        "recorded paths must list both scaffolds in declaration order",
    );

    // The directory now exists; locate the freshly-written JSONL.
    assert!(
        evidence_dir.exists(),
        "record must create evidence/runs/{STORY_ID}/ when it writes the row",
    );
    let files_after_one = list_red_jsonls(&evidence_dir);
    assert_eq!(
        files_after_one.len(),
        1,
        "first record must write exactly one *-red.jsonl file; got {files_after_one:?}",
    );

    let body_one = fs::read_to_string(&files_after_one[0]).expect("read evidence one");
    let row_one: serde_json::Value =
        serde_json::from_str(body_one.trim()).expect("evidence row must be valid JSON");

    // Top-level shape: run_id, story_id, commit, timestamp, verdicts.
    assert_eq!(
        row_one.get("story_id").and_then(|v| v.as_u64()),
        Some(STORY_ID as u64),
        "row must carry story_id={STORY_ID}",
    );
    assert!(
        row_one
            .get("commit")
            .and_then(|v| v.as_str())
            .map(|c| c.len() == 40 && c.chars().all(|ch| ch.is_ascii_hexdigit()))
            .unwrap_or(false),
        "row must carry a 40-char lowercase hex commit",
    );
    let verdicts = row_one
        .get("verdicts")
        .and_then(|v| v.as_array())
        .expect("verdicts must be a JSON array");

    assert_eq!(
        verdicts.len(),
        2,
        "verdicts must carry exactly one entry per acceptance.tests[] entry; got {}",
        verdicts.len(),
    );

    // Every verdict entry must be the documented preserved shape:
    // exactly {file, verdict} keys, no `red_path`, no `diagnostic`.
    let expected_files = [SCAFFOLD_A, SCAFFOLD_B];
    for (i, v) in verdicts.iter().enumerate() {
        let obj = v
            .as_object()
            .expect("each verdict entry must be a JSON object");

        assert_eq!(
            obj.get("file").and_then(|f| f.as_str()),
            Some(expected_files[i]),
            "verdict[{i}].file must match declaration order ({})",
            expected_files[i],
        );
        assert_eq!(
            obj.get("verdict").and_then(|x| x.as_str()),
            Some("preserved"),
            "verdict[{i}].verdict must be \"preserved\" under the grandfather \
             branch (status healthy + evidence dir absent); got {:?}",
            obj.get("verdict"),
        );

        // Shape: preserved rows carry only `file` + `verdict`. Omitted
        // keys, not null. The sub-amendment spells this out, and a
        // strict parser must accept this shape without
        // `additionalProperties` failures.
        let mut keys: Vec<&str> = obj.keys().map(|s| s.as_str()).collect();
        keys.sort();
        assert_eq!(
            keys,
            vec!["file", "verdict"],
            "preserved verdict[{i}] must carry exactly {{file, verdict}} — \
             no red_path, no diagnostic; got keys {keys:?}",
        );
    }

    // ---- Second invocation: the once-per-lifetime invariant. ----
    //
    // The story YAML is unchanged on disk. The evidence directory
    // now exists (we just created it). A second `record` call must
    // therefore route through the normal three-gate path: Gate A
    // fails immediately because status is `healthy`, yielding a
    // PRESERVED classification per scaffold via the normal branch
    // (not the grandfather branch, which only fires when the
    // directory is absent). The observable verdicts are identical
    // — preserved-shape per scaffold — but the chain grows by one
    // append-only row, demonstrating the bridge state retired on
    // first record.

    // Sleep is unnecessary: timestamps include seconds and the test
    // runner is fast enough; if a race did occur the rename would
    // overwrite which would still leave one new file. We instead
    // rely on the timestamp differing, but assert on file count and
    // shape, which is the load-bearing invariant.

    let _outcome_two = builder.record(STORY_ID).expect(
        "second record must succeed via the normal preserved path (Gate A \
                 fails on healthy); the grandfather branch must NOT fire twice",
    );

    let files_after_two = list_red_jsonls(&evidence_dir);
    assert_eq!(
        files_after_two.len(),
        2,
        "second invocation must write a second *-red.jsonl row append-only \
         alongside the first; got {files_after_two:?}",
    );

    // The second row must also carry preserved-shape verdicts for
    // both scaffolds — the classification is unchanged, only the
    // path through the classifier differs.
    // Pick the file that is NOT the first one.
    let second_path = files_after_two
        .iter()
        .find(|p| p != &&files_after_one[0])
        .expect("second invocation must produce a distinct *-red.jsonl");
    let body_two = fs::read_to_string(second_path).expect("read evidence two");
    let row_two: serde_json::Value =
        serde_json::from_str(body_two.trim()).expect("evidence row two must be valid JSON");
    let verdicts_two = row_two
        .get("verdicts")
        .and_then(|v| v.as_array())
        .expect("verdicts_two must be a JSON array");
    assert_eq!(
        verdicts_two.len(),
        2,
        "second row must carry one verdict per scaffold; got {}",
        verdicts_two.len(),
    );
    for (i, v) in verdicts_two.iter().enumerate() {
        let obj = v
            .as_object()
            .expect("each verdict entry must be a JSON object");
        assert_eq!(
            obj.get("verdict").and_then(|x| x.as_str()),
            Some("preserved"),
            "second-invocation verdict[{i}] must be \"preserved\" via the \
             normal three-gate path (Gate A fails on healthy); got {:?}",
            obj.get("verdict"),
        );
        let mut keys: Vec<&str> = obj.keys().map(|s| s.as_str()).collect();
        keys.sort();
        assert_eq!(
            keys,
            vec!["file", "verdict"],
            "second-invocation preserved verdict[{i}] must carry exactly \
             {{file, verdict}}; got keys {keys:?}",
        );
    }
}

fn list_red_jsonls(evidence_dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out: Vec<_> = fs::read_dir(evidence_dir)
        .expect("read evidence dir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.ends_with("-red.jsonl")
        })
        .collect();
    out.sort();
    out
}

fn init_repo_and_commit_seed(root: &Path) -> String {
    let repo = git2::Repository::init(root).expect("git init");
    {
        let mut cfg = repo.config().expect("repo config");
        cfg.set_str("user.name", "test-builder")
            .expect("set user.name");
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
    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, "seed", &tree, &[])
        .expect("commit seed");
    oid.to_string()
}
