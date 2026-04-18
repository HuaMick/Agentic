//! Story 3 acceptance test: a malformed story YAML renders as an
//! `error` row without taking the dashboard down.
//!
//! Justification (from stories/3.yml): proves the dashboard does not
//! crash on bad input — given one story file that fails YAML parse or
//! schema validation, the dashboard renders that story's row with
//! health `error` and a short reason in the `Failing tests` cell, AND
//! continues to render every other story correctly. Without this, a
//! single malformed story file would blank the entire dashboard —
//! making the worst case (we can't see what's broken) coincide with the
//! moment we most need to see what's broken.
//!
//! The scaffold materialises two story files under a `TempDir`
//! `stories/` directory: one well-formed `proposed` story, and one
//! whose YAML is unparseable (unbalanced quotes). It calls
//! `Dashboard::render_table` and asserts (a) the call returns Ok (the
//! dashboard did NOT crash on one bad file), (b) the good story's row
//! classifies as `proposed`, and (c) the bad story's row appears with
//! health `error` and a short reason substring in the Failing tests
//! cell. The bad file's stem is a known integer so the row can be
//! identified by id in the rendered output.
//! Red today is compile-red via the missing `agentic_dashboard` public
//! surface.

use std::fs;
use std::sync::Arc;

use agentic_dashboard::Dashboard;
use agentic_store::{MemStore, Store};
use tempfile::TempDir;

const GOOD_ID: u32 = 9701;
const BAD_ID: u32 = 9702;
const HEAD_SHA: &str = "4444444444444444444444444444444444444444";

const GOOD_FIXTURE: &str = r#"id: 9701
title: "A perfectly well-formed proposed story"

outcome: |
  Fixture that is schema-valid; must render without incident alongside a
  broken sibling file.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/agentic-dashboard/tests/health_malformed_yaml_renders_error_row.rs
      justification: |
        Present so the fixture is schema-valid.
  uat: |
    Render the table; assert this row renders correctly despite a
    sibling malformed file.

guidance: |
  Fixture authored inline for the malformed-yaml-renders-error-row
  scaffold. Not a real story.

depends_on: []
"#;

/// Deliberately broken YAML. `title:` opens a double-quoted string and
/// never closes it; the structural parse fails on the second line.
/// The filename is `<BAD_ID>.yml` so the dashboard has a stable id to
/// attach an `error` row to (stems-as-ids is the loader's convention —
/// the dashboard is expected to pull the id from the filename when the
/// inner YAML cannot be trusted).
const BAD_FIXTURE: &str = r#"id: 9702
title: "unterminated string
status: proposed
"#;

#[test]
fn render_table_emits_error_row_for_malformed_yaml_and_still_renders_good_siblings() {
    let tmp = TempDir::new().expect("tempdir");
    let stories_dir = tmp.path().join("stories");
    fs::create_dir_all(&stories_dir).expect("stories dir");
    fs::write(stories_dir.join(format!("{GOOD_ID}.yml")), GOOD_FIXTURE)
        .expect("write good fixture");
    fs::write(stories_dir.join(format!("{BAD_ID}.yml")), BAD_FIXTURE)
        .expect("write bad fixture");

    let store: Arc<dyn Store> = Arc::new(MemStore::new());
    let dashboard = Dashboard::new(store.clone(), stories_dir.clone(), HEAD_SHA.to_string());

    // (a) Call returns Ok despite the malformed sibling file. The
    // dashboard's whole reason for being is to surface broken state;
    // crashing on the first bad file is the exact anti-feature the
    // story calls out.
    let rendered = dashboard
        .render_table()
        .expect("render_table must not crash on a single malformed story file");

    // (b) Good story row: present, classifies as proposed.
    let good_row = rendered
        .lines()
        .find(|line| line.contains(&GOOD_ID.to_string()))
        .unwrap_or_else(|| {
            panic!(
                "rendered table must contain a row for the good story {GOOD_ID}; got:\n{rendered}"
            )
        });
    assert!(
        good_row.contains("proposed"),
        "good story {GOOD_ID} must still classify as `proposed` despite a sibling \
         malformed file; got row: {good_row:?}\nfull table:\n{rendered}"
    );

    // (c) Bad story row: present (by id, pulled from the filename), with
    // `error` as its health classification.
    let bad_row = rendered
        .lines()
        .find(|line| line.contains(&BAD_ID.to_string()))
        .unwrap_or_else(|| {
            panic!(
                "rendered table must contain an `error` row for the malformed story \
                 {BAD_ID}; got:\n{rendered}"
            )
        });
    assert!(
        bad_row.contains("error"),
        "malformed story {BAD_ID} must render with health `error`; \
         got row: {bad_row:?}\nfull table:\n{rendered}"
    );
    // A short reason string must appear in the Failing tests cell.
    // Story 3's guidance names `"yaml parse"` and `"schema: status"` as
    // example reasons; for this fixture (unbalanced quote) the reason
    // will reference parsing. We accept any of those substrings so the
    // implementation is free to choose its exact wording.
    let has_short_reason = bad_row.contains("yaml")
        || bad_row.contains("parse")
        || bad_row.contains("schema");
    assert!(
        has_short_reason,
        "Failing tests cell for malformed story {BAD_ID} must carry a short \
         reason (one of `yaml`, `parse`, `schema`); got row: {bad_row:?}\n\
         full table:\n{rendered}"
    );
}
