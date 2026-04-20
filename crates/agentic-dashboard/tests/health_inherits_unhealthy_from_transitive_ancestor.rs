//! Story 13 acceptance test: the transitive reach of the ancestor-
//! inheritance rule.
//!
//! Justification (from stories/13.yml): proves the transitive reach:
//! given `<id>` depends_on `<A>`, `<A>` depends_on `<B>`, `<A>`
//! classifies `healthy` (all its own signals are clean), but `<B>` is
//! `under_construction`, the dashboard classifies `<id>` as
//! `unhealthy` — the rule reaches past direct ancestors to the first
//! non-healthy link. Without this, the rule is a direct-only check and
//! the same "healthy ancestor but broken grandparent" loophole story
//! 11 closes at the UAT gate stays open at the read boundary.
//!
//! The scaffold constructs a three-story chain L -> A -> B where A's
//! own signals are all clean (YAML healthy, UAT pass @ HEAD, tests
//! pass) and L's own signals are all clean too, but B is
//! `under_construction`. The assertion: L classifies `unhealthy`
//! because the transitive walk must reach B. Red today is runtime-red
//! because the current classifier does not walk the ancestry chain;
//! it emits `healthy` for L.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::{json, Value};
use tempfile::TempDir;

const HEAD_SHA: &str = "1313130000000000000000000000000000000002";
const ID_B: u32 = 913021; // grandparent — under_construction (broken link)
const ID_A: u32 = 913022; // mid — own signals all healthy, depends_on = [B]
const ID_L: u32 = 913023; // leaf — own signals all healthy, depends_on = [A]

fn fixture(id: u32, status: &str, depends_on: &[u32]) -> String {
    let deps_yaml = if depends_on.is_empty() {
        "depends_on: []".to_string()
    } else {
        let lines: Vec<String> = depends_on.iter().map(|d| format!("  - {d}")).collect();
        format!("depends_on:\n{}", lines.join("\n"))
    };
    format!(
        r#"id: {id}
title: "Fixture {id} for transitive-ancestor inheritance scaffold"

outcome: |
  Fixture row for the transitive-ancestor inheritance scaffold.

status: {status}

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_inherits_unhealthy_from_transitive_ancestor.rs
      justification: |
        Present so the fixture is schema-valid. The live test drives
        Dashboard against this YAML to exercise the transitive walk.
  uat: |
    Render the dashboard; assert L classifies as unhealthy when its
    grandparent B is not healthy, even though its direct parent A is
    healthy.

guidance: |
  Fixture authored inline for the transitive-ancestor inheritance
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
fn leaf_with_healthy_direct_parent_classifies_unhealthy_when_grandparent_is_not_healthy() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");

    // B: under_construction, no UAT signing — classifies under_construction.
    fs::write(
        stories_dir.join(format!("{ID_B}.yml")),
        fixture(ID_B, "under_construction", &[]),
    )
    .expect("write B under_construction");

    // A: healthy, depends_on = [B], own signals all clean.
    fs::write(
        stories_dir.join(format!("{ID_A}.yml")),
        fixture(ID_A, "healthy", &[ID_B]),
    )
    .expect("write A healthy depends_on=[B]");

    // L: healthy, depends_on = [A], own signals all clean.
    fs::write(
        stories_dir.join(format!("{ID_L}.yml")),
        fixture(ID_L, "healthy", &[ID_A]),
    )
    .expect("write L healthy depends_on=[A]");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // Seed A's and L's own signals to be clean.
    seed_healthy(&store, ID_A, "000000913022");
    seed_healthy(&store, ID_L, "000000913023");
    // B carries no UAT signing — classifies `under_construction` from YAML.

    let dashboard = Dashboard::new(store.clone(), stories_dir.clone(), HEAD_SHA.to_string());
    let rendered = dashboard
        .render_json()
        .expect("render_json should succeed on the three-story chain");

    let parsed: Value = serde_json::from_str(&rendered)
        .unwrap_or_else(|e| panic!("render_json output must parse as JSON: {e}; raw:\n{rendered}"));

    let stories = parsed
        .get("stories")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("top-level `stories` must be an array; got: {parsed}"));

    let health_of = |id: u32| -> String {
        stories
            .iter()
            .find(|s| s.get("id").and_then(|v| v.as_u64()) == Some(id as u64))
            .and_then(|s| s.get("health").and_then(|v| v.as_str()))
            .map(|s| s.to_string())
            .unwrap_or_else(|| panic!("stories[] must include id {id}; got: {parsed}"))
    };

    // Preconditions: B is not-healthy; A's own signals are clean, so the
    // only way A ends up `unhealthy` is via the ancestor rule firing on
    // B — which is itself part of what this story pins, so we don't
    // assert on A's classification directly. We DO assert on L:
    // regardless of what A classifies as (healthy or unhealthy-via-
    // inheritance), L must classify `unhealthy` because there is a
    // non-healthy link (B) anywhere in its transitive ancestry.
    let b_health = health_of(ID_B);
    assert_ne!(
        b_health, "healthy",
        "precondition: grandparent B must classify as not-healthy so the \
         transitive walk has something to find; got B.health={b_health}"
    );

    let l_health = health_of(ID_L);
    assert_eq!(
        l_health, "unhealthy",
        "L's own signals are clean and its direct parent A's own signals \
         are clean too, but L's grandparent B is {b_health}. The \
         transitive-ancestor inheritance rule must walk past the direct \
         parent and classify L as `unhealthy`. Got L.health={l_health}; \
         full JSON: {parsed}"
    );
}
