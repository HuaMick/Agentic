//! Story 3 acceptance test: the canopy lens (`--canopy` /
//! `--all-eras`) surfaces retired stories clustered under their era
//! head, with retired ancestors ordered as prior eras resolving
//! forward through the supersession chain.
//!
//! Justification (from stories/3.yml): proves the canopy lens
//! surfaces retired stories grouped by supersession chain with the
//! era head as the grouping key: given a fixture corpus where `A` is
//! `retired (superseded_by: B)`, `B` is
//! `retired (superseded_by: C)`, and `C` is `healthy` (i.e. `C` is
//! the era head because no story supersedes it), invoking
//! `agentic stories health --canopy` (equivalently `--all-eras`)
//! renders rows including A, B, and C clustered together under C as
//! the era head, with A and B ordered as prior eras whose chain
//! resolves through to C. A retired story with no `superseded_by`
//! renders as a single-member terminal group (itself the era head).
//! Without this, the canopy promise — "honest history of the tree's
//! prior eras" — is a prose claim with no observable, and retired
//! stories would appear as an un-grouped flat list indistinguishable
//! from a broken frontier filter.
//!
//! Red today is compile-red: `Dashboard::render_canopy_table` and
//! `Dashboard::render_canopy_json` do not yet exist on the public
//! surface (current API exposes `render_frontier_*` and
//! `render_all_*`; the canopy lens is story 3's amendment addition),
//! and `Status::Retired` is not yet on `agentic_story::Status`.
//! Either missing symbol forces rustc to fail at compile time; the
//! diagnostic will name the first one rustc encounters.

use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use agentic_story::Status;
use serde_json::{json, Value};
use tempfile::TempDir;

const HEAD_SHA: &str = "3131313131313131313131313131313131313131";

// A → B → C supersession chain (C is the era head — no story
// supersedes it). A retired, B retired, C healthy.
const ID_A_RETIRED: u32 = 93201;
const ID_B_RETIRED: u32 = 93202;
const ID_C_HEALTHY: u32 = 93203;
// Terminal-retirement: a retired story with no `superseded_by`.
// Its own era head — a single-member group.
const ID_TERMINAL: u32 = 93204;

fn fixture(id: u32, status: &str, extra: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for canopy era-grouping (story 3 amendment)"

outcome: |
  Fixture row for the canopy-view era-grouping scaffold.

status: {status}
{extra}
patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/canopy_view_shows_retired_with_era_grouping.rs
      justification: |
        Present so the fixture is schema-valid; the live test drives
        Dashboard's canopy renderer against this file.
  uat: |
    Render canopy view; assert retired rows grouped under era head.

guidance: |
  Fixture authored inline for the canopy-view era-grouping scaffold.
  Not a real story.

depends_on: []
"#
    )
}

#[test]
fn canopy_lens_clusters_retired_stories_under_era_head_with_terminal_retirement_as_single_group() {
    // Compile-red anchor #1: Status::Retired must exist on the enum.
    let _retired_variant_exists: Status = Status::Retired;

    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // A retired, superseded by B.
    fs::write(
        stories_dir.join(format!("{ID_A_RETIRED}.yml")),
        fixture(
            ID_A_RETIRED,
            "retired",
            &format!(
                "\nsuperseded_by: {ID_B_RETIRED}\nretired_reason: |\n  A folded into B for this scaffold's era chain.\n"
            ),
        ),
    )
    .expect("write A");
    // B retired, superseded by C.
    fs::write(
        stories_dir.join(format!("{ID_B_RETIRED}.yml")),
        fixture(
            ID_B_RETIRED,
            "retired",
            &format!(
                "\nsuperseded_by: {ID_C_HEALTHY}\nretired_reason: |\n  B folded into C for this scaffold's era chain.\n"
            ),
        ),
    )
    .expect("write B");
    // C healthy — the era head (no story supersedes it).
    fs::write(
        stories_dir.join(format!("{ID_C_HEALTHY}.yml")),
        fixture(ID_C_HEALTHY, "healthy", ""),
    )
    .expect("write C");
    // Terminal-retirement: retired with no successor.
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
    // recognises C as the healthy era head.
    store
        .append(
            "uat_signings",
            json!({
                "id": format!("01900000-0000-7000-8000-0000000{ID_C_HEALTHY}"),
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

    // Compile-red anchor #2: `render_canopy_json` must exist on the
    // Dashboard API. The canopy-JSON shape is the structural witness
    // for era grouping (story 21 guidance "Frontier vs canopy":
    // `era_head_id` on every row lets downstream tooling
    // `group_by(.era_head_id)` without reading the table).
    let canopy_json = dashboard
        .render_canopy_json()
        .expect("render_canopy_json should succeed on a well-formed corpus");
    let parsed: Value = serde_json::from_str(&canopy_json).unwrap_or_else(|e| {
        panic!("canopy JSON must parse via serde_json::from_str: {e}; raw:\n{canopy_json}")
    });
    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));

    // (a) All four fixtures appear — the canopy lens includes
    // retired rows (unlike frontier).
    let ids: Vec<u64> = stories
        .iter()
        .filter_map(|s| s.get("id").and_then(|v| v.as_u64()))
        .collect();
    for expected in [ID_A_RETIRED, ID_B_RETIRED, ID_C_HEALTHY, ID_TERMINAL] {
        assert!(
            ids.contains(&(expected as u64)),
            "canopy JSON must include retired and healthy stories alike \
             (missing id {expected}); got ids: {ids:?}\nfull JSON:\n{parsed}"
        );
    }

    // (b) Group rows by era_head_id. A, B, C share era head C; the
    // terminal-retirement sits in its own single-member group keyed
    // on itself.
    let mut by_era: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for s in stories {
        let id = s
            .get("id")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| panic!("canopy JSON row must carry `id`: {s}"));
        let era_head = s
            .get("era_head_id")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| {
                panic!(
                    "canopy JSON must carry `era_head_id` on every row so downstream \
                     tooling can group_by(.era_head_id) without re-walking superseded_by; \
                     got row: {s}"
                )
            });
        by_era.entry(era_head).or_default().push(id);
    }

    // (c) A, B, C cluster under era head C.
    let abc_group = by_era.get(&(ID_C_HEALTHY as u64)).unwrap_or_else(|| {
        panic!("canopy must group A/B/C under era_head={ID_C_HEALTHY}; got by_era={by_era:?}")
    });
    for expected in [ID_A_RETIRED, ID_B_RETIRED, ID_C_HEALTHY] {
        assert!(
            abc_group.contains(&(expected as u64)),
            "era-head={ID_C_HEALTHY} group must include id {expected}; got {abc_group:?}"
        );
    }
    assert_eq!(
        abc_group.len(),
        3,
        "era-head={ID_C_HEALTHY} group must contain exactly A, B, C; got {abc_group:?}"
    );

    // (d) Terminal-retirement: single-member group keyed on itself.
    let terminal_group = by_era.get(&(ID_TERMINAL as u64)).unwrap_or_else(|| {
        panic!(
            "terminal-retirement story {ID_TERMINAL} must form a single-member group \
             keyed on its own id (itself the era head); got by_era={by_era:?}"
        )
    });
    assert_eq!(
        terminal_group,
        &vec![ID_TERMINAL as u64],
        "terminal-retirement group must contain exactly the terminal id; got {terminal_group:?}"
    );

    // (e) The canopy table also surfaces every retired id as a row
    // (absent the table's own shape, the observable "retired stories
    // shown, not hidden" collapses back to a prose claim).
    let canopy_table = dashboard
        .render_canopy_table()
        .expect("render_canopy_table should succeed");
    for retired_id in [ID_A_RETIRED, ID_B_RETIRED, ID_TERMINAL] {
        let needle = retired_id.to_string();
        let row_present = canopy_table.lines().any(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with(&format!("{needle} |"))
                || trimmed.starts_with(&format!("{needle}|"))
                || trimmed.starts_with(&format!("{needle} "))
        });
        assert!(
            row_present,
            "canopy table must include a row for retired story {retired_id}; \
             got table:\n{canopy_table}"
        );
    }
}
