# agents/assets/

Reusable building blocks referenced by multiple agents. The DRY layer for
agent definitions.

## Subdirectory conventions

- **`definitions/`** — Foundational concepts: shared tool sets, role
  vocabularies, schema contracts. Definitions describe *what something is*.
- **`guidelines/`** — Behavioural rules and operational guidance: when to
  read X, how to handle case Y. Guidelines describe *what to do*.
- **`examples/`** — Concrete canonical samples. Empty day one.
- **`templates/`** — Copy-paste skeletons. Empty day one. The story and
  pattern templates currently live under `docs/guides/` because they are
  also human-facing; they may consolidate here later.

## Extraction rule

Same as the story-writer's pattern extraction rule: extract to an asset
when 2+ agents would share the content. Speculation (one current consumer
plus a hoped-for future one) does not justify extraction. The previously
empty `agents/shared/` directory was deleted and folded into here for the
same reason — it had no content because no concrete duplication justified it.

When you author a new agent and find yourself restating something already
in `assets/`, reference the asset instead. When you find yourself restating
something not in `assets/` for the second time, extract it.

## Current assets

- `definitions/tools-base.yml` — canonical base toolset every agent needs.
- `definitions/session-start-memory.yml` — the do-not-trust-prior-session
  clause, referenced from every agent's `workflow.session_start`.
- `definitions/audit-mode-protocol.yml` — the six-step audit protocol
  (Scope, Scan, Produce a plan, Confirm, Execute, Summarize). Each
  curator's Scan detail stays per-agent; the other five steps reference
  this asset.
- `definitions/identifier-forms.yml` — canonical forms of run_id,
  signer, story_id, and commit. Authored once so test-builder fixtures
  and build-rust producers cannot drift on punctuation
  (e.g. underscore-vs-hyphen in `sandbox:<model>@<run_id>`).
- `guidelines/reference-claude-md.yml` — when and why to read CLAUDE.md;
  removes the scattered restatement across each agent's spec.
- `guidelines/edit-first-curation.yml` — edit-is-default rule shared by
  curator agents (story-writer, guidance-writer).
- `guidelines/no-proof-preservation.yml` — the anti-pattern of tiptoeing
  around fields to preserve a verdict, shared by curator agents.

## Pending extractions (waiting for second consumer)

- A `definitions/story-schema-contract.yml` — extract when test-uat is
  authored and also reads `schemas/story.schema.json` at session start.
