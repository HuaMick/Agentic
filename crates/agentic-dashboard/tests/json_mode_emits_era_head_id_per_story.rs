//! Story 3 acceptance test: JSON-mode output carries an
//! `era_head_id` field on every story row, in BOTH canopy and
//! frontier outputs, so downstream tooling can reconstruct era
//! grouping via `group_by(.era_head_id)` without re-walking the
//! `superseded_by` chain itself.
//!
//! Justification (from stories/3.yml): proves the JSON-mode
//! grouping contract at the library boundary:
//! `agentic stories health --canopy --json` (and, for symmetry, the
//! default `--json` over a corpus containing retired stories even
//! though they are filtered from frontier output) emits a
//! `stories[]` array where each row object carries an `era_head_id`
//! field pointing at the terminal successor of that story's
//! supersession chain (the story itself when it is already
//! terminal, whether healthy or retired-with-no-successor).
//! Downstream tooling can therefore reconstruct era grouping via
//! `group_by(.era_head_id)` without re-walking the `superseded_by`
//! chain itself. Without this, consumers of JSON mode (CI status
//! checks, future web UI) must re-implement the chain-walk against
//! the same `superseded_by` pointers the dashboard already resolved
//! — a duplication that would drift the moment the chain-walk
//! algorithm evolves.
//!
//! Story 10 cross-reference (2026-04-30 amendment). The original
//! fixture relied on healthy stories appearing in the frontier
//! `stories[]` array so the era_head_id assertion had rows to land
//! on. After story 10's healthy-exclusion invariant, healthy stories
//! no longer render in the frontier. The re-authored fixture seeds a
//! non-healthy (proposed) story so the frontier `stories[]` carries
//! the era_head_id field on at least one row; the canopy lens (which
//! renders healthy + retired together) carries it on every row. The
//! era_head_id contract this test pins is unchanged.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use agentic_story::Status;
use serde_json::{json, Value};
use tempfile::TempDir;

const HEAD_SHA: &str = "3333333333333333333333333333333333333333";

// Chain A → B → C; all three fixtures live in the corpus.
const ID_A_RETIRED: u32 = 93301;
const ID_B_RETIRED: u32 = 93302;
const ID_C_HEALTHY: u32 = 93303;
// Terminal-retirement — its own era head.
const ID_TERMINAL: u32 = 93304;
// A currently-healthy story that is NOT retired and NOT superseded
// — its era head is itself by definition.
const ID_SELF_HEAD: u32 = 93305;
// A proposed story added so the frontier `stories[]` array has at
// least one row to land the era_head_id assertion on after story 10's
// healthy-exclusion rule removes C and SELF_HEAD from frontier output.
const ID_PROPOSED_FOR_FRONTIER: u32 = 93306;

fn fixture(id: u32, status: &str, extra: &str) -> String {
    format!(
        r#"id: {id}
title: "Fixture {id} for JSON era_head_id field (story 3 amendment)"

outcome: |
  Fixture row for the JSON `era_head_id` scaffold.

status: {status}
{extra}
patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/json_mode_emits_era_head_id_per_story.rs
      justification: |
        Present so the fixture is schema-valid; the live test drives
        Dashboard's JSON renderers against this file.
  uat: |
    Render canopy --json and frontier --json; assert era_head_id on
    every row.

guidance: |
  Fixture authored inline for the JSON era_head_id scaffold. Not a
  real story.

depends_on: []
"#
    )
}

#[test]
fn canopy_and_frontier_json_rows_carry_era_head_id_pointing_at_chain_terminus() {
    // Compile-red anchor: Status::Retired must exist on the enum.
    let _retired_variant_exists: Status = Status::Retired;

    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // Chain: A (retired → B), B (retired → C), C (healthy, era head).
    fs::write(
        stories_dir.join(format!("{ID_A_RETIRED}.yml")),
        fixture(
            ID_A_RETIRED,
            "retired",
            &format!(
                "\nsuperseded_by: {ID_B_RETIRED}\nretired_reason: |\n  A folded into B for this scaffold's era_head_id check.\n"
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
                "\nsuperseded_by: {ID_C_HEALTHY}\nretired_reason: |\n  B folded into C for this scaffold's era_head_id check.\n"
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
    fs::write(
        stories_dir.join(format!("{ID_SELF_HEAD}.yml")),
        fixture(ID_SELF_HEAD, "healthy", ""),
    )
    .expect("write self-head");
    // Proposed story added so the frontier has rows after story 10's
    // healthy-exclusion rule removes C and SELF_HEAD from rendering.
    // It is not retired and not superseded, so its era_head_id ==
    // its own id.
    fs::write(
        stories_dir.join(format!("{ID_PROPOSED_FOR_FRONTIER}.yml")),
        fixture(ID_PROPOSED_FOR_FRONTIER, "proposed", ""),
    )
    .expect("write proposed-for-frontier");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed healthy signings for the two currently-healthy stories so
    // the classifier accepts them. The proposed story
    // (ID_PROPOSED_FOR_FRONTIER) is NOT seeded.
    for id in [ID_C_HEALTHY, ID_SELF_HEAD] {
        store
            .append(
                "uat_signings",
                json!({
                    "id": format!("01900000-0000-7000-8000-0000000{id}"),
                    "story_id": id,
                    "verdict": "pass",
                    "commit": HEAD_SHA,
                    "signed_at": "2026-04-23T00:00:00Z",
                }),
            )
            .expect("seed uat pass");
        store
            .upsert(
                "test_runs",
                &id.to_string(),
                json!({
                    "story_id": id,
                    "verdict": "pass",
                    "commit": HEAD_SHA,
                    "ran_at": "2026-04-23T00:00:00Z",
                    "failing_tests": [],
                }),
            )
            .expect("seed test_runs pass");
    }

    let dashboard = Dashboard::new(store, stories_dir, HEAD_SHA.to_string());

    // --- Canopy JSON: every row MUST carry `era_head_id`. ---------
    let canopy_json = dashboard
        .render_canopy_json()
        .expect("render_canopy_json should succeed");
    let canopy: Value = serde_json::from_str(&canopy_json)
        .unwrap_or_else(|e| panic!("canopy JSON must parse: {e}; raw:\n{canopy_json}"));
    let canopy_stories = canopy
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("canopy JSON `stories` must be an array; got: {canopy}"));

    let mut era_head_of: std::collections::HashMap<u64, u64> = std::collections::HashMap::new();
    for s in canopy_stories {
        let id = s
            .get("id")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| panic!("canopy row must carry `id`: {s}"));
        let era_head = s
            .get("era_head_id")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| {
                panic!(
                    "canopy JSON row must carry `era_head_id` so downstream tooling can \
                     group_by(.era_head_id) without re-walking superseded_by chains; \
                     got row: {s}"
                )
            });
        era_head_of.insert(id, era_head);
    }

    // (a) A, B, C → era_head_id == C (chain terminus).
    for ancestor in [ID_A_RETIRED, ID_B_RETIRED, ID_C_HEALTHY] {
        let got = era_head_of
            .get(&(ancestor as u64))
            .copied()
            .unwrap_or_else(|| {
                panic!(
                    "canopy must emit era_head_id for story {ancestor}; got era_head_of map: {era_head_of:?}"
                )
            });
        assert_eq!(
            got, ID_C_HEALTHY as u64,
            "canopy era_head_id for story {ancestor} must point at chain terminus \
             {ID_C_HEALTHY} (since C is the last non-retired-with-successor link); \
             got {got}"
        );
    }

    // (b) Terminal-retirement: its own era head (no successor).
    assert_eq!(
        era_head_of.get(&(ID_TERMINAL as u64)).copied(),
        Some(ID_TERMINAL as u64),
        "terminal-retirement story {ID_TERMINAL} must carry era_head_id == self \
         (no superseded_by means the story is itself the chain terminus); \
         got {:?}",
        era_head_of.get(&(ID_TERMINAL as u64))
    );

    // (c) A currently-healthy non-superseded story: era head is self.
    assert_eq!(
        era_head_of.get(&(ID_SELF_HEAD as u64)).copied(),
        Some(ID_SELF_HEAD as u64),
        "healthy non-superseded story {ID_SELF_HEAD} must carry era_head_id == self; \
         got {:?}",
        era_head_of.get(&(ID_SELF_HEAD as u64))
    );

    // --- Frontier JSON: retired AND healthy rows are filtered out
    //     (story 10's healthy-exclusion + story 21's retired-exclusion),
    //     so only the proposed row remains; it STILL carries era_head_id.
    //     The field is present in BOTH lenses so tooling has one stable
    //     way to reconstruct groups regardless of which view produced
    //     the output. ---------------------------------------------
    let frontier_json = dashboard
        .render_frontier_json()
        .expect("render_frontier_json should succeed");
    let frontier: Value = serde_json::from_str(&frontier_json)
        .unwrap_or_else(|e| panic!("frontier JSON must parse: {e}; raw:\n{frontier_json}"));
    let frontier_stories = frontier
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("frontier JSON `stories` must be an array; got: {frontier}"));

    assert!(
        !frontier_stories.is_empty(),
        "frontier JSON must surface the proposed story ({ID_PROPOSED_FOR_FRONTIER}) \
         so the era_head_id assertion has something to land on; got empty stories[]:\n{frontier}"
    );

    let frontier_ids: Vec<u64> = frontier_stories
        .iter()
        .filter_map(|s| s.get("id").and_then(|v| v.as_u64()))
        .collect();
    assert!(
        frontier_ids.contains(&(ID_PROPOSED_FOR_FRONTIER as u64)),
        "frontier JSON must include the proposed story {ID_PROPOSED_FOR_FRONTIER}; \
         got ids: {frontier_ids:?}"
    );

    for s in frontier_stories {
        let id = s
            .get("id")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| panic!("frontier row must carry `id`: {s}"));
        let era_head = s
            .get("era_head_id")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| {
                panic!(
                    "frontier JSON row for story {id} must ALSO carry `era_head_id` — the \
                     field is present in both frontier and canopy outputs so downstream \
                     tooling has a single, stable way to reconstruct grouping regardless \
                     of which lens produced the output; got row: {s}"
                )
            });
        // Retired and healthy rows are filtered from frontier, so every
        // visible row here must be its own era head (a non-retired,
        // non-superseded story whose chain terminus is itself).
        assert_eq!(
            era_head, id,
            "frontier-visible (non-retired, non-superseded) story {id} must carry \
             era_head_id == self; got era_head_id={era_head}, row:\n{s}"
        );
    }
}
