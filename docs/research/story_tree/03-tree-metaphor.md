# 03 — The tree metaphor

## Origin of this thread

The user introduced the metaphor in session 2026-04-20. Their words:

> "with our story dag system i'm envisioning a tree, however trees let
> branches die and are okay if branches compete with each other and
> exist in the same space. should we do the same in our system, how
> does this look?"

The metaphor is doing a lot of useful work, so it's worth unpacking
carefully. It decomposes into three distinct behaviours that biological
trees exhibit but our current story DAG does not.

## The three behaviours

### Behaviour 1 — branches die

Biological trees prune:
- Shading (another branch claimed the light)
- Disease (the branch failed some health check)
- Resource starvation (the root system deprioritised it)

Dead branches stay part of the trunk's structural history but don't
consume active resources.

**Current system:** retirement is destructive surgery. We deleted
`stories/7.yml`, `stories/8.yml`, `stories/14.yml` via `git rm`. The
commits survive but the story corpus pretends they never existed. The
`uat_signings` row for story 14 at commit `cb163e2` is orphan data —
a fossil with no story to attribute to.

**Tree version:** `retired` as a first-class lifecycle status, joining
`proposed | under_construction | healthy | unhealthy`. Retired stories:
- Stay in `stories/` on disk (tests may be archived or deleted).
- Are skipped by story 11's ancestor gate (retired ≠ failed proof).
- Are hidden by the dashboard's `frontier` view, shown in a
  `--canopy` / `--all-eras` mode.
- Carry `retired_reason` and `superseded_by: <story_id>` metadata.
- Their `uat_signings` rows become fossil record, not cruft.

**What this unlocks:** honest history. *"We tried claude-as-component
in story 14; it was the legacy-system mistake one layer down; we
retired it on 2026-04-20 in favour of story 15."* Currently that
narrative lives scattered across commit messages and a transient audit
file (`docs/reviews/claude-as-user-audit.md`).

### Behaviour 2 — branches compete

Multiple buds sprout in one region. The one that captures light most
efficiently grows; the others stall or die. Darwinian at the branch
level, without central planning.

**Current system:** one story per contract. If we want to try two
architectural approaches, we write two stories but their contracts
conflict — only one can ship. We saw this with story 14 (claude-as-
component) vs story 15 (claude-as-user): we had to retire 14 before 15
could land. The process took a coordinated multi-subagent session
(audit → draft → impl → UAT → retire). It worked but was fragile.

**Tree version:** `alternatives_to: [<story_id>]` relationship. Two
stories can pin conflicting observables simultaneously, both
`under_construction`. They compete. Mechanisms:
- A "competition cohort" shared id rendered side-by-side.
- Implementations live on separate branches until one is selected.
- Tests run against both; the one with cleaner red-green and healthier
  UAT wins.
- When one is promoted, the other auto-transitions to `retired` with
  `superseded_by:` pointing at the winner.

**What this unlocks:** real spike / experiment work. Currently, trying
two approaches means picking one to ship and reverting if it fails —
expensive. With competition, fork a cohort, let agents build both,
pick the winner.

**What breaks:** the red-green contract gets fuzzy for competitive
cohorts ("which contract is THE contract?"). The gate logic becomes
more complex. Test isolation has to be strict — two competing impls
can't both edit the same `src/` file on main.

### Behaviour 3 — branches coexist in the same space

Trees at different ages coexist. Old bark stays; new growth is just
where the living happens. Like tree rings — old rings are preserved,
they just aren't where sap flows.

**Current system:** when a contract evolves, we re-scope the existing
story. Story 3 got re-scoped twice — once when frontier-default landed,
again when we dropped the frontier-incompatible tests. Each re-scope
blurred the story's identity.

**Tree version:** stories belong to **eras**. When a contract changes,
author a new story in a new era, and the old story transitions to
`retired (superseded_by: <new_id>, era: <old_era>)`. Old tests survive
as archived reference; new tests drive main.

Implementation note: eras are implicit in `superseded_by:` chains. Don't
add an `era:` field — the chain IS the era history. A story's era is
"the chain of supersession it sits in." Dashboard can group by chain.

## Why this metaphor is more than decorative

The current system is **Newtonian**: deterministic, controllable,
one-impl-per-story, deletion is unusual. Good for known problems.

A tree is **Darwinian**: variation, selection, death, redundancy,
imperfect-but-adaptive. Good for unknown problems.

These aren't exclusive. The current approach works well for foundations
(store, schema, runtime). The tree approach works better for frontier
exploration (new CLI flags, architectural experiments, UX research).

The cloud sandbox direction extends this: **sandboxes are the
mechanism that makes real branch divergence possible.** On a single
local tree, two competing impls step on each other. In separate
sandboxes, they can truly coexist until one is selected.

## What's concrete vs speculative

| Concept | Maturity | Do-now? |
|---------|----------|---------|
| `retired` status | Clear design, small schema change | Yes — maybe 1 story |
| `superseded_by` metadata | Additive, low risk | Yes — bundle with `retired` |
| `--canopy` / era view in dashboard | Clear, additive | Yes — bundle |
| Ancestor gate skips retired | Small code change in story 11 | Yes — bundle |
| `alternatives_to` / competition cohorts | Needs concrete case | **Defer** — build on real need, not speculation |
| Auto-retirement (unused-for-N-sessions) | Speculative | **Defer** |

## Relationship to cloud sandboxes

The `retired` work is useful independently of cloud. It's also necessary
FOR cloud: in a world with many ephemeral sandboxes, branches spawn
casually. Pruning discipline matters more, not less. A sandbox that
produces a failed experimental story should retire the story cleanly,
leaving a navigable record, not a delete.

## OPEN: metaphor vs mechanism

The tree analogy suggests the system should have **less central control
over story creation, not more.** What does less control mean concretely?

- Agents can fork competitive stories without human approval, up to a
  limit?
- Retirement happens automatically when a story has been `proposed` for
  > X sessions with no movement?
- The dashboard surfaces "this cohort has been competing for N
  sessions; consider promoting one"?

This is speculative territory. Worth naming so we know when we're
taking the *shape* of the metaphor vs just its *vocabulary*. The
current recommendation is: take the vocabulary (retirement,
supersession, era, competition) but keep centralised control. Revisit
if the agent fanout genuinely exceeds human curation bandwidth.

## Files to read for more on this

- `docs/reviews/claude-as-user-audit.md` — the audit that led to the
  story 14 retirement. Shows what destructive retirement looks like.
- Session transcript (ask user for location) of 2026-04-20 for the
  original ideation exchange.
- `patterns/standalone-resilient-library.yml` — the pattern the user
  extended this session with the "no LLM subprocess inside the library"
  clause. Shows how non-authoritative-but-durable patterns live
  alongside the story corpus.
