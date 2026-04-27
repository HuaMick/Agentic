//! Story 26 acceptance test: REWARD-HACKING GUARDRAIL at the type level.
//!
//! Justification (from stories/26.yml): the test reads
//! `crates/agentic-test-support/src/lib.rs` via `include_str!` and
//! asserts no `pub use`, `pub fn`, or `pub struct` declaration whose
//! name matches `^(assert|expect|verify|check)_` exists. Rationale is
//! the REWARD-HACKING GUARDRAIL paragraph in
//! `assets/principles/deep-modules.yml`'s
//! `application_to_test_scaffolding` section: a shared assertion helper
//! is a single point a future agent can route around to make N tests
//! pass with one change, converting the per-test red-state contract
//! into a green-by-default convention. Banning the export shape at the
//! source-text level — not just at code-review time — is the cheapest
//! defence; a future PR that adds `pub fn assert_kit_*` fails this
//! test before any reviewer reads it.
//!
//! Authoring choice (test-builder, story 26 scaffold session):
//!   The literal regex scan above is currently green at scaffold time
//!   because lib.rs has no banned helpers. Per process.yml's green-
//!   scaffold rule, a scaffold that passes on a fresh checkout is a
//!   defect — the scaffold must observe a natural red state. The test
//!   therefore pairs the eternal guardrail (banned-name absence) with
//!   the kit's substantive-content contract drawn from the story's
//!   guidance section ("Five names; ... substantial hidden machinery"):
//!   each of the five public names must have an `impl <Name>` block
//!   with at least one declared method. The unit-struct shell at
//!   commit 4d4ba74 declares the five names but has zero `impl`
//!   blocks — so the test fails red on the second clause until
//!   build-rust populates the primitives. Both clauses serve the same
//!   contract: the kit ships substantive setup/fixture material AND
//!   ships no assertion helpers.
//!
//! Red today is runtime-red on the substantive-content clause.

use regex::Regex;

const LIB_SOURCE: &str = include_str!("../src/lib.rs");

#[test]
fn no_assertion_helpers_exported_in_public_surface() {
    // Clause 1 (eternal): no banned export shape may appear in the
    // public surface. Matches `pub use|fn|struct` followed by any
    // whitespace, optionally a path qualifier, then a name beginning
    // with one of the four banned prefixes.
    let banned = Regex::new(
        r"(?m)^\s*pub\s+(use|fn|struct)\s+(?:[A-Za-z0-9_:]+::)?(assert|expect|verify|check)_",
    )
    .expect("compile banned-prefix regex");

    let banned_hits: Vec<&str> = banned
        .find_iter(LIB_SOURCE)
        .map(|m| m.as_str().trim())
        .collect();
    assert!(
        banned_hits.is_empty(),
        "agentic-test-support/src/lib.rs MUST NOT export any name matching \
         `^(assert|expect|verify|check)_` — the REWARD-HACKING GUARDRAIL \
         in assets/principles/deep-modules.yml \
         (application_to_test_scaffolding) bans assertion helpers \
         unconditionally. Offending lines: {banned_hits:?}"
    );

    // Clause 2 (substantive content): each of the five public names
    // declared in the story-26 guidance section must have at least one
    // `impl <Name>` block with at least one method body. A unit-struct
    // shell with zero methods is not a deep module — it has no hidden
    // machinery. Failing this clause until build-rust populates the
    // primitives is the natural red-state proof.
    let expected_names = [
        "FixtureCorpus",
        "StoryFixture",
        "FixtureRepo",
        "RecordingExecutor",
        "RecordedCall",
    ];

    let mut missing: Vec<&str> = Vec::new();
    for name in &expected_names {
        // Match `impl <Name> {` with at least one `fn` or `pub fn` line
        // before the closing brace. We do not parse Rust — a
        // line-counted scan within the impl block is sufficient for a
        // surface guard.
        let impl_block = Regex::new(&format!(
            r"(?ms)^impl\s+{name}\s*\{{(?P<body>.*?)^\}}",
            name = regex::escape(name)
        ))
        .expect("compile impl-block regex");

        let has_method_in_impl = impl_block.captures_iter(LIB_SOURCE).any(|cap| {
            let body = cap.name("body").map(|m| m.as_str()).unwrap_or("");
            body.lines().any(|line| {
                let l = line.trim_start();
                l.starts_with("fn ") || l.starts_with("pub fn ")
            })
        });

        if !has_method_in_impl {
            missing.push(*name);
        }
    }

    assert!(
        missing.is_empty(),
        "agentic-test-support/src/lib.rs MUST declare a non-empty `impl` \
         block for each of the five canonical primitives \
         (FixtureCorpus, StoryFixture, FixtureRepo, RecordingExecutor, \
         RecordedCall) per stories/26.yml guidance \
         (\"Five names; ... substantial hidden machinery\"). \
         Names missing a non-empty impl block: {missing:?}"
    );
}
