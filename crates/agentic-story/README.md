# agentic-story

## What this crate is

The **hero crate**. Defines what a story is: the `Story` type, its acceptance criteria, its lifecycle state machine, verdict records, and evidence format.

This is the primary entity in the entire system. Epics group stories. Phases execute story work. Agents produce story work. Nothing is "done" without a proven story.

## Why it's a separate crate

Story is the domain center. Isolating it here means:

1. Every other crate that touches stories must depend on this one. The compiler makes the dependency direction explicit — `agentic-work` (epics/phases) depends on stories, never the other way.
2. Story format is stable and changes deliberately. Churn here ripples everywhere, so the crate is kept disciplined.
3. Can be used standalone — e.g., a standalone linter or documentation generator for stories doesn't need to pull in the whole system.

## Public API sketch

```rust
pub struct Story {
    pub id: StoryId,
    pub title: String,
    pub outcome: String,          // plain-English value statement
    pub status: Status,           // proposed | under_construction | proven | deprecated | archived
    pub acceptance: Vec<Criterion>,
    pub context: Context,         // epic, depends_on, touches
    pub evidence_ref: EvidencePath,  // path to append-only evidence log
    pub tags: Vec<String>,
    pub notes: String,
}

pub struct Criterion {
    pub id: String,              // e.g., "ac-01"
    pub given: String,
    pub when: String,
    pub then: String,
    pub verify: VerifyCmd,       // executable: shell cmd or cargo test filter
}

pub enum Status { Proposed, UnderConstruction, Proven, Deprecated, Archived }

pub struct Verdict {
    pub story_id: StoryId,
    pub verdict: Pass | Fail,
    pub commit: String,          // git commit hash
    pub run_id: String,
    pub timestamp: DateTime<Utc>,
    pub trace_ref: PathBuf,
}

// Lifecycle transitions are enforced here:
impl Story {
    pub fn promote(&mut self, verdict: &Verdict) -> Result<(), LifecycleError>;
}
```

## Dependencies

- Depends on: `agentic-events` (emits story-level events), `serde`, `chrono`
- Depended on by: `agentic-work`, `agentic-verify`, `agentic-orchestrator`, `agentic-cli`, `agentic-store`

## Design decisions

- **Story YAML is the primary artifact.** Stories live on disk under `stories/<id>.yml`. This crate defines the schema and parse/serialize.
- **Acceptance criteria are executable.** Non-executable criteria are a parse error. This forces prove-it from day one.
- **Evidence is append-only and external.** `evidence_ref` points to a directory of run records, not inlined. Avoids mutable-field concurrency bugs the legacy had with `last_pass_commit`.
- **Lifecycle transitions are gated in code.** `Story::promote()` requires a valid `Verdict`; you cannot set `status = Proven` directly.
- **No priority, no category, no test_status.** Legacy had too many axes. Add fields when a story can't be written without them.

## Open questions

- Should `evidence_ref` default to a convention (`evidence/runs/<story-id>/`) or be explicit per story?
- How do we handle story rewrites after `proven`? New story ID, or re-enter `under_construction`?
- Inline vs separate evidence: settled on separate for concurrency, but worth revisiting if it proves awkward.

## Stress/verify requirements

- Round-trip parse/serialize for all checked-in stories without loss.
- Lifecycle state machine rejects invalid transitions (property test via `proptest`).
- Concurrent verdict writes to the same story don't corrupt the evidence log.
