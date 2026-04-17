# agentic-work

## What this crate is

Epics and phases. The grouping and execution-strategy layer on top of stories. An epic is a named collection of stories with shared context (branch, objective, depends-on). A phase is an execution plan for a subset of a story's work (which agent runs, how many turns, what feedback triggers apply).

## Why it's a separate crate

Stories can exist without epics (a one-off story is valid). Epics and phases are affordances for coordinating multiple stories. Separating them from `agentic-story` keeps the story model clean and makes the dependency direction explicit: work depends on story, not the other way.

## Public API sketch

```rust
pub struct Epic {
    pub name: EpicName,          // e.g., "agentic-rebuild"
    pub objective: String,
    pub branch: Option<String>,
    pub stories: Vec<StoryId>,
    pub depends_on: Vec<EpicName>,
    pub status: EpicStatus,      // planning | active | completed | archived
}

pub struct Phase {
    pub id: PhaseId,
    pub name: String,
    pub agent: AgentRef,         // which agent runs this phase
    pub max_turns: Option<u32>,
    pub timeout: Duration,
    pub feedback_triggers: HashMap<String, PhaseId>,  // e.g., "TEST_FAILURE" -> "build"
    pub status: PhaseStatus,
}
```

## Dependencies

- Depends on: `agentic-story`, `agentic-agent-defs`, `agentic-events`
- Depended on by: `agentic-orchestrator`, `agentic-cli`, `agentic-store`

## Design decisions

- **Epic status is coarse.** No priority, no deferred-reason, no blocked-reason at this layer. Add them only if the orchestrator needs them.
- **Phase dependencies are declared via feedback triggers, not a DAG.** Simpler; matches how legacy actually worked. A DAG can come later as `_deferred/agentic-dependency/`.
- **Epics don't own stories exclusively.** A story can be referenced by multiple epics (migration, reuse). Stories are the primary key; epics are views.

## Open questions

- Do epics need their own lifecycle state machine like stories do? Initially no — derived from contained stories' states.
- Cross-epic dependencies — defer to `_deferred/agentic-dependency/` or include here?

## Stress/verify requirements

- An epic can contain 1000+ stories without performance degradation on list/status.
- Phase state transitions respect feedback triggers under concurrent updates.
