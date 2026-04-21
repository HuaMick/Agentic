# Agentic

A Rust-based agent orchestration system built around story-driven development.

**Status:** Phase 2 — vertical slice operational end-to-end. Cargo workspace active (rustc 1.95.0 via rustup in WSL); seven crates compile green. Twelve stories `healthy` (1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 15); zero under_construction, zero proposed, zero unhealthy. The `dag-primary-lens` epic (stories 10-13) is complete. The `agentic` binary is installable via `./install.sh` (or `cargo install --path crates/agentic-cli` directly; `./install.sh --docker` builds a container image instead) and exposes three top-level subcommands: `agentic uat <id> --verdict <pass|fail>`, `agentic stories health|test`, and `agentic test-build [plan|record]`.

## What this is

A rebuild of [AgenticEngineering](https://github.com/HuaMick/AgenticEngineering) in Rust, starting from first principles. The legacy Python system lives under `legacy/AgenticEngineering/` as a git submodule for reference.

## Core philosophy

1. **Story-driven from the ground up.** Every unit of work is a story with executable acceptance criteria. A story is not `healthy` until a Pass verdict is recorded with evidence. The gate is enforced in code, not policy.
2. **Red-green is a contract, not a convention.** The test-builder agent authors failing scaffolds (using its normal authoring tools); `agentic test-build record` verifies the red state and writes atomic evidence. Build-rust writes implementation source and never edits tests. See ADR-0005.
3. **Slow, stress-tested growth.** The legacy system crashed because it bloated faster than we could verify it. We add code only when a failing test demands it.
4. **Claude is a user of the system, not a component of it.** `agentic-runtime` (the orchestrator crate) spawns `claude` via subprocess to run subagents; product libraries (`agentic-uat`, `agentic-test-builder`, `agentic-store`, etc.) are strictly AI-free and treat claude as an external user — same category as a human developer — who exercises the CLI. Subscription auth via the local `claude` binary, no API billing. See ADR-0003, ADR-0004.
5. **Trait-first, pluggable everywhere it matters.** Runtime, sandbox, store — each is a trait with one impl at a time. See ADR-0002.
6. **Defects amend the owning story, not a new story.** When a defect is found in a healthy story's impl, add a new `acceptance.tests[]` entry to THAT story, auto-revert its status, scaffold the new test red, fix the impl, re-UAT. Stories stay single-owner over their domain; the corpus doesn't fragment.

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
- `agentic-uat` — signed verdict runner with `UatExecutor` trait, dirty-tree refusal, and transitive-ancestor-health gate on `--verdict pass` (stories 1 + 11 shipped).
- `agentic-ci-record` — per-story `test_runs` upserter plus subtree-scoped `CiRunner` with selector grammar and pluggable `TestExecutor` trait (stories 2 + 12 shipped).
- `agentic-dashboard` — DAG-aware four-status story-health view with frontier-default filtering, `--expand`/`--all` flags, selector grammar (`+id`, `id+`, `+id+`), blast-radius columns, subtree drilldown, related-files staleness, and ancestor-inherited unhealthy classification (stories 3 + 9 + 10 + 13 shipped).
- `agentic-cli` — `agentic` binary entrypoint exposing `uat`, `stories health|test`, and `test-build plan|record` (stories 1, 3, 10, 11, 12, 15 shipped).
- `agentic-test-builder` — plan-and-record CLI library backing `agentic test-build`: emits a structured `PlanEntry` per acceptance test, probes user-authored scaffolds via `cargo check` + `cargo test`, and writes atomic red-state JSONL evidence. Strictly AI-free; no claude subprocess, no LLM dependency. User (human or claude-as-agent) authors the scaffolds with their own tools (story 15 shipped, superseding the retired story 7 panic-stub authoring and retired story 14 claude-in-library approach).

## Active agents

- `planner/story-writer` — story and pattern curator.
- `build/build-rust` — implements Rust source to drive scaffolded tests green.
- `teacher/guidance-writer` — curator of agent specs and the shared `assets/` layer.
- `test/test-builder` — authors failing test scaffolds and records red-state evidence per ADR-0005.
- `test/test-uat` — executes a story's UAT walkthrough and invokes `agentic uat` with the signed verdict.

## Current state

**Healthy:** stories 1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 15. Zero `under_construction`, zero `proposed`, zero `unhealthy`.

The **`dag-primary-lens`** epic (`epics/live/dag-primary-lens/`) is
**complete**. Stories 10, 11, 12, and 13 shifted the system's mental
model from "flat list of stories" to "DAG with frontier-of-work,
blast-radius drilldown, UAT ancestor-gating, and subtree-scoped CI."

Stories 7, 8, and 14 were retired during the session that shipped
story 15:
- **Story 7** (deterministic panic-stub scaffolder) and **story 14**
  (library wraps `claude` subprocess to author scaffolds) were folded
  into **story 15** — a plan-and-record CLI under the claude-as-user
  model where the library never spawns an LLM. Story 14 was the
  claude-as-component anti-pattern that killed the legacy Python
  system; catching it before it compounded is one of this session's
  main wins.
- **Story 8** (CLI wiring) was consolidated into stories 1 and 3 on
  2026-04-19 after an audit found the split was along library/binary
  crate boundaries rather than user journeys.

Retired story IDs are not reused. See `stories/README.md` for the
full retirement rationale.

See **`CLAUDE.md`** for driving instructions (including the WSL push quirk) and the current story roster. Full list in `stories/README.md`.

## Quick reference

- **Authoring a story:** `docs/guides/story-authoring.md`.
- **Authoring a pattern:** `docs/guides/pattern-authoring.md`.
- **Invoking an agent:** `docs/guides/invoking-agents.md`.
- **Why these choices:** `docs/decisions/` (ADR-0001 through ADR-0005).
