//! Story 13 acceptance test: the reason-channel token contract — exact
//! string forms for `"own_tests"`, `"own_files"`, `"ancestor:<id>"`.
//!
//! Justification (from stories/13.yml): proves the reason-channel
//! token contract by pinning the exact string forms. When a story
//! classifies `unhealthy` because of a direct `depends_on` parent
//! `<A>`, its `--json` row's `not_healthy_reason` array contains
//! exactly `"ancestor:<A>"` (literal prefix `ancestor:` followed by
//! the parent's numeric story id, no whitespace, no padding). When
//! the rule also fires from an own-signal, the array contains exactly
//! one of `"own_tests"` (own `test_runs.verdict == fail`) or
//! `"own_files"` (own `related_files` changed since the UAT pass
//! commit) alongside the ancestor entry (e.g. `["own_tests",
//! "ancestor:7"]`). These three token forms — `"own_tests"`,
//! `"own_files"`, `"ancestor:<id>"` — are the complete reason
//! vocabulary; nothing else is emitted.
//!
//! The scaffold drives TWO scenarios in one test:
//!   (a) ancestor-only: L depends on A (under_construction). L's own
//!       signals are clean. `not_healthy_reason == ["ancestor:<A>"]`.
//!   (b) ancestor + own_tests: L2 depends on A2 (under_construction).
//!       L2 carries test_runs.verdict=fail. `not_healthy_reason` has
//!       exactly two entries: `"own_tests"` first, then
//!       `"ancestor:<A2>"`.
//! The scenarios share a single fixture corpus, so both assertions
//! exercise the same classifier call. Red today is runtime-red
//! because the current classifier emits no `not_healthy_reason`
//! field.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::{json, Value};
use tempfile::TempDir;

const HEAD_SHA: &str = "1313130000000000000000000000000000000005";
// Scenario (a)
const ID_A: u32 = 913051;
const ID_L: u32 = 913052;
// Scenario (b)
const ID_A2: u32 = 913053;
const ID_L2: u32 = 913054;

fn fixture(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for reason-token forms scaffold"

outcome: |
  Fixture row for the reason-token forms scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_ancestor_reason_names_offending_id.rs
      justification: |
        Present so the fixture is schema-valid. The live test pins
        the exact token strings emitted on `not_healthy_reason`.
  uat: |
    Render the dashboard; assert the exact token forms on
    not_healthy_reason rows.

guidance: |
  Fixture authored inline for the reason-token forms scaffold. Not a
  real story.

{deps_yaml}
"#
    )
}

#[test]
fn ancestor_reason_token_uses_exact_ancestor_colon_id_form_and_composes_with_own_signal_tokens() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // Scenario (a): A under_construction, L healthy/clean depends_on=[A].
    fs::write(
        stories_dir.join(format!("{ID_A}.yml")),
        fixture(ID_A, "under_construction", &[]),
    )
    .expect("write A");
    fs::write(
        stories_dir.join(format!("{ID_L}.yml")),
        fixture(ID_L, "healthy", &[ID_A]),
    )
    .expect("write L");

    // Scenario (b): A2 under_construction, L2 healthy YAML but
    // test_runs=fail depends_on=[A2].
    fs::write(
        stories_dir.join(format!("{ID_A2}.yml")),
        fixture(ID_A2, "under_construction", &[]),
    )
    .expect("write A2");
    fs::write(
        stories_dir.join(format!("{ID_L2}.yml")),
        fixture(ID_L2, "healthy", &[ID_A2]),
    )
    .expect("write L2");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // L: UAT pass@HEAD, test_runs pass — own signals clean.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000913052",
                "story_id": ID_L,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "signed_at": "2026-04-19T00:00:00Z",
            }),
        )
        .expect("seed L uat");
    store
        .upsert(
            "test_runs",
            &ID_L.to_string(),
            json!({
                "story_id": ID_L,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed L tests pass");

    // L2: UAT pass@HEAD, test_runs fail — own_tests fires.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000913054",
                "story_id": ID_L2,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "signed_at": "2026-04-19T00:00:00Z",
            }),
        )
        .expect("seed L2 uat");
    store
        .upsert(
            "test_runs",
            &ID_L2.to_string(),
            json!({
                "story_id": ID_L2,
                "verdict": "fail",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": ["t"],
            }),
        )
        .expect("seed L2 tests fail");

    let dashboard = Dashboard::new(store.clone(), stories_dir.clone(), HEAD_SHA.to_string());
    let rendered = dashboard
        .render_json()
        .expect("render_json should succeed on four-story fixture");

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

    let reason_tokens = |id: u32| -> Vec<String> {
        let row = row_of(id);
        let reason = row.get("not_healthy_reason").unwrap_or_else(|| {
            panic!(
                "row for id {id} must carry `not_healthy_reason` — it is \
                 classified unhealthy; got row: {row}"
            )
        });
        let arr = reason
            .as_array()
            .unwrap_or_else(|| panic!("`not_healthy_reason` must be an array; got {reason:?}"));
        arr.iter()
            .map(|v| {
                v.as_str()
                    .unwrap_or_else(|| panic!("reason tokens must be strings; got {v:?}"))
                    .to_string()
            })
            .collect()
    };

    // Scenario (a): L's reason is exactly `["ancestor:<ID_A>"]`.
    let l_tokens = reason_tokens(ID_L);
    let expected_a: Vec<String> = vec![format!("ancestor:{ID_A}")];
    assert_eq!(
        l_tokens, expected_a,
        "L depends only on A (non-healthy) and its own signals are clean — \
         `not_healthy_reason` must be EXACTLY {expected_a:?} (literal \
         `ancestor:` prefix + parent id, no whitespace, no padding); got \
         {l_tokens:?}"
    );

    // Scenario (b): L2's reason is exactly `["own_tests", "ancestor:<ID_A2>"]`.
    let l2_tokens = reason_tokens(ID_L2);
    let expected_b: Vec<String> = vec!["own_tests".to_string(), format!("ancestor:{ID_A2}")];
    assert_eq!(
        l2_tokens, expected_b,
        "L2 has test_runs.verdict=fail AND depends_on=[A2 non-healthy] — \
         `not_healthy_reason` must be EXACTLY {expected_b:?} (own-signal \
         token `own_tests` first, then the direct-ancestor token in its \
         locked `ancestor:<id>` form); got {l2_tokens:?}"
    );
}
