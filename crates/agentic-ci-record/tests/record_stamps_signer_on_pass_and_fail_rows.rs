//! Story 2 acceptance test: owner-side pin of the additive `signer` field
//! on `test_runs` rows for BOTH Pass and Fail verdicts.
//!
//! Justification (from stories/2.yml):
//!   Proves signer symmetry with `uat_signings` at the
//!   `test_runs` row shape: given a `Recorder::record(...,
//!   SignerSource::Resolve)` call for story `<pass-id>` with
//!   passing tests in an environment where the signer resolves
//!   to `dev@example.com` (via story 18's four-tier chain —
//!   flag → `AGENTIC_SIGNER` env → `git config user.email` →
//!   typed error), the row written to `test_runs` carries
//!   `signer == "dev@example.com"` alongside the existing
//!   `story_id`, `verdict=pass`, `failing_tests=[]`, `commit`,
//!   `ran_at` fields. The same test then drives a Fail row for
//!   story `<fail-id>` under the same resolution scaffold and
//!   asserts the Fail row ALSO carries `signer ==
//!   "dev@example.com"` — symmetry is unconditional, not
//!   outcome-gated. A Pass row without a signer or a Fail row
//!   without a signer would both fail this test. Without this,
//!   the dashboard (story 3) can tell who signed a UAT verdict
//!   but not who ran the CI that reddened a `test_runs` row,
//!   and the cross-table asymmetry surfaces as a permanent
//!   lint at the dashboard join. Pairs with story 18's
//!   `record_writes_resolved_signer_onto_test_runs_row.rs`,
//!   which exercises the same contract from the signer-crate
//!   side; this test is story 2's owner-side pin of the
//!   additive field.
//!
//! Relationship to the story-18 sibling test. Story 18's
//! `record_writes_resolved_signer_onto_test_runs_row.rs` pins the same
//! symmetry from the signer crate's vantage — proving the resolver wire
//! reaches `test_runs`. This test, owned by story 2, pins the row
//! contract from the `test_runs`-writer's vantage: the existing fields
//! (`story_id`, `verdict`, `failing_tests`, `commit`, `ran_at`) are
//! preserved, and `signer` is ADDITIVE on top. The two tests together
//! mean build-rust cannot land the amendment by wiring the signer only
//! through some paths; both sides are pinned to the same observable on
//! the same table.
//!
//! Red today: compile-red via the missing `agentic_ci_record::
//! SignerSource` symbol — the justification names
//! `Recorder::record(..., SignerSource::Resolve)` and this test `use`s
//! `agentic_ci_record::SignerSource`, which does not yet exist on the
//! crate's public surface. Story 18's build-rust pass (agentic-signer)
//! and story 2's subsequent build-rust amendment pass (wiring the
//! resolver into `Recorder::record`) together turn this test green.

use std::sync::Arc;

use agentic_ci_record::{Recorder, RunInput, SignerSource};
use agentic_store::{MemStore, Store};

const RESOLVED_SIGNER: &str = "dev@example.com";

#[test]
fn record_stamps_resolved_signer_on_pass_row_and_on_fail_row_symmetrically() {
    // Tier 2 seed: set AGENTIC_SIGNER so the resolver returns a
    // deterministic value for both subtests below, regardless of the
    // ambient git config on the machine running the tests. The
    // justification names `dev@example.com` — we reproduce that value
    // byte-for-byte so the assertion message is self-describing.
    std::env::set_var("AGENTIC_SIGNER", RESOLVED_SIGNER);

    // --- Subtest 1: Pass row carries signer alongside every
    //     pre-amendment field. ---
    const STORY_ID_PASS: i64 = 20_001;
    let store_pass: Arc<dyn Store> = Arc::new(MemStore::new());
    let recorder_pass = Recorder::new(store_pass.clone());

    recorder_pass
        .record(RunInput::pass(STORY_ID_PASS), SignerSource::Resolve)
        .expect("Pass record must succeed when signer is resolvable");

    let pass_row = store_pass
        .get("test_runs", &STORY_ID_PASS.to_string())
        .expect("store get on Pass row")
        .expect("recorder must have upserted a Pass row keyed by story_id");

    // Additive: signer is present and equals the resolved value.
    let pass_signer = pass_row.get("signer").and_then(|v| v.as_str()).expect(
        "Pass row must carry a string `signer` field (additive; test_runs-uat_signings symmetry)",
    );
    assert!(
        !pass_signer.trim().is_empty(),
        "Pass row `signer` must be non-empty; got {pass_signer:?}"
    );
    assert_eq!(
        pass_signer, RESOLVED_SIGNER,
        "Pass row `signer` must equal the AGENTIC_SIGNER env value resolved by tier 2; \
         got {pass_signer:?}, expected {RESOLVED_SIGNER:?}"
    );

    // Story 2's existing row contract survives: every pre-amendment
    // field stays shaped the way record_pass.rs pins it.  A Pass run
    // MUST carry story_id, verdict=pass, failing_tests=[], commit,
    // ran_at — the signer field is additive on top, not a replacement.
    assert_eq!(
        pass_row.get("story_id").and_then(|v| v.as_i64()),
        Some(STORY_ID_PASS),
        "Pass row must still carry story_id={STORY_ID_PASS}; got row={pass_row}"
    );
    assert_eq!(
        pass_row.get("verdict").and_then(|v| v.as_str()),
        Some("pass"),
        "Pass row must still carry verdict=\"pass\"; got row={pass_row}"
    );
    let pass_failing = pass_row
        .get("failing_tests")
        .and_then(|v| v.as_array())
        .expect("Pass row must still carry failing_tests array");
    assert!(
        pass_failing.is_empty(),
        "Pass run must still record failing_tests=[]; got {pass_failing:?}"
    );
    assert!(
        pass_row.get("commit").and_then(|v| v.as_str()).is_some(),
        "Pass row must still carry a string `commit` field; got row={pass_row}"
    );
    assert!(
        pass_row.get("ran_at").and_then(|v| v.as_str()).is_some(),
        "Pass row must still carry a string `ran_at` field; got row={pass_row}"
    );

    // --- Subtest 2: Fail row ALSO carries signer — symmetry is
    //     unconditional, NOT outcome-gated. ---
    const STORY_ID_FAIL: i64 = 20_002;
    let store_fail: Arc<dyn Store> = Arc::new(MemStore::new());
    let recorder_fail = Recorder::new(store_fail.clone());

    let failing_paths = vec![
        "crates/agentic-foo/tests/a_broken.rs".to_string(),
        "crates/agentic-foo/tests/b_broken.rs".to_string(),
    ];
    recorder_fail
        .record(
            RunInput::fail(STORY_ID_FAIL, failing_paths.clone()),
            SignerSource::Resolve,
        )
        .expect("Fail record must succeed (a Fail verdict is still a recorded row) when signer is resolvable");

    let fail_row = store_fail
        .get("test_runs", &STORY_ID_FAIL.to_string())
        .expect("store get on Fail row")
        .expect("recorder must have upserted a Fail row keyed by story_id");

    let fail_signer = fail_row
        .get("signer")
        .and_then(|v| v.as_str())
        .expect("Fail row must carry a string `signer` field (symmetry is unconditional, not outcome-gated)");
    assert!(
        !fail_signer.trim().is_empty(),
        "Fail row `signer` must be non-empty; got {fail_signer:?}"
    );
    assert_eq!(
        fail_signer, RESOLVED_SIGNER,
        "Fail row `signer` must equal the AGENTIC_SIGNER env value resolved by tier 2 \
         — symmetric with the Pass row above; got {fail_signer:?}, expected {RESOLVED_SIGNER:?}"
    );

    // Fail row preserves every pre-amendment field too.
    assert_eq!(
        fail_row.get("story_id").and_then(|v| v.as_i64()),
        Some(STORY_ID_FAIL),
        "Fail row must still carry story_id={STORY_ID_FAIL}; got row={fail_row}"
    );
    assert_eq!(
        fail_row.get("verdict").and_then(|v| v.as_str()),
        Some("fail"),
        "Fail row must still carry verdict=\"fail\"; got row={fail_row}"
    );
    let fail_failing = fail_row
        .get("failing_tests")
        .and_then(|v| v.as_array())
        .expect("Fail row must still carry failing_tests array");
    assert_eq!(
        fail_failing.len(),
        2,
        "Fail run must still record its failing test basenames; got {fail_failing:?}"
    );
    assert!(
        fail_row.get("commit").and_then(|v| v.as_str()).is_some(),
        "Fail row must still carry a string `commit` field; got row={fail_row}"
    );
    assert!(
        fail_row.get("ran_at").and_then(|v| v.as_str()).is_some(),
        "Fail row must still carry a string `ran_at` field; got row={fail_row}"
    );

    // Cleanup: remove the env var so no sibling integration test inherits
    // the seeded value.  The signer-crate standalone tests and other
    // ci-record tests must see a clean ambient environment.
    std::env::remove_var("AGENTIC_SIGNER");
}
