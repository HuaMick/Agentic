# NEXT-SESSION brief

Start here. This is the action-list for whoever picks up the story_tree
research next.

## State at time of writing

- Branch: `main`
- All 13 stories `healthy`
- `dag-primary-lens` epic complete
- Claude-as-user architectural realignment complete
- `agentic test-build plan|record` (story 15) shipped + amended for
  defect fixes
- Tree clean

The system has never been in a better state to build on.

## User's stated direction for next session

> "if I agree i think the next session we should map out how do we
> implement this, what stories need to be completed locally before we
> go cloud, when we go cloud what technology choices should we pick."

Three workstreams, in order:

1. **Resolve the four tier-1 open questions** (`08-open-questions.md`).
   These block story authoring.
2. **Author the Phase 1 stories** that resolve prerequisites (see
   `07-prerequisites.md` — specifically P1 retirement + P2 signer + P4
   scratch hygiene + the cloud Store impl).
3. **Draft the ADR(s) for sandbox architecture.** Probably one ADR
   covering: cloud Store shape, signer identity, sandbox compute choice,
   claude auth in cloud.

## Reading order for fresh context

If you are a fresh Claude Code session picking this up with no prior
conversation context:

1. Read `README.md` (top of repo) — current status.
2. Read `CLAUDE.md` — driving rules.
3. Read `stories/README.md` — story corpus.
4. Read `docs/decisions/0001` through `0005` — the five ADRs.
5. Read `docs/research/story_tree/README.md` — this folder's index.
6. Read the numbered notes in this folder **in order** (`01` through
   `08`).
7. Finally, read this file (`NEXT-SESSION.md`).

Total time: maybe 30 minutes. Then you have full context.

## Conversational context to preserve

Key things the user said across the conversations that led to this
folder:

- **"the innovation is our story tree"** — the sandbox primitive is
  table-stakes; the tree of stories as a coordination structure is
  the bet.
- **"1 human but with cloud scalability"** — do not build multi-human
  features yet.
- **"keep costs under control"** — order of magnitude $10s/month not
  $100s.
- **"I'm a gcp data engineer"** — default to GCP, respect their
  operational familiarity.
- **"cleanest foundation over stability"** — willing to pay re-UAT
  costs for architectural clarity. This is session-wide philosophy.
- **"to really prove the stories an agent should be able to build
  using only the story"** — reproducibility is load-bearing. Not
  optional.

## The four tier-1 questions to answer first

Copy from `08-open-questions.md`:

1. **Phase 1 scope gate** — MVP (~4 stories) or cleaner foundation
   (~8 stories)?
2. **SurrealDB Cloud vs self-hosted** — my recommendation: self-hosted
   GCE.
3. **Primary sandbox compute** — my recommendation: Cloud Workstations
   + Docker-on-GCE for agent batch.
4. **Claude auth in cloud** — my recommendation: BYO credentials
   mounted into sandbox.

These four unblock everything else.

## Stories to author (draft list)

Pending Q1's resolution, here's the candidate list. User may
prioritise differently.

| id candidate | title | from |
|---|---|---|
| 16 | Story lifecycle adds `retired` status with `superseded_by` metadata | P1 |
| 17 | `uat_signings` rows carry `signer` identity | P2 |
| 18 | `agentic scratch` CLI for ephemeral local worktrees | P4 |
| 19 | `agentic-store` gains cloud-backed SurrealStore-over-HTTP impl | Phase 1 |
| 20 | `agentic stories health` `--canopy` filter shows retired stories | P1 tail |

That's five stories. Fits the "cleaner foundation" shape.

Minimal alternative (just "ship cloud"):
- Story 17 (signer) + Story 19 (cloud Store). Two stories.

## Things this session did NOT do

Name explicitly so next session knows what's still waiting:

- Did not author any new stories. Research only.
- Did not touch ADRs. Amendments should go in a future session after
  Phase 1 decisions crystallise.
- Did not start the retirement or signer implementations. Those are
  story-authoring work, then test-builder, build-rust, test-uat.
- Did not run a reproducibility audit. Deferred per `05-reproducibility.md`.
- Did not make any cost commitments with GCP or third parties. All
  discussion theoretical.

## How to start the next session (concrete)

Verbatim starting prompt suggested for the user:

> "Continue the story_tree research. Read
> `docs/research/story_tree/README.md` and follow the reading order.
> Then resolve the four tier-1 open questions in `08-open-questions.md`
> with me. Once resolved, author the Phase 1 stories so test-builder
> can start scaffolding."

That's a clean handoff. The fresh session will have all context it
needs.

## Troubleshooting for the next session

If anything in this research folder seems outdated or wrong, trust the
authoritative artefacts in this order:

1. `stories/*.yml` — current contracts
2. `CLAUDE.md` — current driving rules
3. `docs/decisions/*.md` — current ADRs
4. `README.md` — current project state
5. `schemas/*.json` — current shapes

The research folder is a sketchpad. It drifts. If it disagrees with
the authoritative artefacts, the authoritative artefacts win.

## Sign-off

Research folder authored in session 2026-04-22 (or check the commit
date for when this was written). All 13 stories healthy at time of
writing. Next session continues the ideation → implementation
transition.

Good luck. The foundations are solid.
