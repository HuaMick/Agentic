//! Story 13 acceptance test: the multi-offender rule — every direct
//! `depends_on` offender is listed in `not_healthy_reason`, ordered
//! ascending by parent id, with healthy direct parents and transitive
//! (grandparent+) offenders EXCLUDED.
//!
//! Justification (from stories/13.yml): proves the multi-offender
//! rule: given a story `<id>` with multiple direct `depends_on`
//! parents `<A>`, `<B>`, `<C>` where both `<A>` and `<C>` classify
//! non-healthy (and `<B>` is healthy), the `--json` row's
//! `not_healthy_reason` array contains BOTH `"ancestor:<A>"` AND
//! `"ancestor:<C>"` — one entry per direct offender, ordered by
//! ascending parent id for deterministic output — and does NOT
//! contain an entry for `<B>` (healthy parent) nor for any
//! grandparent of `<A>` or `<C>` (transitive offenders are reached
//! by drilling into the direct parent's own row).
//!
//! The scaffold materialises a fixture with:
//!   - `A` (under_construction, no UAT) — offending direct parent
//!   - `B` (healthy, own signals clean) — healthy direct parent
//!   - `C` (under_construction, no UAT) — offending direct parent
//!   - `GRAND` (under_construction) — grandparent: a depends_on of C
//!   - `L` (healthy YAML, own signals clean) — depends_on = [A, B, C]
//! Assertions on L's row:
//!   (i) `not_healthy_reason` contains `"ancestor:<A>"` AND
//!       `"ancestor:<C>"` in ascending order of parent id;
//!   (ii) no entry for healthy parent B;
//!   (iii) no entry for transitive offender GRAND — the chain walks
//!        itself; grandparents surface only by drilling into C.
//! Red today is runtime-red because the current classifier does not
//! emit `not_healthy_reason` at all.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::{json, Value};
use tempfile::TempDir;

const HEAD_SHA: &str = "1313130000000000000000000000000000000006";
const ID_GRAND: u32 = 913060; // grandparent — under_construction, depends_on of C
const ID_A: u32 = 913061; // direct parent — offender
const ID_B: u32 = 913062; // direct parent — healthy (NOT an offender)
const ID_C: u32 = 913063; // direct parent — offender, depends_on = [GRAND]
const ID_L: u32 = 913064; // leaf

fn fixture(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for multi-offender reason scaffold"

outcome: |
  Fixture row for the multi-offender reason scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_reason_lists_all_direct_offenders.rs
      justification: |
        Present so the fixture is schema-valid. The live test asserts
        that every direct offender surfaces exactly once, healthy
        parents do not, and transitive offenders do not.
  uat: |
    Render the dashboard; assert L's `not_healthy_reason` lists both
    direct offenders in ascending id order.

guidance: |
  Fixture authored inline for the multi-offender reason scaffold.
  Not a real story.

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
fn leaf_with_mixed_direct_parents_lists_every_direct_offender_and_no_healthy_or_transitive_parent() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    fs::write(
        stories_dir.join(format!("{ID_GRAND}.yml")),
        fixture(ID_GRAND, "under_construction", &[]),
    )
    .expect("write GRAND");
    fs::write(
        stories_dir.join(format!("{ID_A}.yml")),
        fixture(ID_A, "under_construction", &[]),
    )
    .expect("write A");
    fs::write(
        stories_dir.join(format!("{ID_B}.yml")),
        fixture(ID_B, "healthy", &[]),
    )
    .expect("write B");
    fs::write(
        stories_dir.join(format!("{ID_C}.yml")),
        fixture(ID_C, "under_construction", &[ID_GRAND]),
    )
    .expect("write C");
    fs::write(
        stories_dir.join(format!("{ID_L}.yml")),
        fixture(ID_L, "healthy", &[ID_A, ID_B, ID_C]),
    )
    .expect("write L");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    // B: healthy own signals.
    seed_healthy(&store, ID_B, "000000913062");
    // L: own signals clean (healthy own classification).
    seed_healthy(&store, ID_L, "000000913064");
    // A, C, GRAND: no UAT → under_construction from YAML.

    let dashboard = Dashboard::new(store.clone(), stories_dir.clone(), HEAD_SHA.to_string());
    let rendered = dashboard
        .render_json()
        .expect("render_json should succeed on the five-story fixture");

    let parsed: Value = serde_json::from_str(&rendered)
        .unwrap_or_else(|e| panic!("render_json output must parse as JSON: {e}; raw:\n{rendered}"));

    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));

    let l_row = stories
        .iter()
        .find(|s| s.get("id").and_then(|v| v.as_u64()) == Some(ID_L as u64))
        .unwrap_or_else(|| panic!("stories[] must include L (id {ID_L}); got: {parsed}"));

    // L must classify unhealthy because of A and C.
    let l_health = l_row
        .get("health")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("L row must carry health; got: {l_row}"));
    assert_eq!(
        l_health, "unhealthy",
        "L's direct parents A and C are both non-healthy — L must \
         classify `unhealthy`. Got {l_health} on row {l_row}"
    );

    let reason = l_row.get("not_healthy_reason").unwrap_or_else(|| {
        panic!(
            "L's unhealthy row must carry `not_healthy_reason`; got: {l_row}"
        )
    });
    let reason_arr = reason.as_array().unwrap_or_else(|| {
        panic!("`not_healthy_reason` must be a JSON array; got {reason:?}")
    });
    let tokens: Vec<String> = reason_arr
        .iter()
        .map(|v| {
            v.as_str()
                .unwrap_or_else(|| panic!("reason tokens must be strings; got {v:?}"))
                .to_string()
        })
        .collect();

    // Filter to ancestor-form tokens specifically (L's own signals are
    // clean in this fixture — only ancestor tokens should appear — but
    // we filter explicitly so the assertion speaks only to the multi-
    // offender rule).
    let ancestor_tokens: Vec<String> = tokens
        .iter()
        .filter(|t| t.starts_with("ancestor:"))
        .cloned()
        .collect();

    let expected = vec![format!("ancestor:{ID_A}"), format!("ancestor:{ID_C}")];
    assert_eq!(
        ancestor_tokens, expected,
        "`not_healthy_reason` must contain exactly {expected:?} (one entry \
         per direct offender, ordered ascending by parent id). A healthy \
         direct parent (B id {ID_B}) must NOT appear; a transitive \
         offender (GRAND id {ID_GRAND}, grandparent via C) must NOT \
         appear. Got ancestor tokens {ancestor_tokens:?}; full reason \
         array: {tokens:?}"
    );

    // Explicit negatives (redundant with expected equality but give a
    // clearer diagnostic if the vocabulary drifts).
    let b_token = format!("ancestor:{ID_B}");
    assert!(
        !tokens.contains(&b_token),
        "healthy direct parent B (id {ID_B}) must NOT appear in \
         `not_healthy_reason`; got {tokens:?}"
    );
    let grand_token = format!("ancestor:{ID_GRAND}");
    assert!(
        !tokens.contains(&grand_token),
        "transitive offender GRAND (id {ID_GRAND}) must NOT appear in \
         L's reason channel — direct-only by design; got {tokens:?}"
    );
}
