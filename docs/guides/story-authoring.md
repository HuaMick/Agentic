# Authoring a story

A story is the unit of work in this system. It has one job: be self-contained enough that an agent could rebuild the component from scratch using only the story (plus any patterns it references).

## The template

See `docs/guides/story-template.yml` for the copy-paste skeleton. Schema: `schemas/story.schema.json`.

## Fields, one at a time

### `id` — integer, required

Positive integer matching the filename. `stories/1.yml` has `id: 1`. Never reused after deprecation. Never reassigned.

### `title` — string, required

Short human-readable label. 1–120 characters. Shown in list views. **Not part of the proof hash** — renaming a story does not invalidate its proof.

### `outcome` — markdown string, required

Plain English. What value this delivers, who benefits, what's now possible. **Must be expressible as one sentence without conjunctions.** If you need "and" or "also" or "as well as" to state the outcome, the story should be split.

**Good:** "A developer can run `agentic verify <id>` and receive a signed verdict with evidence recorded to disk."

*(Note: "and receive" joins verb phrases of a single action — not two user-observable outcomes. This is fine.)*

**Bad — two outcomes:** "A developer can verify a story **and** the system can audit orphan tests." → split into two stories.

**Bad — implementation spec:** "Implement a Verifier struct in agentic-verify that exposes a verify() method…" → rewrite as outcome.

### `status` — enum, required

One of `proposed`, `under_construction`, `healthy`, `unhealthy`.

- `proposed` and `under_construction` may be written by the story-writer, by humans, or by `build-rust` (which is permitted the single flip `proposed → under_construction` on picking up a story).
- `healthy` is written only by `agentic uat` on a Pass verdict.
- `unhealthy` is computed by the dashboard from evidence signals and is never written to disk.

Attempting to hand-write `healthy` or `unhealthy` is rejected on commit via the audit.

### `patterns` — array of pattern IDs, required (default `[]`)

Slugs of patterns this story applies. Each must reference an existing `patterns/<id>.yml` file. Always write this field explicitly, even as `[]`.

Before writing any substantive `guidance`, scan `patterns/`. If a pattern applies, reference it rather than redefining it in `guidance`. If you see the same design concept across 2+ stories and no pattern exists, extract it to a new pattern.

### `acceptance.tests` — array, required, non-empty

One or more entries of `{file, justification}`. A story has a one-to-many relationship with tests.

Each entry:

- **`file`** — path to the test file, relative to repo root. Must exist (enforced at verify time). Test files live under `crates/*/tests/` (Rust integration tests) or `scripts/verify/` (shell-based). **1-to-1 binding:** each test file is referenced by exactly one story. If a story is removed, its tests are removed with it. Orphan tests (unreferenced by any story) are flagged by audit.
- **`justification`** — what THIS specific test proves and why it's sufficient for its scope. Each test gets its own justification. No aggregate rationale across multiple tests.

For a story with multiple tests, the set of tests together must cover the outcome; individually, each test's justification explains its slice.

### `acceptance.uat` — markdown string, required

Prose walkthrough. A UAT agent or human reads this and executes it end-to-end in a clean environment. Must include a cleanup step.

Unstructured by design. Write it like instructions to a smart colleague.

### `guidance` — markdown string, required

The rebuild-from-scratch context. Non-obvious technical detail that an agent would NOT independently derive from `outcome` and `acceptance` alone.

**Include:**

- State-machine transitions and contracts.
- Fail-closed semantics.
- Architectural boundaries.
- File path conventions external to the code.
- Exit-code contracts.
- JSON/event shapes that cross process boundaries.
- Anything counterintuitive.

**Exclude:**

- Language or framework choice (obvious from repo context).
- Internal function signatures.
- Regex patterns for well-known formats.
- **Anything that already lives in a referenced pattern.** Reference the pattern, don't paraphrase.
- Anything the agent would independently arrive at the same answer on.

**Length:** unconstrained. If a story covers one coherent outcome with one shared fixture and one rebuild brief, it stays one story regardless of length. Split only when the splitting rule (below) fires — not because the file got long.

### `depends_on` — array of integers, optional (default `[]`)

Story IDs that must reach `healthy` before this story can be marked `healthy`. Cycles are rejected at load time. Unknown IDs are rejected at load time. Keep sparse.

## Splitting rule

**Split a story when EITHER is true:**

1. Its outcome cannot be stated in one sentence without conjunctions (and, also, as well as, comma-joined clauses). → Split on the conjunction.
2. Its acceptance tests do not share a common precondition and a common observable. → Split along the precondition boundary.

**Otherwise, do not split.** Length is not a splitting criterion.

**Tiebreakers** — when it's not obvious, split if 2+ of these hold:

- UAT walkthrough has distinct starting contexts.
- Guidance reads as two separate rebuild briefs with minimal shared vocabulary.
- Story mixes a spike (research / learning) with delivery.
- Any single element (outcome / tests / UAT / guidance) contradicts another when read alone.

## Lifecycle in detail

```
proposed ──► under_construction ──► healthy
                   ↑                    │
                   │                    │
                   └─ (edit invalidates proof, auto-revert) ─┘
```

- Start state: `proposed` (file exists, nothing run).
- `proposed → under_construction` is written by the implementing agent (`build-rust`) when it picks up a story; the story-writer may also flip to `under_construction` as an auto-revert when a proof-invalidating edit lands on a previously healthy story.
- `under_construction → healthy` is written only by `agentic uat` on a Pass verdict with a clean working tree, a signed commit hash, and an evidence file.
- Editing `outcome`, `patterns`, `acceptance.tests`, `acceptance.uat`, or `guidance` invalidates proof; a `healthy` story auto-reverts to `under_construction` on the next audit.
- Editing a referenced pattern similarly invalidates all stories that reference it.
- `unhealthy` is a derived view (dashboard-computed from evidence). It signals "a recent run went red" or "proof hash no longer matches the story's content" and is never written to disk.

## Proof hash

The system hashes `outcome + patterns + acceptance + guidance` (in canonical form) and stores the hash with every verdict. On audit: current-hash vs verdict-hash; mismatch auto-reverts status.

`title` and `depends_on` are NOT part of the hash. `title` is a label; `depends_on` is a scheduling concern.

## Authoring workflow (proposed CLI once built)

```
agentic story new "<title>"                 # create stories/<next-id>.yml from template
agentic story lint <id>                     # schema check + referential integrity
agentic story verify <id>                   # run tests, write evidence, promote if Pass
agentic story uat <id>                      # spawn UAT agent, promote to healthy on Pass
agentic story audit                         # check all stories: status vs evidence, orphans, stale proof
agentic story deprecate <id> --reason "..."
agentic story archive <id>
agentic search <terms>                      # search stories by terms (bootstrap: scripts/agentic-search.sh)
```

## Anti-patterns

- **Stories as sprint tickets.** A story is a durable spec, not a task. No dates, no priorities.
- **Guidance that restates a pattern.** If the concept lives in `patterns/`, reference it. Don't paraphrase.
- **Guidance that restates another story.** If two stories' guidance sections overlap substantially, either merge the stories or extract to a pattern.
- **Outcomes with conjunctions.** See splitting rule.
- **Aggregate test justification.** Each test in `acceptance.tests` gets its own justification. No "these tests together cover X."
- **Optimistic UAT.** "Run the command. It should work." is not a UAT. Write observed checks.
- **Orphan tests and UAT scripts.** Every test file and UAT script must be referenced by some story.
- **Proof-preservation edits.** Don't tiptoe around fields to avoid invalidating `healthy`. Editing invalidates proof — that's correct behavior. Re-UAT rebuilds it.
- **Near-duplicate creation.** If `scripts/agentic-search.sh <terms>` would have returned a match, the story-writer failed at step 1.
