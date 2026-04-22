# 08 — Open questions for the next session

Questions that need answering before implementation begins. Organised
by urgency. Each has context + candidate answers + the decision maker.

## Urgency tier 1 — blocks starting work

### Q1 — Scope gate for Phase 1

**Question:** Is Phase 1 "cloud store + signer" (minimum viable,
~4 stories) or "cloud store + signer + retirement + scratch hygiene +
runtime-minimal" (stronger foundation, ~8 stories)?

**Context:** Both are defensible. See `07-prerequisites.md` "Quick
decision tree." User's stated bias is cleanest-foundation-first but
they also want to prove single-human cloud stability soon.

**Decision maker:** user.

**Recommendation:** the middle shape — ship P1 (retirement) + P2
(signer) + P4 (scratch hygiene formalisation) + cloud Store. Four
stories, 1-2 weeks of work. Defers P3 (audit) and P5 (runtime) as
follow-up.

### Q2 — SurrealDB Cloud vs self-hosted SurrealDB on GCE

**Question:** Which cloud store backend?

**Context:** See `06-stack-bet.md` pro/con table. Self-hosted is
cheaper and keeps everything inside user's GCP project; managed is
zero-ops.

**Decision maker:** user (GCP data engineer, has opinions on ops
surface area).

**Recommendation:** self-hosted on GCE `e2-small` (or smaller).
Predictable cost, no third-party vendor, fits the cost envelope.

### Q3 — Primary sandbox compute

**Question:** Cloud Workstations, GitHub Codespaces, or self-built
Docker-on-GCE?

**Context:** See `06-stack-bet.md` discussion. Cloud Workstations
is the GCP-native choice; Codespaces is market leader but not GCP.

**Decision maker:** user.

**Recommendation:** Cloud Workstations for interactive use + Docker
on GCE for ephemeral agent-only sandboxes. Two-tier model.

### Q4 — Claude auth posture in cloud

**Question:** BYO (bring-your-own claude credentials into the
sandbox), shared team account, API key fallback, or Gemma-local?

**Context:** See `04-sandbox-model.md` Decision D. ADR-0003's policy
concern about Anthropic subscription restrictions applies.

**Decision maker:** user (only one affected).

**Recommendation:** BYO credentials mounted into sandbox for claude
work; Gemma-local for high-volume agent batch tasks where claude cost
is disproportionate.

## Urgency tier 2 — affects implementation order but not blocking

### Q5 — Sandbox-orchestrator protocol

**Question:** When the orchestrator dispatches work to a sandbox, what
protocol?

**Options:**
- SSH + `agentic` commands.
- gRPC / HTTP RPC over a protocol the sandbox serves.
- Claude Code's own subagent-spawning (if it supports remote).
- Shared Pub/Sub queue.

**Context:** See `04-sandbox-model.md` OPEN on "what talks to the
sandbox." This may warrant a new ADR.

**Decision maker:** user + orchestrator (collaborative).

**Recommendation:** start with SSH + `agentic` commands because it's
zero new surface. Move to structured RPC only when SSH-scripting
becomes painful.

### Q6 — Where does the orchestrator live

**Question:** On the user's laptop, or in its own sandbox?

**Context:** See `04-sandbox-model.md` OPEN. Local is cheaper and
faster to iterate; cloud-hosted is more symmetric.

**Recommendation:** local orchestrator dispatching to cloud agent
sandboxes. Revisit if it becomes a bottleneck.

### Q7 — What's the Store shape for retirement

**Question:** When we convert retired stories to `status: retired`,
how do we handle the `uat_signings` rows?

**Options:**
- Leave them; they become fossil record pointing at retired stories.
- Mark them `verdict: fossil` or add a `for_retired_story: true` flag.
- Migrate them to a separate `retired_signings` collection.

**Recommendation:** leave them as-is. Retired-story lookup in the
dashboard can cross-reference; no schema surgery needed.

### Q8 — Gemma deployment shape

**Question:** Gemma runs on the sandbox itself, via Vertex AI, or
entirely optional?

**Context:** See `06-stack-bet.md` Gemma discussion.

**Recommendation:** sandbox-local via `ollama` or similar. Zero
cloud cost. Spun up via a sandbox image variant.

## Urgency tier 3 — long-horizon, worth naming

### Q9 — What does sandbox competition look like

**Question:** When we get to tree-metaphor behaviour 2 (competing
branches), what's the UX?

**Context:** Deferred per `03-tree-metaphor.md` recommendation. Build
on real need.

**Note:** marking as OPEN so next-session doesn't forget this is a
future direction.

### Q10 — Reproducibility audit cadence

**Question:** If reproducibility audits become a regular ceremony
(see `05-reproducibility.md`), who runs them and how often?

**Recommendation:** monthly sampled audit of 3 random healthy stories,
run by a dedicated `reproducibility-auditor` subagent type (new
agent spec would be needed). Defer until we actually have cloud
capacity to run them cheaply.

### Q11 — When to introduce `era:` vs trusting `superseded_by:` chains

**Question:** Do we add an explicit `era:` tag, or let
`superseded_by:` chains implicitly carry the era?

**Context:** See `03-tree-metaphor.md` behaviour 3.

**Recommendation:** trust the chain. Add `era:` only if the chain
grows unwieldy (5+ generations of a story slot).

### Q12 — Migration path for existing store contents

**Question:** When we move from local SurrealStore to cloud, do we
migrate data or start fresh?

**Context:** The story YAMLs are the durable record. The store contents
(signings, test_runs) could in principle be rebuilt from the YAMLs by
re-running UATs. But that's expensive.

**Recommendation:** export existing signings + test_runs to JSONL,
import into cloud. Don't try to be clever. Keep the export script as
a reusable tool.

## Philosophical / project-level questions

### Q13 — What's the 12-month vision

**Question:** Where does this project want to be a year from now?

User's existing statements cluster around:
- 1 human + N agents as current phase (3-6 months)
- Team of humans + agents later
- Reproducibility of stories

Worth writing a one-page 12-month vision next session. Helps prioritise.

### Q14 — Which stories should go "in the tree canopy vs the trunk"

**Question:** Not all stories carry equal weight. Some are foundational
(store, schema, UAT). Some are experimental (frontier dashboard view
shape). Should they be marked differently?

**Context:** Tree metaphor hints at this — trunks are old and
load-bearing; canopy is where growth happens.

**Recommendation:** maybe tag stories with `foundational: true` or
similar. Foundational stories are harder to supersede; canopy stories
retire frequently. Not urgent; defer until we hit the pain.

## Questions this document explicitly does NOT ask

- "Should we do this at all?" — decision already made. User said yes;
  focus is implementation.
- "Should we use Rust?" — settled (ADR-0001).
- "Should we use claude or API keys?" — settled (ADR-0003).
- "Should stories be YAML?" — settled (ADR-0002-ish + schema
  convention).

## Action list derived from tier 1 questions

For next session to resolve, in order:

1. Q1 — Phase 1 scope gate (MVP vs cleaner foundation)
2. Q2 — SurrealDB Cloud vs self-hosted
3. Q3 — Primary sandbox compute
4. Q4 — Claude auth posture

Once those four are settled, implementation stories can be authored.
