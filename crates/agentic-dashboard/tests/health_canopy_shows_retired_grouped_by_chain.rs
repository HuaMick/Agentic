//! Story 21 acceptance test: `agentic stories health --canopy`
//! surfaces retired stories grouped by supersession chain.
//!
//! Justification (from stories/21.yml):
//! Proves the canopy lens surfaces retired stories grouped by
//! supersession chain: given a corpus where `A` is `retired
//! (superseded_by: B)`, `B` is `retired (superseded_by: C)`, and
//! `C` is `healthy`, invoking `agentic stories health --canopy`
//! renders one group whose members are (in chain order) A, B, C —
//! and each row in that group carries a cell naming its chain
//! position (e.g. "era 1 of 3", or the successor's short id). A
//! story with `status: retired` and no `superseded_by` renders as
//! a terminal single-member group.
//!
//! Red today is compile-red: `render_canopy_json` / `era_head_id`
//! grouping does not yet exist on the `Dashboard` type. Once story
//! 3's amendment adds the canopy renderer and story 6's amendment
//! adds `Status::Retired` + `superseded_by`, the test runs runtime-
//! red until the era-grouping logic is implemented.

use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use agentic_story::Status;
use serde_json::{json, Value};
use tempfile::TempDir;

const HEAD_SHA: &str = "2121212121212121212121212121212121212121";

// A → B → C supersession chain (C is the era head).
const ID_A_RETIRED: u32 = 92201;
const ID_B_RETIRED: u32 = 92202;
const ID_C_HEALTHY: u32 = 92203;
// Terminal-retirement: a retired story with no superseded_by.
const ID_TERMINAL: u32 = 92204;

fn fixture(id: u32, status: &str, extra: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for canopy-chain scaffold"

outcome: |
  Fixture row for the canopy-chain scaffold.

status: {status}
{extra}
patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_canopy_shows_retired_grouped_by_chain.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Render canopy JSON; assert grouped-by-chain shape.

guidance: |
  Fixture authored inline for the canopy-chain scaffold. Not a real
  story.

depends_on: []
"#
    )
}

#[test]
fn canopy_mode_shows_retired_stories_grouped_by_supersession_chain_with_chain_position_cells() {
    // Cross-reference: Status::Retired must exist on the enum for
    // this test to compile. Compile-red today.
    let _retired_variant_exists: Status = Status::Retired;

    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    fs::write(
        stories_dir.join(format!("{ID_A_RETIRED}.yml")),
        fixture(
            ID_A_RETIRED,
            "retired",
            &format!(
                "\nsuperseded_by: {ID_B_RETIRED}\nretired_reason: |\n  A folded into B.\n"
            ),
        ),
    )
    .expect("write A");
    fs::write(
        stories_dir.join(format!("{ID_B_RETIRED}.yml")),
        fixture(
            ID_B_RETIRED,
            "retired",
            &format!(
                "\nsuperseded_by: {ID_C_HEALTHY}\nretired_reason: |\n  B folded into C.\n"
            ),
        ),
    )
    .expect("write B");
    fs::write(
        stories_dir.join(format!("{ID_C_HEALTHY}.yml")),
        fixture(ID_C_HEALTHY, "healthy", ""),
    )
    .expect("write C");
    fs::write(
        stories_dir.join(format!("{ID_TERMINAL}.yml")),
        fixture(
            ID_TERMINAL,
            "retired",
            "\nretired_reason: |\n  Experiment abandoned with no replacement.\n",
        ),
    )
    .expect("write terminal");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed healthy UAT + passing test_runs for C so the classifier
    // accepts the era head as healthy.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000092203",
                "story_id": ID_C_HEALTHY,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "signed_at": "2026-04-23T00:00:00Z",
            }),
        )
        .expect("seed C uat pass");
    store
        .upsert(
            "test_runs",
            &ID_C_HEALTHY.to_string(),
            json!({
                "story_id": ID_C_HEALTHY,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-23T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed C test_runs pass");

    let dashboard = Dashboard::new(store, stories_dir, HEAD_SHA.to_string());

    // The canopy renderer's JSON mode structurally exposes the
    // era-head grouping per story 21's "Frontier vs canopy"
    // guidance: each story carries an `era_head_id` that points at
    // the terminal successor in its chain (or at itself for
    // terminal-retirement and for stories that are themselves the
    // era head). Downstream tooling reconstructs era shape via
    // `group_by(.era_head_id)`.
    let rendered = dashboard
        .render_canopy_json()
        .expect("render_canopy_json should succeed");
    let parsed: Value = serde_json::from_str(&rendered)
        .unwrap_or_else(|e| panic!("canopy JSON must parse: {e}; raw:\n{rendered}"));

    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));

    // All four fixtures appear in canopy mode — retired stories
    // are included (unlike frontier mode).
    let ids: Vec<u64> = stories
        .iter()
        .filter_map(|s| s.get("id").and_then(|v| v.as_u64()))
        .collect();
    for expected in [ID_A_RETIRED, ID_B_RETIRED, ID_C_HEALTHY, ID_TERMINAL] {
        assert!(
            ids.contains(&(expected as u64)),
            "canopy JSON must include retired and healthy stories alike \
             (missing id {expected}); got ids: {ids:?}"
        );
    }

    // Group by era_head_id — A, B, C must share the same era head
    // (C); the terminal-retirement fixture sits in its own group
    // (era head = itself).
    let mut by_era: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for s in stories {
        let id = s
            .get("id")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| panic!("story row must carry `id`: {s}"));
        let era_head = s
            .get("era_head_id")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| {
                panic!(
                    "canopy JSON must carry `era_head_id` on every row so \
                     downstream tooling can group_by(.era_head_id); got: {s}"
                )
            });
        by_era.entry(era_head).or_default().push(id);
    }

    // A, B, C form one group keyed on C.
    let a_b_c_group = by_era
        .get(&(ID_C_HEALTHY as u64))
        .unwrap_or_else(|| panic!("canopy must group A/B/C under era_head={ID_C_HEALTHY}; got by_era={by_era:?}"));
    for expected in [ID_A_RETIRED, ID_B_RETIRED, ID_C_HEALTHY] {
        assert!(
            a_b_c_group.contains(&(expected as u64)),
            "era-head={ID_C_HEALTHY} group must include id {expected}; got {a_b_c_group:?}"
        );
    }
    assert_eq!(
        a_b_c_group.len(),
        3,
        "era-head={ID_C_HEALTHY} group must contain exactly A, B, C; got {a_b_c_group:?}"
    );

    // Terminal-retirement: a single-member group keyed on itself.
    let terminal_group = by_era.get(&(ID_TERMINAL as u64)).unwrap_or_else(|| {
        panic!(
            "terminal-retirement story {ID_TERMINAL} must form a single-member \
             group keyed on its own id; got by_era={by_era:?}"
        )
    });
    assert_eq!(
        terminal_group,
        &vec![ID_TERMINAL as u64],
        "terminal-retirement group must contain exactly the terminal id; got {terminal_group:?}"
    );

    // Each retired row carries a chain-position cell identifying
    // the successor it points at (or "terminal" for terminal
    // retirement). The contract is the field is present and
    // non-empty; the exact rendering (e.g. "era 1 of 3" vs the
    // successor's id) is the renderer's freedom.
    for s in stories {
        let id = s.get("id").and_then(|v| v.as_u64()).unwrap();
        let status = s.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if status != "retired" {
            continue;
        }
        let cell = s
            .get("chain_position")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                panic!(
                    "canopy JSON row for retired story {id} must carry \
                     `chain_position` (naming successor id or era position); got: {s}"
                )
            });
        assert!(
            !cell.trim().is_empty(),
            "chain_position for retired story {id} must be non-empty; got {cell:?}"
        );
    }

    // And the canopy table rendering mentions each retired id as a
    // row — unlike the frontier view.
    let table = dashboard
        .render_canopy_table()
        .expect("render_canopy_table should succeed");
    for retired_id in [ID_A_RETIRED, ID_B_RETIRED, ID_TERMINAL] {
        let needle = format!("{retired_id}");
        assert!(
            table.lines().any(|line| {
                let trimmed = line.trim_start();
                trimmed.starts_with(&format!("{needle} "))
                    || trimmed.starts_with(&format!("{needle}|"))
                    || trimmed.starts_with(&format!("{needle} |"))
            }),
            "canopy table must include a row for retired story {retired_id}; got table:\n{table}"
        );
    }
}
