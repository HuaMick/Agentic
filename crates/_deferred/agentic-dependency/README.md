# agentic-dependency (deferred)

## What it will be

Cross-epic dependency graph. Priority-aware scheduler that decides which epic is ready to work on based on blockers, priority, and completion state.

## Why deferred

Day one, we work one story at a time, one epic at a time. No DAG needed.

This earns its place when:

1. We have 3+ concurrent epics and real dependencies between them.
2. Priority matters because work queue exceeds capacity.
3. Cross-epic blockers need to be tracked explicitly rather than discovered at failure time.

## What it would look like

- Graph construction from epic `depends_on` fields.
- Cycle detection (reject invalid graphs at load time).
- "Next ready epic" query given current state.
- Priority-aware topological sort.

## Trigger to build

When we have multiple active epics AND one of them blocks on another in a way that wastes agent time.
