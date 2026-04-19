# agentic-story

## What this crate is

The **hero crate**. Defines what a story is: the `Story` type, its acceptance structure (tests + UAT), pattern references, lifecycle state machine, proof-hash algorithm, verdict records, and evidence wiring.

This is the primary entity in the entire system. Epics group stories. Phases execute story work. Agents produce story work. Nothing is "done" without a story in `healthy` state.

## Current state (stories 6 + 9 shipped)

Story 6 shipped the loader: YAML parse, schema validation, DAG check on
`depends_on`, and typed errors naming the offending field. Story 9
extended the loader to accept an optional `related_files: Vec<String>`
field; absent / empty is permissive and preserved round-trip.

## Why it's a separate crate

Story is the domain center. Isolating it here means:

1. Every other crate that touches stories depends on this one. The compiler makes the dependency direction explicit — `agentic-work` (epics/phases) depends on stories, never the other way.
2. Story format is stable and changes deliberately. Churn here ripples everywhere.
3. Can be used standalone — a linter, doc generator, or corpus auditor doesn't need the full system.

## Public API sketch

```rust
pub struct Story {
    pub id: StoryId,                // positive integer
    pub title: String,              // label only, not part of proof hash
    pub outcome: String,
    pub status: Status,             // see state machine below
    pub patterns: Vec<PatternId>,   // slug refs
    pub acceptance: Acceptance,
    pub guidance: String,
    pub depends_on: Vec<StoryId>,   // scheduling concern, not part of proof hash
}

pub struct Acceptance {
    pub tests: Vec<TestEntry>,      // 1-to-many; each has its own justification
    pub uat: String,                // prose journey for a UAT agent
}

pub struct TestEntry {
    pub file: PathBuf,              // must exist at verify time, not parse time
    pub justification: String,      // what THIS specific test proves
}

pub enum Status {
    Proposed,
    UnderConstruction,
    Tested,        // set only by agentic-verify after Pass
    Healthy,       // set only by agentic-verify after UAT pass
    Deprecated,
    Archived,
}

// Lifecycle transitions enforced in code:
impl Story {
    pub fn record_verdict(&mut self, v: &Verdict) -> Result<(), LifecycleError>;
    // Constructing Status::Tested/Healthy is pub(crate) — only this crate can
    // write those values via record_verdict(). Humans can't hand-edit the YAML
    // to Tested/Healthy without being caught by the audit.
}
```

## Proof hash (load-bearing)

Every verdict records a proof hash computed over the canonical serialization of:

- `outcome`
- `patterns` (array of IDs, sorted; each referenced pattern's content is hashed in too — editing a pattern invalidates proof for stories that reference it)
- `acceptance.tests` (array of `{file, justification}` in document order)
- `acceptance.uat`
- `guidance`

Explicitly **not** in the hash:

- `title` — a label; renaming is cosmetic.
- `status` — the thing we're proving, not the thing being proved.
- `depends_on` — a scheduling concern; ordering does not change what the story delivers.
- `id` — a name, same as title.

On `agentic story audit`:

- Current-hash computed from the YAML on disk (plus any referenced patterns).
- Compared to the hash recorded in the latest Pass verdict for this story.
- If equal: status stands.
- If not equal: status auto-reverts to `under_construction`. Proof is for the old content, not the current content.

Canonical serialization is deterministic (sorted keys in JSON; array order preserved where authored, sorted where set-like). The serialization algorithm is part of the contract — changing it requires a versioned hash scheme.

## Dependencies

- Depends on: `agentic-events` (emits story-level events), `serde`, `chrono`, `serde_yaml`
- Depended on by: `agentic-work`, `agentic-verify`, `agentic-orchestrator`, `agentic-cli`, `agentic-store`

## Design decisions

- **Story YAML is the primary artifact.** Stories live on disk under `stories/<id>.yml`. This crate defines the schema (via `schemas/story.schema.json`) and parse/serialize.
- **Acceptance tests are executable and bound 1-to-1 to stories.** Each `TestEntry.file` is referenced by exactly one story. Orphan test files (unreferenced by any story) are flagged by audit.
- **Evidence is append-only and external.** The Story type does not hold evidence; it holds a pointer convention (`evidence/runs/<id>/`). Evidence lives in `agentic-verify` and on disk; see `evidence/README.md`.
- **Lifecycle transitions are gated in code.** `Story::record_verdict()` requires a valid `Verdict` constructed by `agentic-verify`. You cannot set `status = Tested` directly. The compiler enforces this.
- **No priority, no category, no test_status, no `notes`.** Legacy had too many axes. We add fields only when a story can't be authored without them.

## Open questions

- How do we handle story rewrites after `tested`? **Settled:** edit → auto-revert to `under_construction` on next audit. No special flow for "minor edits."
- Canonical-form versioning — if we ever change the hash algorithm, we need a `hash_version` field in the verdict. Defer until first migration.

## Stress/verify requirements

- Round-trip parse/serialize for all checked-in stories without loss.
- Lifecycle state machine rejects invalid transitions under random sequences (property test via `proptest`).
- Proof hash is stable across serializer versions and across whitespace-only YAML edits.
- Concurrent verdict writes to the same story don't corrupt the evidence log.
