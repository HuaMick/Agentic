//! Story 18 acceptance test: CI-record wires the resolved signer onto
//! the `test_runs` row symmetrically with `uat_signings` (Pass AND Fail
//! rows both carry it).
//!
//! Justification (from stories/18.yml acceptance.tests[7]):
//!   Proves the symmetry decision (the second open
//!   question note 10 asked story-writer to commit on):
//!   `test_runs` rows carry the same `signer` field as
//!   `uat_signings`. Given the same resolution scaffold
//!   as the uat-pass test above, a `Recorder::record(...,
//!   SignerSource::Resolve)` call for a story with
//!   passing tests writes a `test_runs` row whose
//!   `signer` equals the resolved value. A Fail variant
//!   of the same test writes a `signer` on the Fail row
//!   too — symmetry is unconditional, not outcome-gated.
//!   Story 2's existing fields (`story_id`, `verdict`,
//!   `failing_tests`, `commit`, `ran_at`) remain on the
//!   row unchanged; `signer` is additive. Without this,
//!   only UAT signings gain attribution and the dashboard
//!   (story 3) can tell who signed a verdict but not who
//!   ran the CI that reddened a row — a cross-table
//!   asymmetry the dashboard join would surface as a
//!   permanent lint.
//!
//! Red today: compile-red via the missing `SignerSource` symbol in
//! `agentic_ci_record` (and the missing `Recorder::record_with_signer`
//! overload that accepts it) — the test `use`s
//! `agentic_ci_record::SignerSource` which does not exist yet.

use std::sync::Arc;

use agentic_ci_record::{Recorder, RunInput, SignerSource};
use agentic_store::{MemStore, Store};

#[test]
fn record_writes_resolved_signer_onto_pass_and_fail_rows() {
    // --- Subtest 1: Pass row carries signer. ---
    const STORY_ID_PASS: i64 = 88801;
    std::env::set_var("AGENTIC_SIGNER", "ci-env-person@example.com");

    let store_pass: Arc<dyn Store> = Arc::new(MemStore::new());
    let recorder_pass = Recorder::new(store_pass.clone());

    recorder_pass
        .record_with_signer(RunInput::pass(STORY_ID_PASS), SignerSource::Resolve)
        .expect("pass record must succeed with resolvable signer");

    let row_pass = store_pass
        .get("test_runs", &STORY_ID_PASS.to_string())
        .expect("store get")
        .expect("recorder must have upserted a Pass row");

    assert_eq!(
        row_pass.get("signer").and_then(|v| v.as_str()),
        Some("ci-env-person@example.com"),
        "Pass row must carry resolved signer; got row={row_pass}"
    );
    // Story 2's existing fields are ADDITIVE — signer is new, they remain.
    assert_eq!(
        row_pass.get("verdict").and_then(|v| v.as_str()),
        Some("pass"),
        "Pass row must still carry verdict=\"pass\"; got row={row_pass}"
    );
    assert!(
        row_pass.get("commit").and_then(|v| v.as_str()).is_some(),
        "Pass row must still carry commit; got row={row_pass}"
    );
    assert!(
        row_pass
            .get("failing_tests")
            .and_then(|v| v.as_array())
            .is_some(),
        "Pass row must still carry failing_tests; got row={row_pass}"
    );
    assert!(
        row_pass.get("ran_at").and_then(|v| v.as_str()).is_some(),
        "Pass row must still carry ran_at; got row={row_pass}"
    );

    // --- Subtest 2: Fail row ALSO carries signer. Symmetry is
    // unconditional, not outcome-gated. ---
    const STORY_ID_FAIL: i64 = 88802;
    let store_fail: Arc<dyn Store> = Arc::new(MemStore::new());
    let recorder_fail = Recorder::new(store_fail.clone());

    recorder_fail
        .record_with_signer(
            RunInput::fail(STORY_ID_FAIL, vec!["crates/foo/tests/a.rs".to_string()]),
            SignerSource::Resolve,
        )
        .expect("fail record must succeed with resolvable signer");

    let row_fail = store_fail
        .get("test_runs", &STORY_ID_FAIL.to_string())
        .expect("store get")
        .expect("recorder must have upserted a Fail row");

    assert_eq!(
        row_fail.get("signer").and_then(|v| v.as_str()),
        Some("ci-env-person@example.com"),
        "Fail row must carry resolved signer symmetrically with Pass; got row={row_fail}"
    );
    assert_eq!(
        row_fail.get("verdict").and_then(|v| v.as_str()),
        Some("fail"),
        "Fail row must still carry verdict=\"fail\"; got row={row_fail}"
    );
    let failing = row_fail
        .get("failing_tests")
        .and_then(|v| v.as_array())
        .expect("Fail row must carry failing_tests array");
    assert_eq!(
        failing.len(),
        1,
        "Fail row must carry the failing test; got {failing:?}"
    );

    // Cleanup.
    std::env::remove_var("AGENTIC_SIGNER");
}
