# 14 — Git coordination model

How the story tree (corpus of YAML contracts) composes with git
branches (file-level coordination). Surfaced 2026-04-23 during ADR
authoring when the user asked for explicit direction.

## User direction (verbatim)

> "Git branches are snapshots of the whole tree, they might
> represent the whole tree + 1 new proposed branch but it's still a
> snapshot of the whole tree not just a single branch, better to
> have separation. When a story-tree branch goes green in its
> sandbox then yes I'd expect it to get added to main branch tree.
> If the branch turns out to be bad then this will push us to build
> the recovery and gating mechanisms that is part of what we are
> trying to innovate with using this story-tree experiment."

Three principles fall out of this:

1. **Separation of vocabulary.** "Story-tree branches" and "git
   branches" are **different concepts that both get called branches.**
   The story tree lives in YAML; git branches are file-level
   snapshots.
2. **Git branches = whole-tree snapshots.** A git branch represents
   the entire story corpus (plus implementation) at some state.
   Never "a branch per story." The mapping is many-to-one the other
   direction: many story-tree branches coexist on one git branch.
3. **Aggressive merge on green; build recovery when it hurts.** A
   sandbox that goes green lands on main. Bad landings are expected
   and desirable — they force the research question of what recovery
   and gating should look like.

## Vocabulary split

| Concept | Where it lives | Mutation pattern |
|---------|----------------|------------------|
| **Story-tree branch** | YAML in `stories/`, the `depends_on` DAG, the `superseded_by` chain | Edited by `story-writer`; status changed by `agentic uat` |
| **Git branch** | `.git/refs/heads/` | Created by `git checkout -b`; moved by commits |
| **Story** | One file `stories/<id>.yml` | Authored, amended, retired — purely a YAML lifecycle |
| **Run** | One row in `runs` table + NDJSON blob | Created by `agentic story build`, immutable after write |
| **Sandbox** | A Docker container lifetime | Created, runs inner loop, exits |

Everything above the line ("story-tree branch," "story") is corpus.
Everything below ("git branch," "run," "sandbox") is coordination
mechanics. Don't conflate.

## What a git branch represents

A git branch is a **snapshot of the whole tree plus zero or more
proposed / in-flight changes.** Specifically:

- **`main`** — the current trunk. The authoritative story corpus +
  the implementation that drives it green.
- **`run/<story-id>-<run-short>`** — a sandbox's working branch,
  cloned from `main` at sandbox launch. Carries: the whole corpus
  as-of-launch (unchanged except the story being built), plus the
  agent's implementation commits. Ephemeral (Phase 0) or pushed to
  origin (Phase 2 when fanout arrives).
- **`rollback/<label>`** — (future) a snapshot of `main` at a
  previous stable commit. For rollback when a merge turns out bad.

The "whole tree" framing matters: when an agent's sandbox branch
goes green, merging it back doesn't rearrange the story DAG. It
lands new implementation + possibly-amended story YAML on main. The
tree is stable; the branch is a staged mutation proposal.

## Sandbox branch lifecycle (Phase 0)

```
agentic story build <id>
  ├─ host: compose docker run args; pass story-id, branch name
  └─ docker run ...

IN SANDBOX (container):
  ├─ git clone /mounted-repo /work/repo
  ├─ cd /work/repo && git checkout -b run/<id>-<short>
  ├─ [inner loop: claude edits + commits on this branch]
  ├─ [inner loop exits: green | inner_loop_exhausted | crashed]
  └─ emit run row + trace to /output/runs/<run-id>/

  On GREEN:
    └─ emit a `branch_state` entry in the run row:
         commits: [ { sha, author, subject, diff-stat }, ... ]
         tip:     <sha of final commit>
         start:   <sha the branch was cut from>

  On EXHAUSTED / CRASHED:
    └─ emit the same `branch_state` entry — the dev can still
       inspect what the agent tried.

  Container exits; /work/repo is destroyed with it.
```

The branch **never leaves the sandbox as a ref.** The run row carries
the commit series as structured data; if the dev wants to apply the
branch to main, they use `agentic story accept <run-id>` (Phase 2
CLI — not Phase 0) or manual `git am` / cherry-pick on the diff.

**Phase 0 auto-merge behaviour (per user direction):** when a run
returns GREEN, the host `agentic story build` command applies the
diff onto `main` automatically — `git am` or equivalent — producing
one or more commits on main as if the agent had worked locally on
main. No human review gate in Phase 0.

Bad merges will happen. That's the point. Per the user: "this will
push us to build the recovery and gating mechanisms that is part of
what we are trying to innovate."

## Failure modes we're inviting (deliberately, Phase 0)

Auto-merge on green without human review produces known failure
shapes. Naming them so future recovery/gating work has targets:

- **Regression in unrelated tests.** Green for this story's tests ≠
  green for the workspace. The inner loop runs `cargo test
  --workspace`, which should catch this — unless the agent selectively
  ran only the story's tests. Gating candidate: "full workspace pass
  before merge" is non-negotiable.
- **Semantic break not covered by tests.** Tests pass + UAT passes
  but the implementation is wrong in ways neither surfaces. This is
  the reproducibility principle's real test — the story's acceptance
  surface must be complete. If we hit this, the amendment is on the
  story, not the gate.
- **Downstream story breakage.** Story A lands green; story B (which
  depends on A) is now red because A's implementation changed a
  shape B was relying on. Classifier (story 13) surfaces this.
  Recovery candidate: revert A, or land an amendment to A.
- **Merge conflict with concurrent sandbox work.** In Phase 0 we
  run one sandbox at a time so this is moot. In Phase 2 fanout,
  two sandboxes racing to merge becomes interesting — winner takes
  main, loser either rebases or gets discarded.

The **research question** the user called out: what gating and
recovery do we actually need? Phase 0 ships with minimal gating
(full-workspace-test + UAT) and no recovery beyond `git revert`.
Phase 2+ discovers what else is load-bearing by doing without and
feeling the pain.

## Recovery mechanisms (future, stubbed)

Not Phase 0 scope. Named here so they're not forgotten:

- **Rollback branches** (`rollback/<label>`). Human creates one
  when about to do something risky, or after a bad merge. Standard
  git hygiene; no innovation.
- **`agentic story revert <story-id-or-run-id>`** — reverses a
  landing: `git revert` the relevant commits + YAML reversion (story
  status goes back from `healthy` to `under_construction`;
  `superseded_by` edges if any also revert).
- **Cascade detection.** When a story goes red or is reverted, walk
  the DAG downward; mark all transitively-dependent stories as
  **needs-revalidation** (possibly a new status, or a classifier
  category). This is where the classifier (story 13) grows.
- **Rebuild from spec as repair.** A reverted story can trigger an
  automatic `agentic story build` run against the reverted state —
  if the agent can produce a working implementation from the spec,
  the repair is automatic. Failing that, human intervenes.

## How this composes with retirement and succession

Retirement + supersession are **YAML-only operations.** No git branch
changes are triggered by retirement itself.

- **Story retired:** YAML edit (`status: retired`,
  `superseded_by: <id>`). Commit the YAML edit on main. Nothing else
  moves.
- **Gate following superseded_by:** story 11's amendment teaches the
  gate to walk the chain (per user direction 2026-04-23). If a story
  depends on a retired ancestor, the gate evaluates the successor's
  health. No git branch involved.
- **Successor story lands:** authored as a new story, goes through
  normal build → sandbox → green → merge-to-main. At merge time,
  the old story's YAML may be edited in the same commit (status
  changes to retired with `superseded_by` pointing at the new
  story's id). One git commit, one atomic transition in the corpus.
- **"Branches that hold previous states" (user's rollback concept):**
  git branches representing main at a past stable point. Created on
  demand by humans, e.g. `git branch rollback/pre-story-22
  <old-sha>`. Not tied to story lifecycle; tied to implementation
  history.

## What this implies for Phase 0 stories

Minimal scope additions to the outlines in note 10:

### Story 20 — `agentic story build <id>` additions

- Sandbox creates `run/<story-id>-<short>` from main tip at launch.
- Inner-loop agent commits to this branch.
- Run row records the branch state (commits, tip sha, start sha).
- On GREEN: host applies the branch's diff to main as one or more
  commits. No human review gate. The GREEN run IS the attestation.
- On EXHAUSTED / CRASHED: no merge. The branch state is in the run
  row; the sandbox's working copy is destroyed with the container.

### Story 16 — `runs` schema additions

Add to the schema:

```
branch_state:
  start_sha:  <sha the sandbox branch was cut from>
  end_sha:    <sha of the final commit (absent if none)>
  commits:    [ { sha, author, subject, stats } ]
  merged:     true | false
  merge_shas: [<sha>, ...]  (when auto-merged to main, the
                            resulting main commits)
```

### Story 18 — signer composition with git

The `signer` field on the signing row is the agent identity
(`sandbox:<model>@<run_id>`) as already specified. The merge commit
on main is authored by a git identity (e.g. `agentic-bot` or the
user's identity) — independent of the story-attestation signer.
Worth flagging in story 18's guidance so it's clear these two
identities are distinct and both land in the corpus history.

## Sub-questions for story-writer / build-rust

1. **Auto-merge implementation shape.** `git am` from a patch
   series, `git cherry-pick`, or `git merge --squash`? Squash
   produces one commit on main per run; cherry-pick preserves the
   agent's commit series. My lean: squash in Phase 0 (main history
   stays readable), revisit if granular history becomes valuable.

2. **Git identity on merge commits.** The merging process is the
   host `agentic story build` command. Whose identity does the
   resulting main commit carry? Options: (a) the dev's `git config
   user.email` (treats the sandbox run as the dev's proxy); (b) a
   dedicated `agentic-bot@localhost` identity; (c) encode the
   sandbox run-id into the commit author line. My lean: (a) —
   simplest, aligned with "the dev ran the command."

3. **Amend-same-story runs.** If a story's sandbox fails three
   times and succeeds on the fourth, is the history "one merge
   commit per run (three of them failing, one succeeding)" or "one
   merge commit for the winner only"? My lean: winners only. Failed
   runs stay in run rows, not in main's history.

4. **Commit message convention for auto-merged main commits.**
   Proposal: subject line `story <id>: <story title>`, body
   includes run-id, signer, start-sha, and a pointer to the run row
   path. This gives `git log` a navigable history.

5. **What happens if the sandbox's start-sha drifts from main's
   current tip by the time the merge happens?** Unlikely in Phase 0
   (single dev, single sandbox at a time) but possible if the dev
   makes manual commits while a sandbox runs. Options: (a) refuse
   the merge and surface as an error in the run row; (b) try to
   rebase the sandbox branch onto current main tip. (a) is safer; (b)
   is more convenient. My lean: (a) for Phase 0.

## What this note is NOT

- **Not the full git workflow guide.** That's `docs/guides/local-
  sandbox-run.md` material.
- **Not a commitment to commit-message shapes.** Those are story-
  writer / build-rust decisions during story 20's authoring.
- **Not the recovery/gating design.** Phase 0 ships minimal gating
  (full workspace test + UAT) and no recovery. Deliberate per user
  direction. Recovery/gating emerges from real failures.

## How this note feeds the ADR

ADR-0006 (outline in note 11) needs a new section **"Git coordination
model"** or equivalent. Content: the three principles at the top of
this note, the sandbox branch lifecycle, and the auto-merge-on-green
posture with explicit acknowledgement that recovery/gating is future
work informed by Phase 0 failures. Sub-questions above are story-
writer / build-rust territory, not ADR territory.

This note grounds the ADR's position; the ADR compresses it into
normative text.
