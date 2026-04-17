# agentic-pattern-enforcement (deferred)

## Not to be confused with `patterns/`

The top-level [`patterns/`](../../../patterns/README.md) directory holds **reusable design guidance** — author-facing `.yml` files that stories reference in their `guidance` sections (e.g. `lazy-loading.yml`, `append-only-log.yml`). That is content, not code, and it already exists as a day-one concept.

This crate is different. It is a **verification-pattern enforcement engine**: machinery that runs cross-cutting checks over every story that claims a named pattern. Same word, different job.

| | `patterns/` (day one) | `agentic-pattern-enforcement` (deferred) |
|---|---|---|
| What | Design-guidance YAML | Rust crate that enforces rules |
| Audience | Story authors | `agentic-verify` pipeline |
| Example | "lazy-loading: here is how to think about deferred init" | "every HTTP handler story must test 2xx/4xx/5xx" |
| State | Alive, referenced by `guidance:` | Not built |

## What this crate will be

A verification-pattern engine. Stories opt in to a pattern by claim:

```yaml
# story.yml
claims:
  - http-handler-tests
```

The engine then enforces templated acceptance criteria across every claiming story:

- `agentic pattern verify <pattern>` — run the pattern's checks against every story that claims it.
- `agentic pattern audit` — flag stories that probably should claim a pattern but don't.
- `agentic pattern list` — enumerate defined verification patterns.

Pattern definitions would live in a new subtree (e.g. `verification-patterns/`) parallel to `patterns/`, with a distinct schema emphasizing executable checks rather than design prose.

## Why deferred

Day one, every story writes its own acceptance criteria in full. No inheritance, no centralized enforcement. This is simpler, forces authors to think about each criterion, and avoids building infrastructure for a problem we haven't seen yet.

## Trigger to build

When we see duplicate verification logic across stories that would benefit from centralized enforcement — specifically, the third time the same acceptance-criteria shape is copy-pasted across unrelated stories, or the first time someone wants to retrofit a cross-cutting rule (e.g. "all public APIs have error-case tests") across the whole corpus.

Until then, repetition in acceptance criteria is accepted as a signal, not a problem.
