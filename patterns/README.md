# patterns/

Reusable design and operational guidance referenced by stories. A pattern centralizes a concept that would otherwise be repeated across multiple stories' `guidance` sections.

## Layout

One file per pattern: `<slug>.yml` where slug is kebab-case (`lazy-loading.yml`, `append-only-log.yml`, `fail-closed-on-dirty-tree.yml`).

## Schema

See `schemas/pattern.schema.json`. Authoring guide: `docs/guides/pattern-authoring.md`. Template: `docs/guides/pattern-template.yml`.

## Lifecycle

Patterns do not have a "proven" lifecycle like stories. They are living design guidance; they're either active (present here) or deprecated (moved to an archive location, TBD when we have our first deprecation).

Editing a pattern invalidates proof for every story that references it. This is deliberate — if design guidance changes, the stories' proofs no longer apply.

## Curation

The `story-writer` agent (under `agents/planner/story-writer/`) also curates patterns. It:

- References existing patterns in new or edited stories instead of restating them.
- Extracts repeated concepts from stories into new patterns.
- Audits for pattern drift and orphan patterns (patterns not referenced by any story).
- Proposes deprecation when a pattern is superseded.

A dedicated `pattern-writer` agent may be added later if pattern volume warrants it.

## Phase 1 status

Empty. First patterns will be extracted as the story corpus grows and repetition appears.
