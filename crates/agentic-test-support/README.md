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

- **FixtureCorpus** — Populated when impl lands in the next commit.
- **StoryFixture** — Populated when impl lands in the next commit.
- **FixtureRepo** — Populated when impl lands in the next commit.
- **RecordingExecutor** — Populated when impl lands in the next commit.
- **RecordedCall** — Populated when impl lands in the next commit.

## Why this kit exists

The deep-modules principle (reference `agents/assets/principles/deep-modules.yml`)
establishes that setup, fixture, and stub-executor material recurs across N test
files when a shared primitive does not exist — the deletion-test failure case
where the module that should exist is missing and N parallel copies are filling
its absence. This kit consolidates that material into a single deep module whose
small public interface hides substantial fixture machinery. See the
`application_to_test_scaffolding` section of the deep-modules asset for the
operational context and the anti-patterns this kit exists to retire.
