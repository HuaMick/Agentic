# stories/

User stories. The **primary work artifact** of the system.

One file per story: `<id>.yml` where `<id>` is a positive integer matching the story's `id` field (e.g., `stories/1.yml` for story 1).

## What a story is

A unit of work defined by:

1. **Outcome** — plain-English value statement. One sentence, no conjunctions.
2. **Acceptance** — one or more executable tests (each with its own justification) plus a prose UAT walkthrough.
3. **Guidance** — non-obvious rebuild-from-scratch context.
4. **Patterns** — IDs of reusable design patterns this story applies (see `patterns/`).
5. **Evidence** — test results are persisted in `agentic-store` (see `evidence/README.md`); only red-state artefacts live on disk under `evidence/runs/<id>/`.

## Lifecycle

```
proposed → under_construction → healthy
                 ↑
                 └── (edit invalidates proof, auto-revert)
```

- `proposed` — default on new stories. Written by the `story-writer` agent or by humans.
- `under_construction` — written by the implementing agent (`build-rust`) when it picks up a `proposed` story and begins work, or auto-reverted by `story-writer` when an edit invalidates a prior Pass verdict.
- `healthy` — written only by `agentic uat` on a Pass verdict.
- `unhealthy` — computed by the dashboard (`agentic stories health`) from signals in `agentic-store`; never written to disk.

A story cannot transition to `healthy` without a Pass verdict from `agentic uat` including:

- A full git commit hash (clean working tree required).
- A trace reference (persisted `test_runs` + `uat_signings` rows).
- A run ID (UUID v4).

## Test file binding

Each test file referenced in `acceptance.tests[].file` is bound **1-to-1** to exactly one story. Tests live under:

- `crates/<crate>/tests/*.rs` — Rust integration tests (preferred).
- `scripts/verify/*.sh` — shell-based verifiers when Rust doesn't fit.

Status-vs-implementation drift (status `proposed` but acceptance tests already passing, status `healthy` but a test currently red, etc.) is flagged by `agentic stories audit` (story 25, under_construction; all 6 acceptance tests green; ready for UAT pending ancestor health on stories 3 and 6). Orphan-test detection is explicitly carved out of story 25's scope; a future story may amend that surface.

## Schema

See `schemas/story.schema.json`. Authoring guide: `docs/guides/story-authoring.md`. Template: `docs/guides/story-template.yml`.

## Current corpus

| id | title | status |
|----|-------|--------|
| 1 | `agentic uat` signs a verdict promoting a story to healthy (library + CLI) | under_construction |
| 2 | `agentic-ci-record` records test-builder test results to `test_runs` | under_construction |
| 3 | `agentic stories health` dashboard (library + CLI) | under_construction |
| 4 | `Store` trait + `MemStore` impl | under_construction |
| 5 | `SurrealStore` backed by `surrealkv` | under_construction |
| 6 | `agentic-story` YAML loader + schema + DAG check | under_construction |
| 9 | Scope dashboard staleness to each story's declared `related_files` | healthy |
| 10 | Render the story corpus as a DAG with frontier-of-work view and blast-radius drilldown | healthy |
| 11 | UAT refuses to sign Pass for a story standing on an unproven ancestor | under_construction |
| 12 | Scope `agentic stories test <selector>` runs to a DAG subtree | under_construction |
| 13 | Classify a story as unhealthy when any transitive ancestor is not healthy | healthy |
| 15 | test-build is a plan-and-record CLI whose user writes the scaffolds | healthy |
| 16 | Persist one run row per inner-loop invocation with a tee'd NDJSON trace | under_construction |
| 17 | Story YAML carries an optional `build_config` block the loader parses | under_construction |
| 18 | Resolve a signer identity and stamp it on every signing and run row | under_construction |
| 19 | `agentic-runtime` un-deferred — Runtime trait and ClaudeCodeRuntime impl | under_construction |
| 20 | `agentic story build <id>` launches the sandbox and returns an attested run | under_construction |
| 21 | Retired status + `superseded_by` chain let the tree prune without deletion | under_construction |
| 23 | `agentic test-build record` emits mixed red/preserved/re-authored verdicts | under_construction |
| 24 | `agentic test-build record` rejects scaffold defects masquerading as compile-red | proposed |
| 25 | `agentic stories audit` surfaces status-vs-implementation drift | under_construction |
| 26 | Extract agentic-test-support | under_construction |
| 27 | Extend asset system to stories | under_construction |

Stories 1-6, 11, and 12 were previously `healthy`. They auto-reverted
to `under_construction` during the Phase 0 batch when defects-amend-the-
owning-story added new acceptance tests (e.g. signer wiring on top of
story 1's UAT path, signer wiring on top of story 2's `test_runs`
shape, the kit-vs-bespoke contract pinning on story 12's ci-record
acceptance entries, related contract tightenings as the runtime/sandbox
stories exposed downstream gaps). The fact that they are not currently
green is what the system is supposed to surface — the moment a defect
lands, proof is invalidated, and a new red-green cycle drives the
tightening back to a Pass verdict.

Story 8 (CLI wiring) was folded into stories 1 and 3 on 2026-04-19 after an
audit found the split was along library/binary crate boundaries rather
than along user journeys — and that story 8's outcome explicitly joined
two distinct observables (signing a verdict AND reading the dashboard).
See each story's `acceptance.tests` for the library-level vs. binary-level
test split.

Stories 7 and 14 were retired on 2026-04-20 in favour of story 15, which
is now the single authority on `agentic test-build`'s contract. Story 14
embedded AI inference INSIDE the test-builder library (the library spawned
`claude` to author scaffold bodies) — the claude-as-component shape the
legacy Python system died of. Story 7's substantive contracts (evidence
atomicity, fail-closed-on-dirty-tree, preservation semantics, thin-
justification refusal, evidence-row shape) were folded into story 15
rather than kept split off, on the principle that a young system
prioritises the cleanest foundation over incremental stability. Story 15
replaces both with a plan/record split whose library never spawns an LLM:
the user (human or claude-as-agent) writes scaffolds with their own
tools, and the CLI atomically verifies and records red-state evidence.
Stories 7 and 14 do not retain their IDs for reuse. Story 15 is a hard
prerequisite for the `dag-primary-lens` epic (stories 10-13). See
`stories/15.yml` for the full scope.

Stories 10, 11, 12, and 13 form the `dag-primary-lens` epic
(`epics/live/dag-primary-lens/epic.yml`). They share a unifying
objective — shift the system's mental model from "flat list" to
"DAG with frontier-of-work, blast-radius, and ancestor-aware
classification" — but have distinct observables (dashboard view
shape, UAT refusal, CI test selection, classifier rule) and
distinct commands (`agentic stories health`, `agentic uat`,
`agentic stories test`, and the classifier change surfaced
through the dashboard), so the epic captures the direction while
four stories capture the executable contracts. Stories 10 and 13
both touch the dashboard's read path but at different layers
(story 10 = view shape; story 13 = classifier rule); stories 11
and 13 both enforce an "ancestors must be healthy" rule but at
opposite sides of the store (story 11 = write / UAT gate;
story 13 = read / classifier). Each story's guidance documents
the split rationale against the splitting-rule.

Check each `<id>.yml` for the authoritative status, outcome, and acceptance.
