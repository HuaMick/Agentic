# Authoring a pattern

A pattern is reusable design or operational guidance referenced by stories. Stories point at patterns instead of redefining them, which centralizes design decisions and makes drift auditable.

## When to create a pattern

Create a pattern when the same concept appears (or is about to appear) in 2+ stories' `guidance` sections. Before that point, inline the concept in the story's guidance. Extraction is driven by observed repetition, not speculation.

Do **not** create a pattern when:

- The concept is obvious from repo context (e.g., "we use Rust").
- It belongs in an Architecture Decision Record instead (cross-cutting architectural choices live in `docs/decisions/`).
- It's an implementation detail of one specific component (belongs in that crate's README or source comments).

## The template

See `docs/guides/pattern-template.yml`. Schema: `schemas/pattern.schema.json`.

## Fields

### `id` — kebab-case slug, required

Must match the filename. `patterns/lazy-loading.yml` has `id: lazy-loading`. Valid characters: `[a-z0-9-]`. No underscores, no caps.

Stable — renaming a pattern requires updating every story that references it.

### `title` — string, required

Short human-readable title. Example: "Lazy loading", "Append-only event log", "Fail-closed on dirty git tree".

### `summary` — markdown string, required

One or two sentences. **What the pattern IS.** Not when to use it, not how to do it — just what it means as a concept.

**Good:** "Load resources on first access rather than eagerly at startup. The first call pays the cost; subsequent calls return the cached result."

**Bad — bleeds into when:** "Lazy loading is good when you have many resources that might not all be used." (Move "might not all be used" to `when_to_use`.)

### `when_to_use` — markdown string, required

Decidable conditions. A reader should be able to answer "does this apply to my situation?" after reading.

**Good:**

- "Initial cost of loading the resource is significant (>10ms or >1MB)."
- "A substantial fraction of runs will not access the resource."
- "There's no security requirement to verify availability at startup."

**Bad:** "Any time you want better performance." (Not decidable.)

### `how_we_do_it` — markdown string, required

**Specific to this codebase.** Cite crates, paths, types, function names where useful. Generic literature belongs in reference books, not here.

**Good:** "We use `once_cell::sync::Lazy<T>` for single-threaded init. For async init, we use `tokio::sync::OnceCell<T>`. The canonical example is in `crates/agentic-agents/src/registry.rs` where `AGENT_REGISTRY: Lazy<Registry>` is initialized on first `Registry::global()` call."

**Bad:** "Typically implemented with a singleton and a flag."

### `trade_offs` — markdown string, required

What you give up for what you gain. Cases where this pattern is the **wrong** choice.

**Good:** "First access is slow — unacceptable for hot paths. Harder to reason about initialization order. Not thread-safe without explicit sync primitives. Do not use for resources that must fail-fast at startup (credentials, required connections)."

### `related_patterns` — array of pattern IDs, optional (default `[]`)

IDs of related or opposing patterns. Bidirectional by convention: if pattern A references B, B should reference A. Not enforced automatically but audited by the story-writer during consistency passes.

## Anti-patterns in pattern authoring

- **Overgeneralization.** A pattern that could apply anywhere applies nowhere. Keep it specific.
- **Documenting literature.** This codebase's conventions, not textbook definitions. If the reader could read a Wikipedia page instead, you're writing the wrong thing.
- **Premature extraction.** Patterns created from 0 or 1 stories drift and go stale. Wait for the second occurrence.
- **Patterns where ADRs belong.** Architectural decisions ("we use SurrealDB") live in `docs/decisions/`. Patterns are reusable design/operational techniques, not one-time choices.
- **Patterns as copy-paste snippets.** Patterns describe technique, not code. If the reader needs the code, they click through to the cited path.

## Updating a pattern

A pattern's content is part of the proof hash of every story that references it. Editing a pattern therefore invalidates proof across all referencing stories. This is correct behavior — if the design guidance changes, the stories' proofs no longer apply to the current state.

When editing a pattern:

1. Run `scripts/agentic-search.sh` for the pattern ID across stories to identify affected stories.
2. Edit the pattern.
3. Expect audit to auto-revert every referencing story to `under_construction`.
4. Re-verification will rebuild proof against the new pattern content.
