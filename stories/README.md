# stories/

User stories. The **primary work artifact** of the system.

One file per story: `<id>.yml` where `<id>` is a positive integer matching the story's `id` field (e.g., `stories/1.yml` for story 1).

## What a story is

A unit of work defined by:

1. **Outcome** — plain-English value statement.
2. **Acceptance** — one or more executable tests (each with its own justification) plus a prose UAT walkthrough.
3. **Guidance** — non-obvious rebuild-from-scratch context.
4. **Patterns** — IDs of reusable design patterns this story applies (see `patterns/`).
5. **Evidence** — append-only log of verify runs, stored externally under `evidence/runs/<id>/`.

## Lifecycle

```
proposed → under_construction → tested → healthy → deprecated → archived
                ↓ (fail)                  ↑
         stays under_construction   (manually deprecated after tested/healthy)
```

- Humans (and the story-writer agent) write `proposed`, `under_construction`, `deprecated`, `archived`.
- Only `agentic-verify` writes `tested` (acceptance tests pass) and `healthy` (UAT journey passes).

A story cannot transition to `tested` without a Pass verdict from `agentic-verify` including:
- A git commit hash (clean working tree required).
- A trace reference (the actual evidence file).
- A run ID (UUID).

## Schema

See `schemas/story.schema.json`.

## Template & authoring guide

- Template: `docs/guides/story-template.yml`
- Authoring guide: `docs/guides/story-authoring.md`

## Phase 1 status

First story is `stories/1.yml` — the meta-story that drives the verify system's own implementation.
