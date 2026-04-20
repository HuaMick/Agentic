# Agentic

A Rust-based agent orchestration system built around story-driven development.

**Status:** Phase 2 — vertical slice operational end-to-end. Cargo workspace active (rustc 1.95.0 via rustup in WSL); seven crates compile green. Seven stories `healthy` (1, 2, 3, 4, 5, 6, 9); three `under_construction` (7, 10, 14); three `proposed` (11, 12, 13). The `dag-primary-lens` epic (stories 10-13) is live under `epics/live/`. The `agentic` binary is installable via `./install.sh` (or `cargo install --path crates/agentic-cli` directly; `./install.sh --docker` builds a container image instead) and `agentic uat <id>` + `agentic stories health` both drive the four-status model against a shared SurrealStore.

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
crates/            Rust workspace (7 active crates + _deferred/ placeholders)
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

- `agentic-store` — `Store` trait + `MemStore` + `SurrealStore` (backed by `surrealkv` embedded LSM; chosen over full `surrealdb` for compile-memory budget, see workspace `Cargo.toml`). Stories 4 + 5 shipped.
- `agentic-story` — YAML loader with schema validation, DAG check on `depends_on`, and the optional `related_files` field (stories 6 + 9 shipped).
- `agentic-uat` — signed verdict runner with `UatExecutor` trait and dirty-tree refusal (story 1 shipped).
- `agentic-ci-record` — per-story `test_runs` upserter (story 2 shipped).
- `agentic-dashboard` — four-status story-health view with table + JSON + drilldown modes (stories 3 + 9 shipped).
- `agentic-cli` — `agentic` binary entrypoint: `agentic uat <id> --verdict <pass|fail>` and `agentic stories health [<id>] [--json]` both wired and shipped via stories 1 + 3.
- `agentic-test-builder` — red-state scaffold authoring + JSONL evidence writer behind the `agentic test-build` subcommand (story 7 shipped; story 14 upgrading panic-stubs to claude-authored acceptance-test bodies).

## Active agents

- `planner/story-writer` — story and pattern curator.
- `build/build-rust` — implements Rust source to drive scaffolded tests green.
- `teacher/guidance-writer` — curator of agent specs and the shared `assets/` layer.
- `test/test-builder` — authors failing test scaffolds and records red-state evidence per ADR-0005.
- `test/test-uat` — executes a story's UAT walkthrough and invokes `agentic uat` with the signed verdict.

## Current state

**Healthy:** stories 1, 2, 3, 4, 5, 6, 9.
**Under construction:** stories 7, 10, 14. Story 7 shipped `healthy` at commit `e5f4997` but its implementation was mutated by story 14's in-flight work — 5 of its 9 tests currently fail and the YAML's `status: healthy` is stale pending re-UAT. Story 10 has library implementation but panic-stub tests awaiting story 14. Story 14 has partial implementation (4/8 tests pass).
**Proposed:** stories 11, 12, 13.

The **`dag-primary-lens`** epic (`epics/live/dag-primary-lens/`) groups
stories 10, 11, 12, and 13 around a single direction: shift the mental
model from "flat list of stories" to "DAG with frontier-of-work,
blast-radius drilldown, UAT ancestor-gating, and subtree-scoped CI."
Depends on healthy stories 1, 2, 3, 6, 9 plus story 14 as a hard
prerequisite.

Story 8 (CLI wiring) was consolidated into stories 1 and 3 on 2026-04-19;
see `stories/README.md` for the split rationale.

See **`CLAUDE.md`** for driving instructions (including the WSL push quirk) and the current story roster. Full list in `stories/README.md`.

## Quick reference

- **Authoring a story:** `docs/guides/story-authoring.md`.
- **Authoring a pattern:** `docs/guides/pattern-authoring.md`.
- **Invoking an agent:** `docs/guides/invoking-agents.md`.
- **Why these choices:** `docs/decisions/` (ADR-0001 through ADR-0005).
