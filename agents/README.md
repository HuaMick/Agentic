# agents/

Authored YAML agent definitions. **The product.**

Each agent lives under `<category>/<agent-name>/` with three files, each enforced by its own JSON Schema under `schemas/`:

- **`contract.yml`** (schema: `schemas/agent-contract.schema.json`) — **scope** (name, category, version, purpose, `owns`, `does_not_touch`, `authority`) and **outcome** (what the agent produces, success criteria). The "API surface" — read this first to decide if the agent is the right one for a task.
- **`inputs.yml`** (schema: `schemas/agent-inputs.schema.json`) — everything the agent reads or uses: `required_reading`, `context` globs, caller-supplied `parameters`, Claude Code `tools` grant, preferred Bash `commands`.
- **`process.yml`** (schema: `schemas/agent-process.schema.json`) — **workflow** (`session_start`, `steps` or named `modes`) and **guidance** (`rules`, `anti_patterns`, `escalation`). How the agent behaves.

The schemas enforce exactly these five buckets (scope, outcome, inputs, workflow, guidance) and forbid top-level keys outside them. This is the same kind of enforcement stories get — so agents cannot quietly invent new categories of guidance to hide slop in.

## Categories

- `orchestration/` — orchestration-planning, orchestration-executor (route work, execute phases). None authored yet.
- `planner/` — **story-writer** (active). epic-creator, planner-build, planner-test, planner-audit still to come.
- `build/` — **build-rust** (active). build-docs-writer still to come.
- `test/` — **test-builder** (active), **test-uat** (active). test-audit still to come.
- `teacher/` — **guidance-writer** (active). Curates agent specs and the shared `assets/` layer.

## Assets

`assets/` holds definitions, guidelines, examples, templates referenced by agents at runtime. Schema-validated so agents don't silently miss fields. Anything that would be "shared between agents" lives here — there is no separate `shared/` directory.

Current content (see [`agents/assets/README.md`](assets/README.md) for the full layout and extraction rules):

- `definitions/tools-base.yml` — canonical base toolset every agent needs.
- `definitions/session-start-memory.yml` — do-not-trust-prior-session clause referenced from every agent's `workflow.session_start`.
- `definitions/audit-mode-protocol.yml` — shared six-step protocol for any curator with an `audit` mode (Scope, Scan, Plan, Confirm, Execute, Summarize).
- `guidelines/reference-claude-md.yml` — when and why to read CLAUDE.md, so individual agents do not restate it.
- `guidelines/edit-first-curation.yml` — edit-is-default rule shared by curator agents.
- `guidelines/no-proof-preservation.yml` — anti-pattern of tiptoeing around fields to preserve a verdict, shared by curator agents.

## Claude Code pointer files

`.claude/agents/*.md` are short hand-written pointer files (roughly 10 lines each) that delegate to the authoritative YAML under this directory. There is no generator — keeping the pointers small enough to maintain by hand is cheaper than the drift problems a round-trip generator would introduce. See ADR-0004.

When you add a new agent here, add a matching pointer under `.claude/agents/` that names it and tells Claude Code where to read its `process.yml`, `manifest.yml`, and `inputs.yml`.

## Current state

Five agents are active:

- **`planner/story-writer/`** — curator of the story and pattern corpora. Search-and-edit is the default; writing new is the exception. Handles stories AND patterns (until a dedicated `pattern-writer` is warranted).
- **`build/build-rust/`** — implements Rust code to make a story's acceptance tests green. Runs the full workspace test suite after every change to detect regressions; refuses to leave baseline-green tests red. Never promotes past `under_construction`.
- **`test/test-builder/`** — writes the failing test scaffolds a story's `acceptance.tests[].file` entries point at, and records the red-state evidence proving the story was red at the commit implementation begins from. Never writes production source. Preserves existing test files by default; re-authors one only under the narrow ADR-0005 amendment carve-out (story `under_construction`, story YAML newer than the test's most recent evidence row, atomic commit of edit + new evidence). Refuses to run on a dirty tree. See ADR-0005.
- **`test/test-uat/`** — executes a story's `acceptance.uat:` prose walkthrough step by step, judges each observable, and invokes `agentic uat <id> --verdict <pass|fail>` so the CLI writes the commit-signed verdict and (on Pass) promotes the story to `healthy`. Refuses to run on a dirty tree; never edits source, tests, or the story YAML — promotion flows through the CLI.
- **`teacher/guidance-writer/`** — curator of agent specs and the shared assets layer. Edit-first; extracts to assets only when 2+ agents would share. Keeps pointer files and READMEs in sync. Cannot author a new agent without explicit user authorization.

Still to come (order roughly follows Phase 2 demand):

- `planner/planner-build` — plans a story's implementation phases.
- `orchestration/orchestration-executor` — runs phases deterministically. Natural home for enforcing the ADR-0005 sequence (story-writer → test-builder → build-rust → uat).

Agents beyond the four above are added when a story demands one.

## Invoking an agent

See `docs/guides/invoking-agents.md`. Summary: Task tool, `subagent_type: general-purpose` if not natively registered, hand the agent its objective and tell it to read `agents/<cat>/<name>/process.yml` as its authoritative spec.
