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
proposed → under_construction → tested → healthy → deprecated → archived
                ↓ (fail)                  ↑
         stays under_construction   (manually deprecated after tested/healthy)
```

- Humans (and the story-writer agent) write `proposed`, `under_construction`, `deprecated`, `archived`.
- Only `agentic-verify` writes `tested` (acceptance tests pass) and `healthy` (UAT journey passes).

A story cannot transition to `tested` without a Pass verdict from `agentic-verify` including:

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
