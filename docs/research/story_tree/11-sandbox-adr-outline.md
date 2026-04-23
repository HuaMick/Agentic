# 11 — Sandbox ADR outline (revised)

Draft outline for the ADR(s) that should codify the Phase 0 Docker-first
+ Phase 0.5 compose + Phase 1 GCP architecture, incorporating findings
from `09-tier1-resolutions.md` (Phase ladder), the story-hardening
loop ideation, and the external-patterns survey in
`12-external-patterns.md`.

**This note replaces the earlier cloud-first ADR outline** with one
shaped around the new insight: sandboxing is commoditising; the
**story tree + reproducibility attestation** is the research bet that
deserves ADR weight.

## Scope

One big ADR covering the full Phase 0 / 0.5 / 1 architecture, plus
one amendment to ADR-0003.

### Proposed new ADR

**ADR-0006: Sandboxed story-hardening loop with reproducibility
attestation.**

Title matters. Earlier drafts called this "cloud sandbox architecture"
— too narrow, too infrastructure-y, missed the bet. The real decision
is: **the story is the iteration target, the sandbox is its
execution environment, and reproducibility-from-story-alone is the
convergence contract.** Call it that.

### Proposed amendment

**ADR-0003 amendment:** clarify that "subscription auth via subprocess"
generalises to cloud-mounted BYO credentials (from GCP Secret Manager
or equivalent) — never API keys, never shared team accounts.

## ADR-0006 outline

Follow the existing template (`0001`–`0005`): **Context, Decision,
Scope, Alternatives considered, Consequences.**

### Context

- All 13 stories healthy at time of writing. System has stabilised on
  claude-as-user architecture (ADR-0003), schemaless Store
  (ADR-0002), red-green contract (ADR-0005).
- User direction for next phase: **1 human + N agents, cloud
  sandboxes for agent work, story tree as the coordination primitive.**
- User's deeper reproducibility principle (see
  `05-reproducibility.md`): *"to really prove the stories an agent
  should be able to build using only the story."* Not aspirational;
  load-bearing.
- Phase 0 (local Docker, single container) precedes Phase 0.5
  (docker-compose) precedes Phase 1 (GCP). Each step has a specific,
  narrow proof obligation. No production-shaped cloud rollout in
  Phase 1 — just compatibility proof.
- External-patterns survey (`12-external-patterns.md`) shows the
  sandbox primitive is commoditising (OpenHands, Claude Managed
  Agents, E2B, Daytona, Modal, Cursor, Replit). We are **not
  competing on sandbox tech.** Our bet is upstream: the
  specification-as-iteration-target + cold-rebuild attestation.

### Decision

Eight load-bearing parts (the eighth — git coordination — surfaced
2026-04-23 in note 14):

1. **Phase 0 is the primary unit of Phase 1 proof.** A single Docker
   image, with the whole build system baked in (agentic binary, all
   agent specs, patterns, schemas, ADRs, CLAUDE.md, guides, Rust
   toolchain, claude CLI), represents the system. One image runs any
   story. The story is mounted at run time; everything else is
   constant. This is the **reproducibility-receipt shape.**

2. **`agentic story build <id>` is the hardening loop's inner unit.**
   The host command launches the container. The container runs one
   inner-loop execution of the story: build → test → uat, iterating
   until green or the iteration budget is exhausted. On exit, a
   structured run row + NDJSON trace are in a mounted `runs/<id>/`
   directory for host inspection.

3. **`agentic-runtime` un-deferred.** The `Runtime` trait +
   `ClaudeCodeRuntime` impl ship as the sanctioned claude-subprocess
   surface. Product libraries remain AI-free (ADR-0003 unchanged).
   The runtime captures the NDJSON stream as telemetry.

4. **The Store moves through three phases in lockstep with compute.**
   Phase 0: embedded SurrealStore inside the sandbox, ephemeral,
   ancestor signings seeded from a mounted snapshot. Phase 0.5:
   SurrealDB in a sibling docker-compose container. Phase 1:
   SurrealDB on GCE `e2-small`. The `Store` trait is the single
   abstraction point; migration is config, not code.

5. **Observability is load-bearing.** The `runs` table (story 16)
   records structured metadata; trace blobs live as filesystem
   NDJSON. The schema follows (where practical) OpenTelemetry GenAI
   semantic conventions — span tree for tool calls, cost/latency/
   model-id per span — so future interop with Langfuse / LangSmith /
   Helicone is config, not a rewrite.

6. **The story tree is the research bet, not the sandbox.** We adopt
   commoditising sandbox tech (Docker, later potentially E2B /
   Daytona / Modal / Claude Managed Agents) as the compute layer. We
   invest effort in what's underexplored: the story-tree coordination
   structure + amendment-on-failure semantics + cold-rebuild
   attestation.

7. **Human-in-the-loop at the outer loop, throughout Phase 0 and
   Phase 1.** When the inner loop fails, the trace surfaces to the
   human, who decides amend / retry / abandon. No autonomous
   amendment loop until there's an independent evaluator (cold-agent
   rebuild is a candidate but must be validated at scale). The
   Reflexion confirmation-bias literature supports this caution
   (see `12-external-patterns.md`).

8. **Git coordination: story-tree branches ≠ git branches.** The
   story tree is corpus (YAML); git branches are whole-tree
   snapshots. Per run: the sandbox creates an ephemeral
   `run/<story-id>-<short>` branch cloned from main at launch. On
   GREEN, the host auto-merges the sandbox's diff to main (one
   squash commit per run). On EXHAUSTED / CRASHED, nothing merges;
   the branch state is captured in the run row for inspection. No
   human review gate in Phase 0. Bad merges are expected and are
   the signal that forces Phase 2+ recovery + gating work. See
   `14-git-coordination.md` for the full treatment.

### Scope

**In scope:**
- GCP as the Phase 1 cloud provider (user's existing operational
  familiarity).
- Single-human threat model. No RBAC, no multi-tenant isolation
  beyond single-user boundary.
- Cost envelope order $10s/mo at Phase 1. Phase 0 / 0.5 are zero
  cloud cost.
- Infrastructure as code (Terraform) for Phase 1 GCP resources.
- The `Store`, `Runtime`, and story-build-inner-loop abstractions as
  the three load-bearing contracts.

**Explicitly out of scope:**
- Multi-human collaboration (Phase 3+ research; 12–36 month vision).
- Parallel / competitive agent fanout (Phase 2; built on real need).
- Autonomous outer-loop story amendment (requires validated
  independent evaluator).
- Cloud Workstations or any interactive cloud dev environment (dev
  works on their laptop, always).
- Staged release / feature gates / user feedback monitoring (Phase 3+).
- Web UI for the story tree (deferred until run volume justifies it).

### Alternatives considered

**Sandbox-as-a-service vendors (E2B, Daytona, Modal).** Rejected for
Phase 0 on cost-and-coupling grounds; Docker-local is cheaper and
exercises the same primitive. Kept under active consideration for
Phase 1+ as a compute-layer swap behind the `Runtime` trait.

**Claude Managed Agents (Anthropic).** < 1 month old at time of
writing; insufficient adoption signal. Architecturally aligned with
our direction (managed containers, persistent sessions) but premature
to commit. Revisit at Phase 1 planning.

**OpenHands Software Agent SDK.** Closest architectural analogue
(event-sourced, 4-package split, Docker workspaces). Reading
recommended; adoption rejected because we have our own story +
red-green + UAT contracts that don't map cleanly onto OpenHands'
primitives. But their `Workspace` and `Runtime` abstractions are
worth cross-referencing when we author the Rust equivalents.

**Cloud Workstations / Codespaces / Gitpod.** Rejected. Dev works on
their laptop; we don't need an interactive cloud dev environment.
Their cost model (per-minute active sessions) also doesn't fit our
cost envelope.

**GKE / Cloud Run / Firecracker directly.** Deferred. Docker-on-GCE
(Phase 1) is sufficient for single-sandbox demonstration. Higher
tiers come in when fanout is a real ask.

**SurrealDB Cloud (managed).** Rejected. Third-party vendor, unclear
pricing, no inside-GCP isolation. Self-hosted on `e2-small` is cheap
and lives in the user's GCP project.

**Formal spec languages (TLA+, Alloy).** Rejected as the story
format. Precision-vs-authorability tradeoff would kill throughput.
Our YAML stories are the middle ground — precise enough to constrain,
loose enough to author.

**Autonomous agent-rewrites-its-own-story loops.** Explicitly
rejected for Phase 0 and Phase 1 on the Reflexion literature's
confirmation-bias grounds. Amendment happens human-in-the-loop until
we validate an independent evaluator.

### Consequences

**Gained:**
- Sandbox-as-branch becomes real; multiple agent attempts don't
  leave local dirt.
- Story corpus + infra (Dockerfile, Terraform) grow together,
  versioned together.
- Reproducibility story (`05-reproducibility.md`) gets a concrete
  ceremony: `agentic story build` is the ritual.
- Observability becomes first-class; run rows are queryable evidence.
- The Phase ladder (0 → 0.5 → 1) means each step proves a specific
  thing; we don't pay for cloud until cloud is the thing being proven.
- The `Runtime` trait preserves option value on sandbox compute —
  swapping in E2B / Daytona / Claude Managed Agents later is config.

**Paid:**
- `agentic-runtime` un-defers now; schema + CLI + dashboard touch-ups
  in several crates.
- Docker image needs disciplined reproducible builds (byte-identical
  per commit tag).
- Claude credential mounting is a UX papercut — the dev must have
  their credentials where the launcher can find them.
- Subscription quota shared across local + cloud sandboxes; monitor
  rate-limit risk as fanout scales.
- Phase 0 scope is larger than the original four-story cloud shape:
  six stories + `agentic-runtime` un-defer (see
  `10-phase1-story-outlines.md`).

**Required:**
- Phase 0 stories 16–20 land first (run trace, `build_config`, signer,
  runtime, story build).
- A schema edit to `schemas/story.schema.json` (build_config field;
  later, `retired` enum).
- A new primitive: **Store snapshot / restore** (for seeding
  ancestor signings into a fresh embedded Store).
- An `infra/sandbox/Dockerfile` with reproducible-build discipline.
- Guides under `docs/guides/`: `local-sandbox-run.md`,
  `byo-claude-credentials.md`, `reading-a-run-row.md`.
- Budget alerts set on day 1 at Phase 1 rollout.

**Risks named explicitly:**
- **Confirmation-bias in any future autonomous amendment loop.**
  Documented in Reflexion-lineage literature (see
  `12-external-patterns.md`). Defence: human-in-the-loop at outer
  loop throughout Phase 0 and Phase 1.
- **Subscription TOS drift.** Anthropic's subscription terms for
  cloud-mounted credentials could shift. ADR-0003 amendment
  documents current posture; watch for policy updates.
- **Sandbox compute market consolidation.** If E2B / Daytona / Modal
  consolidate, or Claude Managed Agents become the de facto standard,
  our `Runtime` trait absorbs the change — but we may discover our
  trait shape doesn't match the winner. Mitigation: keep the trait
  minimal and boundary-driven.

## Proposed ADR-0003 amendment

One paragraph under a new `## Amendment (cloud posture, 2026-04-22)`
section in `docs/decisions/0003-claude-code-subscription-subprocess.md`:

> The subscription-via-subprocess rule generalises to remote cloud
> sandboxes under the following condition: the sandbox mounts the
> authorised user's `~/.claude/.credentials.json` at launch time from
> a per-user secret store (GCP Secret Manager for our Phase 1 infra;
> local filesystem mount for Phase 0) and never persists those
> credentials to the sandbox image or long-lived disk. `--bare`
> remains forbidden; API-key fallback remains forbidden. Shared or
> team claude accounts are out of scope until a separate ADR
> addresses multi-user licensing. `agentic-runtime` inside the
> sandbox is the sole location that invokes `claude`, matching the
> scope clause above the amendment.

Short, additive, preserves the original scope statement intact.

## Non-ADR artefacts Phase 0 / 0.5 / 1 imply

Not everything in this architecture is ADR-shaped:

- **Dockerfile** at `infra/sandbox/Dockerfile` — reproducible,
  byte-identical per commit.
- **docker-compose.yml** at `infra/sandbox/compose.yml` — Phase 0.5's
  two-container setup.
- **Terraform module** under `infra/gcp/` — GCP project, GCE SurrealDB
  host, Secret Manager, budget alerts, IAM bindings. Arrives with
  Phase 1.
- **Guides** under `docs/guides/`: local sandbox run, BYO creds,
  reading a run row, (later) cloud sandbox setup.
- **Store snapshot primitive** — trait extension on `Store` to export
  ancestor-closure signings as a restorable bundle. Arrives with
  story 20.
- **CI for the Dockerfile** — deterministic builds, image caching,
  push to Artifact Registry on main.
- **Migration script** (optional, Phase 0.5 / 1 cutover) — export
  local Store contents, import into server-backed Store. Keep as a
  reusable tool.

## What this ADR is NOT

- **Not a commitment to specific vendor pricing or SKUs.** Pricing
  shifts; ADR names architecture, Terraform names SKUs.
- **Not a commitment to specific version strings.** SurrealDB, Rust
  toolchain, claude CLI versions belong in image lockfiles.
- **Not a commitment to keeping GCP forever.** If self-hosting proves
  painful or Cloud Managed Agents mature, ADR supersession is the
  remediation path.
- **Not the final word on the outer loop.** Phase 0 defines the
  inner loop and human-driven amendment. A follow-up ADR (ADR-0007
  when authored) will codify the outer loop once we have enough
  Phase 0 evidence to know what it should look like.

## Authoring workflow

When the user confirms the content of this outline:

1. Author ADR-0006 at `docs/decisions/0006-sandboxed-story-hardening-loop.md`
   using the five-section template.
2. Author the ADR-0003 amendment inline in
   `docs/decisions/0003-claude-code-subscription-subprocess.md`.
3. Ensure Phase 0 stories 16–20 cite ADR-0006 in their `guidance`
   blocks.
4. Begin Dockerfile + infra scaffolding on a parallel track (not
   story-shaped).

ADRs ship as committed markdown; authority flows from being in-repo
and referenced.
