# 06 — Stack bet: GCP + cost-controlled + Gemma option

## User's constraints

> "I'm a gcp data engineer so i think gcp makes sense, however i want
> to keep costs under control, we now have google gemma 4 which is
> smart and cheap and can even be run on the sandbox locally if needed
> so imagine the token cost is super cheap"

Three constraints to honour:

1. **GCP as cloud of choice.** User has deep operational familiarity;
   don't fight it.
2. **Cost control is load-bearing.** This is research; it can't bill
   like production. Order of magnitude $10s/month not $100s.
3. **Gemma as a cost-reduction lever.** Not a hard commitment; an
   available option.

## GCP services mapping

For each piece of the sandbox architecture, the GCP-native option:

| Need | GCP option | Notes |
|------|------------|-------|
| Cloud-backed store | **SurrealDB Cloud** (not GCP-native) OR self-host on GCE | SurrealDB doesn't have a GCP-managed service. Self-host is fine at 1-user scale. |
| Alternative store | Firestore, Cloud SQL (Postgres), BigQuery | Would require new Store trait impl. Defer unless SurrealDB proves inadequate. |
| Compute (dev env) | **Cloud Workstations** | Managed dev envs. GCP's Codespaces equivalent. IAM-integrated. |
| Compute (batch agent work) | Cloud Run jobs, GKE Autopilot | Cloud Run jobs are cheap for discrete agent tasks. |
| Local model hosting | Vertex AI endpoints (Gemma) OR local on the sandbox itself | Local is cheaper; Vertex is simpler ops. |
| Secret management | Secret Manager | Use for claude credentials when we allow shared accounts. |
| Identity | Cloud Identity / IAM | Overbuilt for single-user; use anyway for forward compat. |
| Observability | Cloud Logging + Monitoring | Standard. Start light. |
| Cost control | Budget alerts + Cloud Billing dashboards | Essential. Set at day 1. |

## Cost envelope — rough sizing

Assumptions: 1 human, small agent fanout (≤5 concurrent sandboxes at
peak), moderate activity (roughly one active sandbox hour per working
day on average).

Rough monthly at current GCP rates (order of magnitude):

| Component | Ballpark | Notes |
|-----------|----------|-------|
| 1 GCE e2-small for self-hosted SurrealDB 24/7 | ~$13 | Or free tier if sized to f1-micro. |
| Cloud Workstations, ~20 active hrs/month | ~$10–20 | Depends on machine type. |
| Claude subscription | existing | Not new cost; BYO. |
| Gemma 4 on sandbox (if used) | ~$0 marginal | Model sits on sandbox disk. Inference is CPU/GPU-local. |
| Cloud Logging + Monitoring | ~$5 | Essential. |
| Secret Manager | ~$1 | Trivial. |
| Network egress | ~$5 | Minimal at this scale. |

**Rough total: ~$35–50/mo.** Fits the "order of magnitude $10s"
constraint with headroom.

Biggest risk to this number: **forgotten sandboxes running 24/7.** Set
TTL / auto-suspend policies from day one.

## The Gemma 4 lever

User's reasoning: Gemma 4 is small, cheap, can run locally on the
sandbox disk. Token cost is effectively zero.

**Where Gemma is a good fit:**

- **High-volume mechanical agent tasks.** Parsing commit messages,
  extracting story titles, renaming crates, bulk doc edits.
- **Pre-processing.** Cleaning noisy text before feeding claude a
  smaller prompt.
- **Local determinism tests.** Reproducibility audits (see
  `05-reproducibility.md`) can run with Gemma for cheap iteration,
  promoting to claude only for final verification.
- **Sandbox bootstrapping.** A new sandbox could provision with a
  local Gemma for any "cheap agent thought" it needs before the human
  pays claude costs.

**Where Gemma is a bad fit:**

- **Long-context planning.** Multi-story architectural decisions.
  Story-writer work. Anything that needs the full corpus plus conversation
  context.
- **Judgement calls.** The user's session with me was full of
  architectural judgement that's beyond Gemma 4's capability.
- **Claude Code integration.** Claude Code's tool use + context
  management doesn't port to Gemma without significant work.

**Recommendation:** treat Gemma as a **second-tier agent pool**, not a
claude replacement. Stories that need heavy thinking go through
claude; stories that are mechanical go through Gemma; the story YAML
itself (authored by the user) doesn't change either way.

This is a natural extension of the claude-as-user principle: both
claude and Gemma are just users of the CLI. The library doesn't care
which.

## Why Gemma-local matters for reproducibility

If Gemma can reproduce a healthy story's implementation (even if
slowly, even if not the first-pass prettiest), that's a strong
reproducibility signal: *the specification is complete enough that a
weaker model can execute it*. Claude being able to do it is expected.
Gemma being able to do it is the meaningful proof.

This argues for the sampled reproducibility audit (see
`05-reproducibility.md` OPEN) being run with Gemma, not claude:
cheaper, and stronger signal.

## Decision: SurrealDB Cloud vs self-host

Quick pro-con for the next session to resolve:

**SurrealDB Cloud (managed):**
- Pro: zero ops overhead, automatic backups, HA.
- Con: pricing may not fit cost envelope at low usage. Harder to
  network-isolate to a single GCP project.
- Con: vendor lock-in to a company's SaaS (is SurrealDB Labs a going
  concern? Not universally proven. Worth diligence.)

**Self-hosted on GCE:**
- Pro: predictable cost, full control, lives in user's GCP project,
  no third party.
- Con: backup/restore is your problem, HA is your problem, version
  upgrades are your problem.

**Recommendation:** **self-host on GCE** for this phase. Cheaper,
lives inside the user's GCP project, no new vendor relationship. The
HA concern is negligible at 1-user scale. Revisit if the system
proves load-bearing at 10+ users.

## Decision: Cloud Workstations vs Codespaces vs something smaller

**Cloud Workstations (GCP):**
- Pro: GCP-native, IAM-integrated, good perf, configurable images.
- Con: higher per-minute cost than Codespaces.
- Con: comparative maturity is less than GitHub Codespaces.

**GitHub Codespaces:**
- Pro: market leader, excellent Rust tooling, cheap free tier.
- Con: not GCP. User would straddle Microsoft/GitHub for compute while
  all other infra is GCP.

**Self-built Docker-on-GCE:**
- Pro: maximum control, minimum cost.
- Con: significant ops work to match the managed options.

**Recommendation:** **Cloud Workstations** for the primary human+agent
experience. Ignore Codespaces for cleanliness; the user already pays
GCP, not a second bill to GitHub. **Self-built Docker-on-GCE** as the
fallback for agent-only batch sandboxes (no VSCode needed, just a
runtime).

This gives two sandbox tiers:

1. **Interactive sandbox** (Cloud Workstations) — the user connects
   via VSCode-in-browser, orchestrates from there.
2. **Agent sandbox** (Docker on GCE) — ephemeral, no GUI, spun up by
   the orchestrator to run a single story's build, destroyed when
   done.

## Terraform / IaC posture

Given GCP data-engineer background, **all cloud infra should be IaC
from day 1**. Terraform is the obvious pick; Pulumi or gcloud-scripts
are alternatives.

This matters for reproducibility: the *sandbox image itself* should be
reproducible. Story X says "I need a Rust sandbox with claude installed
and the repo cloned" → the Terraform / Packer config provisions that
deterministically.

## OPEN: GCP project organisation

Does this live in:
- A new `agentic-research` GCP project, isolated?
- The user's existing project, with tags / labels?

Recommendation: new project. Isolates cost, limits blast radius,
clean teardown if the whole experiment ends.

## OPEN: cost monitoring threshold

What's the cost signal that triggers reassessment?

- $50/mo → expected, no action.
- $100/mo → investigate; may indicate runaway sandboxes.
- $200/mo → hard stop; this violates the cost constraint and means the
  architecture needs rethinking.

Set budget alerts at these tiers on day 1.

## Files to consult for more context

- User's durable memory note on "Store will move from local to cloud."
- `crates/agentic-store/README.md` for current Store trait shape.
- `crates/agentic-store/src/surreal_store.rs` (or similar) for current
  SurrealStore impl — that's the template for a cloud variant.
- `bin/agentic-docker`, `install.sh` (both pre-existing dirty, but
  relevant to containerisation strategy).
