//! Story 27 acceptance test: the asset schema's `current_consumers[]`
//! pattern is widened to also accept story-path consumers.
//!
//! Justification (from stories/27.yml): proves ADR-0007 decision 2 —
//! the asset schema's `current_consumers:` regex is widened from
//! agent-spec-triplet paths only to also accept
//! `^stories/[0-9]+\.yml$`. The test loads (or schema-validates) an
//! asset YAML whose `current_consumers:` array contains the string
//! `stories/27.yml` and asserts the validation passes; the converse
//! case — an asset listing a malformed pseudo-story path like
//! `stories/abc.yml` or `stories/27.yaml` — must still fail. The
//! test pins the EXACT widened regex from ADR-0007
//! (`^(agents/[a-z][a-z0-9-]*/[a-z][a-z0-9-]*/(contract|inputs|process)\.yml|stories/[0-9]+\.yml)$`)
//! so a future schema edit cannot silently narrow the consumer
//! vocabulary back.
//!
//! Implementation strategy: the test reads
//! `schemas/asset.schema.json` straight from the repo root,
//! navigates to `properties.current_consumers.items.pattern`, and
//! compiles that pattern with the `regex` crate. Match-then-assert
//! on the four lexical cases the justification names. This means
//! the schema itself is the unit under test — when ADR-0007 is
//! implemented by editing `schemas/asset.schema.json`, the schema
//! file's pattern string is the surface that flips this test green.
//!
//! Red today is runtime-red: the schema currently carries the
//! agent-only pattern
//! `^agents/[a-z][a-z0-9-]*/[a-z][a-z0-9-]*/(contract|inputs|process)\.yml$`.
//! Compiling that pattern and asking it to match `stories/27.yml`
//! returns false — the assertion fires and the test panics with a
//! diagnostic naming the live schema pattern, the path the test
//! tried to match, and the widened pattern ADR-0007 mandates.

use std::fs;
use std::path::PathBuf;

use regex::Regex;
use serde_json::Value;

/// Find the repo root by walking up from the integration test's
/// `CARGO_MANIFEST_DIR` until a `.git` entry appears. Mirrors the
/// approach in `scripts/agentic-search.sh` so test scaffolds and
/// shell verifiers agree on what "repo root" means.
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

/// Pull the live `current_consumers[].pattern` regex out of
/// `schemas/asset.schema.json`. Returning the raw string (not a
/// compiled Regex) so the assertion failure can name the live
/// pattern verbatim in its diagnostic.
fn live_consumer_pattern() -> String {
    let path = repo_root().join("schemas/asset.schema.json");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let schema: Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("parse {} as JSON: {e}", path.display()));

    schema
        .get("properties")
        .and_then(|p| p.get("current_consumers"))
        .and_then(|c| c.get("items"))
        .and_then(|i| i.get("pattern"))
        .and_then(|p| p.as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            panic!(
                "schemas/asset.schema.json must declare \
                 properties.current_consumers.items.pattern as a string; \
                 the navigation chain failed somewhere along the way"
            )
        })
}

/// The widened pattern ADR-0007 decision 2 mandates, named verbatim
/// in stories/27.yml's justification for this test. Pinning it here
/// (rather than reconstructing it) means a future schema edit that
/// drifts from ADR-0007 produces a precise diagnostic naming the
/// expected pattern.
const ADR_0007_WIDENED_PATTERN: &str = r"^(agents/[a-z][a-z0-9-]*/[a-z][a-z0-9-]*/(contract|inputs|process)\.yml|stories/[0-9]+\.yml)$";

#[test]
fn asset_schema_accepts_story_consumer_paths_per_adr_0007_decision_2() {
    let live = live_consumer_pattern();
    let re = Regex::new(&live).unwrap_or_else(|e| {
        panic!(
            "schemas/asset.schema.json's current_consumers[].pattern must \
             compile as a regex; got pattern={live:?}, error={e}"
        )
    });

    // Half 1: paths the widened ADR-0007 pattern MUST accept.
    let must_match: &[&str] = &[
        "stories/27.yml",
        "stories/1.yml",
        "stories/9999.yml",
        // Sanity: the original agent-triplet shape must keep working
        // — widening the alternation must not narrow the original leg.
        "agents/test/test-builder/inputs.yml",
        "agents/build/build-rust/process.yml",
    ];
    for path in must_match {
        assert!(
            re.is_match(path),
            "asset.schema.json's current_consumers[].pattern must accept \
             {path:?} per ADR-0007 decision 2 (widened to also accept \
             `^stories/[0-9]+\\.yml$`); live pattern was {live:?}, \
             ADR-0007 mandates {ADR_0007_WIDENED_PATTERN:?}"
        );
    }

    // Half 2: malformed pseudo-story paths the pattern MUST still
    // reject. Widening the alternation must not let an integer-less
    // or wrong-extension path slip through.
    let must_reject: &[&str] = &[
        "stories/abc.yml",
        "stories/27.yaml",
        "stories/27.yml.bak",
        "stories/.yml",
        "stories/27/extra.yml",
    ];
    for path in must_reject {
        assert!(
            !re.is_match(path),
            "asset.schema.json's current_consumers[].pattern must REJECT \
             {path:?} (only `^stories/[0-9]+\\.yml$` is valid); live \
             pattern was {live:?}"
        );
    }
}
