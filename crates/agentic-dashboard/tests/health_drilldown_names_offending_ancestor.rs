//! Story 13 acceptance test: drilldown parity with the JSON contract
//! for offending direct ancestors.
//!
//! Justification (from stories/13.yml): proves drilldown parity with
//! the JSON contract: `agentic stories health <id>` for a story
//! classified unhealthy due to its direct `depends_on` parents renders
//! a line listing EVERY offending direct ancestor by id —
//! comma-separated and ordered ascending by parent id, matching the
//! JSON `not_healthy_reason` array's `"ancestor:<id>"` entries
//! one-for-one — plus each offending ancestor's own classification.
//! A fixture with two offending direct parents (e.g. `<A>` and `<C>`,
//! with a healthy `<B>` in between) exercises the multi-offender case.
//!
//! The scaffold materialises a fixture with a healthy direct parent
//! `B` between two offending direct parents `A` and `C` (`A.id < C.id`)
//! and a leaf `L`. It calls `Dashboard::drilldown(L)` and asserts the
//! rendered text contains a line that lists both offenders
//! comma-separated in ascending order AND each offending ancestor's
//! own classification. Red today is runtime-red because the drilldown
//! formatter does not yet have a dedicated "offending ancestors" line;
//! it lists ancestors generically in an `Ancestors:` block without
//! matching the JSON contract's shape.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::json;
use tempfile::TempDir;

const HEAD_SHA: &str = "1313130000000000000000000000000000000007";
const ID_A: u32 = 913071; // offender — lower id, must come first
const ID_B: u32 = 913072; // healthy — must not appear as offender
const ID_C: u32 = 913073; // offender — higher id, must come second
const ID_L: u32 = 913074;

fn fixture(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for drilldown offending-ancestors scaffold"

outcome: |
  Fixture row for the drilldown offending-ancestors scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_drilldown_names_offending_ancestor.rs
      justification: |
        Present so the fixture is schema-valid. The live test calls
        Dashboard::drilldown on L and asserts the rendered view names
        both offending direct ancestors comma-separated in ascending
        id order.
  uat: |
    Render the drilldown for L; assert offending ancestors appear
    comma-separated in ascending id order with classifications.

guidance: |
  Fixture authored inline for the drilldown offending-ancestors
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
fn drilldown_view_lists_every_offending_direct_ancestor_comma_separated_ascending_by_id() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    fs::write(
        stories_dir.join(format!("{ID_A}.yml")),
        fixture(ID_A, "under_construction", &[]),
    )
    .expect("write A under_construction");
    fs::write(
        stories_dir.join(format!("{ID_B}.yml")),
        fixture(ID_B, "healthy", &[]),
    )
    .expect("write B healthy");
    fs::write(
        stories_dir.join(format!("{ID_C}.yml")),
        fixture(ID_C, "under_construction", &[]),
    )
    .expect("write C under_construction");
    fs::write(
        stories_dir.join(format!("{ID_L}.yml")),
        fixture(ID_L, "healthy", &[ID_A, ID_B, ID_C]),
    )
    .expect("write L depends_on=[A, B, C]");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    seed_healthy(&store, ID_B, "000000913072");
    seed_healthy(&store, ID_L, "000000913074");

    let dashboard = Dashboard::new(store.clone(), stories_dir.clone(), HEAD_SHA.to_string());
    let drill = dashboard
        .drilldown(ID_L)
        .expect("drilldown(L) should succeed on a four-story fixture");

    // The drilldown view must carry a line listing every offending direct
    // ancestor id, comma-separated in ascending-by-id order. The exact
    // line label is pinned by the story as "offending ancestors" (or an
    // equivalent token) — we accept any case variant of the word
    // "ancestor" paired with the two ids in order. The key invariants:
    //   (i) both offender ids appear on the same line,
    //   (ii) in ascending id order (A before C),
    //   (iii) B (healthy) does NOT appear as an offender on that line,
    //   (iv) each offender's classification ("under_construction") is
    //        rendered near its id.
    let a_s = ID_A.to_string();
    let c_s = ID_C.to_string();
    let b_s = ID_B.to_string();

    let offender_line = drill
        .lines()
        .find(|ln| {
            let lower = ln.to_ascii_lowercase();
            lower.contains("ancestor") && ln.contains(&a_s) && ln.contains(&c_s)
        })
        .unwrap_or_else(|| {
            panic!(
                "drilldown(L) must render a line naming both offending \
                 direct ancestors A (id {ID_A}) and C (id {ID_C}) together \
                 with some variant of the word `ancestor`; got drilldown:\n\
                 {drill}"
            )
        });

    // (ii) ordering: A's id must appear before C's id on the line.
    let a_pos = offender_line
        .find(&a_s)
        .unwrap_or_else(|| panic!("A id must be on the offender line: {offender_line:?}"));
    let c_pos = offender_line
        .find(&c_s)
        .unwrap_or_else(|| panic!("C id must be on the offender line: {offender_line:?}"));
    assert!(
        a_pos < c_pos,
        "offender line must list A (id {ID_A}) before C (id {ID_C}) — \
         ascending by parent id. Got line: {offender_line:?}"
    );

    // (ii-b) comma-separated: between the two ids there must be a comma.
    let between = &offender_line[a_pos + a_s.len()..c_pos];
    assert!(
        between.contains(','),
        "offender line must separate the two ancestor ids with a comma; \
         got between-text {between:?} on line {offender_line:?}"
    );

    // (iii) healthy parent B must NOT appear on the offender line as an
    // offender entry. (B's id string could appear in unrelated context
    // across the full drilldown — ancestors section etc. — so we scope
    // this check to the offender line only.)
    assert!(
        !offender_line.contains(&b_s),
        "healthy direct parent B (id {ID_B}) must NOT appear on the \
         offender line; got line {offender_line:?}"
    );

    // (iv) each offender's own classification ("under_construction")
    // must appear in the drilldown — the story pins that the drilldown
    // names "each offending ancestor's own classification". We accept
    // this anywhere in the drilldown (the rendering may place it on the
    // same line or a follow-on indented block), so check the full text.
    assert!(
        drill.contains("under_construction"),
        "drilldown must render each offending ancestor's own \
         classification (`under_construction` for A and C); got:\n{drill}"
    );
}
