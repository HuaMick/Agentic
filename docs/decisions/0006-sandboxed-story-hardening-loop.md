# ADR-0006: Sandboxed story-hardening loop with reproducibility attestation

**Status:** accepted
**Date:** 2026-04-23

## Context

The system has stabilised at 12 healthy stories on a local-only
footing. `Store` trait abstracts persistence (ADR-0002); claude runs
via subprocess under the user's subscription (ADR-0003); the corpus
is schemaless-first with a hand-written pointer layer (ADR-0004); red
is a committable-atomic contract (ADR-0005). What's missing is the
outer surface — the loop by which a story transitions from *proposed*
to *healthy* reproducibly, with execution isolated from the human's
laptop and evidence captured for replay.

The user's framing of this surface is load-bearing:

> "1 human with cloud scalability. Sandbox/branches of our tree so I
> can experiment safely using our story tree and maintain system
> stability. To really prove the stories an agent should be able to
> build using only the story."

Two insights fall out of it:

1. **The sandbox is the execution environment. The story tree is the
   coordination structure.** Conflating them would turn every agent
   run into a new story. The corpus stays small; the runs fan out
   freely. See research folder notes 01, 03, 04.

2. **Reproducibility-from-story-alone is the convergence contract.**
   A story is healthy iff an agent, starting from the story YAML plus
   its transitive ancestors, can drive the implementation from red to
   green. External-pattern survey (note 12) confirms this framing is
   not shipped by any competing product; the story tree + cold-
   rebuild attestation is the piece no one else is doing. Sandboxing
   itself is commoditising (OpenHands, Claude Managed Agents, E2B,
   Daytona, Modal, Cursor, Replit, etc.); we are not competing there.

A three-step ladder for how we prove this works:

- **Phase 0** — single Docker container on the dev's laptop. No
  cloud cost. Proves the inner loop works observably.
- **Phase 0.5** — docker-compose with a separate SurrealDB container.
  Proves the cloud-ready data path.
- **Phase 1** — GCP (Cloud Build or Cloud Run jobs + a small GCE box
  for SurrealDB). Proves "works in the cloud" — **compatibility, not
  fanout.**

Each phase has one narrow proof obligation; we don't pay for cloud
until cloud is the thing being proven.

## Decision

Eight load-bearing parts.

### 1. The sandbox bakes in the whole build system

One Docker image, tagged per-commit SHA, contains: the `agentic`
binary, all agent specs under `agents/`, `patterns/`, `schemas/`,
`docs/decisions/`, `docs/guides/`, `CLAUDE.md`, `README.md`, the Rust
toolchain, and the `claude` CLI. Mounted at run time: the specific
`stories/<id>.yml` being built, `~/.claude/.credentials.json`, an
ancestor-closure snapshot, and a `/output/runs/<run-id>/` volume.
The story is the variable; the system is the constant. The image at
tag X + the story YAML at commit Y + the agent at model Z is the
reproducibility receipt.

### 2. `agentic story build <id>` is the hardening loop's inner unit

The host command validates inputs, composes a `docker run` with the
bakes and mounts above, launches the container, and waits. Inside,
the container runs one inner-loop execution: build → test → UAT,
iterating until green or the budget is exhausted. On exit, the run
row + NDJSON trace live in the mounted `runs/<run-id>/` directory.

### 3. `agentic-runtime` un-deferred; `Runtime` trait + `ClaudeCodeRuntime` impl ship

`_deferred/agentic-runtime/` activates into `crates/agentic-runtime/`.
The trait exposes `spawn_claude_session(config) -> RunOutcome`; the
impl spawns `claude -p --output-format stream-json --verbose`, tees
the NDJSON event stream to a trace file + structured run row, and
enforces the iteration budget. Product libraries remain AI-free per
ADR-0003 — the runtime is the sole sanctioned claude-subprocess
surface.

### 4. Store moves through three phases in lockstep with compute

Phase 0: embedded `SurrealStore` inside the sandbox, ephemeral,
ancestor signings seeded from a mounted snapshot. Phase 0.5:
SurrealDB in a sibling docker-compose container; sandbox talks over
docker network via a new `CloudSurrealStore` impl. Phase 1: SurrealDB
on GCE `e2-small` inside a dedicated GCP project. The `Store` trait
is the single abstraction point; migration is config-swap, not
refactor. See ADR-0002.

### 5. Observability is load-bearing, not an afterthought

A new `runs` Store table carries structured per-run metadata:
`run_id`, `story_id`, `story_yaml_snapshot` (SHA), `signer`, timestamps,
`build_config`, `outcome` (`green` / `inner_loop_exhausted` /
`crashed`), per-iteration summaries, and a path pointer to the
NDJSON trace blob. Where practical, the schema follows OpenTelemetry
GenAI semantic conventions so future interop with Langfuse /
LangSmith is config, not rewrite. **Without observability the
hardening loop has no feedback signal** — every other decision in
this ADR assumes we can read what agents did.

### 6. The story tree is the research bet, not the sandbox

We adopt commoditising sandbox tech (Docker now; potentially E2B,
Daytona, Modal, or Claude Managed Agents later) as the compute layer.
Effort invests in what is underexplored: the story-tree coordination
structure, amendment-on-failure semantics, cold-rebuild attestation.
The `Runtime` trait preserves option value on sandbox compute — a
later `E2BRuntime` or `ClaudeManagedAgentsRuntime` drops in behind
the same trait.

### 7. Human-in-the-loop at the outer loop throughout Phase 0 and Phase 1

The **inner loop** (build → test → UAT inside one sandbox) is fully
automated. The **outer loop** (amend the story → re-run the sandbox
→ converge) is human-driven. When a sandbox returns
`inner_loop_exhausted` or `crashed`, the trace surfaces to the human
who decides amend / retry / abandon. No autonomous amendment loop
ships until an independent evaluator is validated at scale.
Reflexion-lineage literature (2025, see note 12) documents
confirmation-bias failure modes when the reflecting model
re-justifies its own error; this is the direct caution.

Budget per story is declared in a new optional `build_config: {
max_inner_loop_iterations, models }` field on the story YAML. The
human seeding the story estimates complexity by picking a budget.

### 8. Git coordination: story-tree branches ≠ git branches

The story tree is corpus (YAML, `depends_on`, `superseded_by`). Git
branches are whole-tree snapshots (corpus + implementation at some
state). Per sandbox run: a `run/<story-id>-<short>` branch is cut
from main's tip at launch, lives inside the container, receives the
agent's commits, and is destroyed when the container exits. On
`green`, the host auto-merges the branch's diff onto main as a
squash commit. On `inner_loop_exhausted` or `crashed`, nothing
merges — the branch state survives only as structured data in the
run row.

Auto-merge on green with no human review gate is deliberate in Phase
0 and Phase 1. Bad merges are expected and are the signal that
forces Phase 2+ recovery + gating work. Retirement + supersession
are YAML-only (no git branch operations); **rollback branches**
(`rollback/<label>`) are standard git release-branch practice,
available when stability matters more than speed.

Sandbox signer identity for agent-signed runs follows the convention
`signer: "sandbox:<model>@<run_id>"`. Human UAT signings remain
`signer: "<email>"`. Git commit author for auto-merged commits uses
the dev's `git config user.email` — these three identities are
distinct and all land in the corpus history.

## Scope

**In scope:**

- GCP as the Phase 1 cloud provider (deferred; not built now).
- Single-human threat model. No RBAC, no multi-tenant isolation
  beyond the single-user boundary.
- Cost envelope of order $10s/month at Phase 1. Phase 0 / 0.5 are
  zero cloud cost.
- Infrastructure as code (Terraform) for Phase 1 GCP resources.
- The `Store`, `Runtime`, and inner-loop abstractions as the three
  load-bearing contracts.
- The `runs` Store table as the observability primitive.

**Explicitly out of scope:**

- Multi-human collaboration (12–36 month vision territory).
- Parallel / competitive agent fanout (Phase 2+; built on real need,
  not speculation).
- Autonomous outer-loop story amendment (requires validated
  independent evaluator; confirmation-bias risk documented).
- Cloud Workstations / Codespaces / any interactive cloud dev
  environment. Dev works on their laptop; cloud is for headless
  agent runs only.
- Staged release, feature gates, user feedback monitoring (Phase 3+).
- Web UI for the story tree (deferred until run volume justifies it;
  the `runs` schema is shaped to render later).

## Alternatives considered

**Managed agent SaaS products (E2B, Daytona, Modal, Claude Managed
Agents).** Rejected for Phase 0 on cost-and-coupling grounds;
Docker-local is cheaper and exercises the same primitive. Kept as
live options for later phases behind the `Runtime` trait. See
research folder note 12 for the full survey.

**OpenHands Software Agent SDK.** Closest architectural analogue
(event-sourced, 4-package SDK/Tools/Workspace/Server split, Docker
workspaces, 72% SWE-Bench Verified). Architecturally informative;
adoption rejected because we have our own story + red-green + UAT
contracts that don't map cleanly onto OpenHands' primitives.
Reference reading, not dependency.

**Cloud Workstations / GitHub Codespaces / Gitpod.** Rejected. These
optimise for interactive cloud dev, which we never want — the dev is
on their laptop always. Cost models (per-minute active sessions)
also don't fit the $10s/mo envelope.

**SurrealDB Cloud (managed).** Rejected. Third-party vendor, unclear
pricing, no inside-GCP isolation. Self-hosted on `e2-small` fits the
envelope and lives in the user's GCP project.

**Formal spec languages (TLA+, Alloy) as story format.** Rejected as
the story format. Precision-vs-authorability tradeoff would kill
corpus throughput. Story YAML remains the middle ground — precise
enough to constrain, loose enough to author.

**Autonomous agent-rewrites-its-own-story loops.** Explicitly
rejected for Phase 0 / Phase 1 on Reflexion-lineage
confirmation-bias grounds. Amendment stays human-in-the-loop until
an independent evaluator (cold-agent rebuild is a candidate) is
validated.

**One big ADR vs three small ADRs** (cloud Store / sandbox compute /
claude auth). Rejected the split. The decisions are interdependent
(self-hosted Store needs sandbox network access; agent sandboxes
need mounted credentials; Gemma-local affects image sizing). One
coherent ADR is easier to reason about than three cross-referencing
ones.

## Consequences

**Gained:**

- Sandbox-as-branch becomes real: multiple agent attempts don't leave
  local dirt; the dev's laptop stays clean.
- Story corpus + infra (Dockerfile, Terraform) grow together,
  versioned together, committed to the same repo.
- The reproducibility principle (see research note 05) gets a
  concrete ceremony: `agentic story build` is the ritual.
- Observability is first-class; run rows are queryable evidence,
  future UI renders them without a data-model migration.
- Phase ladder (0 → 0.5 → 1) means each step proves a specific thing;
  we don't pay for cloud until cloud is the thing being proven.
- The `Runtime` trait preserves option value on sandbox compute —
  swapping in E2B / Daytona / Claude Managed Agents later is config.

**Paid:**

- `agentic-runtime` un-defers now; new crate, new trait, new CLI
  touch-points.
- Docker image needs disciplined reproducible builds (byte-identical
  per commit tag).
- Claude credential mounting is a UX papercut — dev must have
  credentials where the launcher can find them.
- Subscription quota shared across local + any running cloud
  sandboxes; watch for rate-limit surprises as fanout scales.
- Phase 0 story scope is six new stories plus seven existing-story
  amendments (see research folder note 13 for the full impact table).

**Required before Phase 0 is complete:**

- Stories 16 (runs observability), 17 (`build_config` schema), 18
  (signer identity), 19 (`agentic-runtime` un-defer), 20 (`agentic
  story build` command), 21 (retirement lifecycle). Proposed as a
  batch per research folder note 10.
- Schema edits to `schemas/story.schema.json`: the `build_config`
  object (triggered by story 17) and the `"retired"` status enum
  value + `superseded_by` field (triggered by story 21), bundled into
  a single story-6 amendment pass per research folder note 13.
- A new primitive: **Store snapshot / restore** for seeding ancestor
  signings into a fresh embedded Store. Lands as a story-4 amendment
  with mirror in story 5 (`SurrealStore`).
- An `infra/sandbox/Dockerfile` with reproducible-build discipline.
- An ADR-0003 amendment clarifying the BYO-credentials posture in
  cloud sandboxes (authored alongside this ADR).

**Required before Phase 0.5 is complete:**

- Story 22 (`CloudSurrealStore` impl).
- `infra/sandbox/compose.yml` for the two-container layout.

**Risks named:**

- **Confirmation-bias in any future autonomous amendment loop.** See
  Reflexion-lineage 2025 literature. Mitigation: human-in-the-loop at
  the outer loop throughout Phase 0 and Phase 1.
- **Subscription TOS drift.** Anthropic's subscription terms for
  cloud-mounted credentials could shift; ADR-0003 amendment documents
  current posture; revisit on policy updates.
- **Sandbox compute market consolidation.** If E2B / Daytona / Modal
  consolidate or Claude Managed Agents become the de facto standard,
  our `Runtime` trait absorbs the change — but we may discover our
  trait shape doesn't match the winner. Mitigation: keep the trait
  minimal and boundary-driven.
- **Auto-merge on green without review produces bad merges.** This is
  deliberate. Bad merges are the signal that forces Phase 2+ recovery
  + gating work. Main may hold broken states for stretches of
  wall-clock time; eventual-consistency posture applies.

## Migration

No migration required for existing healthy stories. Existing contracts
stay healthy until the Phase 0 batch triggers amendments as part of
normal red-green-refactor cycles. See research folder note 13 for
the sequencing (seven amendments, four touch-ups, one no-change of
the twelve live stories).

Research folder `docs/research/story_tree/` (notes 01–14) is the
derivation trail for this ADR. When the Phase 0 stories ship and this
ADR's scope is exercised, notes 01–08 become purely historical; notes
09–14 stay as operator reference until superseded by updated guides
and a later ADR for Phase 2+ (recovery + gating mechanisms).

## Relationship to prior ADRs

- **ADR-0001** (Rust rebuild): unchanged. This ADR extends the Rust
  system with its outer execution surface.
- **ADR-0002** (SurrealDB embedded): extended by Phase 0.5 when the
  `CloudSurrealStore` impl lands. The `Store` trait contract is
  unchanged; the impl set grows.
- **ADR-0003** (claude via subscription subprocess): amended in
  parallel with this ADR. The amendment generalises
  subscription-via-subprocess to remote cloud sandboxes mounting BYO
  credentials, preserves the API-key / `--bare` prohibition, and
  names `agentic-runtime` inside the sandbox as the sole claude
  invocation surface.
- **ADR-0004** (no bootstrap generator): unchanged. Agent specs
  remain hand-authored; the sandbox consumes them as read-only baked
  content.
- **ADR-0005** (red-green is a contract): extended. The inner loop's
  green criterion is `cargo test --workspace` passing **AND** `agentic
  uat <id> --verdict pass` exiting 0 inside the sandbox. The
  ancestor-health gate (story 11) fires inside the sandbox against
  the seeded ancestor snapshot — preserving the gate semantics
  end-to-end.
