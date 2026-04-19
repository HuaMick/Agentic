# Invoking agents

How a Claude Code session drives the agents defined in `agents/<category>/<name>/`.

## The pointer-file pattern

Every agent has three authoritative YAML files under `agents/<category>/<name>/`:

- `manifest.yml` — identity, outputs, allowed tools.
- `process.yml` — the behavioural specification. This is the one that matters.
- `inputs.yml` — what context the agent needs at session start.

And a single short markdown pointer under `.claude/agents/<name>.md`:

```markdown
---
name: <agent-name>
description: |
  One-paragraph "when to use this agent" blurb for Claude Code's routing heuristic.
tools: Read, Glob, Grep, Write, Edit, Bash
---

Read `agents/<category>/<name>/process.yml` and follow it as your complete
specification. Also at session start, read any files listed in that agent's
inputs.yml under required_reading.
```

The pointer file is roughly ten lines. It delegates to the YAML. **There is no generator; both are hand-written.** See ADR-0004.

## Two invocation paths

### Path A — native subagent type (when the harness has registered the agent)

Once the agent is known to the Claude Code runtime (this happens when `.claude/agents/<name>.md` has been picked up by the host), you can spawn via the Task tool:

```
Task(
  subagent_type: "<agent-name>",
  description: "short action phrase",
  prompt: "<objective and any session-specific context>"
)
```

The harness loads the pointer file, Claude Code reads `process.yml`, session begins.

### Path B — generic agent impersonating the role (fallback)

If the subagent type is not registered in the current Claude Code session (common when an agent was authored in the same session, or when running from a different repo root), use a generic subagent type and instruct it explicitly:

```
Task(
  subagent_type: "general-purpose",
  description: "short action phrase",
  prompt: |
    You are the <agent-name> agent, operating in the Agentic repo at
    `//wsl.localhost/Ubuntu/home/code/Agentic/`.

    Your authoritative specification is `agents/<category>/<name>/process.yml`.
    Read that at session start and follow it as your complete behavioural
    contract. Also read everything listed under `required_reading` in
    `agents/<category>/<name>/inputs.yml`.

    <Objective in plain English>

    <Any session-specific constraints: "don't commit," "one file only,"
    "stop after writing," etc.>

    Deliverable:
    - <What exactly to produce>
    - A short report covering: what you did, what decisions you made,
      any meta-feedback on the process itself.
)
```

This is the validated pattern. The story-writer agent was invoked this way twice in session-of-record (authoring `stories/1.yml`), and both invocations followed `process.yml` identically despite being distinct agent instances.

## Conventions that make agents useful

- **One deliverable per invocation.** Don't ask an agent to "write three stories and run verify and commit." Decompose into separate invocations.
- **Explicit stop conditions.** Agents drift if told only what to do, not when to stop. Always name the deliverable and a "stop" signal.
- **Ask for meta-feedback.** Each invocation should report issues with the process.yml itself — ambiguities, missing references, friction. The feedback improves the agent over time.
- **Confirm before destructive multi-file operations.** Per process.yml rules for 2+ file edits, surface a plan and wait for user confirmation.
- **Don't over-specify.** If `process.yml` already covers the rule, don't repeat it in the prompt. Trust the spec.

## Current agents you can invoke

- **`story-writer`** — authors and maintains stories and patterns. Search-first, edit-default. Full spec: `agents/planner/story-writer/process.yml`. Pointer: `.claude/agents/story-writer.md`.
- **`build-rust`** — implements Rust source to turn test-builder scaffolds green without editing tests. Full spec: `agents/build/build-rust/process.yml`. Pointer: `.claude/agents/build-rust.md`.
- **`test-builder`** — writes failing test scaffolds for a story and records red-state evidence per ADR-0005. Refuses to run on a dirty tree. Full spec: `agents/test/test-builder/process.yml`. Pointer: `.claude/agents/test-builder.md`.
- **`test-uat`** — executes a story's `acceptance.uat` prose walkthrough and invokes `agentic uat <id> --verdict <pass|fail>` so the CLI signs the verdict. Full spec: `agents/test/test-uat/process.yml`. Pointer: `.claude/agents/test-uat.md`.
- **`guidance-writer`** — curator of agent specs and the shared `assets/` layer. Full spec: `agents/teacher/guidance-writer/process.yml`. Pointer: `.claude/agents/guidance-writer.md`.

More will be added as demand arises (expected: `planner-build`, `orchestration-executor`).

## Invoking the story-writer — concrete example

```
Task(
  subagent_type: "general-purpose",
  description: "Author a story via story-writer",
  prompt: |
    You are the story-writer agent, operating in the Agentic repo at
    `//wsl.localhost/Ubuntu/home/code/Agentic/`.

    Your authoritative spec is `agents/planner/story-writer/process.yml`.
    Read it at session start. Also read:
    - schemas/story.schema.json
    - schemas/pattern.schema.json
    - docs/guides/story-authoring.md
    - docs/guides/pattern-authoring.md

    Objective: <plain-English description of what the story should deliver>.

    Rules:
    - Follow process.yml strictly, including the search-first workflow
      via `scripts/agentic-search.sh --quiet <terms>`.
    - No implementation — you only write the story YAML.
    - No commit — leave the file unstaged for human review.

    Deliverable: one file at `stories/<next-id>.yml`, schema-valid, plus
    a short report (under 400 words) covering search performed, splitting
    decision if relevant, and any new meta-feedback on the process.
)
```

## When an agent reports meta-feedback

Meta-feedback from agents is valuable — it highlights ambiguities, missing references, or friction the human author missed. Triage each item:

- **Valid** — fix in the relevant `process.yml`, README, script, or schema. Small changes, committed promptly.
- **False positive** — note it, move on. If it recurs, investigate whether the agent's environment differs from the author's (pathing, tooling).
- **Deferred** — legitimate but low-priority. Capture in an issue, story, or the crate's "Open questions" section.

Never ignore meta-feedback silently. The whole point of the invocation-report pattern is to let the agents improve the system they work in.
