# patterns/

Reusable design and operational guidance referenced by stories. A pattern centralizes a concept that would otherwise be repeated across multiple stories' `guidance` sections.

## Layout

One file per pattern: `<slug>.yml` where slug is kebab-case (`lazy-loading.yml`, `append-only-log.yml`, `fail-closed-on-dirty-tree.yml`).

## Schema

See `schemas/pattern.schema.json`. Authoring guide: `docs/guides/pattern-authoring.md`. Template: `docs/guides/pattern-template.yml`.

## Lifecycle

Patterns do not have a "proven" lifecycle like stories. They are living design guidance; active or deprecated (archive location TBD when we have our first deprecation).

Editing a pattern invalidates proof for every story that references it (the pattern's content is part of each referring story's proof hash). This is deliberate — if design guidance changes, the stories' proofs no longer apply.

## Curation

The `story-writer` agent (under `agents/planner/story-writer/`) also curates patterns. It:

- References existing patterns in new or edited stories instead of restating them.
- Extracts repeated concepts from stories into new patterns.
- Audits for pattern drift and orphan patterns (patterns not referenced by any story).
- Proposes deprecation when a pattern is superseded.

A dedicated `pattern-writer` agent may be added later if pattern volume warrants it.

## When to extract a pattern

Extract when the same concept appears (or is about to appear) in **2+ stories' guidance sections**. Before that point, inline in the story's guidance. Extraction is driven by observed repetition, not speculation.

## Current state

Empty. Story 1 ships with `patterns: []` because no repetition has emerged yet. First extraction will most likely come from a second or third story that shares substantial architectural concerns with story 1 — likely candidates: "fail-closed-on-dirty-tree" (when a second story needs the same semantic), "append-only-log" (when a second artifact beyond evidence needs the same writing discipline).
