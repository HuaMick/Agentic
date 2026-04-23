//! Story 21 acceptance test: the three retired-fossil YAMLs
//! (`stories/7.yml`, `stories/8.yml`, `stories/14.yml`) load through
//! the directory loader with the shape this story's backfill step
//! produces.
//!
//! Justification (from stories/21.yml):
//! Proves the three retroactive backfill fixtures
//! (`stories/7.yml`, `stories/8.yml`, `stories/14.yml`) exist on disk
//! in the shape this story's backfill step produces: each loads
//! cleanly through the `agentic-story` directory loader, each reads
//! back with `status: Retired`, each carries a `superseded_by`
//! pointing at an extant sibling story (7 → 15, 8 → 1, 14 → 15), and
//! each carries a non-empty `retired_reason` string. The test
//! asserts the exact successor ids above, asserts the three target
//! ids (15, 1, 15) resolve to loaded `Story` values in the same
//! corpus (i.e. referential integrity at the supersession edges),
//! and asserts the three fossils' `retired_reason` fields are
//! non-empty prose rather than empty strings.
//!
//! Red today is compile-red: the `Story` type has no
//! `superseded_by` or `retired_reason` field, and the `Status` enum
//! has no `Retired` variant — both are schema additions this story
//! (bundled into story 6's amendment pass) introduces. The
//! fossil YAMLs themselves are corpus artefacts build-rust will land
//! in the same atomic; the loader-shape this test pins is what makes
//! the integration assertable.

use std::path::Path;

use agentic_story::{Status, Story};

/// The three retired-fossil ids and their authoritative successors,
/// per stories/21.yml "Backfill successor choices":
///   - 7  → 15  (evidence-atomicity folded into 15 on 2026-04-20)
///   - 8  → 1   (early CLI-shell story; UAT half inherited by 1)
///   - 14 → 15  (claude-as-component replaced by claude-as-user in 15)
const FOSSILS_TO_SUCCESSORS: &[(u32, u32)] = &[(7, 15), (8, 1), (14, 15)];

#[test]
fn retired_fossil_files_load_as_retired_stories_with_expected_superseded_by_and_non_empty_reason() {
    // Load the live stories/ directory through the same directory
    // loader every other consumer uses. The backfill fossils are
    // corpus artefacts — this test exercises them in situ rather
    // than in a tempdir, so a regression in the live YAMLs (e.g.
    // someone deletes stories/7.yml or flips a successor) is caught
    // by the CI signal immediately, not by a separate audit run.
    let stories_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("agentic-story crate has a parent")
        .parent()
        .expect("workspace root two levels up")
        .join("stories");

    assert!(
        stories_dir.is_dir(),
        "expected live stories dir at {}",
        stories_dir.display()
    );

    let loaded = Story::load_dir(&stories_dir)
        .expect("live stories/ must load cleanly after story-21 backfill");

    for &(fossil_id, expected_successor) in FOSSILS_TO_SUCCESSORS {
        let fossil = loaded
            .iter()
            .find(|s| s.id == fossil_id)
            .unwrap_or_else(|| {
                panic!(
                    "backfill expected stories/{fossil_id}.yml to load as a Story; \
                     none found in the live corpus"
                )
            });

        // The fossil must read back as Retired.
        assert_eq!(
            fossil.status,
            Status::Retired,
            "fossil story {fossil_id} must load with status=Retired; got {:?}",
            fossil.status
        );

        // The fossil carries a superseded_by pointing at the
        // authoritative successor named in stories/21.yml's
        // "Backfill successor choices" section.
        let successor_id = fossil.superseded_by.unwrap_or_else(|| {
            panic!(
                "fossil story {fossil_id} must carry superseded_by={expected_successor}; \
                 got None"
            )
        });
        assert_eq!(
            successor_id, expected_successor,
            "fossil story {fossil_id} must point at its authoritative successor \
             {expected_successor}; got {successor_id}"
        );

        // The fossil carries a non-empty retired_reason prose block.
        // Optional at the schema level (per story 21 guidance, to
        // avoid fabricated reasons on the historical backfill), but
        // the three fossils named here all have real prose authored
        // alongside them and must not be empty strings.
        let reason = fossil.retired_reason.as_ref().unwrap_or_else(|| {
            panic!(
                "fossil story {fossil_id} must carry a retired_reason prose block; \
                 got None"
            )
        });
        assert!(
            !reason.trim().is_empty(),
            "fossil story {fossil_id}'s retired_reason must be non-empty prose; \
             got whitespace-only value {reason:?}"
        );

        // Referential integrity: the successor id resolves to a
        // loaded `Story` in the same corpus. A superseded_by
        // pointing at an unknown id is a loader-time rejection
        // (owned by story 6's amendment, pinned separately), but we
        // re-assert it here so a regression in either layer surfaces
        // through this integration test rather than only through the
        // loader unit test.
        assert!(
            loaded.iter().any(|s| s.id == expected_successor),
            "fossil story {fossil_id}'s successor {expected_successor} must \
             resolve to a loaded Story in the same corpus"
        );
    }
}
