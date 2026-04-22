# 07 — Prerequisites: what needs to land locally before cloud

Cloud is the migration target. Before we migrate, the local system
needs to satisfy some preconditions. Some are already met; some need
work.

## Already satisfied

Check these off; they're already done:

- ✅ `Store` trait with swappable impls (stories 4 + 5). Cloud Store
  will slot in as a new impl.
- ✅ Story YAML + schema + DAG loader (story 6). Corpus is portable
  across environments.
- ✅ UAT verdict signing with commit hash (story 1). Signing works
  anywhere the git commit is resolvable.
- ✅ Claude-as-user architectural posture. Libraries are AI-free.
  Cloud sandboxes inherit this cleanly.
- ✅ Plan + record flow (story 15). The authoring loop is sandbox-
  friendly: no ambient global state, each run is self-contained
  against a story id.
- ✅ Ancestor gate + classifier (stories 11 + 13). Dashboard
  surfaces DAG health for any future multi-sandbox UI.
- ✅ Installable binary (`./install.sh` with Docker variant). Cloud
  sandboxes can install the same way.
- ✅ All 13 stories currently `healthy`. Stable foundation to build
  on.

## Missing — needed before cloud migration

These should land locally before we invest in cloud infra. Each
represents a separate story (or small epic):

### P1 — Retirement lifecycle

Introduce `retired` as a first-class story status with `superseded_by`
metadata. Reasons we need this BEFORE cloud:

- In a world with ephemeral sandboxes and frequent branch experiments,
  pruning discipline matters. `retired` makes pruning visible instead
  of destructive.
- Cloud-stored signings rows for retired stories should have a status
  to attribute to. Currently they become orphans.
- Ancestor gate (story 11) needs to know how to treat retired
  ancestors (skip them, not fail them).

**Scope:** 1 story. Schema change, story 11 gate update, dashboard
filter mode, retroactive conversion of current hard-deleted stories
(7, 8, 14) to `retired` entries in the store.

See `03-tree-metaphor.md` for deeper design notes.

### P2 — Signer identity on signings

Currently every `uat_signings` row is attributed by commit hash but
not by signer. One-user world, fine. Multi-sandbox world, not fine —
rows need to know which sandbox / human signed them.

**Scope:** 1 story. Add `signer: String` field to `uat_signings` (pulled
from `git config user.email` or env var). Story 1's CLI contract
extends to accept signer identity; defaults preserve backwards compat.

This is small, additive, and **must ship before cloud** so all cloud
data carries identity from day one.

### P3 — Reproducibility audit (optional but recommended)

Before investing in cloud-scale infra, run the manual reproducibility
audit proposed in `05-reproducibility.md`:

1. Pick 3 healthy stories at random.
2. For each: extract its YAML, its transitive ancestors' YAMLs,
   relevant patterns, relevant schemas.
3. Attempt to rebuild the implementation from scratch using a fresh
   agent context.
4. Catalogue where the story is incomplete.

**Scope:** not a story; an exercise. Takes a day. Findings feed into
tightening authoring conventions before cloud amplifies drift risk.

### P4 — Ephemeral worktree hygiene

The `.claude/worktrees/` pattern (committed in `e3c3ef3`) is the
local precedent for isolated subagent work. It should be formalised:

- `.gitignore` entry (already there).
- CLI command: `agentic scratch create <name>` → creates a worktree;
  `agentic scratch destroy <name>` → nukes it cleanly.
- Documentation in `docs/guides/`.

**Scope:** 1 story OR a CLAUDE.md convention update + guide doc. The
ephemeral scratch pattern is the local version of cloud sandboxes;
getting it solid first means the cloud version is a "move this
primitive elsewhere" rather than "invent a new primitive in the
cloud."

### P5 — `agentic-runtime` minimum-viable

`agentic-runtime` is currently `_deferred/`. It's the home for:

- Claude subprocess spawning (per ADR-0003).
- Orchestrator-to-agent dispatch.
- Any "talk to an AI" surface.

Until it ships, we're informally doing this via Claude Code sessions
(me, orchestrator). That's fine for local dev but doesn't port cleanly
to cloud.

**Scope:** 1-2 stories. Minimum viable:

- `Runtime` trait with `spawn_subagent(prompt, tools, budget) ->
  Result<SubagentSession>`.
- `ClaudeCodeRuntime` impl that wraps the local `claude` CLI.
- Story 1's UAT flow optionally accepts a runtime (currently it doesn't
  need one, but future stories will).

This may not need to land BEFORE cloud, but it should land before we
expect cloud sandboxes to dispatch to each other. Marked P5 because
the dispatch-between-sandboxes case is itself a future phase.

### P6 — Container image reproducibility

The repo has `Dockerfile` + `bin/agentic-docker`. These need:

- To produce a byte-identical image for a given commit (reproducible
  builds).
- To include everything a sandbox needs (Rust toolchain, agentic
  binary, claude CLI).
- To be CI-built and cached.

**Scope:** small. Probably a single PR, not a full story. A guide doc
in `docs/guides/cloud-sandbox-image.md`.

## Dependencies between prerequisites

```
P4 (scratch hygiene)
    └── enables P6 (container image) by formalising the primitive
P1 (retirement)
    └── independent; nice to ship before cloud store
P2 (signer)
    └── independent; must ship before cloud store
P3 (audit)
    └── independent; informs P1 authoring quality
P5 (runtime)
    └── depends on P2 (signer needs to exist for runtime-spawned work)
    └── depends on P6 (cloud runtime needs container image)
```

## Minimum prerequisite set for "Phase 1: cloud store + signer"

Just ship:

1. **P2 (signer identity)** — essential.
2. **P1 (retirement)** — highly recommended, keeps data clean.

That's 2 stories. Then build the cloud Store impl. Then migrate. The
other prerequisites can ship after.

## What we're explicitly NOT requiring before cloud

- Multi-human auth / RBAC. Single-human cloud is fine; auth is
  Phase 2+.
- Competition / alternatives_to mechanics. Tree-metaphor behaviour 2
  (see `03-tree-metaphor.md`) is speculative; build on real need.
- `agentic sandbox <lifecycle-verb>` CLI. Use Cloud Workstations
  directly first; build a custom lifecycle only if that proves
  insufficient.
- Observability beyond Cloud Logging defaults. Build up as real
  pain appears.
- Migration of existing local SurrealStore data. Just start the cloud
  store fresh; the story YAMLs are the durable record, not the
  store contents.

## Quick decision tree

```
If the goal is "unblock cloud soonest":
    Ship P2 (signer identity)
    Build cloud Store impl
    Migrate
    Everything else follows

If the goal is "cleanest foundation, stability over speed":
    Ship P1 (retirement)
    Ship P2 (signer)
    Ship P4 (scratch hygiene) to formalise the sandbox primitive
    Run P3 (audit) to surface any story-incompleteness bugs
    Then cloud Store + migration

If the goal is "ship cloud and iterate":
    P2 + cloud Store in Phase 1
    P1, P4, P5, P6 in Phase 1.5
    Iterate
```

User's stated bias ("prioritise the cleanest foundation over
stability") suggests the middle path. Next session should resolve.

## OPEN: order of P1 vs P2

Both are small and additive. They could ship in parallel (different
crates — retirement touches story + dashboard + uat; signer touches
uat + ci-record). Or sequentially. Next session decides.
