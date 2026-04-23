# 13 — Impact of Phase 0 on existing healthy stories

When Phase 0 stories 16–22 land, several existing healthy stories
will need amendments — their **contracts** change, not just their
context. This note catalogues the impact so the next session (and
`story-writer`) knows what will auto-revert to `under_construction`
as the Phase 0 pipeline advances.

The pattern is already established by story 11's guidance:

> "Story 1's YAML will need a small guidance update AFTER this story
> ships healthy — by a subsequent story-writer pass, auto-reverting
> story 1 to `under_construction` per the system design. That is
> correct behaviour; attempting to pre-edit story 1 would re-open it
> prematurely."

So: amendments land **after** the new Phase 0 story ships healthy.
The new story drives the old story's re-UAT cycle.

## Classification

Each existing story is marked:

- **Amend** — contract changes; story YAML + tests must be updated;
  auto-reverts to `under_construction` on amendment; re-UAT before
  returning to `healthy`.
- **Touch-up** — small guidance-only edit to reflect new context;
  no test change; no status change.
- **No change** — unaffected by Phase 0.

## Impact table

| Story | Title | Impact | Triggered by |
|-------|-------|--------|--------------|
| 1 | UAT signs verdict + promotes story | **Amend** | Story 18 (signer) |
| 2 | CI test-run recorder | **Amend** (light) | Story 18 (signer symmetry) |
| 3 | Four-status dashboard | **Amend** | Story 21 (retirement) |
| 4 | Store trait + MemStore | **Amend** | Story 20 (snapshot primitive) |
| 5 | SurrealStore | **Amend** | Story 20 (snapshot primitive) |
| 6 | Story YAML loader + schema + DAG | **Amend** | Stories 17, 21 (schema edits) |
| 9 | Dashboard staleness scoped to `related_files` | Touch-up | Story 21 (retired stories skip staleness) |
| 10 | DAG-primary frontier dashboard | Touch-up | Story 21 (retired filtered from frontier) |
| 11 | UAT ancestor-health gate | **Amend** | Story 21 (retired ancestors skipped) |
| 12 | Subtree-scoped test selector | No change | — |
| 13 | Unhealthy-ancestor classifier | Touch-up | Inherits story 11's new gate |
| 15 | `agentic test-build plan/record` | Touch-up | `build_config` awareness (plan context only) |

**Count:** 7 amendments, 4 touch-ups, 1 no-change, out of 12 live
stories.

## User direction on amendment cost (2026-04-23)

The user explicitly confirmed the posture for handling this cost:

> "Pay our costs upfront so they don't grow into weeds."
>
> "Preference for a single UAT — if we need to break things and they
> need to stay broken it's fine at this stage, we have no users so we
> can lean into eventual consistency."

Two direct consequences for the sequencing section below:

1. **Story 6 amends once, not twice.** The schema edits for
   `build_config` (from story 17) and `retired` + `superseded_by`
   (from story 21) are bundled into a single amendment pass on story
   6 — one auto-revert, one re-UAT. This means story 21
   (retirement) is promoted from Phase 0.5 into **Phase 0**, so both
   schema triggers land in the same phase.
2. **Eventual consistency replaces strict ordering.** We propose the
   full batch of new stories (16–21) together, let triggered existing
   stories auto-revert in parallel, and let the pipeline converge
   over whatever wall-clock it takes. Nothing needs to be "fixed
   first" as a hard prerequisite. Stories can sit red across the
   corpus while the dependencies settle.

## Detail per amendment

### Story 1 — add signer to the signing contract

**Current:** `uat_signings` row carries `verdict` + commit hash.

**After story 18:** every row also carries `signer: String`.

**What changes in story 1's YAML:**

- `acceptance.tests[]`: two test files need updates or new siblings:
  - `uat_pass.rs` — assert the signing row has a `signer` field
    equal to the resolved signer (from flag / env / git config).
  - `uat_fail.rs` — same.
- Guidance: add one paragraph describing the signer resolution chain
  and how it composes with the existing commit-hash contract.
- `depends_on`: add `18`.

**Sequence:** story 18 ships healthy first; then story 1 is amended;
auto-reverts to `under_construction`; re-UAT; back to `healthy`.

### Story 2 — signer on test_runs

**Current:** `test_runs` rows carry story id + test outcome + commit.

**After story 18:** rows also carry `signer` for symmetry with
`uat_signings`.

**What changes:** light edits to one test file + guidance.
`depends_on: [18]`.

### Story 3 — dashboard handles retired status

**Current:** dashboard surfaces `proposed | under_construction |
healthy | unhealthy`. Four statuses.

**After story 21:** `retired` is a new status. Default (frontier)
view hides it; `--canopy` / `--all-eras` mode shows it.

**What changes:**

- One new acceptance test pinning frontier-hides-retired.
- One new acceptance test pinning `--canopy` includes retired with
  era grouping by `superseded_by` chain.
- Guidance: add paragraph on era semantics.
- `depends_on`: add `21`.

Non-trivial amendment — story 3 was already re-scoped twice; this is
the third pass. Story-writer may decide to split into a sibling story
("dashboard canopy view") if it crosses a size threshold.

### Story 4 — Store trait gains snapshot/restore

**Current:** `Store` trait has `upsert`, `append`, `get`, `query`.
Four methods.

**After story 20:** trait gains `snapshot_for_story(id)` returning a
`StoreSnapshot` bundle of ancestor-closure signings, plus
`restore(snapshot)` that ingests one.

**What changes:**

- Two new acceptance tests:
  - `snapshot_for_story_returns_ancestor_closure.rs` — given a
    fixture story graph, the snapshot contains exactly the
    transitive-ancestor signings.
  - `restore_roundtrips_snapshot.rs` — `restore(snapshot)` followed
    by a gate query returns the seeded rows.
- Guidance: add a section on snapshot/restore and why it's story
  20's prerequisite.
- `depends_on`: unchanged (this is a trait extension; story 20 depends
  on story 4, not vice-versa). Actually the causal order is: story
  20's story-writer pass needs the trait method to already exist, so
  story 4 is amended BEFORE story 20 is even proposed. Revise: story
  4 amends first; THEN story 20 is authored.

This reorders implementation: story 4 amendment must ship before
story 20 can be authored as proposed. Story-writer flags this.

### Story 5 — SurrealStore implements snapshot/restore

**Current:** `SurrealStore` implements the four `Store` methods.

**After story 20:** must also implement `snapshot_for_story` +
`restore`. Trait-parity with `MemStore` is already pinned (story 4
tests re-run against `SurrealStore` per story 5's contract); the new
methods just get the same treatment.

**What changes:** two acceptance tests mirror story 4's new ones
against `SurrealStore`. Light guidance addition.

`depends_on`: unchanged (already depends on 4).

### Story 6 — schema additions

**Current:** `status` enum: `proposed | under_construction | healthy
| unhealthy`. Four values. Pinned by
`load_invalid_status_enum_is_rejected.rs`.

**After story 17 + story 21:**
- `status` enum gains `retired`. Five values.
- Story objects gain optional `build_config: { max_inner_loop_iterations,
  models }` (story 17).
- Story objects gain optional `superseded_by: <id>` (story 21),
  validated: target ID must exist; target must not also be retired
  with a chain back to self (cycle guard).

**What changes:**

- `schemas/story.schema.json` edit: both the status enum AND the new
  optional fields. Schema edit goes through whichever curator owns
  schemas; story-writer flags the dependency. (Same for story 3's
  dashboard change and story 11's gate change — they all follow the
  schema.)
- Acceptance tests need amendment:
  - `load_invalid_status_enum_is_rejected.rs` — update the invalid
    example (use e.g. `tested` which will remain invalid).
  - New test: `load_retired_status_is_accepted.rs`.
  - New test: `load_build_config_is_parsed.rs`.
  - New test: `load_superseded_by_pointing_at_unknown_id_is_rejected.rs`.
  - New test: `load_superseded_by_cycle_is_rejected.rs`.
- Guidance: significant addition on retirement semantics, build
  config defaults, and supersession chain validation.
- `depends_on`: add `17`, `21`.

This is the **biggest amendment**. Story 6 is the story-corpus
contract holder. Schema edits trigger downstream amendments in
stories 3 and 11.

### Story 11 — ancestor gate skips retired

**Current:** `AncestorNotHealthy` fires when any transitive ancestor
is not `healthy` with a valid `uat_signings.verdict=pass` row.

**After story 21:** retired ancestors are **satisfied** (not errors).
A story depending on a retired ancestor does not fail the gate on
that ancestor. The gate's `reason` sub-enum may gain a
`RetiredAncestorSkipped` audit-only variant or (simpler) just skip
retired ancestors silently during traversal.

**What changes:**

- New acceptance test:
  `uat_permits_retired_ancestor.rs` — given a chain where a mid
  ancestor is `retired` with valid `superseded_by`, the gate
  traverses past it and evaluates the next link as if the retired
  ancestor were satisfied.
- New acceptance test:
  `uat_refuses_when_retired_chain_points_at_unhealthy.rs` — given
  a retired ancestor whose `superseded_by` target is itself
  unhealthy, the gate surfaces the TARGET's failure (not the
  retired ancestor itself).
- Guidance: add a section on retirement semantics — "retired ≠
  failed; retired means the story was superseded and its successor
  carries the health responsibility."
- `depends_on`: add `21`.

**Design question for story-writer:** does the gate follow
`superseded_by` chains to check the successor's health, or does
`retired` just mean "skip, regardless of what replaced it"? My
lean: follow the chain. A story depending on `retired(superseded_by:
X)` is implicitly depending on `X`, and `X`'s health must be proven.
Worth naming explicitly.

### Story 15 — touch-up for build_config awareness

**Current:** `agentic test-build plan <id>` produces plan entries
with fixed top-level keys (file, target_crate, justification,
expected_red_path, fixture_preconditions).

**After story 17:** plan output may optionally include the story's
`build_config` (as context for the user/agent consuming the plan).
Purely additive; existing tests remain valid.

**What changes:** guidance touch-up; possibly one new small test if
we want to pin build_config's presence in plan output. No
contract-breaking change.

## Amendment sequencing — eventual consistency

Given the user direction above, the sequencing is **relaxed**. Instead
of strictly ordering "new story ships → triggered existing story
amends → re-UAT → next new story," we:

1. **Propose all Phase 0 new stories (16–21) as a batch.** Story
   21 joins Phase 0 (not 0.5) so the story-6 schema edits bundle.
2. **Amendments to existing stories fire in parallel** with the new
   stories' red-green cycle. Each triggered existing story
   auto-reverts to `under_construction` as soon as its trigger story
   is proposed (not only when the trigger ships healthy).
3. **Let the corpus converge over wall-clock time.** Multiple
   stories can sit red simultaneously. The dashboard's
   unhealthy-ancestor classifier (story 13) is our friend here — it
   surfaces the transitive red so nothing gets lost.
4. **Story 6 amends once.** Both schema edits (build_config for 17,
   retired+superseded_by for 21) bundle into a single story-6
   amendment pass. One auto-revert, one re-UAT.

Revised shape:

```
Phase 0 new stories (proposed as a batch):
  16 (runs schema)
  17 (build_config schema)
  18 (signer)
  19 (agentic-runtime)
  20 (agentic story build)
  21 (retirement lifecycle)

Amendments to existing stories (fire in parallel):
  1  ← signer field (triggered by 18)
  2  ← signer symmetry (triggered by 18)
  3  ← dashboard retired + canopy view (triggered by 21)
  4  ← Store snapshot primitive (triggered by 20)
  5  ← SurrealStore snapshot impl (triggered by 20)
  6  ← schema: build_config AND retired+superseded_by bundled
        (triggered by 17 AND 21 — one amendment pass)
  11 ← gate skips retired ancestors (triggered by 21)

Touch-ups (guidance-only, no re-UAT):
  9, 10, 13, 15

Phase 0.5 new stories:
  22 (cloud-compatible Store impl)

Phase 0.5 amendments:
  (none — 22 is additive; it adds a new Store impl that the trait
  parity tests from 4+5 cover automatically)
```

**What falls out of this:**

- Phase 0 scope grows from 5 stories (+ runtime un-defer) to 6.
- Phase 0.5 shrinks to just the cloud-Store swap.
- Story 6 re-UATs once, not twice.
- The corpus spends more wall-clock time in a partially-red state.
  Acceptable at zero-user stage per user direction.
- Nothing requires a strict "before X is authored" precondition; the
  batch proposes together.

## Schema edit coordination

Three schema edits land, all in `schemas/story.schema.json`:

1. Add `build_config` object (optional) — bundled with story 17.
2. Add `"retired"` to status enum + `superseded_by` integer field
   (optional, with validators) — bundled with story 21.
3. **Any other additions?** None identified. `signer` lives in Store
   rows, not in the story YAML, so it's not a schema edit.

Schema edits go through whichever curator owns schemas. Per CLAUDE.md
the schema is authoritative — story-writer MUST NOT edit it
unilaterally; they declare the dependency and the schema curator (or
the human) performs the edit synchronously with the story promotion.

## Retroactive backfill

Story 21 (retirement) also triggers a one-time backfill:

- Convert hard-deleted stories 7, 8, 14 back into on-disk YAMLs with
  `status: retired` + `superseded_by: <successor>` (7 → 15, 8 → 1+3,
  14 → 15 per note 02's retirement log).
- This is not a story; it's a data-migration operation that runs
  alongside story 21's landing.

## Touch-ups (no-status-change amendments)

Story 9, 10, 13, 15 get guidance-only edits. No tests change. No
auto-revert. Story-writer adds one paragraph per story to document
the new composition (e.g. "retired ancestors are treated as satisfied
by the gate this classifier inherits from").

## What this note is NOT

- **Not a commitment to the exact amendment shapes.** Each row above
  is a story-writer authoring target, not a pre-baked diff. The
  curator may split, bundle, or scope differently.
- **Not a schedule.** The sequencing above is a dependency order, not
  a calendar. Stories land as the pipeline produces them; the
  amendment cycles fire when their trigger stories ship healthy.
- **Not a substitute for each story's own guidance.** When an
  amendment is proposed, the story's own YAML explains its shape;
  this note just says which stories are in the amendment set.

## What next session does with this note

- Present to the user for confirmation that the 6-amendment / 4-
  touch-up / 1-no-change shape matches their expectation.
- Thread the amendment expectations into the `story-writer` brief
  when proposing stories 16–22.
- Use the sequencing as a checklist for the orchestrating session:
  after each new story lands healthy, run the appropriate amendment
  pass against the triggered existing stories.
