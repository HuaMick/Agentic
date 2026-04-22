# 01 — Goal

## The goal in the user's words

> "1 human but with cloud scalability and sandbox/branches of our tree
> so if I can experiment safely using our story tree and maintain
> system stability then we can think about expanding to >1 human, atm
> keeping the system stable with 1 human and n agents is challenge
> enough."

> "Our cloud sandboxes should be where the agents can do their builds,
> i think this is the common pattern now these days anyway, the
> innovation is our story tree."

## Bounded scope for this phase

**In scope:**

- One human orchestrator (the user).
- Many agents, each building in an isolated cloud sandbox.
- The story tree (our existing DAG + tree-metaphor extensions) as the
  coordination primitive.
- Cost-controlled cloud footprint.
- System stability under concurrent agent work.

**Out of scope for now (explicitly deferred):**

- Multi-human collaboration. The legacy system failed when it tried
  this prematurely. Validate single-user stability first.
- Public / external users of the sandbox infrastructure.
- Quota / billing / tenant isolation beyond what's needed for one
  human's safety.
- Sophisticated sandbox lifecycle (snapshot, fork, merge). Build the
  MVP first.

## The innovation claim

The novel part of this direction is NOT the sandbox primitive. Cloud
sandboxes for agent work are increasingly common (Anthropic's own
managed agent product, Codespaces, Gitpod, Coder, etc).

The innovation is **the story tree as the coordination primitive.**
Specifically:

- Sandboxes are cheap and ephemeral; the story corpus is the durable
  structure.
- Agents don't coordinate via git branches or message queues — they
  coordinate via stories (declare intent, prove red-state, drive green,
  sign verdict).
- The tree structure (DAG + retirement + future competition) gives a
  human a surveyable canopy over arbitrary agent fanout.
- A successful story is reproducible from its YAML alone — any future
  agent in any future sandbox should be able to rebuild it (see
  `05-reproducibility.md`).

## What success looks like at the end of this phase

1. The user can run `agentic sandbox create` (or equivalent) and get a
   cloud box with a branch of the repo checked out, running the full
   `agentic` stack.
2. An agent running in that sandbox can drive a story from `proposed`
   to `healthy` using only `agentic test-build plan/record` + its own
   authoring tools.
3. Multiple sandboxes can run concurrently without corrupting the
   shared story corpus or store.
4. The user can see all active sandboxes and their in-flight stories
   from a single dashboard view.
5. Cost for a month of moderate usage is predictable and bounded —
   order of magnitude $10s not $100s.
6. The local dev experience is preserved for development of the system
   itself (we still dogfood from laptops when we want to).

## What success explicitly does NOT require

- Auth beyond "one user, one trust boundary." No RBAC, no audit logs
  beyond signings, no team management.
- Claude API-based auth. Subscription auth via subprocess per ADR-0003
  remains the rule (see `08-open-questions.md` on how this survives
  cloud).
- Zero-cost at-rest. Cloud persistence costs money; the goal is
  predictability, not zero.

## Why now

Three reasons:

1. **System stability is at a local maximum.** All 13 stories healthy.
   Tooling (plan/record, UAT, ancestor gate, classifier) all working.
   Architectural realignment (claude-as-user) complete. Building on
   top is safer than building into instability.

2. **Store abstraction is already cloud-ready.** The `Store` trait
   (stories 4 + 5) was deliberately designed for this. User's durable
   memory note: *"local SurrealDB is interim; design storage code so
   cloud swap is config, not code."* The bullet is loaded; pull the
   trigger.

3. **The cost of deferring is increasing.** The more stories and
   tooling we build locally, the more breaks when we move to cloud.
   Pay the migration cost while the corpus is small (13 stories, a
   half-dozen crates).

## Why this phase is the whole bet

If we prove one-human-plus-N-agents is stable in a cloud sandbox model,
the multi-human extension becomes a UX addition, not an architecture
pivot. If we can't prove single-human stability, multi-human is a trap.
