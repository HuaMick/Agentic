# agents/

Authored YAML agent definitions. **The product.**

Each agent lives under `<category>/<agent-name>/` with three files:

- `manifest.yml` — identity: name, description, category, version, outputs.
- `process.yml` — how the agent works: ordered steps, guidelines, loop participation.
- `inputs.yml` — what context the agent needs: required parameters, referenced shared inputs.

## Categories

- `orchestration/` — orchestration-planning, orchestration-executor (route work, execute phases)
- `planner/` — epic-creator, planner-build, planner-test, planner-audit (generate plans)
- `build/` — build-rust, build-docs-writer (implement code)
- `test/` — test-builder, test-audit, test-uat (validate implementations)
- `teacher/` — teacher-update-guidance, teacher-update-assets (improve the system)

## Shared inputs

`shared/` holds transitive input layers referenced by multiple agents. DRY mechanism for common context (e.g., "planner-shared.yml" for fields every planner needs).

## Assets

`assets/` holds definitions, guidelines, examples, templates referenced by agents at runtime. Schema-validated so agents don't silently miss fields.

## Claude Code pointer files

`.claude/agents/*.md` are short hand-written pointer files (roughly 10 lines each) that delegate to the authoritative YAML under this directory. There is no generator — keeping the pointers small enough to maintain by hand is cheaper than the drift problems a round-trip generator would introduce.

When you add a new agent here, add a matching pointer under `.claude/agents/` that names it and tells Claude Code where to read its `process.yml`, `manifest.yml`, and `inputs.yml`.

## Phase 1 status

No agents authored yet. First agents will be written as part of the vertical slice: planner-build (to plan story implementation), build-rust (to write the crates), test-builder (to generate acceptance verifiers).
