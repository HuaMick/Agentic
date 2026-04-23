# 09 — Tier-1 resolutions (revised)

Lock-in for the four tier-1 open questions from `08-open-questions.md`,
**revised in the 2026-04-22 ideation session** after user pushback on
the original cloud-heavy framing.

Original recommendations (SurrealDB Cloud vs self-host, Cloud Workstations,
etc.) treated Phase 1 as a production-shaped cloud rollout. That was the
wrong optimisation target. The user reframed it:

> "Works in the cloud is the proof, not scale. Keep it light for Phase 0,
> prove this works in a local docker container first."

This note replaces the earlier version in full. The decisions below are
**user-confirmed** (not just proposed).

## Summary

| # | Question | Revised decision |
|---|----------|------------------|
| Q1 | Phase 1 scope | **Reshaped into Phase 0 / 0.5 / 1 ladder** (below). Phase 0 is local-Docker-only. No cloud spend. |
| Q2 | SurrealDB posture | **Embedded in Phase 0** (ephemeral, inside container). Server/GCE only in Phase 0.5+. |
| Q3 | Sandbox compute | **No Cloud Workstations.** Phase 0 = local Docker. Phase 1 eventually = Cloud Build or Cloud Run jobs. **Dev always works on their laptop** — the cloud is for headless agent runs only. |
| Q4 | Claude auth in cloud | BYO credentials mounted into the sandbox (unchanged in spirit; irrelevant until Phase 1). |

## The Phase ladder

The old binary "local vs cloud" framing collapsed. The new shape is
three small steps, each with a specific proof obligation:

### Phase 0 — Local Docker, single container

- **Goal:** prove the inner loop works, observably, reproducibly,
  locally.
- **Shape:** one Docker image bundling the whole build system.
  `agentic story build <id>` on the host launches the container.
  Story YAML mounted in; claude credentials mounted in; `runs/` volume
  mounted out. Embedded `SurrealStore` inside the container — ephemeral.
- **Deliverable:** a dev can run `agentic story build <id>` on any
  proposed story and either (a) get a signed run row in `runs/<id>/`
  attesting green, or (b) get a failure-trace run row showing where
  the agent got stuck. No cloud cost. No cloud dependency.
- **Scope boundary:** no outer loop yet. Inner loop terminates; human
  inspects the run output and manually decides next steps.

### Phase 0.5 — docker-compose, two containers

- **Goal:** prove the cloud-ready data path (separate Store process)
  works.
- **Shape:** Phase 0's sandbox container + a SurrealDB server container,
  orchestrated via `docker-compose.yml`. The sandbox talks to the
  Store over the docker network. Cloud Store impl (`CloudSurrealStore`
  or similar) exercised end-to-end locally.
- **Deliverable:** runs persist in the separate SurrealDB container,
  not in the sandbox's embedded file. The separation is the proof.
- **Cost:** still zero cloud spend.

### Phase 1 — GCP

- **Goal:** prove "works in the cloud" — compatibility, not scale.
- **Shape:** same image tagged and pushed to Artifact Registry.
  Cloud Build or Cloud Run jobs execute one sandbox at a time.
  SurrealDB on a small GCE `e2-small` (or `f1-micro` free tier).
  No Cloud Workstations. No interactive cloud dev.
- **Deliverable:** one run of `agentic story build <id>` executed in
  the cloud, writing a signed run row to the cloud Store. Then stop.
  Fanout (multiple concurrent sandboxes, outer loop orchestration)
  is Phase 2, not Phase 1.
- **Cost envelope:** $10s/mo. Phase 1 proof can probably be done for
  under $5 by running the GCE box only when demonstrating.

## Q1 resolution in detail — Phase 0 stories

The original "four stories" list from the old note 09 has been
superseded. See `10-phase1-story-outlines.md` for the revised list.
Phase 0 now holds **six new stories** (16–21), proposed as a batch,
with amendments to triggered existing stories firing in parallel
(eventual consistency per user direction 2026-04-23):

1. **Story 16 — Run-trace persistence + `runs` schema**
   (observability is the prerequisite).
2. **Story 17 — `build_config` field on the story schema**
   (triggers story 6 schema amendment).
3. **Story 18 — Signer identity on runs + signings**
   (triggers stories 1, 2 amendments).
4. **Story 19 — `agentic-runtime` un-deferred** (Runtime trait +
   ClaudeCodeRuntime).
5. **Story 20 — `agentic story build <id>`** (triggers stories 4, 5
   amendments for Store snapshot primitive).
6. **Story 21 — Retirement lifecycle** (`retired` status +
   `superseded_by`; triggers stories 3, 6, 11 amendments plus
   touch-ups to 9, 10, 13).

Reproducible Docker image (old item 5 in the previous draft) lives
inside story 20's scope, not as a separate story — it's infra that
ships alongside the CLI subcommand.

**Phase 0.5** shrinks to a single new story:

7. **Story 22 — Cloud-compatible Store impl** (the docker-compose
   two-container proof; trait-parity tests from stories 4 + 5 cover
   it automatically).

The six-plus-runtime-plus-retirement Phase 0 batch is what it
actually takes to make the story-build loop work end-to-end with
corpus hygiene intact. Retirement joined Phase 0 (up from Phase 0.5)
specifically so story 6's schema amendment bundles both schema edits
into one re-UAT — per the user's "single UAT preference."

## Q2 resolution — Store posture by phase

- **Phase 0:** embedded `SurrealStore` inside the sandbox. Ephemeral.
  No network. Run rows written to a mounted `/output/` volume for host
  inspection.
- **Phase 0.5:** SurrealDB in a sibling container. Sandbox talks to it
  via docker network. Cloud Store impl exercised locally.
- **Phase 1:** SurrealDB on GCE `e2-small`. Same cloud Store impl.
  Access restricted to sandbox VPC.

"SurrealDB Cloud vs self-host" is now a non-question for the near term —
it's self-host (embedded, then containerised, then GCE) across the
whole ladder.

## Q3 resolution — compute by phase

- **Phase 0:** Docker Desktop on the user's laptop. Nothing else.
- **Phase 0.5:** docker-compose on the user's laptop. Still nothing else.
- **Phase 1:** Cloud Build (CI-triggered runs) **or** Cloud Run jobs
  (orchestrator-triggered one-shots). Both are pennies-per-run. No
  always-on compute.

**Cloud Workstations explicitly rejected.** They optimise for
interactive cloud dev, which under this vision we never want — the dev
is on their laptop; cloud is for headless agent runs. Workstations
also blow the cost envelope.

**Phase 1 tooling decision (Cloud Build vs Cloud Run jobs):**
deferred until Phase 0 and 0.5 are boring.

## Q4 resolution — claude auth

Unchanged in spirit: BYO credentials from the user's `~/.claude/` into
the sandbox. Phase 0 mounts `~/.claude/.credentials.json` directly into
the container at run time. Phase 1 sources it from GCP Secret Manager.
The ADR-0003 amendment clause in `11-sandbox-adr-outline.md` still
applies.

Signer identity for agent-signed runs takes a new shape:

> `signer: "sandbox:<model>@<run_id>"` — e.g.
> `"sandbox:claude-sonnet-4-6@run-a1b2c3"`.

Run-signing is distinct from human-signing. Human UAT signings remain
`signer: "<email>"`. Sandbox runs are attributed to the model + run id
so traces are replayable and attributable.

## Architectural wrinkle — ancestor gate in a fresh embedded Store

The user confirmed **option (a)** from the ideation session: at
container start, seed the embedded Store with the ancestors' signings
from a mounted snapshot. Preserves story 11's ancestor-health gate
semantics inside the sandbox.

Implication: a new primitive — **Store snapshot / restore** — must
exist. Phase 0's `agentic story build` needs it; the cloud Store impl
(Phase 0.5) acquires a sibling responsibility of "export
ancestor-closure to an embedded-Store seed."

This is a non-trivial addition to Phase 0 scope. Named so it doesn't
get missed.

## Green criterion

The inner loop reports GREEN when **both**:

1. All tests in `story.acceptance.tests[]` pass under `cargo test`.
2. `agentic uat <id> --verdict pass` exits 0 inside the sandbox
   (which requires the ancestor gate passing, hence the snapshot work
   above).

Either failing → the inner loop continues or exhausts. Never a partial
green.

## Downstream tier-2 implications (unchanged but worth re-noting)

- **Q5 (orchestrator protocol):** deferred. Phase 0 has no
  orchestrator — it's one `docker run`, one inner loop.
- **Q6 (orchestrator location):** laptop (via host `agentic` binary).
  Phase 1 may introduce Cloud Build triggers; still laptop-driven.
- **Q7 (retirement signings shape):** fossil record, unchanged.
- **Q8 (Gemma deployment):** now cleanly expressible via the
  `build_config.models:` story field — a story that wants Gemma
  declares it, and the container must have Gemma available.

## What this session did NOT decide

- Phase 2 fanout shape (parallel vs sequential sandbox, winner
  selection, amendment-retry).
- Web UI shape (deferred to when there are enough runs to visualise).
- Multi-dev seeding (12-36 month vision).
- External-pattern adoption — a parallel research thread is underway
  (see forthcoming `12-external-patterns.md`).

## What to do with this note

Advance state from "research sketchpad" to "session-of-record for
Phase 0." Still research; not authoritative. Authority flows through
story YAMLs, ADRs, and schemas — see `10-phase1-story-outlines.md` and
`11-sandbox-adr-outline.md` for the crystallisation plan.
