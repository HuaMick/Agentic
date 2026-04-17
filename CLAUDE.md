# Agentic — Claude Code Instructions

You are working on a Rust rebuild of the AgenticEngineering system. The legacy Python codebase is available under `modules/legacy/AgenticEngineering/` as a git submodule — **reference only, do not edit**.

## Current phase

**Phase 1: design on paper.** The repository contains folder structure and README.md design docs. There is no buildable Rust code yet. Do not add `Cargo.toml` files or source code unless explicitly instructed.

## How to drive this system

Until the `agentic` binary exists, Claude Code is the primary interface. You drive work through:

1. **This `CLAUDE.md`** — project-wide instructions and current phase.
2. **Per-crate `README.md`** files under `crates/` — the source of truth for each crate's purpose, boundaries, and design decisions.
3. **Subagent pointer files** under `.claude/agents/` — hand-written `.md` shims that delegate to the authoritative YAML under `agents/`.
4. **Stories** under `stories/` — executable acceptance criteria. The unit of work.

## Core principles (non-negotiable)

- **Prove-it gate.** A story cannot be marked `proven` without a Pass verdict from `agentic-verify` with a commit hash and trace reference. Even in Phase 1, we design for this.
- **Slow growth.** Don't add a crate, field, or flag without a failing stress test or story that demands it. The legacy system died of feature accretion; we will not repeat that.
- **Subscription auth.** When runtime code is written, it will use the local `claude` binary (subscription auth) via subprocess. Never use raw Anthropic API clients that force per-token billing.
- **Document DB, schemaless-first.** Persistence goes through `agentic-store`, which wraps SurrealDB embedded. Schemaless by default; schema is added per-table only when justified.
- **No tmux.** Runtime spawns `claude` subprocesses directly. Future streaming UI uses PTY libraries, not tmux.

## What to do if something seems broken

If the system (or this repo) seems to be in a bad state, the fallback chain is:

1. Read the relevant `README.md` under `crates/<name>/` to understand intent.
2. Read this `CLAUDE.md` and the top-level `README.md`.
3. Spawn a subagent (defined under `agents/` — once authored) via the Task tool to handle the concern.
4. As a last resort, use Read/Edit/Bash directly against the files — the system is authored content + docs; nothing requires code to be useful in Phase 1.

## Terminology

- **Story** — a unit of work defined by a natural-language outcome plus executable acceptance criteria. Lives under `stories/`.
- **Epic** — a named group of stories with shared context. Lives under `epics/`.
- **Phase** — an execution strategy for a chunk of story work. Attaches to a story or epic.
- **Verdict** — the output of `agentic-verify` for a story: Pass or Fail, with evidence.
- **Evidence** — append-only record of a verify run (commit, verdict, run ID, timestamp, trace reference).
- **Agent** — a YAML-defined role (planner, builder, tester, etc.). The YAML under `agents/` is authoritative; `.claude/agents/*.md` files are short hand-written pointers that delegate into it.

## Reference: the legacy system

The submodule at `modules/legacy/AgenticEngineering/` is the Python predecessor. Read it to understand what patterns worked and what bloated. Do not port code directly — this is a ground-up redesign, not a migration.
