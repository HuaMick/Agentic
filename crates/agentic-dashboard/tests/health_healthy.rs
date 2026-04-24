//! Story 3 acceptance test: the healthy-classification rule.
//!
//! Justification (from stories/3.yml): proves the healthy-classification
//! rule — a story whose YAML has `status: healthy`, whose latest
//! `uat_signings.verdict=pass` commit equals current HEAD, AND whose
//! latest `test_runs.verdict=pass`, renders as `healthy` with a
//! `Healthy at` cell showing the short SHA and a relative age ("3h ago").
//! Without this the only path to `healthy` (story 1) produces no
//! observable signal in the dashboard.
//!
//! The scaffold seeds a `MemStore` with a single `uat_signings` row
//! whose `commit` equals the dashboard's `head_sha` and a Pass
//! `test_runs` row for the same story, then asserts the rendered row
//! (a) classifies as `healthy`, (b) names the 7-char short SHA in the
//! `Healthy at` cell, and (c) carries a relative-age string matching
//! one of `just now | Nm ago | Nh ago | Nd ago`. Red today is
//! compile-red via the missing `agentic_dashboard` public surface.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use serde_json::json;
use tempfile::TempDir;

const STORY_ID: u32 = 9303;
const HEAD_SHA: &str = "1234567890abcdef1234567890abcdef12345678";
const SHORT_SHA: &str = "1234567"; // first 7 chars of HEAD_SHA

const HEALTHY_FIXTURE: &str = r#"id: 9303
title: "A story that was UAT-passed at HEAD"

outcome: |
  A fixture whose YAML status is `healthy`, whose latest UAT signing's
  commit equals HEAD, and whose latest test_runs row is Pass.

status: healthy

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_healthy.rs
      justification: |
        Present so the fixture is schema-valid. The live test drives
        Dashboard::render_table against this YAML with evidence seeded
        so the story is classifiable as healthy.
  uat: |
    Render the table; assert the health column reads `healthy` and the
    `Healthy at` cell shows the short SHA and a relative age.

guidance: |
  Fixture authored inline for the healthy-classification scaffold. Not a
  real story.

depends_on: []
"#;

/// Matches the relative-age vocabulary pinned in story 3's guidance:
/// "just now" | "Nm ago" | "Nh ago" | "Nd ago" (rough buckets; finer
/// than a minute is not useful).
fn looks_like_relative_age(s: &str) -> bool {
    if s.contains("just now") {
        return true;
    }
    // Accept any occurrence of `<digits><unit> ago` where unit is one of
    // s/m/h/d. `s ago` is kept in the regex-style check because the
    // renderer may use seconds for very recent events even though the
    // story names minute as the finest useful bucket — the scaffold
    // must not reject an implementation that is MORE precise.
    let looks_like_unit = |c: char| matches!(c, 's' | 'm' | 'h' | 'd');
    s.split_whitespace().any(|token| {
        let mut chars = token.chars();
        let mut saw_digit = false;
        while let Some(c) = chars.clone().next() {
            if c.is_ascii_digit() {
                saw_digit = true;
                chars.next();
            } else {
                break;
            }
        }
        saw_digit && chars.next().map(looks_like_unit).unwrap_or(false)
    }) && s.contains("ago")
}

#[test]
fn story_healthy_at_head_with_passing_tests_renders_healthy_with_short_sha_and_relative_age() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(stories_dir.join(format!("{STORY_ID}.yml")), HEALTHY_FIXTURE)
        .expect("write healthy fixture");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());

    // uat_signings row: verdict=pass, commit == HEAD.
    store
        .append(
            "uat_signings",
            json!({
                "id": "01900000-0000-7000-8000-000000009303",
                "story_id": STORY_ID,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "signed_at": "2026-04-19T00:00:00Z",
            }),
        )
        .expect("seed uat_signings pass@HEAD");

    // test_runs row: verdict=pass.
    store
        .upsert(
            "test_runs",
            &STORY_ID.to_string(),
            json!({
                "story_id": STORY_ID,
                "verdict": "pass",
                "commit": HEAD_SHA,
                "ran_at": "2026-04-19T00:00:00Z",
                "failing_tests": [],
            }),
        )
        .expect("seed test_runs pass");

    let dashboard = Dashboard::new(store.clone(), stories_dir.clone(), HEAD_SHA.to_string());
    let rendered = dashboard
        .render_table()
        .expect("render_table should succeed on a single healthy story");

    let row = rendered
        .lines()
        .find(|line| line.contains(&STORY_ID.to_string()))
        .unwrap_or_else(|| {
            panic!("rendered table must contain a row for story {STORY_ID}; got:\n{rendered}")
        });

    // (a) Health classification.
    assert!(
        row.contains("healthy"),
        "story {STORY_ID} must classify as `healthy`; got row: {row:?}\n\
         full table:\n{rendered}"
    );
    assert!(
        !row.contains("unhealthy"),
        "row contains the substring `unhealthy` — health column must \
         render bare `healthy`, not `unhealthy`; row: {row:?}"
    );

    // (b) Short SHA in `Healthy at` cell.
    assert!(
        row.contains(SHORT_SHA),
        "Healthy at cell must show the 7-char short SHA `{SHORT_SHA}`; \
         got row: {row:?}\nfull table:\n{rendered}"
    );
    // Full SHA must NOT appear in the table (story 3 guidance: short in
    // table, full in JSON). If it does, the implementation failed to
    // truncate.
    assert!(
        !row.contains(HEAD_SHA),
        "Healthy at cell must show the SHORT SHA, not the full 40-char SHA; \
         got row: {row:?}"
    );

    // (c) Relative-age string: `just now` or `N<s|m|h|d> ago`.
    assert!(
        looks_like_relative_age(row),
        "Healthy at cell must show a relative-age string like \
         `just now` or `Nm ago` / `Nh ago` / `Nd ago`; got row: {row:?}\n\
         full table:\n{rendered}"
    );
}
