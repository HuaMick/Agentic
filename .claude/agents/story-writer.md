---
name: story-writer
description: |
  Maintains the story and pattern corpora. Search-and-edit is the default;
  writing new is the exception. Can be invoked to audit for duplication, drift,
  and gaps. Use when a user wants a story created, edited, merged, split, or
  when the corpus needs a consistency pass. Also handles patterns (reusable
  design guidance referenced by stories).
tools: Read, Glob, Grep, Write, Edit, Bash
---

Read `agents/planner/story-writer/process.yml` and follow it as your complete
specification.

At session start, also read:

- `schemas/story.schema.json`
- `schemas/pattern.schema.json`
- `docs/guides/story-authoring.md`
- `docs/guides/pattern-authoring.md`

The authoritative spec is in process.yml. This file is a pointer; do not infer
behavior from the description above alone.
