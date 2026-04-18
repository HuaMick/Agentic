# stories/

User stories. The **primary work artifact** of the system.

One file per story: `<id>.yml` where `<id>` is a positive integer matching the story's `id` field (e.g., `stories/1.yml` for story 1).

## What a story is

A unit of work defined by:

1. **Outcome** — plain-English value statement. One sentence, no conjunctions.
2. **Acceptance** — one or more executable tests (each with its own justification) plus a prose UAT walkthrough.
3. **Guidance** — non-obvious rebuild-from-scratch context.
4. **Patterns** — IDs of reusable design patterns this story applies (see `patterns/`).
5. **Evidence** — append-only log of verify runs, stored externally under `evidence/runs/<id>/` (see `evidence/README.md`).

## Lifecycle

```
proposed → under_construction → healthy
                 ↑
                 └── (edit invalidates proof, auto-revert)
```

- `proposed` — default on new stories. Written by the story-writer agent or by humans.
- `under_construction` — written by the implementing agent (`build-rust`) when it picks up a `proposed` story, or auto-reverted by the story-writer when an edit invalidates a prior Pass verdict.
- `healthy` — written only by `agentic uat` on a Pass verdict.
- `unhealthy` — computed by the dashboard from evidence signals; never written to disk.

A story cannot transition to `healthy` without a Pass verdict from `agentic uat` including:

- A full git commit hash (clean working tree required).
- A trace reference (the actual evidence file).
- A run ID (UUID v4).

## Test file binding

Each test file referenced in `acceptance.tests[].file` is bound **1-to-1** to exactly one story. Tests live under:

- `crates/<crate>/tests/*.rs` — Rust integration tests (preferred).
- `scripts/verify/*.sh` — shell-based verifiers when Rust doesn't fit.

Orphan test files (unreferenced by any story) are flagged by `agentic story audit`.

## Schema

See `schemas/story.schema.json`. Authoring guide: `docs/guides/story-authoring.md`. Template: `docs/guides/story-template.yml`.

## Current corpus

- **`1.yml`** — "Verify a story end-to-end." The meta-story driving `agentic-verify` implementation. Status: `proposed`. No tests yet — they're authored during Phase 2 alongside the verifier.

Next expected story: something that provides story 1's UAT with a second story to walk through. Candidates in `CLAUDE.md`.
