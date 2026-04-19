# stories/

User stories. The **primary work artifact** of the system.

One file per story: `<id>.yml` where `<id>` is a positive integer matching the story's `id` field (e.g., `stories/1.yml` for story 1).

## What a story is

A unit of work defined by:

1. **Outcome** — plain-English value statement. One sentence, no conjunctions.
2. **Acceptance** — one or more executable tests (each with its own justification) plus a prose UAT walkthrough.
3. **Guidance** — non-obvious rebuild-from-scratch context.
4. **Patterns** — IDs of reusable design patterns this story applies (see `patterns/`).
5. **Evidence** — test results are persisted in `agentic-store` (see `evidence/README.md`); only red-state artefacts live on disk under `evidence/runs/<id>/`.

## Lifecycle

```
proposed → under_construction → healthy
                 ↑
                 └── (edit invalidates proof, auto-revert)
```

- `proposed` — default on new stories. Written by the `story-writer` agent or by humans.
- `under_construction` — written by the implementing agent (`build-rust`) when it picks up a `proposed` story and begins work, or auto-reverted by `story-writer` when an edit invalidates a prior Pass verdict.
- `healthy` — written only by `agentic uat` on a Pass verdict.
- `unhealthy` — computed by the dashboard (`agentic stories health`) from signals in `agentic-store`; never written to disk.

A story cannot transition to `healthy` without a Pass verdict from `agentic uat` including:

- A full git commit hash (clean working tree required).
- A trace reference (persisted `test_runs` + `uat_signings` rows).
- A run ID (UUID v4).

## Test file binding

Each test file referenced in `acceptance.tests[].file` is bound **1-to-1** to exactly one story. Tests live under:

- `crates/<crate>/tests/*.rs` — Rust integration tests (preferred).
- `scripts/verify/*.sh` — shell-based verifiers when Rust doesn't fit.

Orphan test files (unreferenced by any story) are flagged by `agentic story audit`.

## Schema

See `schemas/story.schema.json`. Authoring guide: `docs/guides/story-authoring.md`. Template: `docs/guides/story-template.yml`.

## Current corpus

| id | title | status |
|----|-------|--------|
| 1 | `agentic uat` signs a verdict promoting a story to healthy (library + CLI) | under_construction |
| 2 | `agentic-ci-record` records test-builder test results to `test_runs` | healthy |
| 3 | `agentic stories health` dashboard (library + CLI) | under_construction |
| 4 | `Store` trait + `MemStore` impl | healthy |
| 5 | `SurrealStore` backed by `surrealkv` | healthy |
| 6 | `agentic-story` YAML loader + schema + DAG check | healthy |
| 7 | test-builder meta-story — red-state evidence is a committable atomic | proposed |

Story 8 (CLI wiring) was folded into stories 1 and 3 on 2026-04-19 after an
audit found the split was along library/binary crate boundaries rather
than along user journeys — and that story 8's outcome explicitly joined
two distinct observables (signing a verdict AND reading the dashboard).
See each story's `acceptance.tests` for the library-level vs. binary-level
test split.

Check each `<id>.yml` for the authoritative status, outcome, and acceptance.
