# 04 — The cloud sandbox model

> **⚠️ Status (2026-04-22):** this note is the early exploration that
> led to the Phase 0 / 0.5 / 1 ladder in `09-tier1-resolutions.md`.
> Decision B (Cloud Workstations) is **rejected** — the dev works
> locally; cloud is for headless agent runs only. The rest of the
> note (sandbox-as-atomic-unit framing, four-decision decomposition,
> failure modes, ancestor-gate question) is still useful as context.
> For the current shape, read note 09 first, then use this note for
> historical framing.

## The user's framing

> "If we think of branches as cloud sandboxes, is this something that
> could work?"
>
> "Our cloud sandboxes should be where the agents can do their builds,
> i think this is the common pattern now these days anyway, the
> innovation is our story tree."

## Sandbox as an atomic unit

A **sandbox** is the atomic unit of:

1. **Source isolation** — its own checkout of a git branch.
2. **Runtime isolation** — its own `agentic` binary build, its own
   process tree, its own filesystem.
3. **Data isolation** — its own view of the store (read-shared,
   write-scoped).
4. **Auth isolation** — its own claude credentials, its own signer
   identity.
5. **Lifecycle isolation** — spin up, snapshot, fork, destroy — without
   touching main or other sandboxes.

The fifth is the important one. Git branches can't be destroyed
cleanly — they still contaminate the developer's local machine (dirty
Cargo.lock, leftover artefacts, residual processes). A sandbox can be
nuked without trace.

## What "cloud sandbox" hides — four distinct decisions

The phrase bundles four things. Each deserves its own decision.

### Decision A — Store externalization

The `Store` trait (stories 4+5) was designed for this. User's durable
memory note:

> "Store will move from local to cloud — local SurrealDB is interim;
> design storage code so cloud swap is config, not code."

**Options:**

- **SurrealDB Cloud** — managed hosting for SurrealDB. Drop-in
  semantics; we already depend on `surrealkv` (the embedded engine).
  Smallest migration cost.
- **Self-hosted SurrealDB on GCE / Cloud Run** — cheaper at low scale,
  more ops overhead.
- **Different backend (Postgres, Firestore, Spanner, BigQuery)** —
  would require new `Store` impl. Higher work cost but may fit GCP
  better.

**Recommendation:** start with **SurrealDB Cloud** if managed pricing
is predictable; fall back to self-hosted SurrealDB on a small GCE
instance if cost blows up. Keep the `Store` trait as the single
abstraction point so migration is config-swap not refactor. This is
the cheapest step and unblocks nothing else until it ships.

This is roughly 1 story of work + 1 story to verify cloud store has
trait parity with embedded.

### Decision B — Compute externalization

Where does `agentic uat`, `agentic test-build`, etc. actually run?

**Options ranked by ambition:**

1. **Cloud Codespaces / Gitpod / DevPod** — open a cloud VSCode in a
   browser, work there. No new primitive. Dev-experience: "open a
   sandbox, work, submit a PR." Tests cloud store from a real cloud
   box. Cheapest.

2. **Docker containers with mount points** — extend existing `./install.sh
   --docker` for cloud use. Run on Cloud Run or GKE.

3. **Proper remote compute with `agentic sandbox` CLI** —
   `agentic sandbox create/fork/destroy`. Most ambitious. New
   abstraction.

**Recommendation:** start with option 1 (Codespaces-equivalent). Walk
up the ladder only if real usage proves option 1 insufficient. The
user's instinct ("common pattern now anyway") is correct — managed dev
environments are a mature category.

GCP-specific options:
- **Cloud Workstations** (GCP's managed dev env; their Codespaces
  equivalent). Integrates with Google IAM.
- **Cloud Shell** — too limited for our use case (ephemeral, 12-hour
  sessions max, limited compute).
- **Cloud Run** — great for stateless services, awkward for
  interactive-agent work.
- **GKE Autopilot** — overkill for 1-human.

### Decision C — Identity / auth

Current state: every UAT verdict is signed by whoever ran the binary.
No identity check. Fine for single-user laptop, broken for any other
setup.

**Decisions needed:**

- Who can sign a Pass verdict (any collaborator? author-only? approval
  required?).
- How is identity verified (GitHub OAuth? email? service account?).
- Do signings carry a signer field (`uat_signings.signer: "hua"`)?
- Does story 1's contract need extending?

For **single-human phase**, minimum viable answer:

- A `signer: String` field on `uat_signings` (defaulted from
  `git config user.email` or env var).
- No authorisation checks beyond "can write to the store at all."
- Multi-user will add policy later without needing a store rewrite.

This is ~1 story. Should ship alongside cloud store so the cloud data
has identities from day one, not retrofitted.

### Decision D — Claude auth in cloud

**The wall.** ADR-0003 says:

> "`agentic-runtime` (the orchestrator crate) uses the local `claude`
> binary (subscription auth) via subprocess to spawn subagents."

That model assumes the user is physically on the machine with their
OAuth tokens. Cloud sandboxes break that assumption.

**Options:**

1. **Bring your own claude into the sandbox.** Each developer mounts
   their `~/.claude/.credentials.json` into the container on sandbox
   creation. Works; UX papercut; faithful to ADR-0003.

2. **Shared team claude account.** Sandbox reads a mounted team
   credential. Violates the "user's own subscription" spirit of
   ADR-0003.

3. **Relax ADR-0003 to allow API key for cloud.** Only when invoked
   from authenticated sandboxes. Runs into the Anthropic policy
   concern ADR-0003 already flagged: subscriptions no longer cover
   third-party tools that proxy subscription auth into SaaS.

4. **Run Gemma locally on the sandbox** (user suggested). Cheap,
   offline-capable, no external auth dependency. Works for high-volume
   low-stakes agent work. May not suffice for long-context planning
   tasks.

**Recommendation:** start with **option 1** (BYO). Document it.
Evaluate option 4 in parallel as an option for high-volume agent loops
where a smaller local model is fit-for-purpose. See `06-stack-bet.md`
for the Gemma line of thought.

## Minimum viable sandbox

What's the thinnest possible first sandbox?

1. A GCP Cloud Workstation image with:
   - Rust toolchain (via rustup)
   - `claude` CLI (BYO auth)
   - The repo cloned
   - `agentic` binary installed
2. Configured to use a cloud-backed SurrealStore.
3. The user can `agentic uat <id> --verdict pass` and it works.

That's the MVP. Everything else is incremental.

## What concurrent sandboxes look like

Two sandboxes active simultaneously:

- Both read from the same cloud store.
- Each writes under its own signer identity.
- Each works on its own git branch (or worktree within a shared
  branch — see `.claude/worktrees/` precedent).
- The dashboard shows all active sandboxes and their in-flight stories.
- A `agentic sandbox list` command (future) surfaces who's working on
  what.

The tree metaphor's "branches coexist in the same space" maps directly
here. Two sandboxes exploring two different approaches to the same
problem = coexisting branches in the canopy.

## Failure modes to anticipate

1. **Store write conflicts.** Two sandboxes signing the same story
   simultaneously. Resolution: last-writer-wins plus a signed-by-
   whom field is enough for single-human; multi-human needs optimistic
   concurrency with a version field.

2. **Git ref drift.** Multiple sandboxes pushing to the same branch.
   Resolution: one-sandbox-per-branch rule. Name the branch after the
   sandbox.

3. **Claude rate limits.** Shared subscription, multiple sandboxes
   drawing from it. Resolution: monitor + back off; consider option 4
   (local Gemma) for bulk agent loops.

4. **Runaway cost.** Sandboxes forgotten / left running. Resolution:
   TTL on Cloud Workstation sessions; `agentic sandbox reap` command;
   budget alerts.

## Why sandbox lifecycle matters for the tree metaphor

If a sandbox is cheap to create and destroy, then an agent running a
failed experimental story can just destroy its sandbox when done. The
story retires, the sandbox is gone, main is untouched. Pruning becomes
truly costless.

This is the structural argument for sandboxes beyond dogfooding: the
tree metaphor's branches-die-freely behaviour requires sandboxes to
actually be free to die. Local worktrees are not (they leave dirt
behind); cloud sandboxes are (by construction).

## OPEN: the orchestrator vs the sandbox

Today the orchestrator (me, Claude Code instance) runs on the user's
laptop and spawns subagents locally. In the cloud world:

- Does the orchestrator run in a sandbox too?
- Or does it stay local, dispatching to cloud sandboxes for the
  building?

The former is more symmetric; the latter is cheaper and faster to
iterate. **Recommendation:** start with latter (local orchestrator,
cloud agent sandboxes). Revisit once the protocol between orchestrator
and sandboxes is well-defined.

## OPEN: what talks to the sandbox

When the orchestrator dispatches work to a sandbox, what's the
protocol?

- SSH + `agentic` commands?
- gRPC / HTTP RPC over a protocol the sandbox serves?
- Cloud Pub/Sub messages?
- Claude Code's own subagent-spawning (if it supports remote agents)?

This is a significant design decision. It may warrant its own ADR.
Marked for next session.
