# 05 — Reproducibility as a principle

## The user's formulation

> "I always imagine that to really prove the stories an agent should
> be able to build using only the story, this reproducability seems
> important in our system but i'm not sure where it fits in our system
> stability journey yet (i'm also confused why others with much larger
> budgets are not doing this)."

This is a deep claim. If it holds, it has far-reaching consequences.

## Stating the principle precisely

**Reproducibility claim:** Given only the YAML contents of a `healthy`
story, any agent (of sufficient capability) in any future sandbox
should be able to reproduce the implementation that drives its
acceptance tests from red to green.

Three things have to be true for this to work:

1. **The story is complete.** Every piece of information the agent
   needs to build is in the YAML — justifications pin observables, UAT
   walkthrough describes the human-visible shape, guidance carries
   non-obvious rebuild-from-scratch context. Nothing load-bearing lives
   outside the YAML.

2. **The system is stable.** The crates, patterns, ADRs, schemas that
   the story depends on are themselves reproducible (they too can be
   rebuilt from their own stories). No dangling implicit dependencies.

3. **The agent is capable.** It has a model sophisticated enough to
   bridge from prose-to-Rust, and tooling (`agentic test-build`,
   `cargo`, Edit/Write) to exercise the contract.

## Why this matters — the payoff

If we have reproducibility, then:

- **Stories become the durable artefact; code becomes ephemeral.**
  When the language ecosystem moves on (Rust 2030, some new crate
  conventions), we re-run the agent against the stories. Out pops an
  up-to-date implementation. The stories outlive the code.

- **Forkability is trivial.** A new user takes the story corpus, runs
  the agent, gets a working system. No "you need this specific
  dependency version" / "this works on my machine" fragility.

- **Bug hunting becomes deterministic.** A story that used to be
  healthy but is now red points at either: (a) the story has drifted
  and needs refactoring, or (b) the implementation has regressed. No
  third option. Currently we have a fuzzier "something might have
  drifted somewhere" failure mode.

- **Agent-written software becomes auditable.** A human reviews the
  stories; the code is the agent's rendering of the stories. The trust
  boundary sits where humans can productively judge (English +
  contracts), not where they can't (tens of thousands of lines of
  generated Rust).

- **Competition (tree metaphor, behaviour 2) becomes safe.** Two
  agents with two different style preferences can both render the same
  story — pick whichever passes tests faster / produces cleaner code.
  The story is invariant across both.

## Why this matters — the subtle payoff

There's a more profound implication:

**The story YAML becomes a compressed, human-curated representation of
the system's design intent.** The code is decompression. The
compression ratio is what matters — compact, auditable, debatable
stories yield large, detailed, correct implementations.

This is the trajectory from "coding" to "directing." Humans design
intent; agents execute. The story tree is the directable surface.

## The user's question — why aren't others doing this?

The user's parenthetical:

> "(i'm also confused why others with much larger budgets are not
> doing this)"

Legitimate question. Some hypotheses:

1. **Spec-writing is brutally hard.** A story that fully constrains a
   working implementation is an order of magnitude harder to write
   than "roughly describe what you want and let the agent figure it
   out." Most products are optimising for the latter, because that's
   where the market is. The cost of writing a fully constraining spec
   is the load-bearing cost, and very few people pay it.

2. **The reward shape is long-horizon.** Reproducibility's payoff
   is multi-quarter (stability, forkability, auditability). Most agent
   products are optimising for multi-week payoff (demo shippable,
   customer conversions). VC-backed timelines don't accommodate
   research bets with 2-year payoffs.

3. **Most orchestration tooling assumes human-in-the-loop review at
   every step, not end-to-end reproducibility.** The industry
   settled on "agent writes, human reviews, ship" pattern. That's
   locally rational but it doesn't cumulatively produce reproducible
   systems.

4. **Reproducibility requires determinism.** Eliminating hidden state
   is a lot of work: PATH, env vars, claude outputs, timestamps, order
   of operations, network conditions. Most systems don't bother because
   the cost/benefit at small scale is unfavourable. We're betting it
   flips at the right scale.

5. **Specification language is underdeveloped.** Formal methods folks
   (TLA+, Alloy) have thought about this for decades but their
   specifications are hard to write and hard for agents to execute.
   Our story YAMLs are a middle layer — precise enough to constrain,
   loose enough to write. The discipline to make this work across a
   whole system is a research bet.

6. **Teams with big budgets are aligned around "enterprise agent
   platforms" — frameworks for orchestrating many agents, not for
   reproducing systems from specifications.** The commercial gravity
   is toward selling workflow, not toward proving reproducibility.

7. **The "others aren't doing this" may be survivor bias.** Quiet
   research projects may be doing it; you just don't see them. The
   absence of commercial products doesn't mean the idea is being
   ignored.

## Where reproducibility fits in the stability journey

User's open question:

> "i'm not sure where it fits in our system stability journey yet"

Here's a proposal for where it fits:

**Reproducibility is the strongest possible form of stability.** A
stable system is one that works consistently. A reproducible system is
one that works consistently AND can be rebuilt from scratch with
consistency. The second strictly implies the first but adds a
survivability property.

**Practical implication:** as we ship new stories, we should ask "can
this story be built from scratch by a fresh agent using only the
YAML?" For every `healthy` story currently in the corpus, a worthwhile
exercise: ask an agent to rebuild it into a fresh crate given only the
story YAML and the dependency stories' YAMLs. Does it succeed? Where
does it fail? Each failure surfaces either a story gap (prose missing)
or a system gap (tooling assumption).

This exercise could be a **reproducibility audit**, run periodically.
Not as acceptance gate (too expensive to run often), but as a
meta-test of the corpus.

## Concrete mechanisms that improve reproducibility

1. **Complete justifications.** Every `acceptance.tests[].justification`
   should describe the observable AND the *why* — why this test exists,
   what it prevents. We mostly have this; tighten enforcement.

2. **Zero-dependency-on-external-state UAT prose.** UAT walkthroughs
   should describe a flow that doesn't rely on any repo state the
   story's ancestors don't produce. Currently some walkthroughs
   reference real stories in `stories/` directory for fixtures — that
   couples them to corpus state. Move all UAT fixtures to ephemeral
   scratch branches or `TempDir`s.

3. **Explicit fixture preconditions.** Story 15's PlanEntry already
   has `fixture_preconditions: []`. Make sure this field is used
   consistently — everything a scaffold needs should be named.

4. **Ancestor completeness.** Every `depends_on: [...]` ancestor must
   genuinely constrain the story. No decorative dependencies. A
   fresh agent reading ancestor YAMLs should get exactly the context
   they need, no more.

5. **Schemas for everything authoritative.** `schemas/story.schema.json`
   pins story shape. Expand similarly for patterns, ADRs (at least
   a structural convention), agent specs. The goal: any curator agent
   can authorise-by-schema without appeal to prose conventions.

6. **Determinism in `agentic test-build plan`.** Given the same
   story YAML, `plan` output should be byte-identical across runs.
   (Story 15 already enforces most of this; sanity-check for hidden
   non-determinism.)

## OPEN: at what scale does reproducibility pay off

If the corpus is 13 stories, rebuilding from stories is slower than
just running the code. At 500 stories, it may be faster (you can
parallel-rebuild). At 5000, it may be the only feasible approach.

Where is the crossover? Unknown. But the direction is clear: the cost
of implementation-drift grows superlinearly with corpus size; the cost
of specification-investment grows linearly. At some point they cross.
We want to be on the specification side when they do.

## OPEN: what's the minimum-viable reproducibility test

Proposal:

1. Pick a `healthy` story at random.
2. Copy its YAML + the YAMLs of all transitive ancestors + the
   relevant patterns + schemas into a fresh empty git repo.
3. Have an agent (fresh context, no Cargo.lock, no prior knowledge)
   attempt to reproduce the implementation driving that story's tests
   to green.
4. Measure: did it succeed? If not, what was missing?

This is an hour of wall-time per story. Manageable to run monthly for
a sampled subset. Marked as a future ceremony once cloud sandboxes
make agent spawn cheap.

## Files to read for more context

- `CLAUDE.md` "Core principles" section — "Red-green is a contract"
  is the narrower version of this idea.
- `docs/decisions/0005-red-green-is-a-contract.md` — the ADR that
  codifies the minimal form.
- `stories/15.yml` guidance blocks — the `plan/record` flow is the
  mechanism by which reproducibility is enforced.
- `docs/guides/story-authoring.md` — current authoring conventions.
  May need tightening to enforce reproducibility.
