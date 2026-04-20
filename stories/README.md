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
| 1 | `agentic uat` signs a verdict promoting a story to healthy (library + CLI) | healthy |
| 2 | `agentic-ci-record` records test-builder test results to `test_runs` | healthy |
| 3 | `agentic stories health` dashboard (library + CLI) | healthy |
| 4 | `Store` trait + `MemStore` impl | healthy |
| 5 | `SurrealStore` backed by `surrealkv` | healthy |
| 6 | `agentic-story` YAML loader + schema + DAG check | healthy |
| 7 | test-builder meta-story — red-state evidence is a committable atomic | healthy |
| 9 | Scope dashboard staleness to each story's declared `related_files` | healthy |
| 10 | Render the story corpus as a DAG with frontier-of-work view and blast-radius drilldown | under_construction |
| 11 | UAT refuses to sign Pass for a story standing on an unproven ancestor | proposed |
| 12 | Scope `agentic stories test <selector>` runs to a DAG subtree | proposed |
| 13 | Classify a story as unhealthy when any transitive ancestor is not healthy | proposed |
| 14 | test-builder authors real acceptance tests via the local claude subprocess | proposed |

Story 8 (CLI wiring) was folded into stories 1 and 3 on 2026-04-19 after an
audit found the split was along library/binary crate boundaries rather
than along user journeys — and that story 8's outcome explicitly joined
two distinct observables (signing a verdict AND reading the dashboard).
See each story's `acceptance.tests` for the library-level vs. binary-level
test split.

Story 14 is a hard prerequisite for the `dag-primary-lens` epic (stories
10-13) picked up during story 10's implementation attempt: the `agentic
test-build` binary shipped by story 7 writes panic-stub scaffolds that
build-rust cannot drive to green, so every proposed story in the epic
needs story 14's real-acceptance-test scaffolding to cross the red-green
line. See `stories/14.yml` for the full scope and the splitting analysis
against story 7.

Stories 10, 11, 12, and 13 form the `dag-primary-lens` epic
(`epics/live/dag-primary-lens/epic.yml`). They share a unifying
objective — shift the system's mental model from "flat list" to
"DAG with frontier-of-work, blast-radius, and ancestor-aware
classification" — but have distinct observables (dashboard view
shape, UAT refusal, CI test selection, classifier rule) and
distinct commands (`agentic stories health`, `agentic uat`,
`agentic stories test`, and the classifier change surfaced
through the dashboard), so the epic captures the direction while
four stories capture the executable contracts. Stories 10 and 13
both touch the dashboard's read path but at different layers
(story 10 = view shape; story 13 = classifier rule); stories 11
and 13 both enforce an "ancestors must be healthy" rule but at
opposite sides of the store (story 11 = write / UAT gate;
story 13 = read / classifier). Each story's guidance documents
the split rationale against the splitting-rule.

Check each `<id>.yml` for the authoritative status, outcome, and acceptance.
