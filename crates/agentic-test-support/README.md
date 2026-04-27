# agentic-test-support

Shared test fixture primitives for the agentic workspace. This crate provides
setup, fixture, and stub-executor material that would otherwise be reimplemented
across multiple test files.

## REWARD-HACKING GUARDRAIL — non-negotiable

Assertions stay bespoke per test on purpose. The shared kit ships setup/fixture
material only. No `assert_*`, `expect_*`, `verify_*`, or `check_*` helpers —
exported or otherwise. The reason is hard: a shared assertion helper is a single
point a future agent can route around to make N tests pass with one change, which
converts the per-test red-state contract into a green-by-default convention and
silently re-enables every legacy failure mode the prove-it gate is designed to
prevent. Each test's assertion block is the place its observable is pinned, and
that pinning must remain per-test. See test-builder's `prefer-shared-scaffold`
rule for the operational application and the matching anti-pattern that bans
importing the kit as a route to introduce assertion helpers.

## Catalogue

- **FixtureCorpus** — Manages a temporary directory with a `stories/` subdirectory
  for authoring fixture story YAML. Use this when your test needs a corpus of
  minimal stories with defined `depends_on` edges. Construct via `new()`,
  call `write_story()` to author fixture stories, and pass `stories_dir()` to
  a loader like `agentic_story::Story::load_dir`. The tempdir is automatically
  deleted when the corpus is dropped.

- **StoryFixture** — An in-memory representation of a fixture story with YAML
  authoring support. Use this to construct individual stories programmatically
  via builder-style setters (`with_title()`, `with_outcome()`, etc.) and emit
  schema-clean YAML via `to_yaml()`. Returned by `FixtureCorpus::write_story()`;
  typically used indirectly through the corpus interface.

- **FixtureRepo** — A git repository initialized with a committer email and one
  seed commit. Use this when your test needs a stable git context to capture
  commit SHAs or verify git state. Initialize via `init_with_email()`, retrieve
  the full 40-character SHA via `head_sha()`, and optionally create additional
  commits via `commit_seed()`.

- **RecordingExecutor** — A stub implementation of both `TestExecutor` and
  `UatExecutor` that records every invocation for inspection. Use this when your
  test needs to verify that an executor was called with specific arguments
  without actually running cargo or a real UAT journey. Construct via `default()`,
  call `recorded_calls()` to inspect the per-call arguments (non-destructive),
  and drive invocations through the trait methods.

- **RecordedCall** — A per-invocation record capturing the `story_id` and `files`
  arguments passed to an executor. Returned by `RecordingExecutor::recorded_calls()`;
  exposes public fields for direct inspection by test assertions.

## Why this kit exists

The deep-modules principle (reference `agents/assets/principles/deep-modules.yml`)
establishes that setup, fixture, and stub-executor material recurs across N test
files when a shared primitive does not exist — the deletion-test failure case
where the module that should exist is missing and N parallel copies are filling
its absence. This kit consolidates that material into a single deep module whose
small public interface hides substantial fixture machinery. See the
`application_to_test_scaffolding` section of the deep-modules asset for the
operational context and the anti-patterns this kit exists to retire.
