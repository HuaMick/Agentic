# Agentic

A Rust-based agent orchestration system built around story-driven development.

**Status:** Phase 2 — vertical slice underway. Cargo workspace active (rustc 1.95.0 via rustup in WSL); six crates compile and ~30 tests are green. Three stories shipped (awaiting UAT).

## What this is

A rebuild of [AgenticEngineering](https://github.com/HuaMick/AgenticEngineering) in Rust, starting from first principles. The legacy Python system lives under `legacy/AgenticEngineering/` as a git submodule for reference.

## Core philosophy

1. **Story-driven from the ground up.** Every unit of work is a story with executable acceptance criteria. A story is not `healthy` until a Pass verdict is recorded with evidence. The gate is enforced in code, not policy.
2. **Red-green is a contract, not a convention.** Test-builder owns test authoring; build-rust owns `src/`. Evidence of the red state is a committable atomic. See ADR-0005.
3. **Slow, stress-tested growth.** The legacy system crashed because it bloated faster than we could verify it. We add code only when a failing test demands it.
4. **Claude Code as the default builder.** Until the system can build itself, Claude Code drives the work. Hand-written `.claude/agents/*.md` pointers delegate to authoritative YAML under `agents/`. Subscription auth via the local `claude` binary — no API billing. See ADR-0003, ADR-0004.
5. **Trait-first, pluggable everywhere it matters.** Runtime, sandbox, store — each is a trait with one impl at a time. See ADR-0002.

## Repository layout

```
crates/            Rust workspace (6 active crates + _deferred/ placeholders)
agents/            Authored YAML agent definitions (the product)
agents/assets/     Reusable agent assets (6 active; schema in schemas/asset.schema.json)
.claude/agents/    Hand-written pointer .md files that delegate to agents/
patterns/          Reusable design guidance referenced by stories
schemas/           JSON Schemas — authoritative shape of authored artifacts
stories/           User stories — the unit of work
epics/             Epic folders (groups of stories with shared context)
evidence/          Test-builder red-state artefacts only (verdicts live in agentic-store)
projects/          User-space: projects this harness is being used on
docs/              Architecture notes, guides, Architecture Decision Records
legacy/            Old AgenticEngineering codebase (git submodule, reference only)
scripts/           Human-facing convenience scripts (agentic-search.sh)
```

## Active crates

- `agentic-store` — `Store` trait + `MemStore` + `SurrealStore` (backed by `surrealkv` embedded LSM; chosen over full `surrealdb` for compile-memory budget, see workspace `Cargo.toml`).
- `agentic-story` — YAML loader with schema validation and DAG check on `depends_on`.
- `agentic-uat` — signed verdict runner (unstarted, story 1).
- `agentic-ci-record` — test-builder evidence recorder (red-state scaffold, story 2).
- `agentic-dashboard` — stories health view (unstarted, story 3).
- `agentic-cli` — `agentic` binary entrypoint.

## Active agents

- `planner/story-writer`
- `build/build-rust`
- `teacher/guidance-writer`
- `test/test-builder` (v0.2.0, post-panic-guard fix per ADR-0005)

## Current state

**Shipped (under_construction, awaiting UAT):** stories 4, 5, 6.
**Red-state scaffolds committed, impl pending:** story 2.
**Unstarted:** stories 1, 3, 7.

See **`CLAUDE.md`** for driving instructions (including the WSL push quirk) and the current story roster. Full list in `stories/README.md`.

## Quick reference

- **Authoring a story:** `docs/guides/story-authoring.md`.
- **Authoring a pattern:** `docs/guides/pattern-authoring.md`.
- **Invoking an agent:** `docs/guides/invoking-agents.md`.
- **Why these choices:** `docs/decisions/` (ADR-0001 through ADR-0005).
