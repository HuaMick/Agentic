# assets/

Reusable building blocks referenced by multiple agents and stories. The
DRY layer for cross-corpus concepts. Lives at top-level (was
`agents/assets/` until 2026-04-28; renamed once stories also became
consumers per ADR-0007 — the `agents/` prefix understated the reach).

## Subdirectory conventions

- **`principles/`** — Cross-cutting design heuristics applied across
  many surfaces (story authoring, scaffolding, crate-level APIs).
  Principles describe *how to judge*. Currently houses `deep-modules.yml`.
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
when 2+ consumers would share the content. Consumers may be agents OR
stories per ADR-0007. Speculation (one current consumer plus a hoped-for
future one) does not justify extraction.

When you author a new agent or story and find yourself restating
something already in `assets/`, reference the asset instead. When you
find yourself restating something not in `assets/` for the second time,
extract it.

## Current assets

- `principles/deep-modules.yml` — Ousterhout-via-Pocock deep-modules
  heuristic (interface cost vs hidden functionality; deletion test;
  three friction signals). Referenced by story-writer, test-builder,
  build-rust, and stories that need the deletion test inline (story 26
  is the canonical consumer).
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
- `definitions/story-schema-contract.yml` — why and how every
  story-touching agent (story-writer, build-rust, test-builder,
  test-uat) reads `schemas/story.schema.json` at session start: field
  shapes, the lifecycle enum, and the prove-it-gate connection that
  makes `status: healthy` writable only by `agentic uat`.
- `guidelines/reference-claude-md.yml` — when and why to read CLAUDE.md;
  removes the scattered restatement across each agent's spec.
- `guidelines/edit-first-curation.yml` — edit-is-default rule shared by
  curator agents (story-writer, guidance-writer).
- `guidelines/no-proof-preservation.yml` — the anti-pattern of tiptoeing
  around fields to preserve a verdict, shared by curator agents.

## Pending extractions (waiting for second consumer)

(none currently)
