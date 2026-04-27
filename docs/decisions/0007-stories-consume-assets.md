# ADR-0007: Stories consume assets

**Status:** accepted
**Date:** 2026-04-27

## Context

Cross-corpus concepts — principles, definitions, guidelines — are
needed by both agents and stories. Agents reference them via
`inputs.yml required_reading:` against files under `agents/assets/`.
Stories cannot reference them: the story schema has a `patterns:` field
(referencing `patterns/<id>.yml`, story-specific design guidance) but
nothing for assets. Story authors who need a concept already in the
asset corpus — for example, the deletion-test heuristic in
`agents/assets/principles/deep-modules.yml`, or the canonical SHA shape
in `agents/assets/definitions/identifier-forms.yml` — must inline the
concept's prose into `guidance:` text and cite the asset's path
literally as English.

This is the same fragmentation the deep-modules asset itself rails
against, applied to spec content rather than code:

- The concept's body lives in two places (the asset, plus the inlined
  prose in each story that needs it). The deletion test fails — the
  asset *is* earning its keep, but with no shared reference mechanism
  the story-side copy is forced.
- Cross-corpus consumer reciprocity is invisible. An asset's
  `current_consumers:` list captures agent consumers; story consumers
  are unrepresented. An audit cannot detect "this asset is read by
  story X but X never declared it."
- Guidance text grows. Story 26's draft (the deep-modules-driven
  test-support extraction) inlines the REWARD-HACKING GUARDRAIL
  paragraph from the asset; without an `assets:` field, the guidance
  must repeat the asset content rather than reference it.

The user's framing in the session that produced this ADR: "stories can
also abstract concepts away ... using the guidance asset system they
can also be more condensed ... creating clarity customised to our
system or new concepts that only need to be explained once."

## Decision

**Stories consume assets through the same mechanism agents do, with a
new schema-validated field.**

1. **Story schema gains an `assets:` field.** Array of asset paths
   (repo-root-relative), defaults to `[]`. Items match
   `^assets/.*\.ya?ml$`. Stories declare assets the same way
   they declare patterns — alongside `patterns:`, not merged with it.
   Patterns and assets stay separate concepts: patterns are story-
   specific design templates with their own schema; assets are cross-
   cutting concepts (principles, definitions, guidelines).

2. **Asset schema's `current_consumers:` regex is widened.** Extended
   from agent-spec triplet paths only to also accept
   `^stories/[0-9]+\.yml$`. The combined pattern:
   `^(agents/[a-z][a-z0-9-]*/[a-z][a-z0-9-]*/(contract|inputs|process)\.yml|stories/[0-9]+\.yml)$`.
   At least one consumer remains required.

3. **Loader (`agentic-story`) validates `assets:` entries resolve.**
   Every entry in a story's `assets:` array must point at an existing
   asset YAML on disk at parse time. A missing asset is a load-time
   defect, equivalent to a missing `patterns:` reference.

4. **Cross-corpus reciprocity is a corpus-level invariant.** For every
   asset that lists a story in `current_consumers:`, that story's
   `assets:` field must reference the asset; and conversely, for every
   story that declares an asset, the asset's `current_consumers:` must
   list the story. A reciprocity-audit fn (story 27) catches drift in
   either direction.

5. **Asset paths.** ~~The `agents/assets/` directory stays put for
   now.~~ The directory was renamed to top-level `assets/` on
   2026-04-28 — see "Decision update" at the bottom of this ADR. The
   `agents/` prefix understated the reach once stories became
   consumers; the new path matches the semantics.

## Migration

Story 27 is the meta-story that ships this change. Its acceptance
tests prove (a) loader accepts the new field, (b) loader rejects
unknown asset paths, (c) asset schema accepts story-path consumers,
(d) the reciprocity invariant holds, (e) every asset has at least one
consumer.

After story 27 is healthy:

- Existing stories MAY migrate inlined references into `assets:`
  declarations as a follow-up. There is no forced migration; existing
  stories work as-is until amended for unrelated reasons.
- New stories use the mechanism by default.

## Alternatives considered

**Merge `patterns/` into `agents/assets/`.** Rejected. Patterns and
assets have different schemas (`pattern.schema.json` requires
`when_to_use:`, `assets/` permits arbitrary additional properties per
category). Merging forces one shape to lose information. Keeping them
sibling is cleaner.

**Move `agents/assets/` to top-level `assets/` now.** ~~Deferred.~~
**Done 2026-04-28** — see "Decision update" below. The original
deferral cited path semantics as cosmetic and the rename cost as real.
The user reversed this on 2026-04-28 under a "address foundational
work now, don't defer" directive: while still in foundational mode
the path drift compounds for every new consumer (every new story's
`assets:` field, every new agent's `required_reading:` block) and the
rename only gets more expensive over time, not less.

**Grow `patterns:` to also accept asset paths.** Rejected. The schema
field's name would lie about its contents; the audit semantics would
need to fork by referenced-file extension. Two named fields (`patterns:`
and `assets:`) is clearer than one polymorphic field.

**No schema change; stories cite assets in prose only.** Rejected.
This is the current state; it is exactly what the deep-modules
deletion test fails on. The asset corpus's coverage of cross-cutting
concepts is real, but without a structured reference mechanism the
story side cannot harvest it.

## Consequences

**Gained:**

- Story guidance shortens. A story that needs the deletion-test
  heuristic declares `assets: [assets/principles/deep-modules.yml]`
  rather than inlining the body.
- Cross-corpus reciprocity becomes machine-checkable. The audit fn
  catches drift in either direction.
- The asset corpus visibly serves both halves of the system. The
  `current_consumers:` list of a widely-used asset reveals its real
  reach.

**Given up:**

- Two more places to remember (story author must check both
  `patterns:` and `assets:`). Mitigated by schema validation —
  forgetting one produces a load-time error, not a silent gap.

**Revisit when:**

- A pattern's `when_to_use:` collapses to "see the deletion test in
  deep-modules." That is a signal patterns and assets are merging
  conceptually for that case; consider a pattern → asset migration on
  a per-pattern basis.

## Decision update — 2026-04-28

The original decision (point 5) deferred renaming `agents/assets/` to
top-level `assets/`. The deferral has been reversed.

**What changed.** The directory was moved with `git mv`, every
`required_reading:` reference in agent specs was updated, every story
declaring an asset had its `assets:` field path rewritten, the schemas'
regex patterns shifted from `^agents/assets/.*\.ya?ml$` to
`^assets/.*\.ya?ml$`, and the `scripts/verify/asset_consumer_minimum.sh`
script was updated to match.

**Why now.** The original deferral cited cosmetic upside vs real
rename cost. Two facts pushed the trade-off:

1. The system is in a foundational phase (Phase 0 / Phase 2). Every new
   story that declares assets and every new agent that references them
   compounds the path drift; the rename only gets more expensive over
   time, not less. The user's directive on 2026-04-28: "all our work
   atm is foundational; we should be addressing things now and not
   deferring as a general rule."
2. The `agents/` prefix understates the reach. ADR-0007 itself widened
   the consumer class to include stories — calling the directory
   `agents/assets/` after that mismatched the schema's accepted
   consumers.

**Subdirectory addition.** A `principles/` subdirectory was sanctioned
during the rename (it had been added implicitly when `deep-modules.yml`
landed; `agents/assets/README.md` had not documented it). The moved
`assets/README.md` now lists `principles/` alongside `definitions/`,
`guidelines/`, `examples/`, `templates/`. A principle is a cross-cutting
design heuristic ("how to judge") distinct from a definition ("what
something is") or a guideline ("what to do"). `deep-modules.yml` is
the canonical example.

## Related

- `assets/principles/deep-modules.yml` — the asset whose
  cross-corpus need motivated this ADR.
- `stories/27.yml` — the meta-story shipping this change, with
  acceptance tests that prove the schema + loader + reciprocity
  invariants.
- `stories/26.yml` — the first story to use `assets:` from authoring
  time, demonstrating the abbreviated guidance shape.
- `schemas/story.schema.json`, `schemas/asset.schema.json` — the
  schemas updated by this ADR.
- ADR-0005 (`docs/decisions/0005-red-green-is-a-contract.md`) — same
  pattern: a corpus-level invariant enforced through schema +
  validation, not just convention.
