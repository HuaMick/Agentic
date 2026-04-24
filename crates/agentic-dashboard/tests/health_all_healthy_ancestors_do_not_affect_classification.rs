//! Story 13 acceptance test: the non-interference guarantee — all-
//! healthy ancestry does not produce false positives.
//!
//! Justification (from stories/13.yml): proves the non-interference
//! guarantee: a story whose OWN signals are all clean AND whose entire
//! transitive ancestry classifies `healthy` (or whose `depends_on` is
//! empty) classifies `healthy` — the ancestor check adds no false
//! positives. Without this, the inheritance rule could accidentally
//! reject every story (a silent catastrophic tightening
//! indistinguishable, to the operator, from "the dashboard is
//! broken"), exactly the symmetric concern the
//! `uat_permits_all_healthy_ancestors` test in story 11 guards on the
//! write side.
//!
//! The scaffold builds a three-story healthy chain `L_OK -> A -> B`
//! (every row's own signals clean, every YAML says `healthy`) AND in
//! the same fixture corpus a separate leaf `L_BAD` that depends on a
//! broken parent `BROKEN` (`under_construction`, no UAT). The dual
//! fixture lets the test simultaneously assert:
//!   (1) the all-healthy chain stays all-healthy — classifier does NOT
//!       misfire on clean ancestry, and
//!   (2) the parallel broken-chain leaf still classifies unhealthy
//!       with a `not_healthy_reason` array carrying `"ancestor:<BROKEN>"`.
//! Assertion (2) is the runtime-red driver — today's classifier does
//! not emit `not_healthy_reason` at all, so reading that field on
//! `L_BAD` returns absent and the assertion fires. Once the ancestor
//! rule lands, (1) and (2) become the non-regression guards this
//! story names.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::{json, Value};
use tempfile::TempDir;

const HEAD_SHA: &str = "1313130000000000000000000000000000000004";
// All-healthy chain: B <- A <- L_OK
const ID_B: u32 = 913041;
const ID_A: u32 = 913042;
const ID_L_OK: u32 = 913043;
// Broken side: BROKEN <- L_BAD, no overlap with the healthy chain.
const ID_BROKEN: u32 = 913044;
const ID_L_BAD: u32 = 913045;

fn fixture(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for all-healthy non-interference scaffold"

outcome: |
  Fixture row for the all-healthy non-interference scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_all_healthy_ancestors_do_not_affect_classification.rs
      justification: |
        Present so the fixture is schema-valid. The live test asserts
        that an all-healthy chain stays all-healthy AND that a broken
        parallel chain still produces an ancestor-reason token.
  uat: |
    Render the dashboard; assert every row in the healthy chain is
    healthy and the broken leaf carries an ancestor reason.

guidance: |
  Fixture authored inline for the all-healthy non-interference
  scaffold. Not a real story.

{deps_yaml}
"#
    )
}

fn seed_healthy(store: &Arc<dyn Store>, story_id: u32, uuid_suffix: &str) {
    store
        .append(
            "uat_signings",
            json!({
                "id": format!("01900000-0000-7000-8000-{uuid_suffix}"),
                "story_id": story_id,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "signed_at": "2026-04-19T00:00:00Z",
            }),
        )
        .expect("seed uat_signings pass@HEAD");
    store
        .upsert(
            "test_runs",
            &story_id.to_string(),
            json!({
                "story_id": story_id,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed test_runs pass");
}

#[test]
fn all_healthy_chain_stays_healthy_while_parallel_broken_chain_still_cites_ancestor() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // Healthy chain B <- A <- L_OK.
    fs::write(
        stories_dir.join(format!("{ID_B}.yml")),
        fixture(ID_B, "healthy", &[]),
    )
    .expect("write B healthy");
    fs::write(
        stories_dir.join(format!("{ID_A}.yml")),
        fixture(ID_A, "healthy", &[ID_B]),
    )
    .expect("write A healthy depends_on=[B]");
    fs::write(
        stories_dir.join(format!("{ID_L_OK}.yml")),
        fixture(ID_L_OK, "healthy", &[ID_A]),
    )
    .expect("write L_OK healthy depends_on=[A]");

    // Parallel broken chain: BROKEN <- L_BAD.
    fs::write(
        stories_dir.join(format!("{ID_BROKEN}.yml")),
        fixture(ID_BROKEN, "under_construction", &[]),
    )
    .expect("write BROKEN under_construction");
    fs::write(
        stories_dir.join(format!("{ID_L_BAD}.yml")),
        fixture(ID_L_BAD, "healthy", &[ID_BROKEN]),
    )
    .expect("write L_BAD healthy depends_on=[BROKEN]");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    seed_healthy(&store, ID_B, "000000913041");
    seed_healthy(&store, ID_A, "000000913042");
    seed_healthy(&store, ID_L_OK, "000000913043");
    // BROKEN: no UAT → classifies under_construction.
    seed_healthy(&store, ID_L_BAD, "000000913045");

    let dashboard = Dashboard::new(store.clone(), stories_dir.clone(), HEAD_SHA.to_string());
    let rendered = dashboard
        .render_json()
        .expect("render_json should succeed on the mixed-chain fixture");

    let parsed: Value = serde_json::from_str(&rendered)
        .unwrap_or_else(|e| panic!("render_json output must parse as JSON: {e}; raw:\n{rendered}"));

    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));

    let row_of = |id: u32| -> &Value {
        stories
            .iter()
            .find(|s| s.get("id").and_then(|v| v.as_u64()) == Some(id as u64))
            .unwrap_or_else(|| panic!("stories[] must include id {id}; got: {parsed}"))
    };
    let health_of = |id: u32| -> String {
        row_of(id)
            .get("health")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| panic!("row for id {id} must carry health; got: {}", row_of(id)))
    };

    // (1) Non-interference: the whole healthy chain stays healthy.
    assert_eq!(
        health_of(ID_B),
        "healthy",
        "B (root of healthy chain) must classify healthy; full JSON: {parsed}"
    );
    assert_eq!(
        health_of(ID_A),
        "healthy",
        "A (mid of healthy chain) must classify healthy; full JSON: {parsed}"
    );
    assert_eq!(
        health_of(ID_L_OK),
        "healthy",
        "L_OK (leaf with all-healthy ancestry and clean own signals) must \
         classify healthy — the ancestor-inheritance rule must not misfire \
         on a clean chain. Full JSON: {parsed}"
    );

    // Healthy rows must OMIT `not_healthy_reason` entirely.
    let l_ok_row = row_of(ID_L_OK);
    assert!(
        l_ok_row.get("not_healthy_reason").is_none(),
        "healthy rows must OMIT `not_healthy_reason`; got L_OK row: {l_ok_row}"
    );

    // (2) Red-state driver: the parallel broken-chain leaf still fires
    // the ancestor rule. It must classify `unhealthy` and its
    // `not_healthy_reason` must contain `"ancestor:<BROKEN>"`. Today's
    // classifier does not emit `not_healthy_reason` at all, so this
    // assertion fails until story 13's impl lands.
    let l_bad_row = row_of(ID_L_BAD);
    let l_bad_health = l_bad_row
        .get("health")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("L_BAD row must carry health; got: {l_bad_row}"));
    assert_eq!(
        l_bad_health, "unhealthy",
        "L_BAD depends on BROKEN (under_construction) — must classify \
         `unhealthy` via the ancestor rule; got {l_bad_health} on row \
         {l_bad_row}"
    );
    let reason = l_bad_row.get("not_healthy_reason").unwrap_or_else(|| {
        panic!(
            "L_BAD's unhealthy row must carry a `not_healthy_reason` \
             array; got: {l_bad_row}"
        )
    });
    let reason_arr = reason
        .as_array()
        .unwrap_or_else(|| panic!("`not_healthy_reason` must be a JSON array; got {reason:?}"));
    let tokens: Vec<String> = reason_arr
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    let want = format!("ancestor:{ID_BROKEN}");
    assert!(
        tokens.contains(&want),
        "L_BAD's `not_healthy_reason` must contain the token `{want}` \
         (parent BROKEN is the only offender); got {tokens:?} on row \
         {l_bad_row}"
    );
}
