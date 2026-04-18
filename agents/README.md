# agents/

Authored YAML agent definitions. **The product.**

Each agent lives under `<category>/<agent-name>/` with three files:

- `manifest.yml` — identity: name, description, category, version, outputs, allowed tools.
- `process.yml` — how the agent works: ordered steps, guidelines, anti-patterns, escalation rules.
- `inputs.yml` — what context the agent needs: required reading, context scan globs, required inputs.

## Categories

- `orchestration/` — orchestration-planning, orchestration-executor (route work, execute phases).
- `planner/` — epic-creator, planner-build, planner-test, planner-audit, **story-writer** (generate/maintain plans and stories).
- `build/` — build-rust, build-docs-writer (implement code).
- `test/` — test-builder, test-audit, test-uat (validate implementations).
- `teacher/` — teacher-update-guidance, teacher-update-assets (improve the system).

## Assets

`assets/` holds definitions, guidelines, examples, templates referenced by agents at runtime. Schema-validated so agents don't silently miss fields. Anything that would be "shared between agents" lives here — there is no separate `shared/` directory.

## Claude Code pointer files

`.claude/agents/*.md` are short hand-written pointer files (roughly 10 lines each) that delegate to the authoritative YAML under this directory. There is no generator — keeping the pointers small enough to maintain by hand is cheaper than the drift problems a round-trip generator would introduce. See ADR-0004.

When you add a new agent here, add a matching pointer under `.claude/agents/` that names it and tells Claude Code where to read its `process.yml`, `manifest.yml`, and `inputs.yml`.

## Current state

One agent is active:

- **`planner/story-writer/`** — curator of the story and pattern corpora. Search-and-edit is the default; writing new is the exception. Handles stories AND patterns (until a dedicated `pattern-writer` is warranted).

Still to come (order roughly follows Phase 2 demand):

- `build/build-rust` — implements crates against story acceptance tests.
- `test/test-builder` — generates the test files an `agentic-story.acceptance.tests[].file` points at.
- `test/test-uat` — executes a story's UAT prose journey and emits the verdict that promotes `tested → healthy`.
- `planner/planner-build` — plans a story's implementation phases.
- `orchestration/orchestration-executor` — runs phases deterministically.

No agents beyond `story-writer` are authored yet. They're added when a story demands one.

## Invoking an agent

See `docs/guides/invoking-agents.md`. Summary: Task tool, `subagent_type: general-purpose` if not natively registered, hand the agent its objective and tell it to read `agents/<cat>/<name>/process.yml` as its authoritative spec.
