//! Story 26 acceptance test: YAML emitted by `StoryFixture` (via
//! `FixtureCorpus::write_story()`) round-trips through the production
//! `agentic_story::Story::load_dir` loader without schema violations.
//!
//! Justification (from stories/26.yml): pins fixture-vs-production-dialect
//! parity. A second-best implementation would silently emit a slightly-
//! loose YAML shape (missing `status`, `patterns` absent rather than `[]`,
//! conjunctions in `outcome`) that downstream tests accept but
//! `agentic story lint` would reject; consumers would then drift from
//! the schema the production loader enforces. Without this test, the kit
//! becomes a parallel YAML dialect, and the deep-modules
//! `application_to_test_scaffolding` rationale ("the shared kit ships
//! setup/fixture material only") loses its anchor — fixtures must
//! produce schema-clean stories or they are not fixtures, they are
//! decoys.
//!
//! Red today is compile-red: `FixtureCorpus::new()`, `write_story()`,
//! and `stories_dir()` are not yet declared on the unit-struct shells.

use agentic_story::Story;
use agentic_test_support::FixtureCorpus;

#[test]
fn fixture_corpus_yaml_passes_story_loader_validation() {
    let corpus = FixtureCorpus::new();
    let stories_dir = corpus.stories_dir();

    // Author a small DAG: 91111 (root) <- 91112 (depends on 91111).
    // Both must produce schema-clean YAML loadable by agentic_story.
    corpus.write_story(91111, &[]);
    corpus.write_story(91112, &[91111]);

    // Round-trip: load via the PRODUCTION loader (the same one
    // `agentic story lint` uses). Any schema looseness in the kit
    // surfaces here as a typed StoryError.
    let stories = Story::load_dir(stories_dir.as_path()).unwrap_or_else(|e| {
        panic!(
            "FixtureCorpus YAML must round-trip through agentic_story::Story::load_dir; \
             loader rejected the corpus with: {e}"
        )
    });

    // The loader returned both stories, and their ids match what
    // write_story() requested. Order is unstable (filesystem walk),
    // so collect-and-sort.
    let mut ids: Vec<u32> = stories.iter().map(|s| s.id).collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        vec![91111, 91112],
        "Story::load_dir must return exactly the ids write_story() authored; \
         got {ids:?}"
    );

    // The depends_on edge survives the round-trip — fixture YAML must
    // emit `depends_on:` in a shape the loader parses, not stringify it.
    let dependent = stories
        .iter()
        .find(|s| s.id == 91112)
        .expect("dependent story present");
    assert_eq!(
        dependent.depends_on,
        vec![91111],
        "depends_on edge must round-trip through the loader; got {:?}",
        dependent.depends_on
    );

    // status must parse as one of the five lifecycle values — a fixture
    // that omits or stringifies `status:` would explode the schema.
    // We don't pin which value (the kit picks a reasonable default like
    // `under_construction`); we pin only that the loader accepted it.
    // Reaching this line means `status` parsed cleanly above.
}
