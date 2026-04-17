# epics/

Epic folders. An epic is a named group of stories with shared context (branch, objective, cross-story dependencies).

## Layout

- `live/<epic-name>/` — active epics.
- `completed/<epic-name>/` — archived epics (all stories proven).

Each epic folder contains:

- `epic.yml` — name, objective, branch, story IDs, depends-on.
- (Optionally) notes, context docs, phase definitions.

## Naming

`<objective-slug>` (e.g., `agentic-rebuild`, `verify-system-v1`). No date prefix needed — git history and `epic.yml` timestamps cover that.

## Lifecycle

Epic state is derived from contained stories:

- `planning` — no stories proven yet.
- `active` — at least one story proven.
- `completed` — all stories proven.

Move to `completed/` is triggered manually or by orchestrator once all stories are proven.

## Phase 1 status

The first epic will be `live/agentic-rebuild/`, containing the meta-story and subsequent foundation stories.
