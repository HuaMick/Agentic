# Agentic — Claude Code Instructions

You are working on a Rust rebuild of the AgenticEngineering system. The legacy Python codebase lives under `legacy/AgenticEngineering/` as a git submodule — **reference only, do not edit**.

## Current state (Phase 1.5)

Design is on paper; the first story is written. **No Rust code exists yet.** What's on disk and committed:

- Folder structure with README.md design docs for every crate and top-level dir.
- Schemas: `schemas/story.schema.json`, `schemas/pattern.schema.json`.
- Authoring guides and templates: `docs/guides/story-*` and `docs/guides/pattern-*`.
- `stories/1.yml` — the meta-story (verify a story end-to-end), `status: proposed`.
- First agent authored: `agents/planner/story-writer/` + pointer at `.claude/agents/story-writer.md`.
- Bootstrap search tool: `scripts/agentic-search.sh` (with `--quiet` for agent use).

**Do not add `Cargo.toml` files or source code** unless explicitly instructed. The next phase is "Phase 2: first vertical slice" — `agentic-story` + `agentic-store` + `agentic-verify` only, driven by stories that demand each piece.

## How to drive the system

Until the `agentic` binary exists, Claude Code is the primary interface. You drive work through, in order of authority:

1. **This `CLAUDE.md`** — project-wide instructions and current phase.
2. **Per-crate `README.md`** files under `crates/` — source of truth for each crate's purpose, boundaries, and design decisions.
3. **Agent YAML** under `agents/<category>/<name>/` — authoritative specification for each agent's behaviour.
4. **Pointer files** under `.claude/agents/<name>.md` — hand-written ten-line shims that delegate to the authoritative YAML. When spawning a subagent, this is where Claude Code looks first.
5. **Stories** under `stories/` — executable acceptance criteria. The unit of work.
6. **Patterns** under `patterns/` — reusable design guidance referenced by stories (empty day one; extracted as repetition appears).

### Invoking the story-writer

To author or edit a story, spawn the `story-writer` agent. The canonical path:

- Spawn the Task tool with `subagent_type: general-purpose` (the new `story-writer` is not registered in this harness as a native subagent type yet).
- Hand it the objective plainly.
- Tell it explicitly: "You are the story-writer; your authoritative spec is `agents/planner/story-writer/process.yml`. Read that at session start and follow it." The pointer-file pattern works because the spec is self-contained.
- Use `scripts/agentic-search.sh --quiet <terms>` when you need to search the corpus without stderr noise.

Full invocation guide: `docs/guides/invoking-agents.md`.

## Core principles (non-negotiable)

- **Prove-it gate.** A story cannot be marked `tested` without a Pass verdict from `agentic-verify` (commit hash + evidence file). Even in Phase 1, we design for this.
- **Slow growth.** Don't add a crate, field, or flag without a failing stress test or a story that demands it. The legacy system died of feature accretion; we will not repeat that.
- **Subscription auth.** When runtime code is written, it uses the local `claude` binary (subscription auth) via subprocess. Never use raw Anthropic API clients that force per-token billing. See ADR-0003.
- **Document DB, schemaless-first.** Persistence goes through `agentic-store`, which wraps SurrealDB embedded. Schemaless by default; schema is added per-table only when justified. See ADR-0002.
- **No bootstrap generator.** `.claude/agents/*.md` files are hand-written pointers. YAML is authoritative. See ADR-0004.
- **Edit is the default action on stories.** The story-writer agent searches first and edits an existing story before it considers writing a new one.

## Terminology

- **Story** — a unit of work defined by a natural-language outcome, one or more executable tests, a UAT journey, and rebuild guidance. Lives under `stories/`.
- **Pattern** — reusable design/operational guidance referenced by stories (not restated in every story's guidance). Lives under `patterns/`.
- **Epic** — a named group of stories with shared context. Lives under `epics/`.
- **Phase** — an execution strategy for a chunk of story work. Attaches to a story or epic.
- **Verdict** — the output of `agentic-verify` for a story: Pass or Fail, with evidence.
- **Evidence** — append-only record of a verify run (commit, verdict, run ID, timestamp, per-test results). Lives under `evidence/runs/<story-id>/<timestamp>-<commit>.jsonl`.
- **Agent** — a YAML-defined role (planner, builder, tester, etc.). The YAML under `agents/` is authoritative; `.claude/agents/*.md` files are hand-written pointers that delegate into it.

## Environment quirks (important for future sessions)

- **Windows Git cannot verify GitHub's SSH host key** in this particular WSL-accessed workspace. `git push` and `git fetch` via the Windows-native git fail with "Host key verification failed." **Route through WSL bash instead:**
  ```
  wsl bash -c "cd /home/code/Agentic && git push origin main"
  ```
  WSL's SSH config has GitHub trusted. No git config change needed on your side.
- **Git identity** used on prior commits is `HuaMick <hua.mick@gmail.com>`, matching the legacy repo. Use `-c user.name=... -c user.email=...` on commit if shell git config isn't set.

## Candidate next stories

After story 1 lands, the next slice needs a second story so story 1's UAT walkthrough has something to verify against. In rough order of foundational-ness:

1. **Schema validation rejects malformed stories** — narrow, grounds the parse-time contract, no runtime dependencies.
2. **`agentic search` returns expected hits from a populated corpus** — exercises the bootstrap script; gives the future Rust CLI a target shape.
3. **Story-writer agent produces a valid story given an objective** — dogfoods the agent we just used; makes the pattern self-verifying.

My (prior session's) lean: (1). Smallest surface, most foundational, gives `agentic-verify` something concrete to check against before we build the full verify runtime.

## Reference: the legacy system

The submodule at `legacy/AgenticEngineering/` is the Python predecessor. Read it to understand what patterns worked and what bloated. **Do not port code directly** — this is a ground-up redesign, not a migration. Relevant lessons documented in ADRs under `docs/decisions/`.

## Fallback behaviour if things break

1. Read the relevant `README.md` under `crates/<name>/` to understand intent.
2. Read this `CLAUDE.md` and the top-level `README.md`.
3. Spawn the appropriate subagent via the Task tool.
4. As a last resort, use Read/Edit/Bash directly against the files — Phase 1 is authored content + docs; nothing requires code to be useful yet.
