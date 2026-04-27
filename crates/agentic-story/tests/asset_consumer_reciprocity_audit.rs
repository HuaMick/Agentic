//! Story 27 acceptance test: cross-corpus reciprocity between a
//! story's `assets:` declarations and an asset's `current_consumers:`
//! list.
//!
//! Justification (from stories/27.yml): proves ADR-0007 decision 4
//! (cross-corpus reciprocity) at corpus level — the test loads the
//! live `stories/` directory and the live `assets/` tree and
//! asserts that for every asset whose `current_consumers:` list
//! names a story, that story's `assets:` field references the asset;
//! and conversely, for every story declaring an asset, the asset's
//! `current_consumers:` lists the story. Both directions matter and
//! are checked separately — a one-sided edge is exactly the silent
//! drift the audit exists to catch. The test runs against the real
//! corpus rather than a fixture because reciprocity is a property
//! of the corpus as shipped, not a property of a synthetic minimal
//! example.
//!
//! Implementation strategy: the test calls
//! `agentic_story::audit_asset_reciprocity(repo_root)` — the
//! sanctioned entry point for the audit. The function returns a
//! report value that surfaces dangling edges in BOTH directions.
//! The exact return shape is build-rust's call (a `Result<(), ..>`,
//! a struct with `story_to_asset` and `asset_to_story` Vec fields,
//! or a flat `Vec<DanglingEdge>` are all defensible — the assertion
//! below pins the OBSERVABLE: when the live corpus is reciprocal,
//! the audit reports zero dangling edges; when it is not, the
//! diagnostic names which side is missing.
//!
//! Red today is compile-red: `agentic_story::audit_asset_reciprocity`
//! does not yet exist as a public symbol on the crate. The
//! `use` import below fails to resolve, and `cargo check` errors
//! out before the assertions are reached. Once build-rust adds the
//! function (and the underlying corpus-walk that consumes the new
//! `Story::assets` field plus the asset-side `current_consumers`
//! parser), the same scaffold becomes runtime-red iff the live
//! corpus is non-reciprocal — and green when both sides agree.

use std::path::PathBuf;

// IMPORTANT: this `use` is what produces the compile-red diagnostic
// today. `audit_asset_reciprocity` is the API the justification
// names; it does not yet exist on the agentic-story crate's public
// surface, so this import does not resolve. Once build-rust adds the
// function, the same import becomes the runtime entry point and the
// rest of the test exercises the corpus.
use agentic_story::audit_asset_reciprocity;

/// Walk up from `CARGO_MANIFEST_DIR` until a `.git` entry appears.
fn repo_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join(".git").exists() {
            return dir;
        }
        if !dir.pop() {
            panic!(
                "could not find repo root walking up from CARGO_MANIFEST_DIR={}",
                env!("CARGO_MANIFEST_DIR")
            );
        }
    }
}

#[test]
fn asset_consumer_reciprocity_audit_reports_no_dangling_edges_in_either_direction() {
    let root = repo_root();

    // The audit walks both halves of the corpus:
    //   1. For every story under `stories/*.yml`, every asset path
    //      in its `assets:` field must back-reference the story in
    //      that asset's `current_consumers:` list.
    //   2. For every asset under `assets/**/*.yml`, every
    //      story-shaped entry in its `current_consumers:` list
    //      (anything matching `^stories/[0-9]+\.yml$`) must point
    //      at a story that declares the asset in its `assets:`
    //      field.
    //
    // The function's return shape is build-rust's call; the
    // ASSERTION is on the observable contract: when the corpus is
    // reciprocal, the audit reports success with zero dangling
    // edges. The match arms below name the two failure shapes the
    // justification calls out separately (story->asset missing
    // back-reference; asset->story missing back-reference) and
    // panic with a diagnostic naming the dangling edge so the
    // human reading the failure can fix the corpus without
    // re-deriving the audit.
    let report = audit_asset_reciprocity(&root);

    // The audit's success contract: a `Result<(), AuditError>` (or
    // any equivalent shape build-rust picks) that is `Ok(())` when
    // every edge in either direction has its mate. The expect()
    // call surfaces the audit's typed error as the diagnostic when
    // the corpus is non-reciprocal at the moment the test runs —
    // the same shape every other loader-error test in this crate
    // uses (see e.g. load_unknown_path_is_typed_absence.rs).
    report.expect(
        "the live corpus must be cross-corpus reciprocal: every story \
         declaring an asset must be back-referenced in that asset's \
         current_consumers, and every asset listing a story consumer \
         must point at a story whose assets array references the asset. \
         The audit returned an error naming the first dangling edge it \
         found; a one-sided edge is exactly the drift ADR-0007 \
         decision 4 forbids.",
    );
}
