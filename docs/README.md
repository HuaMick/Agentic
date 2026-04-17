# docs/

Project documentation.

## Layout

- `architecture/` — how the system is put together, at a conceptual level.
- `guides/` — how to do specific things (author a story, write an agent, run verify).
- `decisions/` — Architecture Decision Records (ADRs). Numbered, immutable once accepted, superseded by later ADRs when overturned.

## ADR format

```
# ADR-NNNN: <title>
Status: proposed | accepted | superseded by ADR-XXXX
Date: YYYY-MM-DD

## Context
[What problem are we solving? What forces are at play?]

## Decision
[What did we decide?]

## Consequences
[What becomes easier? What becomes harder? What did we give up?]
```

## What lives here vs in crate READMEs

- Crate README = "what this crate does, why it exists, its public API, its design decisions **scoped to this crate**."
- ADR = "a cross-cutting decision affecting multiple crates or the whole system."
- Guide = "a how-to for a human or agent to accomplish a specific task."
- Architecture doc = "a mental model of how the system fits together."

When in doubt: per-crate decisions go in that crate's README; anything else is an ADR.
