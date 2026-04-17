# stories/

User stories. The **primary work artifact** of the system.

One file per story: `<id>.yml` where ID is `story-NNNN-<slug>` (e.g., `story-0001-verify-story-end-to-end.yml`).

## What a story is

A unit of work defined by:

1. **Outcome** — what value this delivers, in plain English.
2. **Acceptance criteria** — executable Given/When/Then checks.
3. **Evidence** — append-only log of verify runs (stored externally under `evidence/runs/<story-id>/`).

## Lifecycle

```
proposed → under_construction → proven → deprecated → archived
                ↓ (fail)                       ↑
         stays under_construction         (manually deprecated after proven)
```

A story cannot transition to `proven` without a Pass verdict from `agentic-verify` including:
- A git commit hash (clean working tree required).
- A trace reference (the actual evidence file).
- A run ID (UUID).

## Schema

See `schemas/story.schema.json`.

## Template

See the story template in `docs/guides/story-template.md` (authored as part of Phase 1).

## Phase 1 status

Story template is being drafted. First concrete story will be `story-0001-verify-story-end-to-end.yml` — the meta-story that drives the verify system's own implementation.
