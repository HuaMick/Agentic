# Agentic — Claude Code Instructions

You are working on a Rust rebuild of the AgenticEngineering system. The legacy
Python codebase lives under `legacy/AgenticEngineering/` as a git submodule —
**reference only, do not edit**.

For project status (what's shipped, what's in flight, what's next), read
`README.md`. This file is the durable instruction layer and should rarely
change.

## How to drive the system

Until the `agentic` binary exists, Claude Code is the primary interface.
Authority flows through, in order:

1. **This `CLAUDE.md`** — project-wide conventions and durable rules.
2. **`README.md`** — current state.
3. **ADRs under `docs/decisions/`** — architectural decisions; non-negotiable
   for the relevant scope.
4. **Per-crate `README.md`** under `crates/<name>/` — each crate's purpose,
   boundaries, and design.
5. **Schemas under `schemas/`** — authoritative shape of authored artefacts
   (stories, patterns, agents, assets).
6. **Agent specs under `agents/<category>/<name>/`** — three files per agent
   (`contract.yml`, `inputs.yml`, `process.yml`), each schema-validated.
7. **Pointer files under `.claude/agents/<name>.md`** — the Claude Code
   handshake into the authoritative YAML.
8. **Stories under `stories/`** — executable acceptance criteria. The unit
   of work.
9. **Patterns under `patterns/`** — reusable design guidance referenced by
   stories.

### Orchestration role

When Claude Code is the session orchestrator, the **session-orchestrator**
spec at `agents/orchestration/session-orchestrator/process.yml` is its
authoritative behavioral guide — including the orient-before-acting pattern
(verify brief claims against current corpus state before delegating writes),
the post-return verification trigger (re-verify a spawned agent's
deflection-shaped framing via system-investigator before incorporating it
into session truth), and the preserve-before-destroy rule (snapshot
artefacts that support or refute load-bearing claims before any operation
that would destroy them). The **system-investigator**
(`agents/orchestration/system-investigator/`) is the sanctioned tool for
parallel, read-only state-investigation when the orient or post-return
trigger fires; spawn one per independent question, in parallel.

### Spawning a subagent

For agents registered as native Claude Code subagent types, use
`subagent_type: <name>` directly. For agents not yet registered, use
`subagent_type: general-purpose` and brief them to read
`agents/<category>/<name>/{contract,inputs,process}.yml` as their
authoritative spec at session start.

### Surface ownership and joint authority (2026-04-28)

Most surfaces have a single curatorial owner declared in that agent's
`contract.yml owns:`. Two surfaces are **jointly owned by the
orchestrator and guidance-writer** — either may edit, and the
`route-to-the-owner` rule treats both as legitimate routing
destinations:

- **`schemas/**`** — JSON Schemas governing stories, assets, agent
  specs, and patterns. Schema changes carry corpus-wide impact;
  whoever edits flags load-bearing changes (regex / required-field /
  enum shifts) to the user before committing. Mechanical sync (e.g.
  path-pattern updates after a directory move) lands without
  escalation.
- **`.claude/hooks/**`** — PreToolUse hook scripts and their smoke
  tests. New hooks must ship with a `test_<name>.sh` smoke test
  alongside the `.py`. The orchestrator-edit-guard hook itself
  protects test-builder's territory (`scripts/verify/**`,
  `crates/*/tests/**`, `evidence/runs/**`); it does NOT block
  schema or hook edits, by design.

Other unowned surfaces (`CLAUDE.md`, top-level `README.md`,
`docs/decisions/`, `docs/guides/`) remain orchestrator territory by
default — there is no curator subagent for them. The orchestrator
edits these inline when the change is mechanical or narrative
(README status counts, ADR amendments, CLAUDE.md durable rules).

Subagents have no memory of your conversation. Every brief must be
self-contained — include file paths, the story id, and any environment
gotchas the agent will hit.

When the orchestrating Claude Code session is the right hands for a job
(e.g. one-shot cleanup, git operations, multi-agent coordination), do it
inline. When the job is "implement code" or "author content," delegate to
the appropriate subagent — context discipline matters.

## Core principles (non-negotiable)

- **Prove-it gate.** A story cannot be marked `healthy` without a Pass
  verdict from `agentic uat`, with commit-signed evidence persisted in
  `agentic-store`. See ADR-0005.
- **Slow growth.** Don't add a crate, dependency, field, or flag without a
  failing test or a story that demands it. The legacy system died of
  feature accretion; we will not repeat that.
- **Subscription auth.** `agentic-runtime` (the orchestrator crate) uses
  the local `claude` binary (subscription auth) via subprocess to spawn
  subagents. Never use raw Anthropic API clients. Product libraries
  (`agentic-uat`, `agentic-test-builder`, `agentic-store`, etc.) do NOT
  wrap `claude` themselves — claude is a user of the CLI, not a
  component of the libraries. See ADR-0003.
- **Document DB, schemaless-first.** Persistence goes through
  `agentic-store`'s `Store` trait. Schemaless by default; schema added
  per-table only when justified. See ADR-0002.
- **No bootstrap generator.** `.claude/agents/*.md` files are hand-written
  pointers. YAML is authoritative. See ADR-0004.
- **Red-green is a contract.** The test-builder agent authors failing
  scaffolds (using its normal authoring tools); `agentic test-build
  record` verifies the red state and writes atomic evidence. Build-rust
  writes implementation source and never edits tests. See ADR-0005.
- **Edit before write.** Stories, patterns, agent specs, and assets —
  search the existing corpus before authoring new. Each curator agent
  enforces this in its own process.yml.
- **Reference, don't restate.** If content already exists in `assets/`
  or in CLAUDE.md or in an ADR, reference it. Copy-paste duplication is
  drift waiting to happen.
- **Sync is enforced at commit time.** Per stories 28 + 29, the YAML
  `status: healthy` field and the `agentic-store`'s
  `uat_signings UNION manual_signings` tables must agree at every
  commit. The tracked `.githooks/pre-commit` script runs
  `agentic stories audit` and `agentic stories health --all` and
  refuses any commit that fails either gate. Promotions go through
  `agentic uat <id> --verdict pass`; the manual ritual
  (hand-edit YAML + green.jsonl) is no longer the sanctioned path
  and the hook will reject it. One-time per clone:
  `git config core.hooksPath .githooks`. Cross-machine provenance
  loss (a fresh clone whose store doesn't carry rows for stories
  promoted on other machines) is recovered via
  `agentic store backfill <id> --bootstrap`, gated to one-shot per
  story and tagged in `manual_signings` as
  `source: bootstrap-cross-machine` for audit-trail clarity. The
  hook fails open if the `agentic` binary is not on PATH so a
  fresh clone can install it before the gate fires.

## Terminology

- **Story** — a unit of work defined by an outcome, one or more executable
  acceptance tests, a UAT journey, and rebuild guidance.
  Status enum: `proposed | under_construction | healthy | unhealthy | retired`.
- **Pattern** — reusable design/operational guidance referenced by stories.
- **Verdict** — the output of `agentic uat`: Pass or Fail, with commit-signed
  evidence in the `uat_signings` table.
- **Evidence** — entries in the `agentic-store` DB (`test_runs` upserted per
  CI run; `uat_signings` append-only) plus red-state JSONL files under
  `evidence/runs/<story-id>/` per ADR-0005.
- **Agent** — a YAML-defined role under `agents/<category>/<name>/`. Five
  buckets total across three files: `scope` + `outcome` (contract.yml),
  `inputs` (inputs.yml), `workflow` + `guidance` (process.yml).
- **Asset** — shared content under `assets/` (top-level since 2026-04-28
  per ADR-0007 amendment; was `agents/assets/`). Referenced by 2+
  consumers — agents (via `inputs.yml required_reading:`) or stories
  (via `assets:` field per ADR-0007). Required fields: `name`,
  `description`, `current_consumers`.

For the current agent roster and their authority boundaries, see
`agents/README.md`. Per-agent contracts live at
`agents/<category>/<name>/contract.yml`.

## Environment quirks (load-bearing)

- **WSL workspace.** The repo lives in WSL at `/home/code/Agentic` and is
  also accessible from Windows via `\\wsl.localhost\Ubuntu\home\code\Agentic`.
  Most tools work from either side; some need WSL specifically.

- **Cargo via WSL.** The Rust toolchain (`rustup`, `rustc`, `cargo`) is
  installed inside WSL only — not on the Windows PATH. Every cargo
  invocation must route through:
  ```
  wsl bash -c "source ~/.cargo/env && cd /home/code/Agentic && cargo <cmd>"
  ```
  Brief every subagent with this.

- **Git push routing.** Windows-native git cannot verify GitHub's SSH host
  key from this WSL-accessed workspace. `git push` and `git fetch` via the
  Windows git fail with "Host key verification failed." Route through WSL:
  ```
  wsl bash -c "cd /home/code/Agentic && git push origin main"
  ```

- **Git identity.** Shell `user.email` may not be set. Pass identity
  per-commit:
  ```
  git -c user.name="HuaMick" -c user.email="hua.mick@gmail.com" commit ...
  ```

- **Line endings.** `.gitattributes` forces LF on text files so files
  edited through the Windows side don't drift to CRLF and trigger
  test-builder's fail-closed-on-dirty-tree gate.

- **Legacy submodule.** `legacy/AgenticEngineering/` has `ignore = dirty`
  in `.gitmodules` — its internal working-tree state is upstream's
  business and does not surface as repo dirt.

- **Claude Code `isolation: "worktree"` — avoid.** The harness's
  worktree isolation creates `.git` files pointing at Windows-style UNC
  paths (`//wsl.localhost/...`) which WSL git cannot traverse. A
  subagent invoked into such a worktree sees `fatal: not a git
  repository` and silently falls back to editing the main worktree —
  evidence ends up in the wrong place. If you genuinely need parallel
  cargo work in worktrees, create them from inside WSL with `git
  worktree add <wsl-path>` and pass the WSL path to subagents
  explicitly; do not use the harness's `isolation` flag from this
  workspace.

## Reference: the legacy system

The submodule at `legacy/AgenticEngineering/` is the Python predecessor.
Read it to understand what patterns worked and what bloated. **Do not port
code directly** — this is a ground-up redesign, not a migration. Lessons
documented in ADRs under `docs/decisions/`.

## Fallback behaviour if things break

1. Read `README.md` for current state.
2. Read this `CLAUDE.md` for conventions.
3. Read the relevant crate `README.md` under `crates/<name>/`.
4. Read the relevant agent's `contract.yml` to understand its authority.
5. Spawn the appropriate subagent.
6. As a last resort, use Read/Edit/Bash directly — but prefer subagents
   to manage context; the orchestrating session shouldn't grow Rust code
   or test files itself.