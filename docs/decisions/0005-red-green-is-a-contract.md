# ADR-0005: Red-green is a contract, not a convention

**Status:** accepted
**Date:** 2026-04-18

## Context

The legacy AgenticEngineering system ran work in a fixed order: `teach → build → test → cleanup → audit → uat` (see `legacy/AgenticEngineering/modules/AgenticGuidance/assets/definitions/plans.yml`). Two agents divided the test responsibility: `planner-test` defined what to test, and `test-builder` wrote the tests — both ran **after** the build phase. Tests validated already-written code instead of driving it.

Two documented incidents show the failure mode this produced:

- Epic `260214UT_enforce_real_cli_smoke_tests_in_uat_and_fix_orchestrate_command_too_long_bug` shipped broken tmux code because "tests mocked subprocess → never called real tmux; UAT used `--no-tmux` → bypassed the broken code path." Tests written after the code were green against the mock shape the code had accidentally grown into.
- Epic `260131GA_close_plan_teach_test_planning_gap` recorded guidance changes passing self-review and audit validation but failing in actual execution, because the test phase was treated as optional for "guidance-only" changes.

Both are the same root cause: when the test phase is downstream of build, tests get shaped by the code instead of the other way round, and "green" stops meaning "the behaviour is correct."

The new Rust system is already halfway to a red-green-refactor discipline without having named it. `schemas/story.schema.json` requires `acceptance.tests[].justification` per test before a story can advance, so test *criteria* exist at story-write time. `agents/build/build-rust/process.yml` step 23 scaffolds failing tests and then turns them green — red-green in practice. But the discipline lives entirely inside one agent's step; removing or softening that step silently dissolves it, and there is no artefact on disk that proves a story's tests were ever red. Nothing stops a future agent from writing tests to fit code already on disk.

The user has asked to revisit TDD explicitly for the new system. The question this ADR answers is whether red-green should be a named contract with its own enforcement point and its own artefact, or stay an implicit habit of the build agent.

## Decision

**Red-green is a named contract enforced at agent and evidence boundaries.**

1. **A dedicated `test-builder` agent owns test-file creation.** Stories name test files in `acceptance.tests[].file`; test-builder reads the justifications and writes the scaffolds. The scaffolds must compile and must fail. Test-builder may not write production source files. See `agents/test/test-builder/`.

2. **Red-state evidence is a committed artefact.** After test-builder runs, `evidence/runs/<story-id>/<timestamp>-red.jsonl` records commit hash, run id, and a per-test red verdict. The red→green transition is observable on disk, not inferred from agent transcripts.

3. **Fail-closed on a dirty tree.** Test-builder refuses to run when the working tree has uncommitted changes, because red-state evidence that is not a committable atomic is forgeable.

4. **Test-builder does not edit existing test files.** If a file already exists with content, test-builder leaves it alone and reports it preserved. This prevents a later run from re-reddening a test the implementer has already worked on.

5. **Justification thinness is an escalation, not a default.** If an `acceptance.tests[].justification` is too thin to derive a scaffold from (TODO, single word, empty after trim), test-builder stops and surfaces to the story-writer rather than guessing.

6. **`agentic-verify` and `agentic-uat` read the red-state evidence file.** A story whose red-state evidence is absent or whose red verdict shape does not match the current `acceptance.tests[]` is treated as unverified — UAT cannot promote to `healthy`.

Story 7 (`Record each story's red state before implementation begins`) is the meta-story proving this contract end-to-end, the same shape story 1 plays for `agentic uat`.

## Migration

Completed in the same session this ADR was authored. `build/build-rust` v0.3.0 forbids test-file writes in both `contract.yml` (`does_not_touch: crates/*/tests/**, scripts/verify/**`) and `process.yml` (step now reads "Confirm acceptance tests exist and are red"; a missing file is an escalation to test-builder, not a write). There is no overlap window: on any story advanced after this ADR was accepted, test-builder is the sole author of test files. Stories already `under_construction` at acceptance time carry whatever test files their earlier build-rust run produced — the preserve-existing rule makes a later test-builder run a no-op on those files, which is the intended bridge.

## Alternatives considered

**Keep red-green as a convention inside `build-rust`.** Rejected. This is the current state; it reproduces the legacy failure mode one agent-edit away. Nothing on disk witnesses the red state; a future refactor that deletes the scaffold step is invisible until a broken story ships.

**Planner-only (criteria without code).** Rejected. This was the legacy `planner-test` role and it did not prevent build-before-test — the planner handed a spec to test-builder, who ran *after* the build. The order is what matters; a planner at the front is a distraction unless the actual test code lands before implementation.

**Full orchestrator that sequences `story-writer → test-builder → build-rust → uat`.** Deferred, not rejected. A named orchestration-executor is the natural home for this sequence; this ADR establishes the contract the orchestrator would enforce. An orchestrator without a red-state artefact is just more prompt — an artefact without an orchestrator is still verifiable.

**Encode red-green in the schema** (e.g. require `acceptance.tests[].red_evidence` path field). Deferred. The schema currently models *what a story claims*, not *where proof lives*; evidence paths are derived by convention (`evidence/runs/<id>/...`) rather than embedded. Revisit if the derivation proves fragile.

## Consequences

**Gained:**

- "Was this story tested first?" is an on-disk question, answerable with `git log evidence/runs/<id>/`.
- Legacy's "tests written after to fit code" failure mode becomes structurally harder — test-builder refuses to overwrite, so a late test edit is visible as a deviation from the red scaffold.
- A clean boundary between agents: story-writer owns the *claim*, test-builder owns the *failing proof surface*, build-rust owns the *implementation*, uat owns the *promotion*.

**Given up:**

- One more agent to maintain. The cost is roughly three YAML files plus one pointer, and the agent does not run in every session — only when a story advances from proposed to under_construction.
- A second writer in the `crates/*/tests/**` namespace during the migration window. Mitigated by: build-rust's existing "if the file does not exist" guard and test-builder's "preserve existing" rule will not collide; at worst test-builder is a no-op on stories build-rust has already scaffolded.

**Revisit when:**

- The orchestration-executor agent lands. Its job is to enforce the order this ADR names; the ADR and the agent should be consistent.
- A story is found in the corpus whose red-state evidence and current `acceptance.tests[]` disagree — that is the signal the schema should embed the evidence pointer rather than deriving it.

## Related

- `agents/test/test-builder/contract.yml` — authoritative scope for the new agent.
- `agents/test/test-builder/process.yml` — red-state workflow, including fail-closed on dirty tree.
- `stories/7.yml` — meta-story proving the contract end-to-end.
- `agents/build/build-rust/process.yml` step 23 — stop-gap scaffold creation, to be removed post-migration.
- `docs/decisions/0004-no-bootstrap-generator.md` — same pattern: an authoritative YAML spec with a hand-written pointer file.
