# Agentic

A Rust-based agent orchestration system built around story-driven development.

**Status:** Phase 1.5 — design on paper with the first story authored. No buildable code yet.

## What this is

A rebuild of [AgenticEngineering](https://github.com/HuaMick/AgenticEngineering) in Rust, starting from first principles. The legacy Python system lives under `modules/legacy/AgenticEngineering/` as a git submodule for reference.

## Core philosophy

1. **Story-driven from the ground up.** Every unit of work is a story with executable acceptance criteria. A story is not "tested" until a Pass verdict is recorded with evidence. The gate is enforced in code, not policy.
2. **Slow, stress-tested growth.** The legacy system crashed because it bloated faster than we could verify it. Each crate must pass a stress harness before it graduates. We add code only when a failing test demands it.
3. **Claude Code as the default builder.** Until the system can build itself, Claude Code drives the work. Hand-written `.claude/agents/*.md` pointers delegate to authoritative YAML under `agents/`, so Claude Code spawns our agents natively without duplicating role definitions. Subscription auth via the local `claude` binary — no API billing.
4. **Trait-first, pluggable everywhere it matters.** Runtime, sandbox, store — each is a trait with one impl on day one. Upgrading (to containerized sandboxes, streaming runtimes, etc.) is additive, not a rewrite.

## Repository layout

```
crates/            Rust workspace (13 day-one + 6 _deferred/ placeholders)
agents/            Authored YAML agent definitions (the product)
.claude/agents/    Hand-written pointer .md files that delegate to agents/
patterns/          Reusable design guidance referenced by stories (empty day one)
schemas/           JSON Schemas — authoritative shape of authored artifacts
stories/           User stories — the unit of work
epics/             Epic folders (groups of stories with shared context)
evidence/          Append-only verdict records (empty until verify runs)
projects/          User-space: projects this harness is being used on
docs/              Architecture notes, guides, Architecture Decision Records
modules/legacy/    Old AgenticEngineering codebase (git submodule, reference only)
tests/             Workspace-level integration tests (empty — Phase 2)
xtask/             Custom cargo tasks (empty — Phase 2)
scripts/           Human-facing convenience scripts (agentic-search.sh)
```

## Current state

**What's committed (2 commits on `main`):**

- Full folder structure with design-doc READMEs for every crate and top-level dir.
- Story schema + pattern schema; authoring guides and templates.
- `stories/1.yml` — meta-story for the prove-it gate (status: `proposed`).
- `agents/planner/story-writer/` — first active agent, a curator of stories and patterns.
- `scripts/agentic-search.sh` — bootstrap search tool (replaceable by `agentic search` later).
- ADRs capturing the key cross-cutting decisions (see `docs/decisions/`).

**What's next (Phase 2 — first vertical slice):**

- A second story (candidates in `CLAUDE.md`) so story 1's UAT has something to verify against.
- `agentic-story` + `agentic-store` + `agentic-verify` as the MVP crate trio, driven by story-demand.
- Stress harness running against the trio before anything else lands.

See **`CLAUDE.md`** for driving instructions (including the WSL push quirk) and the roster of candidate next stories.

## Quick reference

- **Authoring a story:** `docs/guides/story-authoring.md`.
- **Authoring a pattern:** `docs/guides/pattern-authoring.md`.
- **Invoking an agent:** `docs/guides/invoking-agents.md`.
- **Why these choices:** `docs/decisions/` (ADR-0001 through ADR-0004).
