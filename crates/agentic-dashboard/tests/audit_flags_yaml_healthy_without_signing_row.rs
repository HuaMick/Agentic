//! Story 25 acceptance test: audit flags category 5 (yaml-healthy-
//! without-signing-row) drift via the union of `uat_signings` and
//! `manual_signings`.
//!
//! Justification (from stories/25.yml): proves category 5 — given a
//! fixture corpus containing one story whose YAML has `status: healthy`
//! AND `agentic-store` carries ZERO `uat_signings.verdict=pass` rows
//! AND ZERO `manual_signings.verdict=pass` rows for that story id,
//! the audit report names that story id under a category-5 heading.
//! The query MUST compose both tables: a row in either `uat_signings`
//! (a real `agentic uat` Pass) OR `manual_signings` (a backfilled
//! manual ritual via story 28) satisfies the gate and removes the
//! story from this category. A story whose YAML says `proposed` or
//! `under_construction` does NOT route to category 5; a story whose
//! YAML says `retired` is excluded outright. Without this test, the
//! forged-promotion shape — the manual-ritual stories 11, 17, 23 lived
//! in until backfill, and any future hand-edit that flips
//! `status: healthy` without driving `agentic uat` — is detectable by
//! the dashboard but invisible to the audit's structured report,
//! breaking the symmetry between the two surfaces and forcing the
//! pre-commit hook (story 29) to wrap both tools just to cover the
//! same shape twice. The composition with `manual_signings` is what
//! lets the backfill (story 28) bring stories 11, 17, 23 into
//! compliance without forcing them through a synthetic UAT pass.
//!
//! Red today is compile-red: `AuditReport` carries the original four
//! category fields but does not yet carry a fifth
//! `yaml_healthy_without_signing_row` field, and the `run_audit`
//! library function does not yet consult the `manual_signings` table
//! at all. The scaffold references both, so `cargo check` fails on
//! the missing field.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use agentic_dashboard::audit::{run_audit, AuditReport};
use agentic_store::{MemStore, Store};
use agentic_test_support::FixtureCorpus;

const ID_FORGED_PROMOTION: u32 = 250701;
const ID_UAT_SIGNED: u32 = 250702;
const ID_MANUAL_SIGNED: u32 = 250703;
const HEAD_SHA: &str = "fffffffffffffffffffffffffffffffffffffffe";

fn fixture_yaml(id: u32, status: &str, test_file_path: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for story-25 cat-5 drift scaffold"

outcome: |
  Fixture story for the yaml-healthy-without-signing-row drift scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: {test_file_path}
      justification: |
        Present so the fixture is schema-valid; the audit's category-5
        signal reads only the union of uat_signings + manual_signings,
        not test-file presence on disk.
  uat: |
    Drive the audit against this YAML; assert membership in
    yaml_healthy_without_signing_row when no signing row exists in
    either table.

guidance: |
  Fixture authored inline. Not a real story.

depends_on: []
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

#[test]
fn audit_flags_healthy_yaml_with_no_signing_in_either_table_under_yaml_healthy_without_signing_row()
{
    let corpus = FixtureCorpus::new();
    let stories_dir = corpus.stories_dir();
    let root = corpus.path();

    // Forged-promotion fixture: YAML claims healthy, but ZERO rows in
    // BOTH signing tables. This is the shape stories 11, 17, 23 lived
    // in pre-backfill, and the shape any hand-edit that flips
    // `status: healthy` without driving `agentic uat` produces.
    let forged_test_path = root.join("fixture_tests").join("cat5_forged.rs");
    write_test_source(&forged_test_path);
    fs::write(
        stories_dir.join(format!("{ID_FORGED_PROMOTION}.yml")),
        fixture_yaml(
            ID_FORGED_PROMOTION,
            "healthy",
            forged_test_path.to_str().expect("forged path utf8"),
        ),
    )
    .expect("write forged-promotion fixture");

    // Negative control 1: YAML=healthy AND a `uat_signings` Pass row.
    // This is a normal post-`agentic uat` story; MUST NOT appear in
    // category 5.
    let uat_test_path = root.join("fixture_tests").join("cat5_uat_signed.rs");
    write_test_source(&uat_test_path);
    fs::write(
        stories_dir.join(format!("{ID_UAT_SIGNED}.yml")),
        fixture_yaml(
            ID_UAT_SIGNED,
            "healthy",
            uat_test_path.to_str().expect("uat path utf8"),
        ),
    )
    .expect("write uat-signed fixture");

    // Negative control 2: YAML=healthy AND a `manual_signings` Pass
    // row (NOT a `uat_signings` row). This is the backfilled manual-
    // ritual shape story 28 introduces. The audit MUST accept it as
    // signed — a row in EITHER table satisfies the gate. Without the
    // union, every backfilled story would be flagged forever.
    let manual_test_path = root.join("fixture_tests").join("cat5_manual_signed.rs");
    write_test_source(&manual_test_path);
    fs::write(
        stories_dir.join(format!("{ID_MANUAL_SIGNED}.yml")),
        fixture_yaml(
            ID_MANUAL_SIGNED,
            "healthy",
            manual_test_path.to_str().expect("manual path utf8"),
        ),
    )
    .expect("write manual-signed fixture");

    // Seed the store. The forged-promotion story gets NOTHING in
    // either signing table. The uat-signed control gets a real
    // `uat_signings.verdict=pass` row. The manual-signed control gets
    // a `manual_signings.verdict=pass` row (no uat_signings) — this
    // is the row shape story 28 will write at backfill time.
    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // uat-signed control: keep the dashboard happy by also writing a
    // Pass test_runs row at HEAD so the dashboard doesn't classify
    // it as unhealthy via category 4.
    store
        .append(
            "uat_signings",
            serde_json::json!({
                "id": "01900000-0000-7000-8000-000000025702",
                "story_id": ID_UAT_SIGNED,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "signed_at": "2026-04-26T00:00:00Z",
            }),
        )
        .expect("seed uat_signings Pass for uat-signed control");
    store
        .upsert(
            "test_runs",
            &ID_UAT_SIGNED.to_string(),
            serde_json::json!({
                "story_id": ID_UAT_SIGNED,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-27T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed Pass test_runs for uat-signed control");

    // manual-signed control: the row shape story 28 is expected to
    // write at backfill time. The audit's category-5 query MUST union
    // this with `uat_signings` — a manual-ritual backfill row removes
    // a story from category 5 the same way a real UAT pass does.
    store
        .append(
            "manual_signings",
            serde_json::json!({
                "id": "01900000-0000-7000-8000-000000025703",
                "story_id": ID_MANUAL_SIGNED,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "signed_at": "2026-04-26T01:00:00Z",
                "ritual_evidence": "manual ritual: pre-backfill stories 11/17/23 shape",
            }),
        )
        .expect("seed manual_signings Pass for manual-signed control");
    store
        .upsert(
            "test_runs",
            &ID_MANUAL_SIGNED.to_string(),
            serde_json::json!({
                "story_id": ID_MANUAL_SIGNED,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-27T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed Pass test_runs for manual-signed control");

    let report: AuditReport = run_audit(&stories_dir, root, store.clone(), HEAD_SHA.to_string())
        .expect("audit must succeed against a clean tempdir corpus");

    let cat5_ids: Vec<u32> = report
        .yaml_healthy_without_signing_row
        .iter()
        .map(|entry| entry.id)
        .collect();

    // The forged-promotion story MUST appear under category 5.
    assert!(
        cat5_ids.contains(&ID_FORGED_PROMOTION),
        "audit must flag story {ID_FORGED_PROMOTION} (YAML=healthy, ZERO \
         uat_signings rows, ZERO manual_signings rows) under \
         yaml_healthy_without_signing_row; got ids={cat5_ids:?}"
    );

    // The uat-signed control MUST NOT appear under category 5 — a real
    // UAT Pass row satisfies the gate.
    assert!(
        !cat5_ids.contains(&ID_UAT_SIGNED),
        "audit must NOT flag uat-signed control {ID_UAT_SIGNED} (YAML=healthy, \
         valid uat_signings.verdict=pass row) under yaml_healthy_without_signing_row; \
         got ids={cat5_ids:?}"
    );

    // The manual-signed control MUST NOT appear under category 5 — a
    // backfilled `manual_signings` row satisfies the gate via the
    // union with `uat_signings`. THIS is the assertion that pins the
    // composition: without `uat_signings UNION manual_signings`, this
    // story would be flagged forever even though story 28 backfilled
    // it. The audit and the backfill are useless without each other
    // here.
    assert!(
        !cat5_ids.contains(&ID_MANUAL_SIGNED),
        "audit must NOT flag manual-signed control {ID_MANUAL_SIGNED} (YAML=healthy, \
         ZERO uat_signings rows, but a valid manual_signings.verdict=pass row) under \
         yaml_healthy_without_signing_row — the audit's category-5 query MUST consult \
         BOTH tables (uat_signings UNION manual_signings); got ids={cat5_ids:?}"
    );

    // Cross-category isolation: the forged-promotion story (YAML=healthy)
    // must NOT appear under categories 1, 2, or 3 (those are reserved
    // for proposed / under_construction statuses). It MAY overlap with
    // category 4 only if a Fail test_runs row exists — and we did not
    // seed one, so it must NOT appear there either.
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
            !ids_in_cat.contains(&ID_FORGED_PROMOTION),
            "story {ID_FORGED_PROMOTION} (YAML=healthy, no signing row in either \
             table) must appear under ONLY yaml_healthy_without_signing_row; \
             also found under {label}: {ids_in_cat:?}"
        );
    }
}
