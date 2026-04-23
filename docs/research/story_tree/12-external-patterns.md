# 12 — External patterns survey (April 2026)

Snapshot of what's being built elsewhere in the agent-sandboxing /
spec-driven-iteration space, compiled by a research subagent on
2026-04-22. Skeptical framing per the user's direction:

> "Sandboxing agents is quite an emerging pattern these days, I suspect
> our story tree is the real innovation here (but who knows things are
> moving so fast these days), see if there's any existing patterns we
> can learn from but be skeptical — I don't want fancy unproven patterns
> adopted just because others are doing it (everyone is experimenting
> atm, nothing is proven)."

## Headline finding

The user's suspicion is **confirmed by the survey**:

- **Sandboxing as a primitive is commoditising fast.** Six-plus well-
  funded products, three-plus research systems, multiple SaaS vendors.
  Isolation is a solved problem; the market is converging.
- **Treating the spec as the iteration target — with amendment on
  failure, convergence defined as "a cold agent can rebuild from the
  spec alone" — is not shipped anywhere I could find.**
- **Our bet (story tree + amendment on failure + reproducibility-from-
  spec-alone as the healthy gate) is the genuinely underexplored piece.**

Secondary findings:

- The Reflexion / self-critique literature from 2025 documents
  **confirmation-bias failure modes** when the reflecting model
  re-justifies its own error. Direct caution for any "agent rewrites
  its own story" design. Our "cold agent must rebuild from story
  alone" convergence contract is a reasonable defence, but treat as
  a hypothesis to validate, not a settled pattern.
- OpenTelemetry's GenAI semantic conventions are emerging as the
  portable layer for agent trace observability. Worth adopting for
  story 16's `runs` schema if we want future interop with Langfuse /
  LangSmith / Helicone.

## Sandboxing primitives — who does what

**Anthropic Claude Managed Agents** (released Apr 8 2026).
Cloud-native managed containers, built-in sandboxing, persistent
sessions, configured via API. Anthropic's own "don't build your own
runtime" offering. **Maturity: beta (weeks old).**
- [platform.claude.com/agent-sdk/hosting](https://platform.claude.com/docs/en/agent-sdk/hosting)
- [Cobus Greyling writeup](https://cobusgreyling.medium.com/claude-managed-agents-0f47df3caa6f)

**Claude Code subagents / native sandboxing.** Task-tool subagents,
worktree isolation, filesystem + network scoping. Docs explicitly
offer Docker, gVisor, Firecracker self-host options. **Maturity:
production for the IDE use case; sandboxing itself is recent.**
- [code.claude.com/sandboxing](https://code.claude.com/docs/en/sandboxing)

**OpenHands Software Agent SDK** (MLSys 2026 paper). Stateless,
event-sourced, 4-package split (SDK/Tools/Workspace/Server).
DockerWorkspace + Kubernetes runtime. 72% SWE-Bench Verified on
Sonnet 4.5. **The closest architectural analogue to Agentic's shape.**
**Maturity: production-tier research; strong OSS adoption.**
- [arxiv 2511.03690](https://arxiv.org/html/2511.03690v1)
- [repo](https://github.com/OpenHands/software-agent-sdk)

**Devin 2.0 (Cognition).** Per-task VM, cloud IDE, parallel instances,
Interactive Planning phase. Closed source. **Maturity: production
SaaS; real-world resolution rates remain controversial.**
- [cognition.ai/devin-2](https://cognition.ai/blog/devin-2)

**Cursor Background Agents / Windsurf.** Cloud VMs; up to 8 parallel
agents in git worktrees; 2026 added granular network/FS ACLs.
**Maturity: production.**
- [cursor.com/changelog/2-5](https://cursor.com/changelog/2-5)

**Replit Agent 3/4.** Managed containers on Azure, SOC2, 200-min
autonomy, parallel forks with auto-merge. **Maturity: production
consumer/SMB.**
- [InfoQ Agent 3 write-up](https://www.infoq.com/news/2025/09/replit-agent-3/)

**Sandbox-as-a-service.** E2B (Firecracker microVMs, ~150 ms cold
start), Daytona (Docker, sub-90 ms, $24M Series A Feb 2026), Modal
(gVisor, GPU-first), Kubernetes `agent-sandbox` SIG project.
**Maturity: production infra; multiple competing vendors is itself a
sign of pre-consolidation.**
- [superagent 2026 benchmark](https://www.superagent.sh/blog/ai-code-sandbox-benchmark-2026)
- [kubernetes-sigs/agent-sandbox](https://github.com/kubernetes-sigs/agent-sandbox)

**Dagger.** Container-native pipeline engine, now positioned as agent
substrate ("Speck" spec-driven agent on top). **Maturity: Dagger
itself production; agent angle experimental.**
- [dagger.io](https://dagger.io/)

**SWE-agent, GitHub Copilot Workspace, Aider.** SWE-agent =
Docker-per-task research harness (mature in benchmarks, not products).
Copilot Workspace = hosted task agent, repo-scoped. Aider = no
sandbox; runs on host. **Maturity: mixed; Aider is intentionally
un-sandboxed.**

**What ships inside the sandbox:** almost universally **code + runtime
only**. Agentic baking CLI + specs + schemas + guidance into the image
is uncommon — OpenHands comes closest with its Workspace package,
but guidance is typically fetched as context, not baked.

## Spec-driven iteration — is the spec the iteration target?

**Kiro (AWS, mid-2025).** Three-phase workflow (EARS-notation
requirements → design → tasks). Specs are living documents. But
"amend the spec on failure" is a **human workflow convention**, not
an enforced primitive.
- [AWS Kiro case study](https://aws.amazon.com/blogs/industries/from-spec-to-production-a-three-week-drug-discovery-agent-using-kiro/)

**GitHub Spec-Kit.** 71K stars, 20+ agent integrations. Static spec
generator; reconciliation on divergence is manual. **Maturity: broad
tooling, shallow semantics.**
- [Augment roundup](https://www.augmentcode.com/tools/best-spec-driven-development-tools)

**Reflexion lineage (2023 → 2025 critiques).** Self-critique on task
trajectory, not on the spec. 2025 papers (MAR, SLSC-MCTS,
Critique-Guided Improvement) document **confirmation bias** and
local-minima failure — the reflecting model re-justifies its own
error. **Direct caution for any "agent rewrites its own story"
design.**
- [arxiv 2503.16024](https://arxiv.org/html/2503.16024v2)
- [arxiv 2512.20845](https://arxiv.org/html/2512.20845v1)

**Addy Osmani's "good spec" framework.** Pragmatic, widely cited,
but prose advice not a system.
- [addyosmani.com/blog/good-spec](https://addyosmani.com/blog/good-spec/)

**Net:** nobody surveyed treats "story failed → amend the story →
re-run → accept only when a cold agent can rebuild from story alone"
as the **convergence contract**. Spec-driven tools treat the spec as
a generator input, not as the attestation artefact.

## Observability / runs

OpenTelemetry GenAI semantic conventions are the emerging portable
layer. LangSmith = ops-heavy; Langfuse = OSS self-hostable baseline
(MIT); Braintrust = eval-focused; Arize/Helicone round out the top
five. A "run" is universally modelled as a **trace tree of spans**
(LLM calls, tool calls, retrievals) with cost/latency/score metadata.
Red-state evidence files + commit-signed UAT verdicts are **not** a
pattern anyone ships.
- [agenticcareers comparison](https://agenticcareers.co/blog/ai-agent-observability-stack-2026)
- [langflow blog](https://www.langflow.org/blog/llm-observability-explained-feat-langfuse-langsmith-and-langwatch)

**Implication for our story 16 design:** consider mapping our `runs`
row + NDJSON trace shape onto OpenTelemetry GenAI conventions. Not as
a constraint on Phase 0 — embedded Store with structured JSON is fine
— but as a forward-compat anchor. If we ever want to ship traces to
Langfuse for visualisation, the schema shape determines whether it's
config or a rewrite.

## Production-proven vs demo-ware

**Production:** E2B, Modal, Daytona, Replit Agent, Cursor, Langfuse,
LangSmith, Dagger.

**Beta / recent:** Claude Managed Agents (< 1 month old), OpenHands
SDK (research-grade production), Kiro.

**Experimental:** nearly every "living spec" tool; Reflexion-family
self-critique (literature shows degradation in agentic tasks).

**Demo-ware:** most "spec-kit wrapper" repos with star counts but no
case studies.

## Where the Agentic system's shape differs — novel or just differently named

| Feature | Status | Notes |
|---------|--------|-------|
| **Story-as-iteration-target with amendment on failure** | **Genuinely uncommon.** | Kiro/spec-kit let humans edit specs; they don't treat spec amendment as the control-flow primitive when an agent fails. Reflexion literature warns about confirmation-bias if agents self-amend without independent evaluator. |
| **Sandbox bakes specs + schemas + guidance, not just code** | Uncommon. | OpenHands Workspace is directionally similar but guidance is usually prompt-time context, not image-baked. |
| **Cold-agent reproducibility as the healthy-gate** | No prior art found. | SWE-bench reproducibility is benchmark scaffolding, not a product contract. |
| **Commit-signed UAT verdicts + red-state JSONL evidence** | Not standard. | Observability vendors persist traces; none persist a cryptographically-attested "this story rebuilds green" artefact. |
| **Docker-local → cloud swap via config** | Not novel. | Matches OpenHands' runtime-provider abstraction and the E2B/Daytona/Modal direction. Well-aligned with the herd. |
| **1-human / N-agent worktree parallelism** | Not novel. | Cursor (8 parallel), Devin 2.0, Replit Agent 4 all ship this. |
| **Story tree with retirement + supersession + competition** | Unique. | No direct analogue; closest kin is GitHub's branch model but applied to specs not code. |

## Recommendations to carry forward

1. **Don't build a bespoke sandbox.** Phase 0's Docker approach is
   correct. Keep the abstraction thin so we can swap in E2B, Daytona,
   Modal, or Claude Managed Agents as the cloud compute layer later —
   matching the `Store` trait pattern.
2. **Name the story tree + reproducibility attestation as the
   research bet** in ADR-0006. That's the part no one else has
   validated; the sandbox primitive is table stakes.
3. **Adopt OpenTelemetry GenAI conventions for the `runs` trace
   shape** (story 16). Specifically: span tree for tool calls, cost
   and latency metadata per span, model-identity in span attributes.
   Future interop with Langfuse / LangSmith becomes config, not code.
4. **Keep the human confirmation in the outer loop** at Phase 0 /
   Phase 1. The Reflexion confirmation-bias literature is a real
   warning; we shouldn't automate amendment until we have an
   independent evaluator (cold-agent rebuild IS that evaluator, but
   we haven't validated it at scale yet).
5. **Read OpenHands SDK in depth** (not to copy, but to cross-check
   primitive boundaries). Their 4-package split (SDK/Tools/Workspace/
   Server) is our closest analogue. Worth a separate research note if
   we hit architectural friction.

## What to do with this note

Inform ADR-0006 (see `11-sandbox-adr-outline.md`). Specifically:

- The "Alternatives considered" section gets fuller: name each of the
  SaaS sandbox vendors and why we're not using them in Phase 0 but
  keeping the door open.
- The "Decision" section should name the story-tree + reproducibility
  attestation as the research bet, distinct from the commoditising
  sandbox layer.
- The "Consequences" section should include the confirmation-bias
  caution from the Reflexion literature as an explicit risk.

This note is research, not authoritative. Lives in the sketchpad.
Sources cited inline for when the next session wants to drill deeper.
