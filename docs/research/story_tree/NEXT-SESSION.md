# NEXT-SESSION brief (revised 2026-04-22)

Start here. This is the action-list for whoever picks up the story_tree
research next.

## State at time of writing

- Branch: `main`
- All 12 live story YAMLs currently healthy (IDs 1–6, 9–13, 15;
  retired IDs 7, 8, 14 still hard-deleted pending story 21)
- `dag-primary-lens` epic complete
- Claude-as-user architectural realignment complete
- `agentic test-build plan|record` (story 15) shipped + amended
- Tree clean
- Research folder has notes 01–12 and this brief; **the research
  has converged**. Phase 0 / 0.5 / 1 shape is ideation-ratified by
  the user in the 2026-04-22 ideation session.

## Sessions 2026-04-22 + 2026-04-23 outcomes

Large ideation session with the user reshaped the Phase 1 direction
from cloud-first to **Docker-local-first**. Key shifts:

- **Phase ladder (Phase 0 / 0.5 / 1)** introduced. Phase 0 = local
  Docker, single container. Phase 0.5 = docker-compose, two
  containers. Phase 1 = GCP, but only enough to prove "works in the
  cloud" — not fanout.
- **Cloud Workstations rejected** entirely. Dev works on their laptop.
- **Story-hardening loop** named as the real primitive. Inner loop =
  build-test-uat inside one sandbox. Outer loop = human-in-the-loop
  amendment on failure. Budget + model selection declared in
  `build_config` on the story.
- **Observability is the upstream prerequisite.** `runs` table +
  NDJSON trace. Story 16 is the first Phase 0 story for this reason.
- **`agentic-runtime` un-defers** — it's needed for Phase 0 (claude
  subprocess + trace capture).
- **Store snapshot/restore** emerged as a new primitive (for seeding
  ancestor signings into a fresh embedded Store inside the sandbox).
- **External-patterns research confirmed user's suspicion:**
  sandboxing is commoditising; the story tree + reproducibility-from-
  spec-alone is the genuine research bet (see note 12).
- **Existing-stories impact analysis** (note 13, session
  2026-04-23): of 12 live healthy stories, **6 need amendments**
  (stories 1, 2, 3, 4, 5, 6, 11), **4 need touch-ups** (9, 10, 13,
  15), **1 no change** (12). The amendment pattern follows story 11's
  precedent: new Phase 0 story ships healthy → triggered existing
  story auto-reverts to `under_construction` → re-UAT. See note 13
  for the full sequencing.
- **Research folder cleanup** (session 2026-04-23): status banners
  added to superseded notes (04, 06, 07, 08) pointing future agents
  to the current state. README rewritten to separate current /
  context / historical notes.
- **Git coordination model** (note 14, session 2026-04-23):
  story-tree branches ≠ git branches; git branches are whole-tree
  snapshots; Phase 0 auto-merges sandbox branches on green with no
  human review gate; bad merges are the research signal that
  forces the recovery + gating work in Phase 2+.

## Stories to author (revised 2026-04-23 — eventual consistency)

Candidate IDs. Per user direction *"pay costs upfront; single UAT;
lean into eventual consistency,"* **all six Phase 0 stories are
proposed together as a batch.** No strict ordering between them.
Amendments to existing stories fire in parallel.

| id | title | phase | depends_on |
|----|-------|-------|-----------|
| 16 | Run-trace persistence + `runs` schema | Phase 0 | 4, 5 |
| 17 | `build_config` on story schema | Phase 0 | 6 |
| 18 | Signer identity on runs + signings | Phase 0 | 1, 16 |
| 19 | `agentic-runtime` un-deferred | Phase 0 | 16 |
| 20 | `agentic story build <id>` | Phase 0 | 11, 15, 16–19 |
| 21 | Retirement lifecycle | Phase 0 | 6, 11 |
| 22 | Cloud-compatible Store impl | Phase 0.5 | 4, 5, 16, 18 |

**Story 21 moved into Phase 0** so the two story-6 schema amendments
(build_config from 17; retired + superseded_by from 21) bundle into
a single story-6 re-UAT.

ID-reuse policy: **never reuse retired IDs.** Start at 16; leave 7,
8, 14 permanently vacant.

See `10-phase1-story-outlines.md` for the full outlines and
`13-existing-stories-impact.md` for the amendment plan.

## Decisions locked in

From `09-tier1-resolutions.md` (revised):

- **Phase 0 sandbox:** single Docker container, embedded SurrealStore,
  story mounted at run time, claude creds mounted read-only, runs/
  volume mounted out.
- **Ancestor gate:** option (a) — at container start, seed the
  embedded Store with ancestor signings from a mounted snapshot.
  Preserves story 11 semantics inside the sandbox.
- **Green criterion:** BOTH tests passing AND `agentic uat --verdict
  pass` exits 0. Either failing → continue or exhaust.
- **Sandbox signer identity convention:** `signer: "sandbox:<model>@
  <run_id>"`. Humans unchanged (email).
- **`agentic story build <id>`:** always launches a container from
  Phase 0 onward. No in-process escape hatch.
- **Cloud Store:** Phase 0.5+. Phase 0 uses embedded.
- **Cloud compute:** Phase 1 uses Cloud Build or Cloud Run jobs.
  Never Cloud Workstations.

## Three workstreams, in order

1. **Commit the research folder** as a checkpoint. The sketchpad has
   converged; committing marks the ideation phase complete and
   separates it from the authoritative-artefact phase that follows.
2. **Author ADR-0006** per note 11 + ADR-0003 amendment. Stories
   16–21 will cite ADR-0006 in their guidance blocks.
3. **Invoke `story-writer`** with `10-phase1-story-outlines.md` and
   `13-existing-stories-impact.md` as directive input. Propose all
   six Phase 0 stories (16–21) as a batch plus the seven existing-
   story amendments. Story 22 follows in Phase 0.5.

Parallel infra track (not story-shaped, doesn't block authoring):

- `infra/sandbox/Dockerfile`, reproducible build.
- `infra/sandbox/compose.yml` for Phase 0.5.
- `infra/gcp/` Terraform module for Phase 1 (much later).
- Guides under `docs/guides/`: local sandbox run, BYO creds,
  reading a run row.

## Amendment flow (revised 2026-04-23 — eventual consistency)

Per user direction, sequencing relaxes to "propose the batch; let the
pipeline converge." No waiting. Multiple stories can sit red
simultaneously; the dashboard's classifier (story 13) surfaces the
transitive red.

```
Phase 0 batch — new stories proposed together:
  16  runs observability       → no existing amendments
  17  build_config schema      → triggers story 6 amend (BUNDLED with 21)
  18  signer identity          → triggers story 1, 2 amends
  19  agentic-runtime          → no existing amendments
  20  agentic story build      → triggers story 4, 5 amends (snapshot)
  21  retirement lifecycle     → triggers story 3, 6, 11 amends
                                 + touch-ups to 9, 10, 13

Story 6 amends once. Schema edit bundles build_config (from 17) AND
retired + superseded_by (from 21). One auto-revert, one re-UAT.

Phase 0.5:
  22  cloud-compatible Store   → no existing amendments (additive)
```

**Impact count:** 7 existing stories amend, 4 touch up, 1 no-change,
of 12 live stories. See `13-existing-stories-impact.md` for the
per-story breakdown.

## Conversational context to preserve

Verbatim from the 2026-04-22 session where direction shifted:

- **"Works in the cloud is what we want as a proof not scale in the
  cloud."** This is the Phase ladder's north star.
- **"I'm thinking we start by building as a docker image locally
  first?"** Phase 0 origin.
- **"Cloud workstations are expensive we should leverage local where
  we can."** Workstations rejection origin.
- **"The container would take our whole build system with it."** The
  bake-in-everything insight; matches legacy system's bundling shape.
- **"The sandbox needs to prove itself — we have this system to ensure
  max reliability that can be derived from the agents."** Why
  reproducibility-from-story-alone is the healthy gate.
- **"Observability — the core to this working is our system building
  its own observability and guardrails to ensure agents succeed,
  without observability I can't direct the system to build the right
  guardrails."** Why story 16 leads.
- **"A human is going to seed the story so as part of that they can
  also guestimate how many loops it would take … basically a story
  branch build config that could even set what models we want to
  use."** Why `build_config` is Phase 0 scope.
- **"I suspect our story tree is the real innovation here."**
  Confirmed by note 12's survey. Name this explicitly in ADR-0006.

From earlier sessions (still load-bearing):

- **"The innovation is our story tree."**
- **"1 human but with cloud scalability."**
- **"Keep costs under control"** — $10s/mo not $100s.
- **"I'm a gcp data engineer."**
- **"Cleanest foundation over stability."**
- **"To really prove the stories an agent should be able to build
  using only the story."**

## Reading order for fresh context

If you are a fresh Claude Code session picking this up with no prior
conversation context:

1. Read `README.md` (top of repo) — current status.
2. Read `CLAUDE.md` — driving rules.
3. Read `stories/README.md` — story corpus.
4. Read `docs/decisions/0001` through `0005` — the five ADRs.
5. Read `docs/research/story_tree/README.md` — this folder's index.
6. Read numbered notes in order (`01` through `12`).
7. Finally, read this file.

Total time: ~60 minutes with the expanded notes. Then full context.

## Things this session did NOT do

Named explicitly so next session knows what's still pending:

- Did not author stories. Outlines only (note 10).
- Did not author ADR. Outline only (note 11).
- Did not write any code, Rust or Terraform or Dockerfile.
- Did not touch `stories/*.yml` or schema files.
- Did not commit anything.
- Did not confirm decisions with the user in the strict sense — they
  are user-ratified in conversation but not ratified by being in
  authoritative corpus artefacts.

## How to start the next session (concrete)

Verbatim starting prompt suggested for the user:

> "Walk me through the revised `docs/research/story_tree/09`, `10`,
> `11`, and the new `12`, confirming the Phase 0 shape. If I confirm,
> invoke `story-writer` with `10-phase1-story-outlines.md` as
> directive input to propose stories 16, 17, 18, 19, 20. Then author
> ADR-0006 + the ADR-0003 amendment per the outline in note 11.
> Commit the research folder itself as a separate commit before
> starting story-writer."

## Troubleshooting for the next session

If anything in this research folder seems outdated or wrong, trust
the authoritative artefacts in this order:

1. `stories/*.yml` — current contracts
2. `CLAUDE.md` — current driving rules
3. `docs/decisions/*.md` — current ADRs
4. `README.md` — current project state
5. `schemas/*.json` — current shapes

The research folder is a sketchpad; it drifts. If it disagrees with
the authoritative artefacts, the authoritative artefacts win.

## Sign-off

Research folder extended in session 2026-04-22 with:
- Notes 09, 10, 11 rewritten in full.
- New note 12 (external-patterns survey).
- This brief refreshed.

All live stories still `healthy`. The gap between research and
implementation is now the smallest it has been. One confirmation
session, then `story-writer` walks the Phase 0 outlines into proposed
YAMLs, and the test-builder → build-rust → test-uat pipeline starts.

Good luck.
