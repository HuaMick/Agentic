//! Story 27 acceptance test: the schema-and-loader happy path for the
//! new `assets:` field on stories.
//!
//! Justification (from stories/27.yml): proves the happy path for the new
//! schema field — a story YAML whose root carries
//! `assets: [agents/assets/principles/deep-modules.yml]` loads cleanly
//! through `Story::load` (and `Story::load_dir`) into a typed `Story`
//! value whose `assets` field round-trips as a `Vec<String>` (or
//! equivalent path-typed collection) preserving order and content.
//! Stories without an `assets:` block load with `assets` as the empty
//! default — explicitly absent and explicitly empty are both accepted
//! and observably indistinguishable downstream, matching the existing
//! `patterns:` field's posture.
//!
//! Per ADR-0007 decision 1 the story schema gains a root-level array
//! `assets:` whose items match `^agents/assets/.*\.ya?ml$`, defaulting
//! to `[]`. The scaffold writes a fixture YAML carrying a single
//! `assets:` entry and a sibling fixture that omits `assets:` entirely;
//! both must load, the first preserving the entry verbatim, the second
//! defaulting to an empty `Vec<String>`.
//!
//! Red today is compile-red: `agentic_story::Story` does not yet
//! declare an `assets` field, so the field accesses below do not
//! resolve. The same red shape story 9's `related_files` scaffolds
//! relied on — a deliberate mirror so build-rust can lift the same
//! pattern.

use std::fs;

use agentic_story::Story;
use tempfile::TempDir;

/// Fixture with a single `assets:` entry. The path is the deep-modules
/// principle — chosen because the asset exists on disk at repo root,
/// so the directory-load path of `loader_rejects_unknown_asset_path.rs`
/// is exercising the converse leg of the same contract.
const WITH_ASSETS_YAML: &str = r#"id: 271
title: "Fixture for the assets-field round-trip test"

outcome: |
  A developer can load this fixture from disk and observe the
  assets array preserved in memory exactly as it was authored,
  including the order of its entries.

status: proposed

patterns: []

assets:
  - agents/assets/principles/deep-modules.yml

acceptance:
  tests:
    - file: crates/agentic-story/tests/loader_accepts_assets_field.rs
      justification: |
        The very scaffold you are reading. Present so this fixture is
        itself schema-valid.
  uat: |
    Read the loaded Story, eyeball the assets field, confirm it
    matches the fixture on disk.

guidance: |
  Fixture authored inline for the assets-field round-trip test. Not a
  real story.

depends_on: []
"#;

/// Fixture that OMITS `assets:` entirely — mirrors every story on disk
/// today. Must load without error; `assets` must default to an empty
/// vec (not Option::None, not an absent-sentinel) so downstream code
/// does not have to unwrap, parity with `patterns:` and
/// `related_files:`.
const WITHOUT_ASSETS_YAML: &str = r#"id: 272
title: "Fixture omitting the assets field"

outcome: |
  A developer can load this fixture whose YAML has no assets key and
  observe a Story whose assets is an empty Vec, not a parse error and
  not an absent sentinel.

status: proposed

patterns: []

acceptance:
  tests:
    - file: crates/agentic-story/tests/loader_accepts_assets_field.rs
      justification: |
        The very scaffold you are reading. Present so this fixture is
        itself schema-valid.
  uat: |
    Load this fixture; confirm no error; confirm assets is empty.

guidance: |
  Fixture authored inline for the absent-is-empty assets-field test.
  Not a real story.

depends_on: []
"#;

#[test]
fn loader_accepts_assets_field_round_trips_and_defaults_to_empty() {
    let dir = TempDir::new().expect("create temp dir");

    // Half 1: an explicit single-entry assets array round-trips with
    // content and order intact.
    let with_path = dir.path().join("271.yml");
    fs::write(&with_path, WITH_ASSETS_YAML).expect("write with-assets fixture");
    let with_story: Story =
        Story::load(&with_path).expect("a story declaring assets must load successfully");

    let expected_with: Vec<String> = vec!["agents/assets/principles/deep-modules.yml".to_string()];
    assert_eq!(
        with_story.assets, expected_with,
        "assets must round-trip as a Vec<String> with content and order \
         exactly equal to the fixture on disk; got {:?}",
        with_story.assets
    );

    // Half 2: a story that omits `assets:` entirely loads with the
    // field defaulting to an empty Vec — explicitly absent and
    // explicitly empty are observably indistinguishable downstream.
    let without_path = dir.path().join("272.yml");
    fs::write(&without_path, WITHOUT_ASSETS_YAML).expect("write without-assets fixture");
    let without_story: Story = Story::load(&without_path)
        .expect("a story that omits the assets field must load successfully");

    assert!(
        without_story.assets.is_empty(),
        "when assets is omitted from the YAML the Story's assets must \
         be an empty Vec; got {:?}",
        without_story.assets
    );
    assert_eq!(
        without_story.assets.len(),
        0,
        "omitted assets must produce a Vec of length 0; got len={}",
        without_story.assets.len()
    );
}
