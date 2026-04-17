# Agentic

A Rust-based agent orchestration system built around story-driven development.

**Status:** Early design. Folder structure and design-doc READMEs only. No buildable code yet.

## What this is

A rebuild of [AgenticEngineering](https://github.com/HuaMick/AgenticEngineering) in Rust, starting from first principles. The legacy system lives under `modules/legacy/AgenticEngineering/` as a git submodule for reference.

## Core philosophy

1. **Story-driven from the ground up.** Every unit of work is a story with executable acceptance criteria. A story is not "done" until a Pass verdict is recorded with evidence. This gate is enforced in code, not policy.
2. **Slow, stress-tested growth.** The legacy system crashed because it bloated faster than we could verify it. Each crate must pass a stress harness before it graduates. We add code only when a failing test demands it.
3. **Claude Code as the default builder.** Until the system can build itself, Claude Code drives the work. Hand-written `.claude/agents/*.md` pointers delegate to the authoritative YAML under `agents/`, so Claude Code spawns our agents natively without duplicating role definitions. Uses the Pro/Max subscription via the local `claude` binary — no API billing.
4. **Trait-first, pluggable everywhere it matters.** Runtime, sandbox, store — each is a trait with one impl on day one. Upgrading (to containerized sandboxes, streaming runtimes, etc.) is additive, not a rewrite.

## Repository layout

```
crates/            Rust workspace (day-one crates + _deferred/ placeholders)
agents/            Authored YAML agent definitions (the product)
.claude/           Claude Code integration — hand-written pointer .md files to agents/
patterns/          Reusable design guidance referenced by stories
schemas/           JSON Schemas for story, epic, phase, agent manifest, events
stories/           User stories — executable acceptance criteria per story
epics/             Epic folders (groups of stories with shared context)
projects/          User-space: projects this harness is being used on (empty day one)
docs/              Architecture notes, guides, Architecture Decision Records
modules/legacy/    Old AgenticEngineering codebase (git submodule, reference only)
tests/             Workspace-level integration tests
xtask/             Custom cargo tasks (release checks, etc.)
scripts/           Human-facing convenience scripts
```

## Current phase

**Phase 1 — design on paper.** Every crate and top-level folder has a README.md describing what it will be, what boundaries it enforces, and open design questions. No `Cargo.toml`, no Rust code yet. The READMEs are the design document.

**Phase 2 (next)** — story system vertical slice. `agentic-story` + `agentic-store` + `agentic-verify` only. A single meta-story ("Users can verify a story end-to-end") drives its own implementation.

See `CLAUDE.md` for Claude Code driving instructions.
